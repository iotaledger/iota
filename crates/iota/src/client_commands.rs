// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::{Eq, min},
    collections::{BTreeMap, BTreeSet, HashSet, btree_map::Entry},
    fmt::{Debug, Display, Formatter, Write},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Context, anyhow, bail, ensure};
use bip32::DerivationPath;
use clap::*;
use colored::Colorize;
use fastcrypto::{
    encoding::{Base64, Encoding},
    hash::HashFunction,
    traits::{EncodeDecodeBase64, ToFromBytes},
};
use futures::{StreamExt, TryStreamExt};
use iota_config::verifier_signing_config::VerifierSigningConfig;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    Coin, DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse, DynamicFieldInfo,
    DynamicFieldPage, IotaCoinMetadata, IotaData, IotaExecutionStatus, IotaObjectData,
    IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery, IotaParsedData,
    IotaProtocolConfigValue, IotaRawData, IotaTransactionBlockEffects,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
};
use iota_keys::keystore::{AccountKeystore, StoredKey};
use iota_move::manage_package::resolve_lock_file_path;
use iota_move_build::{
    BuildConfig, CompiledPackage, build_from_resolution_graph, check_conflicting_addresses,
    check_invalid_dependencies, check_unpublished_dependencies, gather_published_ids,
    implicit_deps,
};
use iota_package_management::{
    LockCommand, PublishedAtError,
    system_package_versions::{latest_system_packages, system_packages_for_protocol},
};
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk::{
    IOTA_COIN_TYPE, IOTA_DEVNET_GAS_URL, IOTA_DEVNET_URL, IOTA_LOCAL_NETWORK_GAS_URL,
    IOTA_LOCAL_NETWORK_URL, IOTA_LOCAL_NETWORK_URL_0, IOTA_TESTNET_GAS_URL, IOTA_TESTNET_URL,
    IotaClient, PagedFn,
    apis::ReadApi,
    iota_client_config::{IotaClientConfig, IotaEnv},
    wallet_context::WalletContext,
};
use iota_sdk_types::crypto::{Intent, IntentMessage};
use iota_source_validation::{BytecodeSourceVerifier, ValidationMode};
use iota_types::{
    account_abstraction::{
        account::AuthenticatorFunctionRefV1Key, authenticator_function::AuthenticatorFunctionRefV1,
    },
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{DefaultHash, EmptySignInfo, SignatureScheme},
    digests::{ChainIdentifier, TransactionDigest},
    dynamic_field::{self, Field},
    error::IotaError,
    gas::{GasCostSummary, get_gas_balance},
    gas_coin::GasCoin,
    iota_serde,
    message_envelope::Envelope,
    metrics::BytecodeVerifierMetrics,
    move_authenticator::MoveAuthenticator,
    move_package::{MovePackage, UpgradeCap},
    object::Owner,
    parse_iota_type_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    signature::GenericSignature,
    transaction::{
        CallArg, InputObjectKind, SenderSignedData, Transaction, TransactionData,
        TransactionDataAPI, TransactionKind,
    },
    type_input::TypeInput,
};
use json_to_table::json_to_table;
use move_binary_format::CompiledModule;
use move_bytecode_verifier_meter::Scope;
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::TypeTag,
};
use move_package::{BuildConfig as MoveBuildConfig, source_package::parsed_manifest::Dependencies};
use move_symbol_pool::Symbol;
use prometheus::Registry;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use strum::{Display, EnumString};
use tabled::{
    builder::Builder as TableBuilder,
    settings::{
        Alignment as TableAlignment, Border as TableBorder, Modify as TableModify,
        Panel as TablePanel, Style as TableStyle,
        object::{Cell as TableCell, Columns as TableCols, Rows as TableRows},
        span::Span as TableSpan,
        style::HorizontalLine,
    },
};
use tracing::{debug, warn};

use crate::{
    PrintableResult,
    clever_error_rendering::render_clever_error_opt,
    client_ptb::ptb::{PTB, PTBCommandResult},
    displays::Pretty,
    key_identity::{KeyIdentity, get_identity_address, get_identity_address_from_keystore},
    keytool::Key,
    signing::{SignData, get_shared_object_version, sign_secure, sign_transaction},
    upgrade_compatibility::check_compatibility,
    verifier_meter::{AccumulatingMeter, Accumulator},
};

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// Only to be used within CLI
pub const GAS_SAFE_OVERHEAD: u64 = 1000;

#[derive(Parser)]
pub enum IotaClientCommands {
    /// Default address used for commands when none specified
    ActiveAddress,
    /// Default environment used for commands when none specified
    ActiveEnv,
    /// Add an existing account address to the keystore
    AddAccount {
        /// The object ID of the account
        address: IotaAddress,
        /// The alias must start with a letter and can contain only letters,
        /// digits, hyphens (-), or underscores (_)
        #[arg(long)]
        alias: Option<String>,
    },
    /// Obtain the Addresses managed by the client.
    Addresses {
        /// Sort by alias instead of address
        #[arg(long, short = 's')]
        sort_by_alias: bool,
    },
    /// List the coin balance of an address
    Balance {
        /// Address (or its alias)
        #[arg(value_parser)]
        address: Option<KeyIdentity>,
        /// Show balance for the specified coin (e.g., 0x2::iota::IOTA).
        /// All coins will be shown if none is passed.
        #[arg(long, required = false)]
        coin_type: Option<String>,
        /// Show a list with each coin's object ID and balance
        #[arg(long, required = false)]
        with_coins: bool,
    },
    /// Call Move function
    Call {
        /// Object ID of the package, which contains the module
        #[arg(long)]
        package: ObjectID,
        /// The name of the module in the package
        #[arg(long)]
        module: String,
        /// Function name in module
        #[arg(long)]
        function: String,
        /// Type arguments to the generic function being called.
        /// All must be specified, or the call will fail.
        #[arg(
            long,
            value_parser = parse_iota_type_tag,
            num_args(1..),
        )]
        type_args: Vec<TypeTag>,
        /// Simplified ordered args like in the function syntax
        /// ObjectIDs, Addresses must be hex strings
        #[arg(long, num_args(1..))]
        args: Vec<IotaJsonValue>,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Query the chain identifier from the rpc endpoint.
    ChainIdentifier,
    /// Query a dynamic field by its address.
    #[command(name = "dynamic-field")]
    DynamicFieldQuery {
        /// The ID of the parent object
        #[arg(name = "object_id")]
        id: ObjectID,
        /// Optional paging cursor
        #[arg(long)]
        cursor: Option<ObjectID>,
        /// Maximum item returned per page
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// List all IOTA environments
    Envs,
    /// Execute a Signed Transaction. This is useful when the user prefers to
    /// sign elsewhere and use this command to execute.
    ExecuteSignedTx {
        /// BCS serialized transaction data bytes without its type tag, as
        /// base64 encoded string. This is the output of iota client command
        /// using --serialize-unsigned-transaction.
        #[arg(long)]
        tx_bytes: String,
        /// A list of Base64 encoded signatures `flag || signature || pubkey`,
        /// separated by space.
        #[arg(long, num_args(1..), required = true)]
        signatures: Vec<String>,
    },
    /// Execute a combined serialized SenderSignedData string.
    ExecuteCombinedSignedTx {
        /// BCS serialized sender signed data, as base64 encoded string. This is
        /// the output of iota client command using
        /// --serialize-signed-transaction.
        #[arg(long)]
        signed_tx_bytes: String,
    },
    /// Create signature using the private key for the given address (or its
    /// alias) in iota keystore or the auth args for a MoveAuthenticator. Any
    /// signature commits to a [struct IntentMessage] consisting of the
    /// Base64 encoded of the BCS serialized transaction bytes itself and
    /// its intent. If intent is absent, default will be used.
    Sign {
        #[arg(long)]
        address: KeyIdentity,
        #[arg(long)]
        data: String,
        #[arg(long)]
        intent: Option<Intent>,
        /// Auth input objects or primitive values for the Move authenticate
        /// function
        #[arg(long, num_args = 1..)]
        auth_call_args: Option<Vec<String>>,
        /// Auth type arguments for the Move authenticate function
        #[arg(long, num_args = 1..)]
        auth_type_args: Option<Vec<String>>,
    },
    /// Request gas coin from faucet. By default, it will use the active address
    /// and the active network.
    Faucet {
        /// Address (or its alias)
        #[arg(long, value_parser)]
        address: Option<KeyIdentity>,
        /// The url to the faucet
        #[arg(long)]
        url: Option<String>,
    },
    /// Obtain all gas objects owned by the address.
    /// An address' alias can be used instead of the address.
    Gas {
        /// Address (or its alias) owning the objects
        #[arg(name = "owner_address", value_parser)]
        address: Option<KeyIdentity>,
    },
    /// Merge two coin objects into one coin
    MergeCoin {
        /// The address of the coin to merge into.
        #[arg(long)]
        primary_coin: ObjectID,
        /// The address of the coin to be merged.
        #[arg(long)]
        coin_to_merge: ObjectID,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Generate new address and keypair with optional key scheme {ed25519 |
    /// secp256k1 | secp256r1} which defaults to ed25519, optional alias which
    /// defaults to a random one, optional word length { word12 | word15 |
    /// word18 | word21 | word24} which defaults to word12, and optional
    /// derivation path which defaults to m/44'/4218'/0'/0'/0' for ed25519,
    /// m/54'/4218'/0'/0/0 for secp256k1 or m/74'/4218'/0'/0/0 for secp256r1.
    NewAddress {
        #[arg(long, default_value_t = SignatureScheme::ED25519)]
        key_scheme: SignatureScheme,
        /// The alias must start with a letter and can contain only letters,
        /// digits, hyphens (-), or underscores (_).
        #[arg(long)]
        alias: Option<String>,
        #[arg(long)]
        word_length: Option<String>,
        #[arg(long)]
        derivation_path: Option<DerivationPath>,
    },
    /// Add new IOTA environment.
    NewEnv {
        /// The alias for the environment.
        #[arg(long)]
        alias: String,
        /// The RPC Url, for example http://127.0.0.1:9000.
        #[arg(long, value_hint = ValueHint::Url)]
        rpc: String,
        /// Optional GraphQL Url, for example http://127.0.0.1:8000.
        #[arg(long, value_hint = ValueHint::Url)]
        graphql: Option<String>,
        /// Optional WebSocket Url, for example ws://127.0.0.1:9000.
        #[arg(long, value_hint = ValueHint::Url)]
        ws: Option<String>,
        #[arg(long, help = "Basic auth in the format of username:password")]
        basic_auth: Option<String>,
        /// Optional faucet Url, for example http://127.0.0.1:9123/v1/gas.
        #[arg(long, value_hint = ValueHint::Url)]
        faucet: Option<String>,
    },
    /// Get object info
    Object {
        /// Object ID of the object to fetch
        #[arg(name = "object_id")]
        id: ObjectID,
        /// Return the bcs serialized version of the object
        #[arg(long)]
        bcs: bool,
    },
    /// Obtain all objects owned by the address. It also accepts an address by
    /// its alias.
    Objects {
        /// Address owning the object. If no address is provided, it will show
        /// all objects owned by `iota client active-address`.
        #[arg(name = "owner_address")]
        address: Option<KeyIdentity>,
    },
    /// Pay coins to recipients following specified amounts, with input coins.
    /// Length of recipients must be the same as that of amounts.
    Pay {
        /// The input coins to be used for pay recipients, following the
        /// specified amounts.
        #[arg(long, num_args(1..))]
        input_coins: Vec<ObjectID>,
        /// The recipient addresses, must be of same length as amounts.
        /// Aliases of addresses are also accepted as input.
        #[arg(long, num_args(1..))]
        recipients: Vec<KeyIdentity>,
        /// The amounts to be paid, following the order of recipients.
        #[arg(long, num_args(1..))]
        amounts: Vec<u64>,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Pay all residual IOTA coins to the recipient with input coins, after
    /// deducting the gas cost. The input coins also include the coin for
    /// gas payment, so no extra gas coin is required.
    PayAllIota {
        /// The input coins to be used for pay recipients, including the gas
        /// coin.
        #[arg(long, num_args(1..))]
        input_coins: Vec<ObjectID>,
        /// The recipient address (or its alias if it's an address in the
        /// keystore).
        #[arg(long)]
        recipient: KeyIdentity,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Pay IOTA coins to recipients following specified amounts, with input
    /// coins. Length of recipients must be the same as that of amounts.
    /// The input coins also include the coin for gas payment, so no extra gas
    /// coin is required.
    PayIota {
        /// The input coins to be used for pay recipients, including the gas
        /// coin. If not provided, coins will be selected automatically which
        /// fulfill the requested amounts.
        #[arg(long, num_args(1..))]
        input_coins: Option<Vec<ObjectID>>,
        /// The recipient addresses, must be of same length as amounts.
        /// Aliases of addresses are also accepted as input.
        #[arg(long, num_args(1..))]
        recipients: Vec<KeyIdentity>,
        /// The amounts to be paid, following the order of recipients.
        #[arg(long, num_args(1..))]
        amounts: Vec<u64>,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Run a PTB from the provided args
    PTB(PTB),
    /// Publish Move modules
    Publish {
        /// Path to directory containing a Move package
        #[arg(name = "package_path", global = true, default_value = ".")]
        package_path: PathBuf,
        /// Package build options
        #[command(flatten)]
        build_config: MoveBuildConfig,
        /// Publish the package without checking whether dependency source code
        /// compiles to the on-chain bytecode.
        #[arg(long)]
        skip_dependency_verification: bool,
        /// Check that the dependency source code compiles to the on-chain
        /// bytecode before publishing the package (currently the
        /// default behavior)
        #[arg(long, conflicts_with = "skip_dependency_verification")]
        verify_deps: bool,
        /// Also publish transitive dependencies that have not already been
        /// published.
        #[arg(long)]
        with_unpublished_dependencies: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Execute, dry-run, dev-inspect or otherwise inspect an already serialized
    /// transaction.
    SerializedTx {
        /// Base64-encoded BCS-serialized TransactionData.
        tx_bytes: String,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Execute, dry-run, dev-inspect or otherwise inspect an already serialized
    /// transaction kind.
    SerializedTxKind {
        /// Base64-encoded BCS-serialized TransactionKind.
        tx_bytes: String,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Split a coin object into multiple coins.
    #[command(group(ArgGroup::new("split").required(true).args(&["amounts", "count"])))]
    SplitCoin {
        /// ID of the coin object to split
        #[arg(long)]
        coin_id: ObjectID,
        /// Specific amounts to split out from the coin, separated by space,
        /// e.g. `--amounts 1 2 1000000000` (1 NANO, 2 NANOS, 1 IOTA)
        #[arg(long, num_args(1..))]
        amounts: Option<Vec<u64>>,
        /// Count of equal-size coins to split into
        #[arg(long)]
        count: Option<u64>,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Switch active address and env (e.g. testnet, devnet, localnet, ...).
    Switch {
        /// An address to be used as the active address for subsequent
        /// commands. It accepts also the alias of the address.
        #[arg(long)]
        address: Option<KeyIdentity>,
        /// The alias of the env (e.g. testnet, devnet, localnet, ...) to be
        /// used for subsequent commands.
        #[arg(long)]
        env: Option<String>,
    },
    /// Get a transaction block with the effects, events and object changes of
    /// its execution
    #[command(name = "tx-block")]
    TransactionBlock {
        /// Digest of the transaction block
        #[arg(name = "digest")]
        digest: TransactionDigest,
    },
    /// Transfer object
    Transfer {
        /// Recipient address (or its alias if it's an address in the keystore)
        #[arg(long)]
        to: KeyIdentity,
        /// ID of the object to transfer
        #[arg(long)]
        object_id: ObjectID,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Upgrade Move modules
    Upgrade {
        /// Path to directory containing a Move package
        #[arg(name = "package_path", global = true, default_value = ".")]
        package_path: PathBuf,
        /// ID of the upgrade capability for the package being upgraded.
        #[arg(long, short = 'c')]
        upgrade_capability: ObjectID,
        /// Package build options
        #[command(flatten)]
        build_config: MoveBuildConfig,
        /// Verify package compatibility locally before publishing.
        #[arg(long)]
        verify_compatibility: bool,
        /// Upgrade the package without checking whether dependency source code
        /// compiles to the on-chain bytecode
        #[arg(long)]
        skip_dependency_verification: bool,
        /// Check that the dependency source code compiles to the on-chain
        /// bytecode before upgrading the package (currently the default
        /// behavior)
        #[arg(long, conflicts_with = "skip_dependency_verification")]
        verify_deps: bool,
        /// Also publish transitive dependencies that have not already been
        /// published.
        #[arg(long)]
        with_unpublished_dependencies: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Run the bytecode verifier on the package
    VerifyBytecodeMeter {
        /// Path to directory containing a Move package, (defaults to the
        /// current directory)
        #[arg(name = "package", long, global = true)]
        package_path: Option<PathBuf>,
        /// Protocol version to use for the bytecode verifier (defaults to the
        /// latest protocol version)
        #[arg(name = "protocol-version", long)]
        protocol_version: Option<u64>,
        /// Paths to specific pre-compiled module bytecode to verify (instead of
        /// an entire package). Multiple modules can be verified by
        /// passing multiple --module flags. They will be treated as if
        /// they were one package (subject to the overall package limit).
        #[arg(name = "module", long, action = clap::ArgAction::Append, global = true)]
        module_paths: Vec<PathBuf>,
        /// Package build options
        #[command(flatten)]
        build_config: MoveBuildConfig,
    },
    /// Verify local Move packages against on-chain packages, and optionally
    /// their dependencies.
    VerifySource {
        /// Path to directory containing a Move package
        #[arg(name = "package_path", global = true, default_value = ".")]
        package_path: PathBuf,
        /// Package build options
        #[command(flatten)]
        build_config: MoveBuildConfig,
        /// Verify on-chain dependencies.
        #[arg(long)]
        verify_deps: bool,
        /// Don't verify source (only valid if --verify-deps is enabled).
        #[arg(long)]
        skip_source: bool,
        /// If specified, override the addresses for the package's own modules
        /// with this address. Only works for unpublished modules (whose
        /// addresses are currently 0x0).
        #[arg(long)]
        address_override: Option<ObjectID>,
    },
    /// Remove an existing address by its alias or hexadecimal string.
    /// Warning: removes the private key from the keystore with no way to
    /// recover it if it was not backed up.
    RemoveAddress { address: KeyIdentity },
}

/// Arguments used to provide coins for a gas payment
#[derive(Args, Debug, Default)]
pub struct PaymentArgs {
    /// IDs of gas objects to be used for gas payment. If none are provided,
    /// coins are selected automatically to cover the gas budget.
    #[arg(long, num_args(1..))]
    pub gas: Vec<ObjectID>,
}

impl PaymentArgs {
    /// Output the payment args as a vec of strings for CLI usage.
    pub fn into_args(self) -> Vec<String> {
        self.gas
            .into_iter()
            .map(|gas_id| format!("--gas {gas_id}"))
            .collect()
    }
}

/// Arguments related to setting gas data, apart from payment coins.
#[derive(Args, Debug, Default)]
pub struct GasDataArgs {
    /// An optional gas budget for this transaction (in NANOs). If gas budget is
    /// not provided, the tool will first perform a dry run to estimate the
    /// gas cost, and then it will execute the transaction. Please note that
    /// this incurs a small cost in performance due to the additional
    /// dry run call.
    #[arg(long)]
    pub gas_budget: Option<u64>,
    /// An optional gas price for this transaction (in NANOs). If gas price is
    /// not provided, the tool will use the current reference gas price from
    /// RPC.
    ///
    /// Transactions with a gas price lower than the reference will not be
    /// signed by enough validators to execute. Transactions accessing
    /// congested shared objects are prioritized by gas price, so setting a
    /// higher gas price higher than the reference can ensure the
    /// transaction accesses the shared object sooner.
    #[arg(long)]
    pub gas_price: Option<u64>,
    /// An optional field to specify a gas sponsor address. If provided, the gas
    /// owner is set to this address, rather than the transaction's sender.
    ///
    /// Note that if the CLI does not have access to the sponsor's keys, it will
    /// not be able to sign and execute transactions that have a sponsor
    /// set.
    #[arg(long)]
    pub gas_sponsor: Option<IotaAddress>,
}

impl GasDataArgs {
    /// Output the gas data args as a vec of strings for CLI usage.
    pub fn into_args(self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(gas_budget) = self.gas_budget {
            args.push(format!("--gas-budget {gas_budget}"));
        }
        if let Some(gas_price) = self.gas_price {
            args.push(format!("--gas-price {gas_price}"));
        }
        if let Some(gas_sponsor) = self.gas_sponsor {
            args.push(format!("--gas-sponsor @{gas_sponsor}"));
        }
        args
    }
}

/// Arguments specifying how to use a transaction after it has been built.
#[derive(Args, Debug, Default)]
pub struct TxProcessingArgs {
    /// Compute the transaction digest and print it out, but do not execute the
    /// transaction.
    #[arg(long)]
    pub tx_digest: bool,
    /// Perform a dry run of the transaction, without executing it.
    #[arg(long)]
    pub dry_run: bool,
    /// Perform a dev inspect of the transaction, without executing it.
    #[arg(long)]
    pub dev_inspect: bool,
    /// Instead of executing the transaction, serialize the bcs bytes of the
    /// unsigned transaction data (TransactionData) using base64 encoding,
    /// and print out the string <TX_BYTES>. The string can be used to
    /// execute transaction with `iota client execute-signed-tx --tx-bytes
    /// <TX_BYTES>`.
    #[arg(long)]
    pub serialize_unsigned_transaction: bool,
    /// Instead of executing the transaction, serialize the bcs bytes of the
    /// signed transaction data (SenderSignedData) using base64 encoding,
    /// and print out the string <SIGNED_TX_BYTES>. The string can be used
    /// to execute transaction with `iota client execute-combined-signed-tx
    /// --signed-tx-bytes <SIGNED_TX_BYTES>`.
    #[arg(long)]
    pub serialize_signed_transaction: bool,
    /// Set the transaction sender to this address. When not specified, the
    /// sender is inferred by finding the owner of the gas payment. Note
    /// that when setting this field, the transaction will fail to execute
    /// if the sender's private key is not in the keystore; similarly, it
    /// will fail when using this with `--serialize-signed-transaction` flag if
    /// the private key corresponding to this address is not in keystore.
    #[arg(long, required = false, value_parser)]
    pub sender: Option<IotaAddress>,
    /// Select which fields of the response to display.
    /// If not provided, all fields are displayed.
    /// The fields are: input, effects, events, object_changes,
    /// balance_changes.
    #[arg(long, required = false, num_args = 0.., value_parser = parse_display_option, default_value = "input,effects,events,object_changes,balance_changes")]
    pub display: HashSet<DisplayOption>,
    /// Auth input objects or primitive values for the Move authenticate
    /// function
    #[arg(long, num_args = 1..)]
    pub auth_call_args: Option<Vec<String>>,
    /// Auth type arguments for the Move authenticate function
    #[arg(long, num_args = 1..)]
    pub auth_type_args: Option<Vec<String>>,
}

impl TxProcessingArgs {
    /// Output the tx processing args as a vec of strings for CLI usage.
    pub fn into_args(self) -> Vec<String> {
        let mut args = Vec::new();
        if self.tx_digest {
            args.push("--tx-digest".to_string());
        }
        if self.dry_run {
            args.push("--dry-run".to_string());
        }
        if self.dev_inspect {
            args.push("--dev-inspect".to_string());
        }
        if self.serialize_unsigned_transaction {
            args.push("--serialize-unsigned-transaction".to_string());
        }
        if self.serialize_signed_transaction {
            args.push("--serialize-signed-transaction".to_string());
        }
        if let Some(sender) = self.sender {
            args.push(format!("--sender @{sender}"));
        }
        if !self.display.is_empty() {
            let display_fields = self
                .display
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            args.push(format!("--display {display_fields}"));
        }
        args
    }
}

#[derive(Clone, Debug, Display, EnumString, Hash, Eq, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum DisplayOption {
    Input,
    Effects,
    Events,
    ObjectChanges,
    BalanceChanges,
}

#[derive(serde::Deserialize)]
struct FaucetResponse {
    error: Option<String>,
}

impl IotaClientCommands {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<IotaClientCommandResult, anyhow::Error> {
        let ret = match self {
            IotaClientCommands::AddAccount { address, alias } => {
                let client = context.get_client().await?;

                let _auth_info = fetch_auth_info(&client, address).await?;

                context
                    .config_mut()
                    .keystore_mut()
                    .add_key(alias, StoredKey::Account(address))?;

                let alias = context.config().keystore().get_alias_by_address(&address)?;

                IotaClientCommandResult::AddAccount(AddAccountOutput { address, alias })
            }
            IotaClientCommands::RemoveAddress { address } => {
                let address = get_identity_address(Some(address), context).await?;

                if context.config().keystore().get_key(&address).is_ok() {
                    context.config_mut().keystore_mut().remove_key(&address)?;
                    if context
                        .config()
                        .active_address()
                        .is_some_and(|a| a == address)
                    {
                        context.config_mut().set_active_address(None);
                        context.config().save()?;
                    }
                } else {
                    bail!("Address \"{address}\" does not exist in keystore");
                }

                IotaClientCommandResult::RemoveAddress(address)
            }
            IotaClientCommands::Addresses { sort_by_alias } => {
                let active_address = context.active_address()?;
                let mut addresses = context
                    .config()
                    .keystore()
                    .addresses_with_alias()
                    .into_iter()
                    .map(|(address, alias)| {
                        // Unwrap is safe here because the address comes from iterating the keystore
                        let source = context
                            .config()
                            .keystore()
                            .get_key(address)
                            .unwrap()
                            .source()
                            .to_string();
                        (alias.alias.to_string(), *address, source)
                    })
                    .collect::<Vec<_>>();

                if sort_by_alias {
                    addresses.sort_by(|a, b| a.0.cmp(&b.0));
                }

                let output = AddressesOutput {
                    active_address,
                    addresses,
                };
                IotaClientCommandResult::Addresses(output)
            }
            IotaClientCommands::Balance {
                address,
                coin_type,
                with_coins,
            } => {
                let address = get_identity_address(address, context).await?;
                let client = context.get_client().await?;

                let objects =
                    PagedFn::collect::<Vec<_>>(async |cursor: Option<ObjectID>| match coin_type {
                        Some(ref coin_type) => {
                            client
                                .coin_read_api()
                                .get_coins(address, Some(coin_type.clone()), cursor, None)
                                .await
                        }
                        None => {
                            client
                                .coin_read_api()
                                .get_all_coins(address, cursor, None)
                                .await
                        }
                    })
                    .await?;

                fn canonicalize_type(type_: &str) -> Result<String, anyhow::Error> {
                    Ok(TypeTag::from_str(type_)
                        .context("Cannot parse coin type")?
                        .to_canonical_string(/* with_prefix */ true))
                }

                let mut coins_by_type = BTreeMap::new();
                for c in objects {
                    let coins = match coins_by_type.entry(canonicalize_type(&c.coin_type)?) {
                        Entry::Vacant(entry) => {
                            let metadata = client
                                .coin_read_api()
                                .get_coin_metadata(c.coin_type.clone())
                                .await
                                .with_context(|| {
                                    format!(
                                        "Cannot fetch the coin metadata for coin {}",
                                        c.coin_type
                                    )
                                })?;

                            &mut entry.insert((metadata, vec![])).1
                        }
                        Entry::Occupied(entry) => &mut entry.into_mut().1,
                    };

                    coins.push(c);
                }
                let iota_type_tag = canonicalize_type(IOTA_COIN_TYPE)?;

                // show IOTA first
                let ordered_coins_iota_first = coins_by_type
                    .remove(&iota_type_tag)
                    .into_iter()
                    .chain(coins_by_type.into_values())
                    .collect();

                IotaClientCommandResult::Balance(ordered_coins_iota_first, with_coins)
            }
            IotaClientCommands::DynamicFieldQuery { id, cursor, limit } => {
                let client = context.get_client().await?;
                let df_read = client
                    .read_api()
                    .get_dynamic_fields(id, cursor, Some(limit))
                    .await?;
                IotaClientCommandResult::DynamicFieldQuery(df_read)
            }
            IotaClientCommands::Upgrade {
                package_path,
                upgrade_capability,
                build_config,
                skip_dependency_verification,
                verify_deps,
                verify_compatibility,
                with_unpublished_dependencies,
                payment,
                gas_data,
                processing,
            } => {
                let sender = if let Some(sender) = processing.sender {
                    sender
                } else {
                    context.infer_sender(&payment.gas).await?
                };
                let client = context.get_client().await?;
                let read_api = client.read_api();
                let chain_id = read_api.get_chain_identifier().await.ok();

                check_protocol_version_and_warn(read_api).await?;

                if !package_path.exists() {
                    bail!("Package path '{}' does not exist", package_path.display());
                }

                let package_path =
                    package_path
                        .canonicalize()
                        .map_err(|e| IotaError::ModulePublishFailure {
                            error: format!("Failed to canonicalize package path: {e}"),
                        })?;
                let build_config = resolve_lock_file_path(build_config, Some(&package_path))?;
                let previous_id = if let Some(ref chain_id) = chain_id {
                    iota_package_management::set_package_id(
                        &package_path,
                        build_config.install_dir.clone(),
                        chain_id,
                        AccountAddress::ZERO,
                    )?
                } else {
                    None
                };
                let env_alias = context.active_env().map(|e| e.alias().clone()).ok();
                let verify =
                    check_dep_verification_flags(skip_dependency_verification, verify_deps)?;
                let upgrade_result = upgrade_package(
                    read_api,
                    build_config.clone(),
                    &package_path,
                    upgrade_capability,
                    with_unpublished_dependencies,
                    !verify,
                    env_alias,
                )
                .await;

                // Restore original ID, then check result.
                if let (Some(chain_id), Some(previous_id)) = (&chain_id, previous_id) {
                    let _ = iota_package_management::set_package_id(
                        &package_path,
                        build_config.install_dir.clone(),
                        chain_id,
                        previous_id,
                    )?;
                }

                let (upgrade_policy, compiled_package) =
                    upgrade_result.map_err(|e| anyhow!("{e}"))?;

                let compiled_modules =
                    compiled_package.get_package_bytes(with_unpublished_dependencies);
                let package_id = compiled_package.published_at.clone()?;
                let package_digest =
                    compiled_package.get_package_digest(with_unpublished_dependencies);
                let dep_ids = compiled_package.get_published_dependencies_ids();

                if verify_compatibility {
                    let protocol_version =
                        read_api.get_protocol_config(None).await?.protocol_version;

                    ensure!(
                        ProtocolVersion::MAX >= protocol_version,
                        "On-chain protocol version ({}) is ahead of the latest known version ({}) in the CLI.\
                        Please update the CLI to the latest version if you want to use --verify-compatibility flag.",
                        protocol_version.as_u64(),
                        ProtocolVersion::MAX.as_u64()
                    );

                    let protocol_config = ProtocolConfig::get_for_version(
                        protocol_version,
                        match chain_id
                            .as_ref()
                            .and_then(ChainIdentifier::from_chain_short_id)
                        {
                            Some(chain_id) => chain_id.chain(),
                            None => Chain::Unknown,
                        },
                    );

                    check_compatibility(
                        read_api,
                        package_id,
                        compiled_package,
                        package_path,
                        upgrade_policy,
                        protocol_config,
                    )
                    .await?;
                }

                let tx_kind = client
                    .transaction_builder()
                    .upgrade_tx_kind(
                        package_id,
                        compiled_modules,
                        dep_ids,
                        upgrade_capability,
                        upgrade_policy,
                        package_digest.to_vec(),
                    )
                    .await?;

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                let result = dry_run_or_execute_or_serialize(
                    sender,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?;

                if let IotaClientCommandResult::TransactionBlock(ref response) = result {
                    if let Err(e) = iota_package_management::update_lock_file(
                        context,
                        LockCommand::Upgrade,
                        build_config.install_dir,
                        build_config.lock_file,
                        response,
                    )
                    .await
                    {
                        eprintln!(
                            "{} {e}",
                            "Warning: Issue while updating `Move.lock` for published package."
                                .bold()
                                .yellow()
                        )
                    };
                };
                result
            }
            IotaClientCommands::Publish {
                package_path,
                build_config,
                skip_dependency_verification,
                verify_deps,
                with_unpublished_dependencies,
                payment,
                gas_data,
                processing,
            } => {
                if build_config.test_mode {
                    return Err(IotaError::ModulePublishFailure {
                        error:
                            "The `publish` subcommand should not be used with the `--test` flag\n\
                            \n\
                            Code in published packages must not depend on test code.\n\
                            In order to fix this and publish the package without `--test`, \
                            remove any non-test dependencies on test-only code.\n\
                            You can ensure all test-only dependencies have been removed by \
                            compiling the package normally with `iota move build`."
                                .to_string(),
                    }
                    .into());
                }

                let sender = if let Some(sender) = processing.sender {
                    sender
                } else {
                    context.infer_sender(&payment.gas).await?
                };
                let client = context.get_client().await?;
                let read_api = client.read_api();
                let chain_id = read_api.get_chain_identifier().await.ok();

                check_protocol_version_and_warn(read_api).await?;

                if !package_path.exists() {
                    bail!("Package path '{}' does not exist", package_path.display());
                }

                let package_path =
                    package_path
                        .canonicalize()
                        .map_err(|e| IotaError::ModulePublishFailure {
                            error: format!("Failed to canonicalize package path: {e}"),
                        })?;
                let build_config = resolve_lock_file_path(build_config, Some(&package_path))?;
                let previous_id = if let Some(ref chain_id) = chain_id {
                    iota_package_management::set_package_id(
                        &package_path,
                        build_config.install_dir.clone(),
                        chain_id,
                        AccountAddress::ZERO,
                    )?
                } else {
                    None
                };
                let verify =
                    check_dep_verification_flags(skip_dependency_verification, verify_deps)?;
                let compile_result = compile_package(
                    read_api,
                    build_config.clone(),
                    &package_path,
                    with_unpublished_dependencies,
                    !verify,
                )
                .await;
                // Restore original ID, then check result.
                if let (Some(chain_id), Some(previous_id)) = (chain_id, previous_id) {
                    let _ = iota_package_management::set_package_id(
                        &package_path,
                        build_config.install_dir.clone(),
                        &chain_id,
                        previous_id,
                    )?;
                }

                let compiled_package = compile_result?;
                let compiled_modules =
                    compiled_package.get_package_bytes(with_unpublished_dependencies);
                let dep_ids = compiled_package.get_published_dependencies_ids();

                let tx_kind = client
                    .transaction_builder()
                    .publish_tx_kind(sender, compiled_modules, dep_ids)
                    .await?;
                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;
                let result = dry_run_or_execute_or_serialize(
                    sender,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?;

                if let IotaClientCommandResult::TransactionBlock(ref response) = result {
                    if let Err(e) = iota_package_management::update_lock_file(
                        context,
                        LockCommand::Publish,
                        build_config.install_dir,
                        build_config.lock_file,
                        response,
                    )
                    .await
                    {
                        eprintln!(
                            "{} {e}",
                            "Warning: Issue while updating `Move.lock` for published package."
                                .bold()
                                .yellow()
                        )
                    };
                };
                result
            }
            IotaClientCommands::VerifyBytecodeMeter {
                protocol_version,
                module_paths,
                package_path,
                build_config,
            } => {
                let client = context.get_client().await?;
                let read_api = client.read_api();
                let protocol_version =
                    protocol_version.map_or(ProtocolVersion::MAX, ProtocolVersion::new);
                let protocol_config =
                    ProtocolConfig::get_for_version(protocol_version, Chain::Unknown);

                let registry = &Registry::new();
                let bytecode_verifier_metrics = Arc::new(BytecodeVerifierMetrics::new(registry));

                let (pkg_name, modules) = match (module_paths, package_path) {
                    (paths, Some(_)) if !paths.is_empty() => {
                        bail!("Cannot specify both a module path and a package path")
                    }

                    (paths, None) if !paths.is_empty() => {
                        let mut modules = Vec::with_capacity(paths.len());
                        for path in paths {
                            let module_bytes =
                                fs::read(path).context("Failed to read module file")?;
                            let module = CompiledModule::deserialize_with_defaults(&module_bytes)
                                .context("Failed to deserialize module")?;
                            modules.push(module);
                        }
                        ("<unknown>".to_string(), modules)
                    }

                    (_, package_path) => {
                        let package_path = package_path.unwrap_or_else(|| PathBuf::from("."));
                        let package =
                            compile_package_simple(read_api, build_config, &package_path, None)
                                .await?;
                        let name = package
                            .package
                            .compiled_package_info
                            .package_name
                            .to_string();
                        (name, package.get_modules().cloned().collect())
                    }
                };

                let signing_limits = Some(VerifierSigningConfig::default().limits_for_signing());
                let mut verifier = iota_execution::verifier(
                    &protocol_config,
                    signing_limits,
                    &bytecode_verifier_metrics,
                );

                println!(
                    "Running bytecode verifier for {} module{}",
                    modules.len(),
                    if modules.len() != 1 { "s" } else { "" },
                );

                let mut meter = AccumulatingMeter::new();
                verifier.meter_compiled_modules(&protocol_config, &modules, &mut meter)?;

                let mut used_ticks = meter.accumulator(Scope::Package).clone();
                used_ticks.name = pkg_name;

                let meter_config = VerifierSigningConfig::default().meter_config_for_signing();

                let exceeded = matches!(
                    meter_config.max_per_pkg_meter_units,
                    Some(allowed_ticks) if allowed_ticks < used_ticks.max_ticks(Scope::Package)
                ) || matches!(
                    meter_config.max_per_mod_meter_units,
                    Some(allowed_ticks) if allowed_ticks < used_ticks.max_ticks(Scope::Module)
                ) || matches!(
                    meter_config.max_per_fun_meter_units,
                    Some(allowed_ticks) if allowed_ticks < used_ticks.max_ticks(Scope::Function)
                );

                IotaClientCommandResult::VerifyBytecodeMeter {
                    success: !exceeded,
                    max_package_ticks: meter_config.max_per_pkg_meter_units,
                    max_module_ticks: meter_config.max_per_mod_meter_units,
                    max_function_ticks: meter_config.max_per_fun_meter_units,
                    used_ticks,
                }
            }
            IotaClientCommands::Object { id, bcs } => {
                // Fetch the object ref
                let client = context.get_client().await?;
                if !bcs {
                    let object_read = client
                        .read_api()
                        .get_object_with_options(id, IotaObjectDataOptions::full_content())
                        .await?;
                    IotaClientCommandResult::Object(object_read)
                } else {
                    let raw_object_read = client
                        .read_api()
                        .get_object_with_options(id, IotaObjectDataOptions::bcs_lossless())
                        .await?;
                    IotaClientCommandResult::RawObject(raw_object_read)
                }
            }
            IotaClientCommands::TransactionBlock { digest } => {
                let client = context.get_client().await?;
                let tx_read = client
                    .read_api()
                    .get_transaction_with_options(
                        digest,
                        IotaTransactionBlockResponseOptions {
                            show_input: true,
                            show_raw_input: false,
                            show_effects: true,
                            show_events: true,
                            show_object_changes: true,
                            show_balance_changes: false,
                            show_raw_effects: false,
                        },
                    )
                    .await?;
                IotaClientCommandResult::TransactionBlock(tx_read)
            }
            IotaClientCommands::Call {
                package,
                module,
                function,
                type_args,
                args,
                payment,
                gas_data,
                processing,
            } => {
                // Convert all numeric input to String, this will allow number input from the
                // CLI without failing IotaJSON's checks.
                let args = args
                    .into_iter()
                    .map(|value| {
                        IotaJsonValue::new(convert_number_to_string(value.to_json_value()))
                    })
                    .collect::<Result<_, _>>()?;

                let type_args = type_args
                    .into_iter()
                    .map(|arg| arg.into())
                    .collect::<Vec<_>>();

                let client = context.get_client().await?;

                let tx_kind = client
                    .transaction_builder()
                    .move_call_tx_kind(package, &module, &function, type_args, args)
                    .await?;

                let sender = if let Some(sender) = processing.sender {
                    sender
                } else {
                    context.infer_sender(&payment.gas).await?
                };
                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    sender,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::Transfer {
                to,
                object_id,
                payment,
                gas_data,
                processing,
            } => {
                let signer = context.get_object_owner(&object_id).await?;
                let to = get_identity_address(Some(to), context).await?;
                let client = context.get_client().await?;
                let tx_kind = client
                    .transaction_builder()
                    .transfer_object_tx_kind(object_id, to)
                    .await?;

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::Pay {
                input_coins,
                recipients,
                amounts,
                payment,
                gas_data,
                processing,
            } => {
                ensure!(
                    !input_coins.is_empty(),
                    "Pay transaction requires a non-empty list of input coins"
                );
                ensure!(
                    !recipients.is_empty(),
                    "Pay transaction requires a non-empty list of recipient addresses"
                );
                ensure!(
                    recipients.len() == amounts.len(),
                    format!(
                        "Found {:?} recipient addresses, but {:?} recipient amounts",
                        recipients.len(),
                        amounts.len()
                    ),
                );
                let recipients = futures::stream::iter(recipients)
                    .then(|x| async { get_identity_address(Some(x), context).await })
                    .try_collect::<Vec<IotaAddress>>()
                    .await?;
                let signer = context.get_object_owner(&input_coins[0]).await?;
                let client = context.get_client().await?;
                let tx_kind = client
                    .transaction_builder()
                    .pay_tx_kind(input_coins.clone(), recipients.clone(), amounts.clone())
                    .await?;

                ensure!(
                    !payment.gas.iter().any(|gas| input_coins.contains(gas)),
                    "Gas coin is in input coins of `pay` command, use `pay-iota` instead!"
                );

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::PayIota {
                input_coins,
                recipients,
                amounts,
                mut gas_data,
                processing,
            } => {
                ensure!(
                    !input_coins.as_ref().is_some_and(|v| v.is_empty()),
                    "PayIota transaction requires a non-empty list of input coins"
                );
                ensure!(
                    !recipients.is_empty(),
                    "PayIota transaction requires a non-empty list of recipient addresses"
                );
                ensure!(
                    recipients.len() == amounts.len(),
                    format!(
                        "Found {:?} recipient addresses, but {:?} recipient amounts",
                        recipients.len(),
                        amounts.len()
                    ),
                );

                let recipients = futures::stream::iter(recipients)
                    .then(|x| async { get_identity_address(Some(x), context).await })
                    .try_collect::<Vec<IotaAddress>>()
                    .await?;
                let signer =
                    get_identity_address(processing.sender.map(Into::into), context).await?;
                let client = context.get_client().await?;
                let tx_kind = client
                    .transaction_builder()
                    .pay_iota_tx_kind(recipients, amounts.clone())?;

                let input_coins = if let Some(coins) = input_coins {
                    coins
                } else {
                    // Estimate the gas cost if needed
                    let gas_budget = if let Some(gas_budget) = gas_data.gas_budget {
                        gas_budget
                    } else {
                        let gas_price = context.get_reference_gas_price().await?;
                        estimate_gas_budget(
                            context,
                            signer,
                            tx_kind.clone(),
                            gas_price,
                            Vec::new(),
                            None,
                        )
                        .await?
                    };
                    // Ensure that we do not need to estimate again later
                    gas_data.gas_budget = Some(gas_budget);
                    select_coins_for_amount(
                        amounts.iter().sum::<u64>() + gas_budget,
                        signer,
                        context,
                    )
                    .await?
                };

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&input_coins)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::PayAllIota {
                input_coins,
                recipient,
                gas_data,
                processing,
            } => {
                ensure!(
                    !input_coins.is_empty(),
                    "PayAllIota transaction requires a non-empty list of input coins"
                );
                let recipient = get_identity_address(Some(recipient), context).await?;
                let signer = context.get_object_owner(&input_coins[0]).await?;
                let client = context.get_client().await?;
                let tx_kind = client.transaction_builder().pay_all_iota_tx_kind(recipient);

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&input_coins)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::Objects { address } => {
                let address = get_identity_address(address, context).await?;
                let client = context.get_client().await?;
                let objects = PagedFn::collect(async |cursor: Option<ObjectID>| {
                    client
                        .read_api()
                        .get_owned_objects(
                            address,
                            Some(IotaObjectResponseQuery::new_with_options(
                                IotaObjectDataOptions::full_content(),
                            )),
                            cursor,
                            None,
                        )
                        .await
                })
                .await?;
                IotaClientCommandResult::Objects(objects)
            }
            IotaClientCommands::NewAddress {
                key_scheme,
                alias,
                derivation_path,
                word_length,
            } => {
                let (address, phrase, scheme) = context
                    .config_mut()
                    .keystore_mut()
                    .generate_and_add_new_key(
                        key_scheme,
                        alias.clone(),
                        derivation_path,
                        word_length,
                    )?;

                let alias = match alias {
                    Some(x) => x,
                    None => context.config().keystore().get_alias_by_address(&address)?,
                };

                if context.config().active_address().is_none() {
                    context.config_mut().set_active_address(address);
                    context.config().save()?;
                }
                let key = context
                    .config()
                    .keystore()
                    .get_key(&address)
                    .map(Key::from)
                    .expect("missing generated key");

                IotaClientCommandResult::NewAddress(NewAddressOutput {
                    alias,
                    address,
                    public_base64_key: key.public_base64_key.unwrap(),
                    public_base64_key_with_flag: key.public_base64_key_with_flag.unwrap(),
                    key_scheme: scheme,
                    recovery_phrase: phrase,
                })
            }
            IotaClientCommands::Gas { address } => {
                let address = get_identity_address(address, context).await?;
                let coins = context
                    .gas_objects(address)
                    .await?
                    .iter()
                    // Ok to unwrap() since `get_gas_objects` guarantees gas
                    .map(|(_val, object)| GasCoin::try_from(object).unwrap())
                    .collect();
                IotaClientCommandResult::Gas(coins)
            }
            IotaClientCommands::Faucet { address, url } => {
                let address = get_identity_address(address, context).await?;
                let url = if let Some(url) = url {
                    url
                } else {
                    let active_env = context.active_env().map_err(|_| {
                        anyhow::anyhow!(
                            "No URL for faucet was provided and there is no active network."
                        )
                    })?;

                    let faucet_url = if let Some(faucet_url) = active_env.faucet() {
                        faucet_url
                    } else {
                        match active_env.rpc().as_str() {
                            IOTA_DEVNET_URL => IOTA_DEVNET_GAS_URL,
                            IOTA_TESTNET_URL => IOTA_TESTNET_GAS_URL,
                            IOTA_LOCAL_NETWORK_URL | IOTA_LOCAL_NETWORK_URL_0 => {
                                IOTA_LOCAL_NETWORK_GAS_URL
                            }
                            _ => bail!(
                                "Cannot recognize the active network. Please provide the gas faucet full URL."
                            ),
                        }
                    };
                    faucet_url.to_string()
                };
                request_tokens_from_faucet(address, url).await?;
                IotaClientCommandResult::NoOutput
            }
            IotaClientCommands::ChainIdentifier => {
                let ci = context
                    .get_client()
                    .await?
                    .read_api()
                    .get_chain_identifier()
                    .await?;
                IotaClientCommandResult::ChainIdentifier(ci)
            }
            IotaClientCommands::SplitCoin {
                coin_id,
                amounts,
                count,
                payment,
                gas_data,
                processing,
            } => {
                match (amounts.as_ref(), count) {
                    (None, None) => bail!("You must use one of amounts or count options."),
                    (Some(_), Some(_)) => bail!("Cannot specify both amounts and count."),
                    (None, Some(0)) | (None, Some(1)) => {
                        bail!("Coin split count must be greater than 1.")
                    }
                    _ => { /*no_op*/ }
                }

                let client = context.get_client().await?;
                let signer = context.get_object_owner(&coin_id).await?;
                let gas_coins_page = client
                    .coin_read_api()
                    .get_coins(signer, None, None, None)
                    .await?;
                // If we only have a single coin, we have to split from the gas coin
                let tx_kind = if gas_coins_page.data.len() == 1 {
                    if let Some(amounts) = amounts {
                        client
                            .transaction_builder()
                            .pay_iota_tx_kind(vec![signer; amounts.len()], amounts)?
                    } else if let Some(count_to_compute) = count {
                        let amount = gas_coins_page.data[0].balance / count_to_compute;
                        // Reduce by 1 as the gas coin is not included in the split
                        let count_to_split = count_to_compute.saturating_sub(1);
                        client.transaction_builder().pay_iota_tx_kind(
                            vec![signer; count_to_split as usize],
                            vec![amount; count_to_split as usize],
                        )?
                    } else {
                        unreachable!("amount or count must be provided")
                    }
                } else {
                    client
                        .transaction_builder()
                        .split_coin_tx_kind(coin_id, amounts, count)
                        .await?
                };

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::MergeCoin {
                primary_coin,
                coin_to_merge,
                payment,
                gas_data,
                processing,
            } => {
                let client = context.get_client().await?;
                let signer = context.get_object_owner(&primary_coin).await?;

                let tx_kind = client
                    .transaction_builder()
                    .merge_coins_tx_kind(primary_coin, coin_to_merge)
                    .await?;

                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    signer,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::SerializedTx {
                tx_bytes,
                processing,
            } => {
                let Ok(bytes) = Base64::decode(&tx_bytes) else {
                    bail!("Invalid Base64 encoding");
                };

                let Ok(tx_data): Result<TransactionData, _> = bcs::from_bytes(&bytes) else {
                    bail!("Failed to parse --tx-bytes as TransactionData");
                };

                let sender = tx_data.sender();
                let gas_payment = tx_data.gas().to_owned();
                let gas_data = GasDataArgs {
                    gas_budget: Some(tx_data.gas_budget()),
                    gas_price: Some(tx_data.gas_price()),
                    gas_sponsor: Some(tx_data.gas_owner()),
                };
                let tx_kind = tx_data.into_kind();

                dry_run_or_execute_or_serialize(
                    sender,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::SerializedTxKind {
                tx_bytes,
                payment,
                gas_data,
                processing,
            } => {
                let Ok(bytes) = Base64::decode(&tx_bytes) else {
                    bail!("Invalid Base64 encoding");
                };

                let Ok(tx_kind): Result<TransactionKind, _> = bcs::from_bytes(&bytes) else {
                    bail!("Failed to parse --tx-bytes as TransactionKind");
                };

                let client = context.get_client().await?;
                let sender = if let Some(sender) = processing.sender {
                    sender
                } else {
                    context.infer_sender(&payment.gas).await?
                };
                let gas_payment = client
                    .transaction_builder()
                    .input_refs(&payment.gas)
                    .await?;

                dry_run_or_execute_or_serialize(
                    sender,
                    tx_kind,
                    context,
                    gas_payment,
                    gas_data,
                    processing,
                )
                .await?
            }
            IotaClientCommands::Switch { address, env } => {
                let mut addr = None;

                if address.is_none() && env.is_none() {
                    bail!("No address, an alias, or env specified. Please specify one.");
                }

                if let Some(address) = address {
                    let address = get_identity_address(Some(address), context).await?;
                    if !context.config().keystore().addresses().contains(&address) {
                        bail!("Address {} not managed by wallet", address);
                    }
                    context.config_mut().set_active_address(address);
                    addr = Some(address.to_string());
                }

                if let Some(ref env) = env {
                    Self::switch_env(context.config_mut(), env)?;
                }
                context.config().save()?;
                IotaClientCommandResult::Switch(SwitchResponse { address: addr, env })
            }
            IotaClientCommands::ActiveAddress => {
                IotaClientCommandResult::ActiveAddress(context.active_address().ok())
            }
            IotaClientCommands::ExecuteSignedTx {
                tx_bytes,
                signatures,
            } => {
                let data = bcs::from_bytes(
                    &Base64::try_from(tx_bytes)
                    .map_err(|_| anyhow!("Invalid Base64 encoding"))?
                    .to_vec()
                    .map_err(|_| anyhow!("Invalid Base64 encoding"))?
                ).map_err(|_| anyhow!("Failed to parse tx bytes, check if it matches the output of iota client commands with --serialize-unsigned-transaction"))?;

                let mut sigs = Vec::new();
                for sig in signatures {
                    sigs.push(
                        GenericSignature::from_bytes(
                            &Base64::try_from(sig)
                                .map_err(|_| anyhow!("Invalid Base64 encoding"))?
                                .to_vec()
                                .map_err(|e| anyhow!(e))?,
                        )
                        .map_err(|_| anyhow!("Invalid generic signature"))?,
                    );
                }
                let transaction = Transaction::from_generic_sig_data(data, sigs);

                let response = context.execute_transaction_may_fail(transaction).await?;
                IotaClientCommandResult::TransactionBlock(response)
            }
            IotaClientCommands::ExecuteCombinedSignedTx { signed_tx_bytes } => {
                let data: SenderSignedData = bcs::from_bytes(
                    &Base64::try_from(signed_tx_bytes)
                        .map_err(|_| anyhow!("Invalid Base64 encoding"))?
                        .to_vec()
                        .map_err(|_| anyhow!("Invalid Base64 encoding"))?
                ).map_err(|_| anyhow!("Failed to parse SenderSignedData bytes, check if it matches the output of iota client commands with --serialize-signed-transaction"))?;
                let transaction = Envelope::<SenderSignedData, EmptySignInfo>::new(data);
                let response = context.execute_transaction_may_fail(transaction).await?;
                IotaClientCommandResult::TransactionBlock(response)
            }
            IotaClientCommands::Sign {
                address,
                data,
                intent,
                auth_call_args,
                auth_type_args,
            } => {
                let address = get_identity_address_from_keystore(
                    address,
                    context.config_mut().keystore_mut(),
                )?;
                let intent = intent.unwrap_or_else(Intent::iota_transaction);
                let msg: TransactionData =
                    bcs::from_bytes(&Base64::decode(&data).map_err(|e| {
                        anyhow!("Cannot deserialize data as TransactionData {:?}", e)
                    })?)?;
                let intent_msg = IntentMessage::new(intent, msg.clone());
                let raw_intent_msg: String = Base64::encode(bcs::to_bytes(&intent_msg)?);
                let mut hasher = DefaultHash::default();
                hasher.update(bcs::to_bytes(&intent_msg)?);
                let digest = hasher.finalize().digest;

                let iota_signature = if auth_call_args.is_some() || auth_type_args.is_some() {
                    let client = context.get_client().await?;
                    create_move_authenticator_signature(
                        &client,
                        address,
                        auth_call_args.as_ref(),
                        auth_type_args.as_ref(),
                    )
                    .await?
                } else {
                    GenericSignature::Signature(sign_secure(
                        context.config_mut().keystore_mut(),
                        &address,
                        &intent_msg.value,
                        intent_msg.intent,
                    )?)
                };

                IotaClientCommandResult::Sign(SignData {
                    iota_address: address,
                    raw_tx_data: data,
                    intent,
                    raw_intent_msg,
                    digest: Base64::encode(digest),
                    iota_signature: iota_signature.encode_base64(),
                })
            }
            IotaClientCommands::NewEnv {
                alias,
                rpc,
                graphql,
                ws,
                basic_auth,
                faucet,
            } => {
                if context.config().get_env(&alias).is_some() {
                    warn!("Environment config with name [{alias}] already exists.");
                }
                let env = IotaEnv::new(alias, rpc)
                    .with_graphql(graphql)
                    .with_ws(ws)
                    .with_basic_auth(basic_auth)
                    .with_faucet(faucet);

                // Check urls are valid and server is reachable
                env.create_rpc_client(None, None).await?;
                context.config_mut().set_env(env.clone());
                context.config().save()?;
                IotaClientCommandResult::NewEnv(env)
            }
            IotaClientCommands::ActiveEnv => IotaClientCommandResult::ActiveEnv(
                context.active_env().ok().map(|e| e.alias().clone()),
            ),
            IotaClientCommands::Envs => IotaClientCommandResult::Envs(
                context.config().envs().clone(),
                context.active_env().ok().map(|e| e.alias().clone()),
            ),
            IotaClientCommands::VerifySource {
                package_path,
                mut build_config,
                verify_deps,
                skip_source,
                address_override,
            } => {
                let mode = match (!skip_source, verify_deps, address_override) {
                    (false, false, _) => {
                        bail!("Source skipped and not verifying deps: Nothing to verify.")
                    }

                    (false, true, _) => ValidationMode::deps(),
                    (true, false, None) => ValidationMode::root(),
                    (true, true, None) => ValidationMode::root_and_deps(),
                    (true, false, Some(at)) => ValidationMode::root_at(*at),
                    (true, true, Some(at)) => ValidationMode::root_and_deps_at(*at),
                };

                build_config.implicit_dependencies = implicit_deps(latest_system_packages());
                let build_config = resolve_lock_file_path(build_config, Some(&package_path))?;
                let chain_id = context
                    .get_client()
                    .await?
                    .read_api()
                    .get_chain_identifier()
                    .await?;
                let compiled_package = BuildConfig {
                    config: build_config,
                    run_bytecode_verifier: true,
                    print_diags_to_stderr: true,
                    chain_id: Some(chain_id),
                }
                .build(&package_path)?;

                let client = context.get_client().await?;
                BytecodeSourceVerifier::new(client.read_api())
                    .verify(&compiled_package, mode)
                    .await?;

                IotaClientCommandResult::VerifySource
            }
            IotaClientCommands::PTB(ptb) => match ptb.execute(context).await? {
                PTBCommandResult::CommandResult(iota_client_command_result) => {
                    *iota_client_command_result
                }
                res => {
                    let s = res.to_styled_str();
                    println!("{}", s.ansi());
                    IotaClientCommandResult::NoOutput
                }
            },
        };
        Ok(ret.prerender_clever_errors(context).await)
    }

    pub fn switch_env(config: &mut IotaClientConfig, env: &str) -> Result<(), anyhow::Error> {
        ensure!(
            config.get_env(env).is_some(),
            "Environment config not found for [{env:?}], add new environment config using the `iota client new-env` command."
        );
        config.set_active_env(env.to_owned());
        Ok(())
    }
}

/// Process the `--skip-dependency-verification` and `--verify-dependencies`
/// flags for a publish or upgrade command. Prints deprecation warnings as
/// appropriate and returns true if the dependencies should be verified
fn check_dep_verification_flags(
    skip_dependency_verification: bool,
    verify_dependencies: bool,
) -> anyhow::Result<bool> {
    match (skip_dependency_verification, verify_dependencies) {
        (true, true) => bail!(
            "[error]: --skip-dependency-verification and --verify-deps are mutually exclusive"
        ),

        (false, false) => {
            eprintln!(
                "{}: Dependency sources are no longer verified automatically during publication and upgrade. \
                You can pass the `--verify-deps` option if you would like to verify them as part of publication or upgrade.",
                "[Note]".bold().yellow()
            );
        }

        (true, false) => {
            eprintln!(
                "{}: Dependency sources are no longer verified automatically during publication and upgrade, \
                so the `--skip-dependency-verification` flag is no longer necessary.",
                "[Warning]".bold().yellow()
            );
        }

        (false, true) => {}
    }
    Ok(verify_dependencies)
}

async fn compile_package_simple(
    read_api: &ReadApi,
    mut build_config: MoveBuildConfig,
    package_path: &Path,
    chain_id: Option<String>,
) -> Result<CompiledPackage, anyhow::Error> {
    build_config.implicit_dependencies = implicit_deps(latest_system_packages());
    let config = BuildConfig {
        config: resolve_lock_file_path(build_config, Some(package_path))?,
        run_bytecode_verifier: false,
        print_diags_to_stderr: false,
        chain_id: chain_id.clone(),
    };
    let resolution_graph = config.resolution_graph(package_path, chain_id.clone())?;
    let mut compiled_package =
        build_from_resolution_graph(resolution_graph, false, false, chain_id)?;
    pkg_tree_shake(read_api, false, &mut compiled_package).await?;

    Ok(compiled_package)
}

pub(crate) async fn upgrade_package(
    read_api: &ReadApi,
    build_config: MoveBuildConfig,
    package_path: &Path,
    upgrade_capability: ObjectID,
    with_unpublished_dependencies: bool,
    skip_dependency_verification: bool,
    env_alias: Option<String>,
) -> Result<(u8, CompiledPackage), anyhow::Error> {
    let mut compiled_package = compile_package(
        read_api,
        build_config,
        package_path,
        with_unpublished_dependencies,
        skip_dependency_verification,
    )
    .await?;

    pkg_tree_shake(
        read_api,
        with_unpublished_dependencies,
        &mut compiled_package,
    )
    .await?;

    compiled_package.published_at.as_ref().map_err(|e| match e {
        PublishedAtError::NotPresent => {
            anyhow!("No 'published-at' field in Move.toml or 'published-id' in Move.lock for package to be upgraded.")
        }
        PublishedAtError::Invalid(v) => anyhow!(
            "Invalid 'published-at' field in Move.toml or 'published-id' in Move.lock of package to be upgraded. \
                         Expected an on-chain address, but found: {v:?}"
        ),
        PublishedAtError::Conflict {
            id_lock,
            id_manifest,
        } => {
            let env_alias = format!("(currently {})", env_alias.unwrap_or_default());
            anyhow!(
                "Conflicting published package address: `Move.toml` contains published-at address \
                 {id_manifest} but `Move.lock` file contains published-at address {id_lock}. \
                 You may want to:
 - delete the published-at address in the `Move.toml` if the `Move.lock` address is correct; OR
 - update the `Move.lock` address using the `iota manage-package` command to be the same as the `Move.toml`; OR
 - check that your `iota active-env` {env_alias} corresponds to the chain on which the package is published (i.e., devnet, testnet, mainnet); OR
 - contact the maintainer if this package is a dependency and request resolving the conflict."
            )
        }
    })?;

    let resp = read_api
        .get_object_with_options(
            upgrade_capability,
            IotaObjectDataOptions::default().with_bcs().with_owner(),
        )
        .await?;

    let Some(data) = resp.data else {
        bail!("Could not find upgrade capability at {upgrade_capability}");
    };

    let upgrade_cap: UpgradeCap = data
        .bcs
        .ok_or_else(|| anyhow!("Fetch upgrade capability object but no data was returned"))?
        .try_as_move()
        .ok_or_else(|| anyhow!("Upgrade capability is not a Move Object"))?
        .deserialize()?;
    // We keep the existing policy -- no fancy policies or changing the upgrade
    // policy at the moment. To change the policy you can call a Move function in
    // the `package` module to change this policy.
    let upgrade_policy = upgrade_cap.policy;

    Ok((upgrade_policy, compiled_package))
}

pub(crate) async fn compile_package(
    read_api: &ReadApi,
    mut build_config: MoveBuildConfig,
    package_path: &Path,
    with_unpublished_dependencies: bool,
    skip_dependency_verification: bool,
) -> Result<CompiledPackage, anyhow::Error> {
    let protocol_config = read_api.get_protocol_config(None).await?;

    build_config.implicit_dependencies =
        implicit_deps_for_protocol_version(protocol_config.protocol_version)?;
    let config = resolve_lock_file_path(build_config, Some(package_path))?;
    let run_bytecode_verifier = true;
    let print_diags_to_stderr = true;
    let chain_id = read_api.get_chain_identifier().await.ok();
    let config = BuildConfig {
        config,
        run_bytecode_verifier,
        print_diags_to_stderr,
        chain_id: chain_id.clone(),
    };
    let resolution_graph = config.resolution_graph(package_path, chain_id.clone())?;
    let (_, dependencies) = gather_published_ids(&resolution_graph, chain_id.clone());

    check_conflicting_addresses(&dependencies.conflicting, false)?;
    check_invalid_dependencies(&dependencies.invalid)?;

    if !with_unpublished_dependencies {
        check_unpublished_dependencies(&dependencies.unpublished)?;
    };
    let mut compiled_package = build_from_resolution_graph(
        resolution_graph,
        run_bytecode_verifier,
        print_diags_to_stderr,
        chain_id,
    )?;

    pkg_tree_shake(
        read_api,
        with_unpublished_dependencies,
        &mut compiled_package,
    )
    .await?;

    let protocol_config = read_api.get_protocol_config(None).await?;

    // Check that the package's Move version is compatible with the chain's
    if let Some(Some(IotaProtocolConfigValue::U32(min_version))) = protocol_config
        .attributes
        .get("min_move_binary_format_version")
    {
        for module in compiled_package.get_modules_and_deps() {
            if module.version() < *min_version {
                return Err(IotaError::ModulePublishFailure {
                    error: format!(
                        "Module {} has a version {} that is \
                         lower than the minimum version {min_version} supported by the chain.",
                        module.self_id(),
                        module.version(),
                    ),
                }
                .into());
            }
        }
    }

    // Check that the package's Move version is compatible with the chain's
    if let Some(Some(IotaProtocolConfigValue::U32(max_version))) =
        protocol_config.attributes.get("move_binary_format_version")
    {
        for module in compiled_package.get_modules_and_deps() {
            if module.version() > *max_version {
                let help_msg = if module.version() == 7 {
                    "This is because you used enums in your Move package but tried to publish it to \
                    a chain that does not yet support enums in Move."
                } else {
                    ""
                };
                return Err(IotaError::ModulePublishFailure {
                    error: format!(
                        "Module {} has a version {} that is \
                         higher than the maximum version {max_version} supported by the chain.{help_msg}",
                        module.self_id(),
                        module.version(),
                    ),
                }
                .into());
            }
        }
    }

    if !compiled_package.is_system_package() {
        if let Some(already_published) = compiled_package.published_root_module() {
            return Err(IotaError::ModulePublishFailure {
                error: format!(
                    "Modules must all have 0x0 as their addresses. \
                     Violated by module {:?}",
                    already_published.self_id(),
                ),
            }
            .into());
        }
    }
    if with_unpublished_dependencies {
        compiled_package.verify_unpublished_dependencies(&dependencies.unpublished)?;
    }
    if !skip_dependency_verification {
        let verifier = BytecodeSourceVerifier::new(read_api);
        if let Err(e) = verifier
            .verify(&compiled_package, ValidationMode::deps())
            .await
        {
            return Err(IotaError::ModulePublishFailure {
                error: format!(
                    "[warning] {e}\n\
                     \n\
                     This may indicate that the on-chain version(s) of your package's dependencies \
                     may behave differently than the source version(s) your package was built \
                     against.\n\
                     \n\
                     Fix this by rebuilding your packages with source versions matching on-chain \
                     versions of dependencies, or ignore this warning by re-running with the \
                     --skip-dependency-verification flag."
                ),
            }
            .into());
        } else {
            eprintln!(
                "{}",
                "Successfully verified dependencies on-chain against source."
                    .bold()
                    .green(),
            );
        }
    } else {
        eprintln!("{}", "Skipping dependency verification".bold().yellow());
    }

    if compiled_package
        .get_package_bytes(with_unpublished_dependencies)
        .is_empty()
    {
        return Err(IotaError::ModulePublishFailure {
            error: "No modules found in the package".to_string(),
        }
        .into());
    }

    compiled_package
        .package
        .compiled_package_info
        .build_flags
        .update_lock_file_toolchain_version(package_path, env!("CARGO_PKG_VERSION").into())
        .map_err(|e| IotaError::ModuleBuildFailure {
            error: format!("Failed to update Move.lock toolchain version: {e}"),
        })?;

    Ok(compiled_package)
}

/// Return the correct implicit dependencies for the [version], producing a
/// warning or error if the protocol version is unknown or old
pub(crate) fn implicit_deps_for_protocol_version(
    version: ProtocolVersion,
) -> anyhow::Result<Dependencies> {
    if version > ProtocolVersion::MAX + 2 {
        eprintln!(
            "[{}]: The network is using protocol version {:?}, but this binary only recognizes protocol version {:?}; \
            the system packages used for compilation (e.g. MoveStdlib) may be out of date. If you have errors related to \
            system packages, you may need to update your CLI.",
            "warning".bold().yellow(),
            ProtocolVersion::MAX,
            version
        )
    }

    Ok(implicit_deps(system_packages_for_protocol(version)?.0))
}

impl Display for IotaClientCommandResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();
        match self {
            IotaClientCommandResult::AddAccount(new_account) => {
                let mut builder = TableBuilder::default();
                builder.push_record(vec!["address", new_account.address.to_string().as_str()]);
                builder.push_record(vec!["alias", new_account.alias.as_str()]);

                let mut table = builder.build();
                table.with(TableStyle::rounded());
                table.with(TablePanel::header("Added account to the keystore."));

                table.with(
                    TableModify::new(TableCell::new(0, 0))
                        .with(TableBorder::default().corner_bottom_right('┬')),
                );
                table.with(
                    TableModify::new(TableCell::new(0, 0))
                        .with(TableBorder::default().corner_top_right('─')),
                );

                write!(f, "{table}")?
            }
            IotaClientCommandResult::Addresses(addresses) => {
                let mut builder = TableBuilder::default();
                builder.set_header(vec!["alias", "address", "source", "active"]);
                for (alias, address, source) in &addresses.addresses {
                    let active_address = if address == &addresses.active_address {
                        "*".to_string()
                    } else {
                        "".to_string()
                    };
                    builder.push_record([
                        alias.to_string(),
                        address.to_string(),
                        source.to_string(),
                        active_address,
                    ]);
                }
                let mut table = builder.build();
                let style = TableStyle::rounded();
                table.with(style);
                write!(f, "{table}")?
            }
            IotaClientCommandResult::Balance(coins, with_coins) => {
                if coins.is_empty() {
                    return write!(f, "No coins found for this address.");
                }
                let mut builder = TableBuilder::default();
                pretty_print_balance(coins, &mut builder, *with_coins);
                let mut table = builder.build();
                table.with(TablePanel::header("Balance of coins owned by this address"));
                table.with(TableStyle::rounded().horizontals([HorizontalLine::new(
                    1,
                    TableStyle::modern().get_horizontal(),
                )]));
                table.with(tabled::settings::style::BorderSpanCorrection);
                write!(f, "{table}")?;
            }
            IotaClientCommandResult::DynamicFieldQuery(df_refs) => {
                let df_refs = DynamicFieldOutput {
                    has_next_page: df_refs.has_next_page,
                    next_cursor: df_refs.next_cursor,
                    data: df_refs.data.clone(),
                };

                let json_obj = json!(df_refs);
                let mut table = json_to_table(&json_obj);
                let style = TableStyle::rounded().horizontals([]);
                table.with(style);
                write!(f, "{table}")?
            }
            IotaClientCommandResult::Gas(gas_coins) => {
                let gas_coins = gas_coins
                    .iter()
                    .map(GasCoinOutput::from)
                    .collect::<Vec<_>>();
                if gas_coins.is_empty() {
                    write!(f, "No gas coins are owned by this address")?;
                    return Ok(());
                }

                let mut builder = TableBuilder::default();
                builder.set_header(vec![
                    "gasCoinId",
                    "nanosBalance (NANOS)",
                    "iotaBalance (IOTA)",
                ]);
                for coin in &gas_coins {
                    builder.push_record(vec![
                        coin.gas_coin_id.to_string(),
                        coin.nanos_balance.to_string(),
                        coin.iota_balance.to_string(),
                    ]);
                }
                let mut table = builder.build();
                table.with(TableStyle::rounded());
                if gas_coins.len() > 10 {
                    table.with(TablePanel::header(format!(
                        "Showing {} gas coins and their balances.",
                        gas_coins.len()
                    )));
                    table.with(TablePanel::footer(format!(
                        "Showing {} gas coins and their balances.",
                        gas_coins.len()
                    )));
                    table.with(TableStyle::rounded().horizontals([
                        HorizontalLine::new(1, TableStyle::modern().get_horizontal()),
                        HorizontalLine::new(2, TableStyle::modern().get_horizontal()),
                        HorizontalLine::new(
                            gas_coins.len() + 2,
                            TableStyle::modern().get_horizontal(),
                        ),
                    ]));
                    table.with(tabled::settings::style::BorderSpanCorrection);
                }
                write!(f, "{table}")?;
            }
            IotaClientCommandResult::NewAddress(new_address) => {
                let mut builder = TableBuilder::default();
                builder.push_record(vec!["alias", new_address.alias.as_str()]);
                builder.push_record(vec!["address", new_address.address.to_string().as_str()]);
                builder.push_record(vec![
                    "publicBase64Key",
                    new_address.public_base64_key.as_str(),
                ]);
                builder.push_record(vec![
                    "publicBase64KeyWithFlag",
                    new_address.public_base64_key_with_flag.as_str(),
                ]);
                builder.push_record(vec![
                    "keyScheme",
                    new_address.key_scheme.to_string().as_str(),
                ]);
                builder.push_record(vec![
                    "recoveryPhrase",
                    new_address.recovery_phrase.to_string().as_str(),
                ]);

                let mut table = builder.build();
                table.with(TableStyle::rounded());
                table.with(TablePanel::header(
                    "Created new keypair and saved it to keystore.",
                ));

                table.with(
                    TableModify::new(TableCell::new(0, 0))
                        .with(TableBorder::default().corner_bottom_right('┬')),
                );
                table.with(
                    TableModify::new(TableCell::new(0, 0))
                        .with(TableBorder::default().corner_top_right('─')),
                );

                write!(f, "{table}")?
            }
            IotaClientCommandResult::Object(object_read) => match object_read.object() {
                Ok(obj) => {
                    let object = ObjectOutput::from(obj);
                    let json_obj = json!(&object);
                    let mut table = json_to_table(&json_obj);
                    table.with(TableStyle::rounded().horizontals([]));
                    writeln!(f, "{table}")?
                }
                Err(e) => writeln!(f, "Internal error, cannot read the object: {e}")?,
            },
            IotaClientCommandResult::Objects(object_refs) => {
                if object_refs.is_empty() {
                    writeln!(f, "This address has no owned objects.")?
                } else {
                    let objects = ObjectsOutput::from_vec(object_refs.to_vec());
                    match objects {
                        Ok(objs) => {
                            let json_obj = json!(objs);
                            let mut table = json_to_table(&json_obj);
                            table.with(TableStyle::rounded().horizontals([]));
                            writeln!(f, "{table}")?
                        }
                        Err(e) => write!(f, "Internal error: {e}")?,
                    }
                }
            }
            IotaClientCommandResult::TransactionBlock(response) => {
                write!(writer, "{response}")?;
            }
            IotaClientCommandResult::RawObject(raw_object_read) => {
                let raw_object = match raw_object_read.object() {
                    Ok(v) => match &v.bcs {
                        Some(IotaRawData::MoveObject(o)) => {
                            format!("{:?}\nNumber of bytes: {}", o.bcs_bytes, o.bcs_bytes.len())
                        }
                        Some(IotaRawData::Package(p)) => {
                            let mut temp = String::new();
                            let mut bcs_bytes = 0usize;
                            for m in &p.module_map {
                                temp.push_str(&format!("{m:?}\n"));
                                bcs_bytes += m.1.len()
                            }
                            format!("{temp}Number of bytes: {bcs_bytes}")
                        }
                        None => "Bcs field is None".to_string().red().to_string(),
                    },
                    Err(err) => format!("{err}").red().to_string(),
                };
                writeln!(writer, "{raw_object}")?;
            }
            IotaClientCommandResult::RemoveAddress(address) => {
                write!(writer, "Removed address \"{address}\" from keystore.")?
            }
            IotaClientCommandResult::ComputeTransactionDigest(tx_data) => {
                writeln!(writer, "{}", tx_data.digest())?;
            }
            IotaClientCommandResult::SerializedUnsignedTransaction(tx_data) => {
                writeln!(
                    writer,
                    "{}",
                    fastcrypto::encoding::Base64::encode(bcs::to_bytes(tx_data).unwrap())
                )?;
            }
            IotaClientCommandResult::SerializedSignedTransaction(sender_signed_tx) => {
                writeln!(
                    writer,
                    "{}",
                    fastcrypto::encoding::Base64::encode(bcs::to_bytes(sender_signed_tx).unwrap())
                )?;
            }
            IotaClientCommandResult::SyncClientState => {
                writeln!(writer, "Client state sync complete.")?;
            }
            IotaClientCommandResult::ChainIdentifier(ci) => {
                writeln!(writer, "{ci}")?;
            }
            IotaClientCommandResult::Switch(response) => {
                write!(writer, "{response}")?;
            }
            IotaClientCommandResult::ActiveAddress(response) => {
                match response {
                    Some(r) => write!(writer, "{r}")?,
                    None => write!(writer, "None")?,
                };
            }
            IotaClientCommandResult::ActiveEnv(env) => {
                write!(writer, "{}", env.as_deref().unwrap_or("None"))?;
            }
            IotaClientCommandResult::NewEnv(env) => {
                writeln!(writer, "Added new IOTA env [{}] to config.", env.alias())?;
            }
            IotaClientCommandResult::Envs(envs, active) => {
                let mut builder = TableBuilder::default();
                builder.set_header(["alias", "url", "active"]);
                for env in envs {
                    builder.push_record(vec![env.alias().clone(), env.rpc().clone(), {
                        if Some(env.alias().as_str()) == active.as_deref() {
                            "*".to_string()
                        } else {
                            "".to_string()
                        }
                    }]);
                }
                let mut table = builder.build();
                table.with(TableStyle::rounded());
                write!(f, "{table}")?
            }
            IotaClientCommandResult::VerifySource => {
                writeln!(writer, "Source verification succeeded!")?;
            }
            IotaClientCommandResult::VerifyBytecodeMeter {
                success,
                max_package_ticks,
                max_module_ticks,
                max_function_ticks,
                used_ticks,
            } => {
                let mut builder = TableBuilder::default();

                /// Convert ticks to string, using commas as thousands
                /// separators
                fn format_ticks(ticks: u128) -> String {
                    let ticks = ticks.to_string();
                    let mut formatted = String::with_capacity(ticks.len() + ticks.len() / 3);
                    for (i, c) in ticks.chars().rev().enumerate() {
                        if i != 0 && (i % 3 == 0) {
                            formatted.push(',');
                        }
                        formatted.push(c);
                    }
                    formatted.chars().rev().collect()
                }

                // Build up the limits table
                builder.push_record(vec!["Limits"]);
                builder.push_record(vec![
                    "packages".to_string(),
                    max_package_ticks.map_or_else(|| "None".to_string(), format_ticks),
                ]);
                builder.push_record(vec![
                    "  modules".to_string(),
                    max_module_ticks.map_or_else(|| "None".to_string(), format_ticks),
                ]);
                builder.push_record(vec![
                    "    functions".to_string(),
                    max_function_ticks.map_or_else(|| "None".to_string(), format_ticks),
                ]);

                // Build up usage table
                builder.push_record(vec!["Ticks Used"]);
                let mut stack = vec![used_ticks];
                while let Some(usage) = stack.pop() {
                    let indent = match usage.scope {
                        Scope::Transaction => 0,
                        Scope::Package => 0,
                        Scope::Module => 2,
                        Scope::Function => 4,
                    };

                    builder.push_record(vec![
                        format!("{:indent$}{}", "", usage.name),
                        format_ticks(usage.ticks),
                    ]);

                    stack.extend(usage.children.iter().rev())
                }

                let mut table = builder.build();

                let message = if *success {
                    "Package will pass metering check!"
                } else {
                    "Package will NOT pass metering check!"
                };

                // Add overall header and footer message;
                table.with(TablePanel::header(message));
                table.with(TablePanel::footer(message));

                // Set-up spans for headers
                table.with(TableModify::new(TableRows::new(0..2)).with(TableSpan::column(2)));
                table.with(TableModify::new(TableRows::single(5)).with(TableSpan::column(2)));

                // Styling
                table.with(TableStyle::rounded());
                table.with(TableModify::new(TableCols::new(1..)).with(TableAlignment::right()));

                // Separators before and after headers/footers
                let hl = TableStyle::modern().get_horizontal();
                let last = table.count_rows() - 1;
                table.with(HorizontalLine::new(2, hl));
                table.with(HorizontalLine::new(5, hl));
                table.with(HorizontalLine::new(6, hl));
                table.with(HorizontalLine::new(last, hl));

                table.with(tabled::settings::style::BorderSpanCorrection);

                writeln!(f, "{table}")?;
            }
            IotaClientCommandResult::Sign(sign_data) => {
                let mut builder = TableBuilder::default();
                builder.push_record(vec![
                    "iota_address",
                    sign_data.iota_address.to_string().as_str(),
                ]);
                builder.push_record(vec!["raw_tx_data", sign_data.raw_tx_data.as_str()]);
                builder.push_record(vec!["intent", &format!("{:?}", sign_data.intent)]);
                builder.push_record(vec!["raw_intent_msg", sign_data.raw_intent_msg.as_str()]);
                builder.push_record(vec!["digest", sign_data.digest.as_str()]);
                builder.push_record(vec!["iota_signature", sign_data.iota_signature.as_str()]);

                let mut table = builder.build();
                table.with(TableStyle::rounded());
                table.with(TablePanel::header("Signed transaction data."));

                table.with(
                    TableModify::new(TableCell::new(0, 0))
                        .with(TableBorder::default().corner_bottom_right('┬')),
                );

                writeln!(f, "{table}")?;
            }
            IotaClientCommandResult::NoOutput => {}
            IotaClientCommandResult::DryRun(response) => {
                writeln!(f, "{}", Pretty(response))?;
            }
            IotaClientCommandResult::DevInspect(response) => {
                writeln!(f, "{}", Pretty(response))?;
            }
        }
        write!(f, "{}", writer.trim_end_matches('\n'))
    }
}

fn convert_number_to_string(value: Value) -> Value {
    match value {
        Value::Number(n) => Value::String(n.to_string()),
        Value::Array(a) => Value::Array(a.into_iter().map(convert_number_to_string).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (k, convert_number_to_string(v)))
                .collect(),
        ),
        _ => value,
    }
}

impl Debug for IotaClientCommandResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = crate::unwrap_err_to_string(|| match self {
            IotaClientCommandResult::Gas(gas_coins) => {
                let gas_coins = gas_coins
                    .iter()
                    .map(GasCoinOutput::from)
                    .collect::<Vec<_>>();
                Ok(serde_json::to_string_pretty(&gas_coins)?)
            }
            IotaClientCommandResult::Object(object_read) => {
                let object = object_read.object()?;
                Ok(serde_json::to_string_pretty(&object)?)
            }
            IotaClientCommandResult::RawObject(raw_object_read) => {
                let raw_object = raw_object_read.object()?;
                Ok(serde_json::to_string_pretty(&raw_object)?)
            }
            _ => Ok(serde_json::to_string_pretty(self)?),
        });
        write!(f, "{s}")
    }
}

impl IotaClientCommandResult {
    pub fn objects_response(&self) -> Option<Vec<IotaObjectResponse>> {
        use IotaClientCommandResult::*;
        match self {
            Object(o) | RawObject(o) => Some(vec![o.clone()]),
            Objects(o) => Some(o.clone()),
            _ => None,
        }
    }

    pub fn tx_block_response(&self) -> Option<&IotaTransactionBlockResponse> {
        use IotaClientCommandResult::*;
        match self {
            TransactionBlock(b) => Some(b),
            _ => None,
        }
    }

    pub async fn prerender_clever_errors(mut self, context: &mut WalletContext) -> Self {
        match &mut self {
            IotaClientCommandResult::DryRun(DryRunTransactionBlockResponse { effects, .. })
            | IotaClientCommandResult::TransactionBlock(IotaTransactionBlockResponse {
                effects: Some(effects),
                ..
            }) => {
                let client = context.get_client().await.expect("Cannot connect to RPC");
                prerender_clever_errors(effects, client.read_api()).await
            }
            IotaClientCommandResult::TransactionBlock(IotaTransactionBlockResponse {
                effects: None,
                ..
            }) => (),
            IotaClientCommandResult::ActiveAddress(_)
            | IotaClientCommandResult::ActiveEnv(_)
            | IotaClientCommandResult::AddAccount(_)
            | IotaClientCommandResult::Addresses(_)
            | IotaClientCommandResult::Balance(_, _)
            | IotaClientCommandResult::ComputeTransactionDigest(_)
            | IotaClientCommandResult::ChainIdentifier(_)
            | IotaClientCommandResult::DynamicFieldQuery(_)
            | IotaClientCommandResult::DevInspect(_)
            | IotaClientCommandResult::Envs(_, _)
            | IotaClientCommandResult::Gas(_)
            | IotaClientCommandResult::NewAddress(_)
            | IotaClientCommandResult::NewEnv(_)
            | IotaClientCommandResult::NoOutput
            | IotaClientCommandResult::Object(_)
            | IotaClientCommandResult::Objects(_)
            | IotaClientCommandResult::RawObject(_)
            | IotaClientCommandResult::RemoveAddress(_)
            | IotaClientCommandResult::SerializedSignedTransaction(_)
            | IotaClientCommandResult::SerializedUnsignedTransaction(_)
            | IotaClientCommandResult::Sign(_)
            | IotaClientCommandResult::Switch(_)
            | IotaClientCommandResult::SyncClientState
            | IotaClientCommandResult::VerifyBytecodeMeter { .. }
            | IotaClientCommandResult::VerifySource => (),
        }
        self
    }
}

impl PrintableResult for IotaClientCommandResult {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressesOutput {
    pub active_address: IotaAddress,
    pub addresses: Vec<(String, IotaAddress, String)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFieldOutput {
    pub has_next_page: bool,
    pub next_cursor: Option<ObjectID>,
    pub data: Vec<DynamicFieldInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAccountOutput {
    pub address: IotaAddress,
    pub alias: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAddressOutput {
    pub alias: String,
    pub address: IotaAddress,
    pub public_base64_key: String,
    pub public_base64_key_with_flag: String,
    pub key_scheme: SignatureScheme,
    pub recovery_phrase: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOutput {
    pub object_id: ObjectID,
    pub version: SequenceNumber,
    pub digest: String,
    pub obj_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<Owner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_tx: Option<TransactionDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_rebate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<IotaParsedData>,
}

impl From<&IotaObjectData> for ObjectOutput {
    fn from(obj: &IotaObjectData) -> Self {
        let obj_type = match obj.type_.as_ref() {
            Some(x) => x.to_string(),
            None => "unknown".to_string(),
        };
        Self {
            object_id: obj.object_id,
            version: obj.version,
            digest: obj.digest.to_string(),
            obj_type,
            owner: obj.owner,
            prev_tx: obj.previous_transaction,
            storage_rebate: obj.storage_rebate,
            content: obj.content.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GasCoinOutput {
    pub gas_coin_id: ObjectID,
    pub nanos_balance: u64,
    pub iota_balance: String,
}

impl From<&GasCoin> for GasCoinOutput {
    fn from(gas_coin: &GasCoin) -> Self {
        Self {
            gas_coin_id: *gas_coin.id(),
            nanos_balance: gas_coin.value(),
            iota_balance: format_balance(gas_coin.value() as u128, 9, 2, None),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectsOutput {
    pub object_id: ObjectID,
    pub version: SequenceNumber,
    pub digest: String,
    pub object_type: String,
}

impl ObjectsOutput {
    fn from(obj: IotaObjectResponse) -> Result<Self, anyhow::Error> {
        let obj = obj.into_object()?;
        // this replicates the object type display as in the iota explorer
        let object_type = match obj.type_ {
            Some(iota_types::base_types::ObjectType::Struct(x)) => {
                let address = x.address().to_string();
                // check if the address has length of 64 characters
                // otherwise, keep it as it is
                let address = if address.len() == 64 {
                    format!("0x{}..{}", &address[..4], &address[address.len() - 4..])
                } else {
                    address
                };
                format!("{}::{}::{}", address, x.module(), x.name(),)
            }
            Some(iota_types::base_types::ObjectType::Package) => "Package".to_string(),
            None => "unknown".to_string(),
        };
        Ok(Self {
            object_id: obj.object_id,
            version: obj.version,
            digest: Base64::encode(obj.digest),
            object_type,
        })
    }
    fn from_vec(objs: Vec<IotaObjectResponse>) -> Result<Vec<Self>, anyhow::Error> {
        objs.into_iter()
            .map(ObjectsOutput::from)
            .collect::<Result<Vec<_>, _>>()
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum IotaClientCommandResult {
    ActiveAddress(Option<IotaAddress>),
    ActiveEnv(Option<String>),
    AddAccount(AddAccountOutput),
    Addresses(AddressesOutput),
    Balance(Vec<(Option<IotaCoinMetadata>, Vec<Coin>)>, bool),
    ChainIdentifier(String),
    ComputeTransactionDigest(TransactionData),
    DynamicFieldQuery(DynamicFieldPage),
    DryRun(DryRunTransactionBlockResponse),
    DevInspect(DevInspectResults),
    Envs(Vec<IotaEnv>, Option<String>),
    Gas(Vec<GasCoin>),
    NewAddress(NewAddressOutput),
    NewEnv(IotaEnv),
    NoOutput,
    Object(IotaObjectResponse),
    Objects(Vec<IotaObjectResponse>),
    RawObject(IotaObjectResponse),
    RemoveAddress(IotaAddress),
    SerializedSignedTransaction(SenderSignedData),
    SerializedUnsignedTransaction(TransactionData),
    Sign(SignData),
    Switch(SwitchResponse),
    SyncClientState,
    TransactionBlock(IotaTransactionBlockResponse),
    VerifyBytecodeMeter {
        success: bool,
        max_package_ticks: Option<u128>,
        max_module_ticks: Option<u128>,
        max_function_ticks: Option<u128>,
        used_ticks: Accumulator,
    },
    VerifySource,
}

#[derive(Serialize, Clone)]
pub struct SwitchResponse {
    /// Active address
    pub address: Option<String>,
    pub env: Option<String>,
}

impl Display for SwitchResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();

        if let Some(addr) = &self.address {
            writeln!(writer, "Active address switched to {addr}")?;
        }
        if let Some(env) = &self.env {
            writeln!(writer, "Active environment switched to [{env}]")?;
        }
        write!(f, "{writer}")
    }
}

/// Request tokens from the Faucet for the given address
pub async fn request_tokens_from_faucet(
    address: IotaAddress,
    url: String,
) -> Result<(), anyhow::Error> {
    let address_str = address.to_string();
    let json_body = json![{
        "FixedAmountRequest": {
            "recipient": &address_str
        }
    }];

    // make the request to the faucet JSON RPC API for coin
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::USER_AGENT, USER_AGENT)
        .json(&json_body)
        .send()
        .await?;

    match resp.status() {
        StatusCode::ACCEPTED | StatusCode::CREATED | StatusCode::OK => {
            let faucet_resp: FaucetResponse = resp.json().await?;

            if let Some(err) = faucet_resp.error {
                bail!("Faucet request was unsuccessful: {err}")
            } else {
                println!(
                    "Request successful. It can take up to 1 minute to get the coin. Run iota client gas to check your gas coins."
                );
            }
        }
        StatusCode::TOO_MANY_REQUESTS => {
            bail!(
                "Faucet service received too many requests from this IP address. Please try again after 60 minutes."
            );
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            bail!("Faucet service is currently overloaded or unavailable. Please try again later.");
        }
        status_code => {
            bail!("Faucet request was unsuccessful: {status_code}");
        }
    }
    Ok(())
}

fn pretty_print_balance(
    coins_by_type: &Vec<(Option<IotaCoinMetadata>, Vec<Coin>)>,
    builder: &mut TableBuilder,
    with_coins: bool,
) {
    let format_decimals = 2;
    let mut table_builder = TableBuilder::default();
    if !with_coins {
        table_builder.set_header(vec!["coin", "balance (raw)", "balance", ""]);
    }
    for (metadata, coins) in coins_by_type {
        let (name, symbol, coin_decimals) = if let Some(metadata) = metadata {
            (
                metadata.name.as_str(),
                metadata.symbol.as_str(),
                metadata.decimals,
            )
        } else {
            ("unknown", "unknown_symbol", 9)
        };

        let balance = coins.iter().map(|x| x.balance as u128).sum::<u128>();
        let mut inner_table = TableBuilder::default();
        inner_table.set_header(vec!["coinId", "balance (raw)", "balance", ""]);

        if with_coins {
            let coin_numbers = if coins.len() != 1 { "coins" } else { "coin" };
            let balance_formatted = format!(
                "({} {})",
                format_balance(balance, coin_decimals, format_decimals, Some(symbol)),
                symbol
            );
            let summary = format!(
                "{}: {} {coin_numbers}, Balance: {} {}",
                name,
                coins.len(),
                balance,
                balance_formatted
            );
            for c in coins {
                inner_table.push_record(vec![
                    c.coin_object_id.to_string().as_str(),
                    c.balance.to_string().as_str(),
                    format_balance(
                        c.balance as u128,
                        coin_decimals,
                        format_decimals,
                        Some(symbol),
                    )
                    .as_str(),
                ]);
            }
            let mut table = inner_table.build();
            table.with(TablePanel::header(summary));
            table.with(
                TableStyle::rounded()
                    .horizontals([
                        HorizontalLine::new(1, TableStyle::modern().get_horizontal()),
                        HorizontalLine::new(2, TableStyle::modern().get_horizontal()),
                    ])
                    .remove_vertical(),
            );
            table.with(tabled::settings::style::BorderSpanCorrection);
            builder.push_record(vec![table.to_string()]);
        } else {
            table_builder.push_record(vec![
                name,
                balance.to_string().as_str(),
                format_balance(balance, coin_decimals, format_decimals, Some(symbol)).as_str(),
            ]);
        }
    }

    let mut table = table_builder.build();
    table.with(
        TableStyle::rounded()
            .horizontals([HorizontalLine::new(
                1,
                TableStyle::modern().get_horizontal(),
            )])
            .remove_vertical(),
    );
    table.with(tabled::settings::style::BorderSpanCorrection);
    builder.push_record(vec![table.to_string()]);
}

fn divide(value: u128, divisor: u128) -> (u128, u128) {
    let integer_part = value / divisor;
    let fractional_part = value % divisor;
    (integer_part, fractional_part)
}

fn format_balance(
    value: u128,
    coin_decimals: u8,
    format_decimals: usize,
    symbol: Option<&str>,
) -> String {
    let mut suffix = if let Some(symbol) = symbol {
        format!(" {symbol}")
    } else {
        "".to_string()
    };

    let mut coin_decimals = coin_decimals as u32;
    let billions = 10u128.pow(coin_decimals + 9);
    let millions = 10u128.pow(coin_decimals + 6);
    let thousands = 10u128.pow(coin_decimals + 3);
    let units = 10u128.pow(coin_decimals);

    let (whole, fractional) = if value > billions {
        coin_decimals += 9;
        suffix = format!("B{suffix}");
        divide(value, billions)
    } else if value > millions {
        coin_decimals += 6;
        suffix = format!("M{suffix}");
        divide(value, millions)
    } else if value > thousands {
        coin_decimals += 3;
        suffix = format!("K{suffix}");
        divide(value, thousands)
    } else {
        divide(value, units)
    };

    let mut fractional = format!("{fractional:0width$}", width = coin_decimals as usize);
    fractional.truncate(format_decimals);

    format!("{whole}.{fractional}{suffix}")
}

/// Helper function to reduce code duplication for executing dry run
pub async fn execute_dry_run(
    context: &mut WalletContext,
    signer: IotaAddress,
    kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: u64,
    gas_payment: Vec<ObjectRef>,
    sponsor: Option<IotaAddress>,
) -> Result<IotaClientCommandResult, anyhow::Error> {
    let client = context.get_client().await?;
    let gas_budget = match gas_budget {
        Some(gas_budget) => gas_budget,
        None => {
            let max_gas_budget = max_gas_budget(&client).await?;
            if gas_payment.is_empty() {
                max_gas_budget
            } else {
                let mut gas_budget = 0;
                let gas_coins = client
                    .read_api()
                    .multi_get_object_with_options(
                        gas_payment.iter().map(|object_ref| object_ref.0).collect(),
                        IotaObjectDataOptions::bcs_lossless(),
                    )
                    .await?;
                for gas_coin in gas_coins {
                    gas_budget += get_gas_balance(
                        &gas_coin
                            .into_object()?
                            .try_into()
                            .expect("couldn't convert gas coin into object"),
                    )?
                }
                let final_gas_budget = min(gas_budget, max_gas_budget);
                if final_gas_budget == gas_budget {
                    let warn_msg = format!(
                        "Gas budget is equal to the total gas balance of the provided gas coins: {gas_budget}. Manually provide a lower --gas-budget if you need to split a coin from the gas coin."
                    );
                    warn!(warn_msg);
                    eprintln!("{}", warn_msg.yellow().bold());
                }
                final_gas_budget
            }
        }
    };
    debug!("Gas budget for dry run: {gas_budget}");
    let tx_data = TransactionData::new_with_gas_coins_allow_sponsor(
        kind,
        signer,
        gas_payment,
        gas_budget,
        gas_price,
        sponsor.unwrap_or(signer),
    );
    debug!("Executing dry run");
    let response = client.read_api().dry_run_transaction_block(tx_data).await?;
    debug!("Finished executing dry run {response:?}");
    let resp = IotaClientCommandResult::DryRun(response)
        .prerender_clever_errors(context)
        .await;
    Ok(resp)
}

/// Call a dry run with the transaction data to estimate the gas budget.
/// The estimated gas budget is computed as following:
/// * the maximum between A and B, where:
///
/// A = computation cost + GAS_SAFE_OVERHEAD * reference gas price
/// B = computation cost + storage cost - storage rebate + GAS_SAFE_OVERHEAD *
/// reference gas price overhead
///
/// This gas estimate is computed exactly as in the TypeScript SDK
/// <https://github.com/iotaledger/iota/blob/3c4369270605f78a243842098b7029daf8d883d9/sdk/typescript/src/transactions/TransactionBlock.ts#L845-L858>
pub async fn estimate_gas_budget(
    context: &mut WalletContext,
    signer: IotaAddress,
    kind: TransactionKind,
    gas_price: u64,
    gas_payment: Vec<ObjectRef>,
    sponsor: Option<IotaAddress>,
) -> Result<u64, anyhow::Error> {
    let client = context.get_client().await?;
    let dry_run =
        execute_dry_run(context, signer, kind, None, gas_price, gas_payment, sponsor).await;

    if let Ok(IotaClientCommandResult::DryRun(dry_run)) = dry_run {
        let rgp = client.read_api().get_reference_gas_price().await?;
        Ok(estimate_gas_budget_from_gas_cost(
            dry_run.effects.gas_cost_summary(),
            rgp,
        ))
    } else {
        bail!(
            "Could not determine the gas budget. Error: {}",
            dry_run.unwrap_err()
        )
    }
}

pub fn estimate_gas_budget_from_gas_cost(
    gas_cost_summary: &GasCostSummary,
    reference_gas_price: u64,
) -> u64 {
    let safe_overhead = GAS_SAFE_OVERHEAD * reference_gas_price;
    let computation_cost_with_overhead = gas_cost_summary.computation_cost + safe_overhead;

    let gas_usage = gas_cost_summary.net_gas_usage() + safe_overhead as i64;
    computation_cost_with_overhead.max(if gas_usage < 0 { 0 } else { gas_usage as u64 })
}

/// Queries the protocol config for the maximum gas allowed in a transaction.
pub async fn max_gas_budget(client: &IotaClient) -> Result<u64, anyhow::Error> {
    let cfg = client.read_api().get_protocol_config(None).await?;
    Ok(match cfg.attributes.get("max_tx_gas") {
        Some(Some(iota_json_rpc_types::IotaProtocolConfigValue::U64(y))) => *y,
        _ => bail!(
            "Could not automatically find the maximum gas allowed in a transaction from the \
            protocol config. Please provide a gas budget with the --gas-budget flag."
        ),
    })
}

/// Dry run, execute, or serialize a transaction.
///
/// This basically extracts the logical code for each command that deals with
/// dry run, executing, or serializing a transaction and puts it in a function
/// to reduce code duplication.
pub(crate) async fn dry_run_or_execute_or_serialize(
    signer: IotaAddress,
    tx_kind: TransactionKind,
    context: &mut WalletContext,
    gas_payment: Vec<ObjectRef>,
    gas_data: GasDataArgs,
    processing: TxProcessingArgs,
) -> Result<IotaClientCommandResult, anyhow::Error> {
    let GasDataArgs {
        gas_budget,
        gas_price,
        gas_sponsor,
    } = gas_data;

    let TxProcessingArgs {
        tx_digest,
        dry_run,
        dev_inspect,
        serialize_unsigned_transaction,
        serialize_signed_transaction,
        sender,
        display,
        auth_call_args,
        auth_type_args,
    } = processing;
    ensure!(
        !serialize_unsigned_transaction || !serialize_signed_transaction,
        "Cannot specify both flags: --serialize-unsigned-transaction and --serialize-signed-transaction."
    );
    let gas_price = if let Some(gas_price) = gas_price {
        gas_price
    } else {
        context.get_reference_gas_price().await?
    };

    let client = context.get_client().await?;

    let signer = sender.unwrap_or(signer);

    if dev_inspect {
        return execute_dev_inspect(
            context,
            signer,
            tx_kind,
            gas_budget,
            gas_price,
            gas_payment,
            gas_sponsor,
            None,
        )
        .await;
    }

    if dry_run {
        return execute_dry_run(
            context,
            signer,
            tx_kind,
            gas_budget,
            gas_price,
            gas_payment.clone(),
            gas_sponsor,
        )
        .await;
    }

    let gas_budget = match gas_budget {
        Some(gas_budget) => gas_budget,
        None => {
            debug!("Estimating gas budget");
            let budget = estimate_gas_budget(
                context,
                signer,
                tx_kind.clone(),
                gas_price,
                gas_payment.clone(),
                gas_sponsor,
            )
            .await?;
            debug!("Finished estimating gas budget");
            budget
        }
    };

    let gas_payment = if !gas_payment.is_empty() {
        gas_payment
    } else {
        let input_objects: Vec<_> = tx_kind
            .input_objects()?
            .iter()
            .filter_map(|o| match o {
                InputObjectKind::ImmOrOwnedMoveObject((id, _, _)) => Some(*id),
                _ => None,
            })
            .collect();

        let gas_payment = client
            .transaction_builder()
            .select_gas(
                gas_sponsor.unwrap_or(signer),
                None,
                gas_budget,
                input_objects,
                gas_price,
            )
            .await?;

        vec![gas_payment]
    };
    debug!("Preparing transaction data");
    let tx_data = TransactionData::new_with_gas_coins_allow_sponsor(
        tx_kind,
        signer,
        gas_payment,
        gas_budget,
        gas_price,
        gas_sponsor.unwrap_or(signer),
    );
    debug!("Finished preparing transaction data");

    if serialize_unsigned_transaction {
        Ok(IotaClientCommandResult::SerializedUnsignedTransaction(
            tx_data,
        ))
    } else if tx_digest {
        Ok(IotaClientCommandResult::ComputeTransactionDigest(tx_data))
    } else {
        let auth_args = if auth_call_args.is_some() || auth_type_args.is_some() {
            let auth_info = fetch_auth_info(&client, signer).await?;
            let (type_args, json_args) =
                process_auth_args(auth_call_args.as_ref(), auth_type_args.as_ref(), signer)?;
            let call_args = resolve_auth_call_args(
                &client,
                auth_info.value.package,
                &auth_info.value.module,
                &auth_info.value.function,
                &type_args,
                json_args,
            )
            .await?;
            Some((
                call_args,
                type_args.into_iter().map(TypeInput::from).collect(),
            ))
        } else {
            None
        };

        let signature =
            sign_transaction(context, &tx_data, &tx_data.sender(), auth_args.clone()).await?;

        let mut signatures = vec![signature];

        if let Some(gas_sponsor) = gas_sponsor {
            if gas_sponsor != signer {
                let signature =
                    sign_transaction(context, &tx_data, &gas_sponsor, auth_args).await?;

                signatures.push(signature);
            }
        }

        let sender_signed_data = SenderSignedData::new(tx_data, signatures);

        if serialize_signed_transaction {
            Ok(IotaClientCommandResult::SerializedSignedTransaction(
                sender_signed_data,
            ))
        } else {
            let transaction = Transaction::new(sender_signed_data);
            debug!("Executing transaction: {:?}", transaction);
            let mut response = client
                .quorum_driver_api()
                .execute_transaction_block(
                    transaction,
                    opts_from_cli(display),
                    Some(ExecuteTransactionRequestType::WaitForLocalExecution),
                )
                .await?;
            debug!("Transaction executed");

            if let Some(effects) = response.effects.as_mut() {
                prerender_clever_errors(effects, client.read_api()).await;
            }
            let effects = response.effects.as_ref().ok_or_else(|| {
                anyhow!("Effects from IotaTransactionBlockResult should not be empty")
            })?;
            if let IotaExecutionStatus::Failure { error } = effects.status() {
                bail!("Error executing transaction '{}': {error}", response.digest);
            }
            Ok(IotaClientCommandResult::TransactionBlock(response))
        }
    }
}

async fn execute_dev_inspect(
    context: &mut WalletContext,
    signer: IotaAddress,
    tx_kind: TransactionKind,
    gas_budget: Option<u64>,
    gas_price: u64,
    gas_objects: Vec<ObjectRef>,
    gas_sponsor: Option<IotaAddress>,
    skip_checks: Option<bool>,
) -> Result<IotaClientCommandResult, anyhow::Error> {
    let client = context.get_client().await?;
    let gas_budget = gas_budget.map(iota_serde::BigInt::from);

    let dev_inspect_args = DevInspectArgs {
        gas_sponsor,
        gas_budget,
        gas_objects: (!gas_objects.is_empty()).then_some(gas_objects),
        skip_checks,
        show_raw_txn_data_and_effects: None,
    };
    let dev_inspect_result = client
        .read_api()
        .dev_inspect_transaction_block(
            signer,
            tx_kind,
            Some(iota_serde::BigInt::from(gas_price)),
            None,
            Some(dev_inspect_args),
        )
        .await?;
    Ok(IotaClientCommandResult::DevInspect(dev_inspect_result))
}

pub(crate) async fn prerender_clever_errors(
    effects: &mut IotaTransactionBlockEffects,
    read_api: &ReadApi,
) {
    let IotaTransactionBlockEffects::V1(effects) = effects;
    if let IotaExecutionStatus::Failure { error } = &mut effects.status {
        if let Some(rendered) = render_clever_error_opt(error, read_api).await {
            *error = rendered;
        }
    }
}

fn opts_from_cli(opts: HashSet<DisplayOption>) -> IotaTransactionBlockResponseOptions {
    if opts.is_empty() {
        IotaTransactionBlockResponseOptions::new()
            .with_input()
            .with_effects()
            .with_events()
            .with_object_changes()
            .with_balance_changes()
    } else {
        IotaTransactionBlockResponseOptions {
            show_input: opts.contains(&DisplayOption::Input),
            show_raw_input: false,
            show_effects: true,
            show_raw_effects: false,
            show_events: opts.contains(&DisplayOption::Events),
            show_object_changes: opts.contains(&DisplayOption::ObjectChanges),
            show_balance_changes: opts.contains(&DisplayOption::BalanceChanges),
        }
    }
}

pub(crate) fn parse_display_option(s: &str) -> Result<HashSet<DisplayOption>, String> {
    let mut options = HashSet::new();

    // Split the input string by commas and try to parse each part
    for part in s.split(',') {
        let part = part.trim(); // Trim whitespace
        match DisplayOption::from_str(part) {
            Ok(option) => {
                options.insert(option);
            }
            Err(_) => return Err(format!("Invalid display option: {part}")), /* Return error if
                                                                              * invalid */
        }
    }

    Ok(options)
}

/// Warn the user if the CLI does not match the version of current on-chain
/// protocol.
async fn check_protocol_version_and_warn(read_api: &ReadApi) -> Result<(), anyhow::Error> {
    let protocol_cfg = read_api.get_protocol_config(None).await?;
    let on_chain_protocol_version = protocol_cfg.protocol_version.as_u64();
    let cli_protocol_version = ProtocolVersion::MAX.as_u64();

    if cli_protocol_version != on_chain_protocol_version {
        let warning_msg = format!(
            "[warning] The CLI's protocol version is {cli_protocol_version}, but the active \
            network's protocol version is {on_chain_protocol_version}."
        );
        let help_msg = if cli_protocol_version < on_chain_protocol_version {
            "Consider installing the latest version of the CLI - \
            https://docs.iota.org/developer/references/cli \n\n \
            If publishing/upgrading returns a dependency verification error, then install the \
            latest CLI version."
        } else {
            "Consider waiting for the network to have upgraded to the same version, \
            or using a previous version of the CLI for this operation."
        };

        eprintln!("{}", format!("{warning_msg}\n{help_msg}").yellow().bold())
    }

    Ok(())
}

/// Try to convert this object into a package.
fn to_package(o: IotaObjectResponse) -> anyhow::Result<MovePackage> {
    let id = o.object_id()?;
    let Some(IotaRawData::Package(p)) = o.into_object()?.bcs else {
        bail!("Object {id} not a package");
    };

    Ok(p.to_move_package(u64::MAX /* safe as this pkg comes from the network */)?)
}

/// Fetch move packages
async fn fetch_move_packages(
    read_api: &ReadApi,
    immediate_dep_packages: &BTreeMap<Symbol, ObjectID>,
) -> Result<Vec<MovePackage>, anyhow::Error> {
    let package_ids: Vec<_> = immediate_dep_packages.values().cloned().collect(); // a map from id to pkg name for finding package names for error reporting.
    let pkg_id_to_name: BTreeMap<_, _> = immediate_dep_packages
        .iter()
        .map(|(name, id)| (id, name))
        .collect();

    let objects = read_api
        .multi_get_object_with_options(package_ids, IotaObjectDataOptions::bcs_lossless())
        .await?;

    let mut packages = Vec::with_capacity(objects.len());
    for o in objects {
        let id = o.object_id()?;
        packages.push(to_package(o).with_context(|| {
            format!(
                "Failed to fetch package {}",
                pkg_id_to_name
                    .get(&id)
                    .map_or("of unknown name", |x| x.as_str())
            )
        })?);
    }

    Ok(packages)
}

// Fetch the original ids of all the transitive dependencies of the immediate
// package dependencies
async fn trans_deps_original_ids(
    read_api: &ReadApi,
    immediate_dep_packages: &BTreeMap<Symbol, ObjectID>,
) -> Result<BTreeSet<ObjectID>, anyhow::Error> {
    let pkgs = fetch_move_packages(read_api, immediate_dep_packages).await?;
    let linkage_table = pkgs
        .iter()
        .flat_map(|pkg| pkg.linkage_table().keys())
        .copied()
        .collect();

    Ok(linkage_table)
}

/// Filter out a package's dependencies which are not referenced in the source
/// code. The algorithm finds the immediate dependencies of this package, and
/// the original ids of each transitive dependencies for all these immediate
/// package dependencies. For packages that are not referenced in the source
/// code, they will be filtered out from the list of dependencies.
pub(crate) async fn pkg_tree_shake(
    read_api: &ReadApi,
    with_unpublished_dependencies: bool,
    compiled_package: &mut CompiledPackage,
) -> Result<(), anyhow::Error> {
    // these are packages that are immediate dependencies of the root package
    let immediate_dep_packages =
        compiled_package.find_immediate_deps_pkgs_to_keep(with_unpublished_dependencies)?;

    // for every immediate dependency package, we need to use its linkage table to
    // determine its transitive dependencies and ensure that we keep the
    // required packages, so fetch those tables
    let trans_deps_orig_ids = trans_deps_original_ids(read_api, &immediate_dep_packages).await?;
    let pkg_name_to_orig_id: BTreeMap<_, _> = compiled_package
        .package
        .deps_compiled_units
        .iter()
        .map(|(pkg_name, module)| (*pkg_name, ObjectID::from(module.unit.address.into_inner())))
        .collect();

    // for every published package in the original list of published dependencies,
    // get its original id and then check if that id exists in the linkage
    // table. If it does, then we need to keep this package. Similarly, all
    // immediate dep packages must stay
    compiled_package.dependency_ids.published.retain(|pkg, _| {
        immediate_dep_packages.contains_key(pkg)
            || pkg_name_to_orig_id
                .get(pkg)
                .is_some_and(|id| trans_deps_orig_ids.contains(id))
    });

    Ok(())
}

async fn select_coins_for_amount(
    amount: u64,
    sender: IotaAddress,
    context: &mut WalletContext,
) -> anyhow::Result<Vec<ObjectID>> {
    let mut coins = Vec::new();

    let mut gas_coins = context
        .gas_objects(sender)
        .await?
        .iter()
        // Ok to unwrap() since `gas_objects` guarantees gas
        .map(|(_val, object)| GasCoin::try_from(object).unwrap())
        .collect::<Vec<_>>();
    // Sort in ascending order
    gas_coins.sort_unstable_by_key(|c| c.value());
    let mut amount_remaining = amount;
    while amount_remaining > 0 {
        if let Some(coin) = gas_coins.pop() {
            amount_remaining = amount_remaining.saturating_sub(coin.value());
            coins.push(*coin.id());
        } else {
            anyhow::bail!(
                "insufficient funds for requested amount: {amount}, available: {}",
                amount - amount_remaining
            );
        }
    }

    Ok(coins)
}

/// Resolves authentication call arguments, removing the signer arg added by the
/// VM.
pub(crate) async fn resolve_auth_call_args(
    client: &IotaClient,
    package: ObjectID,
    module: &str,
    function: &str,
    type_args: &[TypeTag],
    json_args: Vec<IotaJsonValue>,
) -> Result<Vec<CallArg>, anyhow::Error> {
    let mut call_args = client
        .transaction_builder()
        .resolve_and_check_json_args_to_call_args(
            package,
            &Identifier::from_str(module)?,
            &Identifier::from_str(function)?,
            type_args,
            json_args,
        )
        .await?;
    call_args.remove(0); // remove signer arg as it's added by the VM
    Ok(call_args)
}

/// Fetches AuthenticatorFunctionRefV1 for a signer.
pub(crate) async fn fetch_auth_info(
    client: &IotaClient,
    signer: IotaAddress,
) -> Result<Field<AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1>, anyhow::Error> {
    let authenticator_function_ref_id = dynamic_field::derive_dynamic_field_id(
        signer,
        &AuthenticatorFunctionRefV1Key::tag().into(),
        &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
    )?;

    let response = client
        .read_api()
        .get_object_with_options(
            authenticator_function_ref_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?;

    if let Some(error) = response.error {
        bail!("Failed to fetch AuthenticatorFunctionRefV1 object {error}");
    }

    let auth_info = response
        .into_object()?
        .bcs
        .ok_or_else(|| anyhow::anyhow!("missing bcs"))?
        .try_into_move()
        .ok_or_else(|| anyhow::anyhow!("invalid move type"))?
        .deserialize::<Field<AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1>>()?;

    Ok(auth_info)
}

/// Processes authentication arguments.
pub(crate) fn process_auth_args(
    auth_call_args: Option<&Vec<String>>,
    auth_type_args: Option<&Vec<String>>,
    signer: IotaAddress,
) -> Result<(Vec<TypeTag>, Vec<IotaJsonValue>), anyhow::Error> {
    let type_args = auth_type_args
        .as_ref()
        .map(|args| {
            args.iter()
                .map(|arg| TypeTag::from_str(arg))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    let mut json_args: Vec<_> = auth_call_args
        .as_ref()
        .map(|args| {
            args.iter()
                .map(|arg| {
                    // Use bracket-aware parsing only for array-like args
                    // (e.g. "[0xAA,0xBBCC]"), so that nested vectors are
                    // correctly split. Everything else is kept as a JSON
                    // string, which the resolver handles for u64, hex
                    // bytes, addresses, object IDs, etc.
                    if arg.starts_with('[') {
                        IotaJsonValue::from_str(arg).unwrap_or_else(|_| {
                            IotaJsonValue::new(serde_json::to_value(arg).unwrap()).unwrap()
                        })
                    } else {
                        IotaJsonValue::new(serde_json::to_value(arg).unwrap()).unwrap()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    json_args.insert(
        0,
        IotaJsonValue::new(serde_json::to_value(signer).unwrap()).unwrap(),
    );

    Ok((type_args, json_args))
}

/// Creates a MoveAuthenticator signature for account addresses.
async fn create_move_authenticator_signature(
    client: &IotaClient,
    address: IotaAddress,
    auth_call_args: Option<&Vec<String>>,
    auth_type_args: Option<&Vec<String>>,
) -> Result<GenericSignature, anyhow::Error> {
    let auth_info = fetch_auth_info(client, address).await?;

    let (type_args, json_args) = process_auth_args(auth_call_args, auth_type_args, address)?;

    let call_args = resolve_auth_call_args(
        client,
        auth_info.value.package,
        &auth_info.value.module,
        &auth_info.value.function,
        &type_args,
        json_args,
    )
    .await?;

    let initial_shared_version = get_shared_object_version(client, &address).await?;

    Ok(GenericSignature::MoveAuthenticator(
        MoveAuthenticator::new_v1(
            call_args,
            type_args.into_iter().map(TypeInput::from).collect(),
            CallArg::Object(iota_types::transaction::ObjectArg::SharedObject {
                id: ObjectID::from(address),
                initial_shared_version,
                mutable: false,
            }),
        ),
    ))
}

#[cfg(test)]
mod tests_process_auth_args {
    use iota_types::base_types::IotaAddress;
    use serde_json::Value as JsonValue;

    use super::process_auth_args;

    #[test]
    fn test_no_args() {
        let signer = IotaAddress::ZERO;
        let (type_args, json_args) = process_auth_args(None, None, signer).unwrap();
        assert!(type_args.is_empty());
        // Should only contain the signer
        assert_eq!(json_args.len(), 1);
    }

    #[test]
    fn test_simple_hex_arg() {
        let signer = IotaAddress::ZERO;
        let args = vec!["0xAABBCC".to_string()];
        let (_, json_args) = process_auth_args(Some(&args), None, signer).unwrap();
        // signer + 1 arg
        assert_eq!(json_args.len(), 2);
        assert_eq!(
            json_args[1].to_json_value(),
            JsonValue::String("0xAABBCC".to_string())
        );
    }

    #[test]
    fn test_vector_arg() {
        let signer = IotaAddress::ZERO;
        let args = vec!["[0xAABBCC,0xDDEE]".to_string()];
        let (_, json_args) = process_auth_args(Some(&args), None, signer).unwrap();
        assert_eq!(json_args.len(), 2);
        let val = &json_args[1].to_json_value();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str().unwrap(), "0xAABBCC");
        assert_eq!(arr[1].as_str().unwrap(), "0xDDEE");
    }

    #[test]
    fn test_triple_nested_vector() {
        let signer = IotaAddress::ZERO;
        // Simulates: vector<vector<vector<u8>>> with nested brackets
        let args = vec!["[[0xAA,0xBB],[0xCC]]".to_string()];
        let (_, json_args) = process_auth_args(Some(&args), None, signer).unwrap();
        assert_eq!(json_args.len(), 2);
        let val = &json_args[1].to_json_value();
        assert!(val.is_array());
        let outer = val.as_array().unwrap();
        assert_eq!(outer.len(), 2);
        // First inner array: ["0xAA", "0xBB"]
        assert!(outer[0].is_array());
        let first = outer[0].as_array().unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].as_str().unwrap(), "0xAA");
        assert_eq!(first[1].as_str().unwrap(), "0xBB");
        // Second inner array: ["0xCC"]
        assert!(outer[1].is_array());
        let second = outer[1].as_array().unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].as_str().unwrap(), "0xCC");
    }

    #[test]
    fn test_multiple_args_with_nested_vector() {
        let signer = IotaAddress::ZERO;
        // Simulates: merkle_root, merkle_proof (vector<vector<u8>>), signature
        let merkle_root = "0xABCD";
        let proof = "[0x1234,0x5678]";
        let signature = "0xEF01";
        let args = vec![
            merkle_root.to_string(),
            proof.to_string(),
            signature.to_string(),
        ];
        let (_, json_args) = process_auth_args(Some(&args), None, signer).unwrap();
        // signer + 3 args
        assert_eq!(json_args.len(), 4);
        // merkle_root is a string
        assert_eq!(
            json_args[1].to_json_value(),
            JsonValue::String(merkle_root.to_string())
        );
        // proof is an array
        let proof_val = &json_args[2].to_json_value();
        assert!(proof_val.is_array());
        assert_eq!(proof_val.as_array().unwrap().len(), 2);
        // signature is a string
        assert_eq!(
            json_args[3].to_json_value(),
            JsonValue::String(signature.to_string())
        );
    }

    #[test]
    fn test_type_args() {
        let signer = IotaAddress::ZERO;
        let type_args = vec!["u64".to_string(), "bool".to_string()];
        let (types, _) = process_auth_args(None, Some(&type_args), signer).unwrap();
        assert_eq!(types.len(), 2);
    }
}
