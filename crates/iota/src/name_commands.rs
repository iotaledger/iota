// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    mem::take,
    str::FromStr,
    time::{Duration, SystemTime},
};

use anyhow::{bail, ensure};
use blake2::Digest;
use chrono::{Utc, prelude::DateTime};
use clap::Parser;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockResponse,
};
use iota_names::{
    IotaNamesNft, NameRegistration, SubnameRegistration,
    config::IotaNamesConfig,
    name::Name,
    registry::{NameRecord, RegistryEntry, ReverseRegistryEntry},
};
use iota_protocol_config::Chain;
use iota_sdk::{IotaClient, PagedFn, wallet_context::WalletContext};
use iota_types::{
    IOTA_CLOCK_OBJECT_ID, IOTA_FRAMEWORK_PACKAGE_ID, TypeTag,
    base_types::{IotaAddress, ObjectID},
    collection_types::{Entry, VecMap},
    digests::{ChainIdentifier, TransactionDigest},
    dynamic_field::Field,
    error::IotaObjectResponseError,
};
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    identifier::Identifier,
    language_storage::StructTag,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;
use tabled::{
    Table,
    builder::Builder as TableBuilder,
    settings::{Style as TableStyle, style::HorizontalLine},
};

use crate::{
    PrintableResult,
    client_commands::{
        GasDataArgs, IotaClientCommandResult, IotaClientCommands, PaymentArgs, TxProcessingArgs,
    },
    client_ptb::ptb::PTB,
    key_identity::{KeyIdentity, get_identity_address},
};

/// Minimum coin amount (in NANOs) required for gas payment eligibility.
/// Coins below this value (9_000_000 NANOs = 0.009 IOTA) are ignored for gas
/// payment.
const MIN_COIN_AMOUNT_FOR_GAS_PAYMENT: u64 = 9_000_000;

/// Tool to register and manage names and subnames
#[derive(Parser)]
pub enum NameCommand {
    /// Check the availability of a name and return its price if available.
    /// Subnames are always free to register by the parent name owner.
    Availability { name: Name },
    /// Burn an expired IOTA-Names NFT
    Burn {
        /// The name. Ex. my-name.iota
        name: Name,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Get user data by its key
    GetUserData {
        /// The name. Ex. my-name.iota
        name: Name,
        /// A key representing data in the table. If not provided then all
        /// records will be returned.
        key: Option<String>,
    },
    /// List the names and subnames owned by the given address, or the
    /// active address. Note that leaf subnames are not listed.
    List { address: Option<KeyIdentity> },
    /// Lookup the address of a name
    Lookup { name: Name },
    /// Register a name
    Register {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        coin: Option<ObjectID>,
        /// The address or alias to which the name will point. If the flag is
        /// specified without a value, the current active address will be used.
        #[arg(long)]
        set_target_address: Option<Option<KeyIdentity>>,
        /// Set the reverse lookup for the name. This will fail if the
        /// `set-target-address` flag is provided with an address other than the
        /// sender or if no target address is set.
        #[arg(long)]
        set_reverse_lookup: bool,
        /// Coupons to apply discounts to the price.
        #[arg(long, num_args(1..))]
        coupons: Vec<String>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Renew an existing name. Cost is the name price * years.
    Renew {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The number of years to renew the name. Must be within [1-5]
        /// interval.
        years: u8,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        coin: Option<ObjectID>,
        /// Coupons to apply discounts to the price.
        #[arg(long, num_args(1..))]
        coupons: Vec<String>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Lookup a name by its address if reverse lookup was set
    ReverseLookup {
        /// The address for which to look up its name. Defaults to the active
        /// address.
        address: Option<KeyIdentity>,
    },
    /// Set the reverse lookup of the name to the transaction sender address
    SetReverseLookup {
        /// Name for which to set the reverse lookup
        name: Name,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Set the target address for a name
    SetTargetAddress {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The address to which the name will point. Defaults to the current
        /// active address.
        new_address: Option<KeyIdentity>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Set arbitrary keyed user data
    SetUserData {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The key representing the data in the table
        key: String,
        /// The value in the table
        value: String,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Commands for managing subnames
    #[command(subcommand)]
    Subname(SubnameCommand),
    /// Transfer a registered name to another address via the owned NFT
    Transfer {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The address to which the name will be transferred
        address: KeyIdentity,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Unset reverse lookup
    UnsetReverseLookup {
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Unset the target address for a name
    UnsetTargetAddress {
        /// The name. Ex. my-name.iota
        name: Name,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Unset keyed user data
    UnsetUserData {
        /// The name. Ex. my-name.iota
        name: Name,
        /// The key representing the data in the table
        key: String,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
}

impl NameCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<NameCommandResult, anyhow::Error> {
        let iota_client = context.get_client().await?;

        Ok(match self {
            Self::Availability { name } => {
                let name_str = name.to_string();

                let price = match get_registry_entry(&name, &iota_client).await {
                    Ok(_) => None,
                    Err(RpcError::IotaObjectResponse(IotaObjectResponseError::NotExists {
                        ..
                    })) => Some(if name.is_subname() {
                        0
                    } else {
                        fetch_pricing_config(&iota_client)
                            .await?
                            .get_price(name.label(1).unwrap())?
                    }),
                    Err(e) => return Err(e.into()),
                };

                NameCommandResult::Availability {
                    name: name_str,
                    price,
                }
            }
            Self::Burn {
                name,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let nft =
                    get_owned_nft_by_name::<NameRegistration>(&name, processing.sender, context)
                        .await?;

                if !nft.has_expired() {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();
                    bail!("NFT for {name} has not expired yet: {expiration_datetime}");
                }

                let burn_function = if nft.name().parent().is_some() {
                    "burn_expired_subname"
                } else {
                    "burn_expired"
                };
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: burn_function.to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Burn {
                        burned: nft,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::GetUserData { name, key } => {
                let entry = get_registry_entry(&name, &iota_client).await?;

                if let Some(key) = key {
                    let Some(value) = entry
                        .name_record
                        .data
                        .contents
                        .into_iter()
                        .find(|entry| entry.key == key)
                    else {
                        bail!("no value found for key `{key}`");
                    };
                    NameCommandResult::UserData(VecMap {
                        contents: vec![value],
                    })
                } else {
                    NameCommandResult::UserData(entry.name_record.data)
                }
            }
            Self::List { address } => {
                let address = get_identity_address(address, context).await?;
                let mut nfts = get_owned_nfts::<NameRegistration>(address, context).await?;
                let subname_nfts = get_owned_nfts::<SubnameRegistration>(address, context).await?;
                nfts.extend(subname_nfts.into_iter().map(|nft| nft.into_inner()));
                NameCommandResult::List(nfts)
            }
            Self::Lookup { name } => {
                let entry = get_registry_entry(&name, &iota_client).await?;
                NameCommandResult::Lookup {
                    name,
                    target_address: entry.name_record.target_address,
                }
            }
            Self::Register {
                name,
                coin,
                set_target_address,
                set_reverse_lookup,
                coupons,
                verbose,
                payment,
                gas_data,
                mut processing,
            } => {
                ensure!(
                    name.num_labels() == 2,
                    "name to register must consist of two labels"
                );
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let label = name.label(1).unwrap();
                let mut price = fetch_pricing_config(&iota_client).await?.get_price(label)?;

                if !coupons.is_empty() {
                    price = CouponHouse::new(&iota_client)
                        .await?
                        .apply_coupons(&coupons, price, &iota_client)
                        .await?;
                }

                let sender = processing.sender;
                let name_str = name.to_string();
                let coin =
                    select_coin_arg_for_payment(name_str.as_str(), coin, price, sender, context)
                        .await?;
                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_string(),
                    "--assign sender".to_string(),
                    format!("--split-coins {coin} [{price}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {}::payment::init_registration @{} '{name_str}'",
                        iota_names_config.package_address, iota_names_config.object_id
                    ),
                    "--assign register_intent".to_string(),
                ];

                if !coupons.is_empty() {
                    let coupons_package_address = get_coupons_package_address(&iota_client).await?;

                    for coupon in coupons {
                        args.push(format!("--move-call {coupons_package_address}::coupon_house::apply_coupon register_intent @{} '{coupon}' @{IOTA_CLOCK_OBJECT_ID}", iota_names_config.object_id,
                        ));
                    }
                }

                args.extend_from_slice(&[
                    format!(
                        "--move-call {}::payments::handle_base_payment <{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA> @{} register_intent coins.0",
                        iota_names_config.payments_package_address, iota_names_config.object_id
                    ),
                    "--assign receipt".to_string(),
                    format!(
                        "--move-call {}::payment::register receipt @{} @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id
                    ),
                    "--assign nft".to_string(),
                ]);

                if let Some(identity) = &set_target_address {
                    let target_address =
                        get_identity_address(identity.clone().or(sender.map(Into::into)), context)
                            .await?;
                    let sender = get_identity_address(sender.map(Into::into), context).await?;
                    if set_reverse_lookup && target_address != sender {
                        bail!("cannot set reverse lookup if target address is not the sender");
                    }
                    args.push(format!(
                        "--move-call {}::controller::set_target_address @{} nft some(@{target_address}) @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ));
                }
                if set_reverse_lookup {
                    if set_target_address.is_none() {
                        bail!("cannot set reverse lookup without first setting the target address");
                    }
                    args.push(format!(
                        "--move-call {}::controller::set_reverse_lookup @{} '{name_str}'",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ));
                }
                args.push("--transfer-objects [nft] sender".to_string());
                let display = take(&mut processing.display);
                args.extend(payment.into_args());
                args.extend(gas_data.into_args());
                args.extend(processing.into_args());
                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Register {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<NameRegistration>(&name, sender, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Renew {
                name,
                years,
                coin,
                coupons,
                verbose,
                payment,
                gas_data,
                mut processing,
            } => {
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let label = name.label(1).unwrap();
                let mut price = fetch_renewal_config(context)
                    .await?
                    .pricing
                    .get_price(label)?
                    * years as u64;

                if !coupons.is_empty() {
                    price = CouponHouse::new(&iota_client)
                        .await?
                        .apply_coupons(&coupons, price, &iota_client)
                        .await?;
                }

                let sender = processing.sender;
                let name_str = name.to_string();
                let coin =
                    select_coin_arg_for_payment(name_str.as_str(), coin, price, sender, context)
                        .await?;
                let nft_id = get_owned_nft_by_name::<NameRegistration>(&name, sender, context)
                    .await?
                    .id();
                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_string(),
                    "--assign sender".to_string(),
                    format!("--split-coins {coin} [{price}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {}::payment::init_renewal @{} @{nft_id} {years}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ),
                    "--assign renew_intent".to_string(),
                ];

                if !coupons.is_empty() {
                    let coupons_package_address = get_coupons_package_address(&iota_client).await?;

                    for coupon in coupons {
                        args.push(format!("--move-call {coupons_package_address}::coupon_house::apply_coupon renew_intent @{} '{coupon}' @{IOTA_CLOCK_OBJECT_ID}", iota_names_config.object_id,
                        ));
                    }
                }

                args.extend_from_slice(&[
                    format!(
                        "--move-call {}::payments::handle_base_payment <{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA> @{} renew_intent coins.0",
                        iota_names_config.payments_package_address, iota_names_config.object_id
                    ),
                    "--assign receipt".to_string(),
                    format!(
                        "--move-call {}::payment::renew receipt @{} @{nft_id} @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ),
                ]);

                let display = take(&mut processing.display);
                args.extend(payment.into_args());
                args.extend(gas_data.into_args());
                args.extend(processing.into_args());

                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Renew {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<NameRegistration>(&name, sender, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::ReverseLookup { address } => {
                let address = get_identity_address(address, context).await?;
                let entry = get_reverse_registry_entry(address, &iota_client).await?;

                NameCommandResult::ReverseLookup {
                    address,
                    name: entry.map(|e| e.name),
                }
            }
            Self::SetReverseLookup {
                name,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                // Check ownership of the name off-chain to avoid potentially wasting gas
                let sender =
                    get_identity_address(processing.sender.map(Into::into), context).await?;
                get_proxy_nft_by_name(&name, Some(sender), context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "set_reverse_lookup".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::new(serde_json::to_value(name.to_string())?)?,
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    let Some(entry) = get_reverse_registry_entry(sender, &iota_client).await?
                    else {
                        return Ok(NameCommandResult::CommandResult(Box::new(
                            IotaClientCommandResult::TransactionBlock(res),
                        )));
                    };
                    Ok(NameCommandResult::SetReverseLookup {
                        entry,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::SetTargetAddress {
                name,
                new_address,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let entry = get_registry_entry(&name, &iota_client).await?;
                if entry.name_record.is_leaf_record() {
                    bail!(
                        "cannot set target address for leaf subname; try removing and recreating the subname instead."
                    );
                }
                let sender = processing.sender;
                let new_address =
                    get_identity_address(new_address.or(sender.map(Into::into)), context).await?;
                if entry
                    .name_record
                    .target_address
                    .is_some_and(|a| a == new_address)
                {
                    bail!("target address is already set to the given value");
                }

                let nft = get_proxy_nft_by_name(&name, sender, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: nft.controller_package_id(&iota_client).await?,
                    module: nft.controller_module_name().to_owned(),
                    function: "set_target_address".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::to_value(vec![new_address])?)?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    let entry = get_registry_entry(&name, &iota_client).await?;
                    Ok(NameCommandResult::SetTargetAddress {
                        entry,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::SetUserData {
                name,
                key,
                value,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let sender = processing.sender;
                let nft = get_proxy_nft_by_name(&name, sender, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: nft.controller_package_id(&iota_client).await?,
                    module: nft.controller_module_name().to_owned(),
                    function: "set_user_data".to_owned(),
                    type_args: vec![],
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::Value::String(key.clone()))?,
                        IotaJsonValue::new(serde_json::Value::String(value.clone()))?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::SetUserData {
                        key,
                        value,
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Subname(subname_command) => subname_command.execute(context).await?,
            Self::Transfer {
                name,
                address,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let address = get_identity_address(Some(address), context).await?;
                let sender = processing.sender;
                let nft = get_proxy_nft_by_name(&name, sender, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: IOTA_FRAMEWORK_PACKAGE_ID,
                    module: "transfer".to_owned(),
                    function: "public_transfer".to_owned(),
                    type_args: vec![nft.type_(iota_names_config.package_address.into()).into()],
                    args: vec![
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::to_value(address)?)?,
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Transfer {
                        name,
                        to: address,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UnsetReverseLookup {
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let iota_names_config = get_iota_names_config(&iota_client).await?;
                let address =
                    get_identity_address(processing.sender.map(Into::into), context).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "unset_reverse_lookup".to_owned(),
                    type_args: Default::default(),
                    args: vec![IotaJsonValue::from_object_id(iota_names_config.object_id)],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::UnsetReverseLookup {
                        address,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UnsetTargetAddress {
                name,
                payment,
                gas_data,
                processing,
                verbose,
            } => {
                let entry = get_registry_entry(&name, &iota_client).await?;
                if entry.name_record.is_leaf_record() {
                    bail!("cannot unset target address for leaf subname");
                }
                if entry.name_record.target_address.is_none() {
                    bail!("target address is already unset");
                }

                let sender = processing.sender;

                let nft = get_proxy_nft_by_name(&name, sender, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: nft.controller_package_id(&iota_client).await?,
                    module: nft.controller_module_name().to_owned(),
                    function: "set_target_address".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::to_value(Vec::<IotaAddress>::new())?)?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    let entry = get_registry_entry(&name, &iota_client).await?;
                    Ok(NameCommandResult::UnsetTargetAddress {
                        entry,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UnsetUserData {
                name,
                key,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let sender = processing.sender;
                let nft = get_proxy_nft_by_name(&name, sender, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: nft.controller_package_id(&iota_client).await?,
                    module: nft.controller_module_name().to_owned(),
                    function: "unset_user_data".to_owned(),
                    type_args: vec![],
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::Value::String(key.clone()))?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::UnsetUserData {
                        key,
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
        })
    }
}

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum SubnameCommand {
    /// Register a new leaf subname, which will NOT create an NFT but instead
    /// is managed by its parent NFT. Note that leaf subnames are not listed by
    /// the `list` command.
    RegisterLeaf {
        /// The subname. Ex. my-subname.my-name.iota
        name: Name,
        /// The address to which the subname will point. Defaults to the
        /// active address.
        target_address: Option<KeyIdentity>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Register a new node subname, which will create an NFT for management
    RegisterNode {
        /// The subname. Ex. my-subname.my-name.iota
        name: Name,
        /// Expiration timestamp in one of the following formats:
        ///  - YYYY-MM-DD HH:MM:SS +0000 (Ex. 2015-02-18 23:16:09 -0500)
        ///  - YYYY-MM-DD HH:MM:SS.MMM +0000 (Ex. 2015-02-18 23:16:09.123 -0500)
        ///  - unix timestamp (Ex. 1424297769000)
        ///
        /// Defaults to the parent's expiration
        #[arg(long, short = 'e', verbatim_doc_comment)]
        expiration_timestamp: Option<Timestamp>,
        /// Whether to allow further subname creation.
        #[arg(long, short = 'c')]
        allow_creation: bool,
        /// Whether to allow expiration time extension.
        #[arg(long, short = 't')]
        allow_time_extension: bool,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Update the metadata flags for a subname
    UpdateMetadata {
        /// The subname. Ex. my-subname.my-name.iota
        name: Name,
        /// Whether to allow further subname creation.
        #[arg(long, short = 'c')]
        allow_creation: std::primitive::bool, // https://github.com/clap-rs/clap/issues/4626
        /// Whether to allow expiration time extension.
        #[arg(long, short = 't')]
        allow_time_extension: std::primitive::bool, // https://github.com/clap-rs/clap/issues/4626
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
    /// Extend the expiration of a subname
    ExtendExpiration {
        /// The subname. Ex. my-subname.my-name.iota
        name: Name,
        /// The new expiration time, which must be after the current expiration
        /// time, in one of the following formats:
        ///  - YYYY-MM-DD HH:MM:SS +0000 (Ex. 2015-02-18 23:16:09 -0500)
        ///  - YYYY-MM-DD HH:MM:SS.MMM +0000 (Ex. 2015-02-18 23:16:09.123 -0500)
        ///  - unix timestamp (Ex. 1424297769000)
        #[arg(verbatim_doc_comment)]
        expiration_timestamp: Timestamp,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        payment: PaymentArgs,
        #[command(flatten)]
        gas_data: GasDataArgs,
        #[command(flatten)]
        processing: TxProcessingArgs,
    },
}

impl SubnameCommand {
    pub async fn execute(self, context: &mut WalletContext) -> anyhow::Result<NameCommandResult> {
        let iota_client = context.get_client().await?;

        Ok(match self {
            Self::RegisterLeaf {
                name,
                target_address,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let Some(parent) = name.parent() else {
                    bail!("invalid subname: {name}");
                };

                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let sender = processing.sender;
                let parent = get_proxy_nft_by_name(&parent, sender, context).await?;
                ensure!(!parent.has_expired(), "parent NFT has expired");
                let package_id = parent.subname_package_id(&iota_client).await?;
                let module_name = parent.subname_module_name();

                let target_address =
                    get_identity_address(target_address.or(sender.map(Into::into)), context)
                        .await?;

                let res = IotaClientCommands::Call {
                    package: package_id,
                    module: module_name.to_owned(),
                    function: "new_leaf".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(parent.id()),
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                        IotaJsonValue::new(JsonValue::String(name.to_string()))?,
                        IotaJsonValue::new(JsonValue::String(target_address.to_string()))?,
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::RegisterLeafSubname {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::RegisterNode {
                name,
                expiration_timestamp,
                allow_creation,
                allow_time_extension,
                verbose,
                payment,
                gas_data,
                mut processing,
            } => {
                let Some(parent) = name.parent() else {
                    bail!("invalid subname: {name}");
                };

                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let sender = processing.sender;
                let parent = get_proxy_nft_by_name(&parent, sender, context).await?;
                ensure!(!parent.has_expired(), "parent NFT has expired");
                let package_id = parent.subname_package_id(&iota_client).await?;
                let module_name = parent.subname_module_name();

                let expiration_timestamp =
                    expiration_timestamp.unwrap_or(Timestamp(parent.expiration_timestamp_ms()));
                ensure!(
                    expiration_timestamp
                        .as_system_time()
                        .duration_since(SystemTime::now())
                        .is_ok(),
                    "expiration timestamp is not in the future"
                );

                let expiration_timestamp = expiration_timestamp.0;
                let parent_id = parent.id();

                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_owned(),
                    "--assign sender".to_owned(),
                    format!(
                        "--move-call {package_id}::{module_name}::new \
                        @{} @{parent_id} @{IOTA_CLOCK_OBJECT_ID} \
                        '{name}' {expiration_timestamp} {allow_creation} {allow_time_extension}",
                        iota_names_config.object_id
                    ),
                    "--assign nft".to_owned(),
                    "--transfer-objects [nft] sender".to_owned(),
                ];
                let display = take(&mut processing.display);
                args.extend(payment.into_args());
                args.extend(gas_data.into_args());
                args.extend(processing.into_args());
                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::RegisterNodeSubname {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<SubnameRegistration>(&name, sender, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UpdateMetadata {
                name,
                allow_creation,
                allow_time_extension,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let Some(parent) = name.parent() else {
                    bail!("invalid subname: {name}");
                };
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let sender = processing.sender;
                let parent = get_proxy_nft_by_name(&parent, sender, context).await?;
                let package_id = parent.subname_package_id(&iota_client).await?;
                let module_name = parent.subname_module_name();

                let res = IotaClientCommands::Call {
                    package: package_id,
                    module: module_name.to_owned(),
                    function: "edit_setup".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(parent.id()),
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                        IotaJsonValue::new(JsonValue::String(name.to_string()))?,
                        IotaJsonValue::new(JsonValue::Bool(allow_creation))?,
                        IotaJsonValue::new(JsonValue::Bool(allow_time_extension))?,
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::UpdateMetadata {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::ExtendExpiration {
                name,
                expiration_timestamp,
                verbose,
                payment,
                gas_data,
                processing,
            } => {
                let sender = processing.sender;
                let nft =
                    get_owned_nft_by_name::<SubnameRegistration>(&name, sender, context).await?;
                ensure!(
                    expiration_timestamp.as_system_time() > nft.expiration_time(),
                    "new expiration time is not after old expiration: {}",
                    chrono::DateTime::<chrono::Utc>::from(nft.expiration_time())
                );
                let iota_names_config = get_iota_names_config(&iota_client).await?;
                let subnames_package = fetch_package_id_by_module_and_name(
                    &iota_client,
                    &Identifier::from_str("subnames")?,
                    &Identifier::from_str("SubnamesAuth")?,
                )
                .await?;

                let res = IotaClientCommands::Call {
                    package: subnames_package,
                    module: "subnames".to_owned(),
                    function: "extend_expiration".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(JsonValue::Number(expiration_timestamp.0.into()))?,
                    ],
                    payment,
                    gas_data,
                    processing,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::ExtendExpiration {
                        record: get_registry_entry(&name, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<SubnameRegistration>(&name, sender, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum NameCommandResult {
    Availability {
        name: String,
        price: Option<u64>,
    },
    Burn {
        burned: NameRegistration,
        digest: TransactionDigest,
    },
    CommandResult(Box<IotaClientCommandResult>),
    ExtendExpiration {
        record: NameRecord,
        nft: SubnameRegistration,
        digest: TransactionDigest,
    },
    List(Vec<NameRegistration>),
    Lookup {
        name: Name,
        target_address: Option<IotaAddress>,
    },
    Register {
        record: NameRecord,
        nft: NameRegistration,
        digest: TransactionDigest,
    },
    RegisterLeafSubname {
        record: NameRecord,
        digest: TransactionDigest,
    },
    RegisterNodeSubname {
        record: NameRecord,
        nft: SubnameRegistration,
        digest: TransactionDigest,
    },
    Renew {
        record: NameRecord,
        nft: NameRegistration,
        digest: TransactionDigest,
    },
    ReverseLookup {
        address: IotaAddress,
        name: Option<Name>,
    },
    SetReverseLookup {
        entry: ReverseRegistryEntry,
        digest: TransactionDigest,
    },
    SetTargetAddress {
        entry: RegistryEntry,
        digest: TransactionDigest,
    },
    SetUserData {
        key: String,
        value: String,
        record: NameRecord,
        digest: TransactionDigest,
    },
    Transfer {
        name: Name,
        to: IotaAddress,
        digest: TransactionDigest,
    },
    UnsetReverseLookup {
        address: IotaAddress,
        digest: TransactionDigest,
    },
    UnsetTargetAddress {
        entry: RegistryEntry,
        digest: TransactionDigest,
    },
    UnsetUserData {
        key: String,
        record: NameRecord,
        digest: TransactionDigest,
    },
    UserData(VecMap<String, String>),
    UpdateMetadata {
        record: NameRecord,
        digest: TransactionDigest,
    },
}

impl std::fmt::Display for NameCommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Availability { name, price } => match price {
                Some(price) => {
                    write!(f, "\"{name}\" is available for {price} NANOs")
                }
                None => {
                    write!(f, "\"{name}\" is not available")
                }
            },
            Self::Burn {
                burned,
                digest: transaction,
            } => {
                writeln!(f, "Burned NFT:")?;
                format_nft(f, burned)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::CommandResult(res) => res.fmt(f),
            Self::ExtendExpiration {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Successfully extended expiration")?;
                format_name_record(f, record)?;
                writeln!(f, "\nNFT:")?;
                format_subname_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::List(nfts) => {
                let mut table_builder = TableBuilder::default();

                table_builder.set_header(["id", "name", "expiration"]);

                for nft in nfts {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();

                    table_builder.push_record([
                        nft.id().to_string(),
                        nft.name_str().to_owned(),
                        format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms()),
                    ]);
                }

                let mut table = table_builder.build();
                table.with(
                    tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
                        1,
                        TableStyle::modern().get_horizontal(),
                    )]),
                );
                write!(f, "{table}")
            }
            Self::Lookup {
                name,
                target_address,
            } => {
                if let Some(target_address) = target_address {
                    write!(f, "{target_address}")
                } else {
                    write!(f, "no target address found for '{name}'")
                }
            }
            Self::Register {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Registered record:")?;
                format_name_record(f, record)?;
                writeln!(f, "\nCreated NFT:")?;
                format_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::RegisterLeafSubname {
                record,
                digest: transaction,
            } => {
                writeln!(f, "Registered record:")?;
                format_name_record(f, record)?;
                writeln!(f, "\nTransaction digest: {transaction}")?;
                write!(
                    f,
                    "IMPORTANT NOTE: leaf subnames are not listed by the `list` command. Make sure to keep track of them."
                )
            }
            Self::RegisterNodeSubname {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Registered record:")?;
                format_name_record(f, record)?;
                writeln!(f, "\nCreated NFT:")?;
                format_subname_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::Renew {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Renewed record:")?;
                format_name_record(f, record)?;
                writeln!(f, "\nUpdated NFT:")?;
                format_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::ReverseLookup { address, name } => {
                if let Some(name) = name {
                    write!(f, "{name}")
                } else {
                    write!(f, "no reverse lookup set for address '{address}'")
                }
            }
            Self::SetReverseLookup {
                entry,
                digest: transaction,
            } => {
                writeln!(f, "Successfully set reverse lookup for {}", entry.address)?;
                format_reverse_registry_entry(f, entry)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::SetTargetAddress {
                entry,
                digest: transaction,
            } => {
                writeln!(f, "Successfully set target address for {}", entry.name)?;
                format_registry_entry(f, entry)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::SetUserData {
                key,
                value,
                record,
                digest: transaction,
            } => {
                writeln!(f, "Successfully set user data \"{key}\" to \"{value}\"")?;
                format_name_record(f, record)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::Transfer {
                name,
                to,
                digest: transaction,
            } => {
                writeln!(f, "Successfully transferred {name} to {to}")?;
                write!(f, "Transaction digest: {transaction}")
            }
            Self::UserData(entries) => {
                let mut table_builder = TableBuilder::default();
                table_builder.set_header(["key", "value"]);

                for entry in &entries.contents {
                    table_builder.push_record([&entry.key, &entry.value]);
                }

                let mut table = table_builder.build();
                table.with(
                    tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
                        1,
                        TableStyle::modern().get_horizontal(),
                    )]),
                );
                write!(f, "{table}")
            }
            Self::UnsetReverseLookup {
                address,
                digest: transaction,
            } => {
                writeln!(f, "Successfully unset reverse lookup for {address}")?;
                write!(f, "Transaction digest: {transaction}")
            }
            Self::UnsetTargetAddress {
                entry,
                digest: transaction,
            } => {
                writeln!(f, "Successfully unset target address for {}", entry.name)?;
                format_registry_entry(f, entry)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::UnsetUserData {
                key,
                record,
                digest: transaction,
            } => {
                writeln!(f, "Successfully unset key \"{key}\"")?;
                format_name_record(f, record)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::UpdateMetadata {
                record,
                digest: transaction,
            } => {
                writeln!(f, "Successfully updated metadata")?;
                format_name_record(f, record)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
        }
    }
}

fn format_registry_entry(f: &mut std::fmt::Formatter, entry: &RegistryEntry) -> std::fmt::Result {
    let data = [
        ("ID", entry.id.to_string()),
        ("Name", entry.name.to_string()),
    ];
    let mut table_builder = Table::builder(data);
    table_builder.set_header(["field", "value"]);

    build_name_record_table(&mut table_builder, &entry.name_record);

    let mut table = table_builder.build();
    table.with(
        tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]),
    );
    write!(f, "{table}")
}

fn format_reverse_registry_entry(
    f: &mut std::fmt::Formatter,
    entry: &ReverseRegistryEntry,
) -> std::fmt::Result {
    let data = [
        ("ID", entry.id.to_string()),
        ("Address", entry.address.to_string()),
        ("Name", entry.name.to_string()),
    ];
    let mut table_builder = Table::builder(data);
    table_builder.set_header(["field", "value"]);
    let mut table = table_builder.build();

    table.with(
        tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]),
    );
    write!(f, "{table}")
}

fn format_name_record(f: &mut std::fmt::Formatter, record: &NameRecord) -> std::fmt::Result {
    let mut table_builder = TableBuilder::default();

    build_name_record_table(&mut table_builder, record);
    table_builder.set_header(["field", "value"]);

    let mut table = table_builder.build();
    table.with(
        tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]),
    );
    write!(f, "{table}")
}

fn build_name_record_table(table_builder: &mut TableBuilder, record: &NameRecord) {
    table_builder.push_record(["NFT ID", record.nft_id.bytes.to_string().as_str()]);
    table_builder.push_record([
        "Target Address",
        record
            .target_address
            .map(|address| address.to_string())
            .unwrap_or_else(|| "none".to_owned())
            .as_str(),
    ]);

    let expiration_datetime = DateTime::<Utc>::from(record.expiration_time())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    table_builder.push_record([
        "Expiration".to_string(),
        format!("{} ({expiration_datetime})", record.expiration_timestamp_ms),
    ]);

    for entry in &record.data.contents {
        table_builder.push_record([&entry.key, &entry.value]);
    }
}

fn format_nft(f: &mut std::fmt::Formatter, nft: &NameRegistration) -> std::fmt::Result {
    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    let data = [
        ("ID", nft.id().to_string()),
        ("Name", nft.name_str().to_owned()),
        (
            "Expiration",
            format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms()),
        ),
    ];

    let mut table_builder = Table::builder(data);
    table_builder.set_header(["field", "value"]);
    let mut table = table_builder.build();
    table.with(
        tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]),
    );
    write!(f, "{table}")
}

fn format_subname_nft(f: &mut std::fmt::Formatter, nft: &SubnameRegistration) -> std::fmt::Result {
    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    let data = [
        ("ID", nft.id().to_string()),
        ("Name", nft.name_str().to_owned()),
        (
            "Expiration",
            format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms()),
        ),
    ];

    let mut table_builder = Table::builder(data);
    table_builder.set_header(["field", "value"]);
    let mut table = table_builder.build();
    table.with(
        tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
            1,
            TableStyle::modern().get_horizontal(),
        )]),
    );
    write!(f, "{table}")
}

impl std::fmt::Debug for NameCommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = crate::unwrap_err_to_string(|| Ok(serde_json::to_string_pretty(self)?));
        write!(f, "{s}")
    }
}

impl PrintableResult for NameCommandResult {}

async fn get_owned_nfts<T: DeserializeOwned + IotaNamesNft>(
    address: IotaAddress,
    context: &mut WalletContext,
) -> anyhow::Result<Vec<T>> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let nft_type = T::type_(iota_names_config.package_address.into());
    let responses = PagedFn::collect::<Vec<_>>(async |cursor| {
        client
            .read_api()
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::StructType(nft_type.clone())),
                    Some(IotaObjectDataOptions::bcs_lossless()),
                )),
                cursor,
                None,
            )
            .await
    })
    .await?;

    responses
        .into_iter()
        .map(|res| {
            let data = res.data.expect("missing object data");
            data.bcs
                .expect("missing bcs")
                .try_as_move()
                .expect("invalid move type")
                .deserialize::<T>()
        })
        .collect::<Result<_, _>>()
}

#[derive(Copy, Clone)]
pub struct Timestamp(u64);

impl Timestamp {
    fn as_system_time(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(self.0)
    }
}

impl FromStr for Timestamp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(if s.chars().all(|c| c.is_numeric()) {
            s.parse()
                .map_err(|e| anyhow::anyhow!("invalid unix timestamp: {e}"))?
        } else {
            fn parse(s: &str, f: &str) -> anyhow::Result<u64> {
                let (dt, rem) = chrono::NaiveDateTime::parse_and_remainder(s, f)
                    .map_err(|e| anyhow::anyhow!("invalid date and time: {e}"))?;
                Ok(if rem.trim().is_empty() {
                    dt.and_utc().timestamp_millis() as _
                } else {
                    chrono::DateTime::parse_from_str(s, &format!("{f} %z"))
                        .map_err(|e| anyhow::anyhow!("invalid timezone: {e}"))?
                        .timestamp_millis() as _
                })
            }
            parse(s, "%F %X").or_else(|_| parse(s, "%F %X%.3f"))?
        }))
    }
}

async fn get_owned_nft_by_name<T: DeserializeOwned + IotaNamesNft>(
    name: &Name,
    sender: Option<IotaAddress>,
    context: &mut WalletContext,
) -> anyhow::Result<T> {
    let name = name.to_string();
    let address = get_identity_address(sender.map(Into::into), context).await?;

    for nft in get_owned_nfts::<T>(address, context).await? {
        if nft.name_str() == name {
            return Ok(nft);
        }
    }

    bail!("no matching owned {} found for {name}", T::TYPE_NAME)
}

async fn get_proxy_nft_by_name(
    name: &Name,
    sender: Option<IotaAddress>,
    context: &mut WalletContext,
) -> anyhow::Result<IotaNamesNftProxy> {
    Ok(if name.is_sln() {
        IotaNamesNftProxy::Name(get_owned_nft_by_name(name, sender, context).await?)
    } else {
        IotaNamesNftProxy::Subname(get_owned_nft_by_name(name, sender, context).await?)
    })
}

pub async fn get_registry_entry(
    name: &Name,
    client: &IotaClient,
) -> Result<RegistryEntry, RpcError> {
    let iota_names_config = get_iota_names_config(client).await?;
    let object_id = iota_names_config.record_field_id(name);

    get_object_from_bcs(client, object_id).await
}

async fn get_reverse_registry_entry(
    address: IotaAddress,
    client: &IotaClient,
) -> anyhow::Result<Option<ReverseRegistryEntry>> {
    let iota_names_config = get_iota_names_config(client).await?;
    let object_id = iota_names_config.reverse_record_field_id(&address);
    let response = client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await?;

    if response.data.is_some() {
        Ok(Some(deserialize_move_object_from_bcs(response)?))
    } else {
        Ok(None)
    }
}

async fn get_iota_names_config(client: &IotaClient) -> anyhow::Result<IotaNamesConfig> {
    Ok(if let Ok(config) = IotaNamesConfig::from_env() {
        config
    } else {
        let chain_identifier = client.read_api().get_chain_identifier().await?;
        let chain = ChainIdentifier::from_chain_short_id(&chain_identifier)
            .map(|c| c.chain())
            .unwrap_or(Chain::Unknown);

        IotaNamesConfig::from_chain(&chain)
    })
}

async fn fetch_pricing_config(client: &IotaClient) -> anyhow::Result<PricingConfig> {
    let iota_names_config = get_iota_names_config(client).await?;
    let config_type = StructTag::from_str(&format!(
        "{}::iota_names::ConfigKey<{}::pricing_config::PricingConfig>",
        iota_names_config.package_address, iota_names_config.package_address
    ))?;
    let layout = MoveTypeLayout::Struct(Box::new(MoveStructLayout {
        type_: config_type.clone(),
        fields: vec![MoveFieldLayout::new(
            Identifier::from_str("dummy_field")?,
            MoveTypeLayout::Bool,
        )],
    }));
    let object_id = iota_types::dynamic_field::derive_dynamic_field_id(
        iota_names_config.object_id,
        &TypeTag::Struct(Box::new(config_type)),
        &IotaJsonValue::new(serde_json::json!({ "dummy_field": false }))?.to_bcs_bytes(&layout)?,
    )?;

    let entry = get_object_from_bcs::<Field<DummyKey, PricingConfig>>(client, object_id)
        .await
        .map_err(|e| anyhow::anyhow!("couldn't fetch pricing config: {e}"))?;

    Ok(entry.value)
}

async fn fetch_renewal_config(context: &mut WalletContext) -> anyhow::Result<RenewalConfig> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let config_type = StructTag::from_str(&format!(
        "{}::iota_names::ConfigKey<{}::pricing_config::RenewalConfig>",
        iota_names_config.package_address, iota_names_config.package_address
    ))?;
    let layout = MoveTypeLayout::Struct(Box::new(MoveStructLayout {
        type_: config_type.clone(),
        fields: vec![MoveFieldLayout::new(
            Identifier::from_str("dummy_field")?,
            MoveTypeLayout::Bool,
        )],
    }));
    let object_id = iota_types::dynamic_field::derive_dynamic_field_id(
        iota_names_config.object_id,
        &TypeTag::Struct(Box::new(config_type)),
        &IotaJsonValue::new(serde_json::json!({ "dummy_field": false }))?.to_bcs_bytes(&layout)?,
    )?;

    let entry = get_object_from_bcs::<Field<DummyKey, RenewalConfig>>(&client, object_id)
        .await
        .map_err(|e| anyhow::anyhow!("couldn't fetch renewal config: {e}"))?;

    Ok(entry.value)
}

async fn handle_transaction_result<Fun, F>(
    res: IotaClientCommandResult,
    verbose: bool,
    fun: Fun,
) -> anyhow::Result<NameCommandResult>
where
    Fun: FnOnce(IotaTransactionBlockResponse) -> F,
    F: futures::Future<Output = anyhow::Result<NameCommandResult>>,
{
    if verbose {
        println!("{res}\n");
    }
    Ok(
        if let IotaClientCommandResult::TransactionBlock(res) = res {
            if !res.errors.is_empty() {
                bail!("transaction failed: {}", res.errors.join("; "));
            }
            fun(res).await?
        } else {
            NameCommandResult::CommandResult(Box::new(res))
        },
    )
}

pub enum IotaNamesNftProxy {
    Name(NameRegistration),
    Subname(SubnameRegistration),
}

macro_rules! def_enum_fns {
    ($($vis:vis fn $fn:ident(&self)$( -> $ret:ty)?;)+) => {
        $($vis fn $fn(&self)$( -> $ret)? {
            match self {
                IotaNamesNftProxy::Name(nft) => nft.$fn(),
                IotaNamesNftProxy::Subname(nft) => nft.$fn(),
            }
        })+
    };
}

impl IotaNamesNftProxy {
    def_enum_fns! {
        fn expiration_timestamp_ms(&self) -> u64;
        fn has_expired(&self) -> bool;
        fn id(&self) -> ObjectID;
    }

    fn type_(&self, package_id: AccountAddress) -> StructTag {
        match self {
            IotaNamesNftProxy::Name(_) => NameRegistration::type_(package_id),
            IotaNamesNftProxy::Subname(_) => SubnameRegistration::type_(package_id),
        }
    }

    async fn controller_package_id(&self, client: &IotaClient) -> anyhow::Result<ObjectID> {
        Ok(match self {
            IotaNamesNftProxy::Name(_) => {
                let names_config = get_iota_names_config(client).await?;
                names_config.package_address.into()
            }
            IotaNamesNftProxy::Subname(_) => {
                fetch_package_id_by_module_and_name(
                    client,
                    &Identifier::from_str("subname_proxy")?,
                    &Identifier::from_str("SubnameProxyAuth")?,
                )
                .await?
            }
        })
    }

    async fn subname_package_id(&self, client: &IotaClient) -> anyhow::Result<ObjectID> {
        Ok(match self {
            IotaNamesNftProxy::Name(_) => {
                fetch_package_id_by_module_and_name(
                    client,
                    &Identifier::from_str("subnames")?,
                    &Identifier::from_str("SubnamesAuth")?,
                )
                .await?
            }
            IotaNamesNftProxy::Subname(_) => {
                fetch_package_id_by_module_and_name(
                    client,
                    &Identifier::from_str("subname_proxy")?,
                    &Identifier::from_str("SubnameProxyAuth")?,
                )
                .await?
            }
        })
    }

    fn controller_module_name(&self) -> &'static str {
        match self {
            IotaNamesNftProxy::Name(_) => "controller",
            IotaNamesNftProxy::Subname(_) => "subname_proxy",
        }
    }

    fn subname_module_name(&self) -> &'static str {
        match self {
            IotaNamesNftProxy::Name(_) => "subnames",
            IotaNamesNftProxy::Subname(_) => "subname_proxy",
        }
    }
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct DummyKey {
    dummy_field: bool,
}

#[derive(Debug, Deserialize)]
struct Range(u64, u64);

impl Range {
    fn contains(&self, number: u64) -> bool {
        self.0 <= number && number <= self.1
    }
}

#[derive(Debug, Deserialize)]
struct PricingConfig {
    pricing: VecMap<Range, u64>,
}

#[derive(Debug, Deserialize)]
struct RenewalConfig {
    pricing: PricingConfig,
}

impl PricingConfig {
    pub fn get_price(&self, label: &str) -> anyhow::Result<u64> {
        for Entry { key, value } in &self.pricing.contents {
            if key.contains(label.chars().count() as u64) {
                return Ok(*value);
            }
        }
        bail!(
            "segment length {} (`{label}`) is outside of allowed ranges [{}]",
            label.len(),
            self.pricing
                .contents
                .iter()
                .map(|c| format!("{}..={}", c.key.0, c.key.1))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

// Returns "@<coin id>" or "gas" to be used as coin payment argument in a PTB
async fn select_coin_arg_for_payment(
    name: &str,
    coin: Option<ObjectID>,
    price: u64,
    sender: Option<IotaAddress>,
    context: &mut WalletContext,
) -> anyhow::Result<String> {
    Ok(match coin {
        Some(coin) => format!("@{coin}"),
        None => {
            let gas_result = IotaClientCommands::Gas {
                address: sender.map(Into::into),
            }
            .execute(context)
            .await?;
            let mut balance = 0;
            if let IotaClientCommandResult::Gas(coins) = gas_result {
                if coins
                    .iter()
                    // Ignore coins insufficient for the gas payment
                    .filter(|c| c.value() >= MIN_COIN_AMOUNT_FOR_GAS_PAYMENT)
                    .count()
                    == 1
                {
                    return Ok("gas".to_string());
                }
                for coin in coins {
                    if coin.value() >= price {
                        return Ok(format!("@{}", coin.id()));
                    }
                    balance += coin.value();
                }
            }
            if balance > price {
                bail!("merge coins first to register/renew the name '{name}'");
            } else {
                bail!("insufficient balance {balance}/{price} to register/renew the name '{name}'");
            }
        }
    })
}

// Fetch the package ID of a package that got authorized for the IOTA-Names
// object by it's module name and struct name.
async fn fetch_package_id_by_module_and_name(
    client: &IotaClient,
    module_name: &Identifier,
    struct_name: &Identifier,
) -> anyhow::Result<ObjectID> {
    let names_config = get_iota_names_config(client).await?;
    let dynamic_fields_page = client
        .read_api()
        .get_dynamic_fields(names_config.object_id, None, None)
        .await?;
    for dynamic_field in dynamic_fields_page.data {
        if let TypeTag::Struct(ref tag) = dynamic_field.name.type_ {
            for param in &tag.type_params {
                if let TypeTag::Struct(ref param_tag) = param {
                    if &param_tag.module == module_name && &param_tag.name == struct_name {
                        return Ok(ObjectID::from(param_tag.address));
                    }
                }
            }
        }
    }
    bail!("failed to find package ID for {module_name}::{struct_name}")
}

#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    #[error("{0}")]
    Any(#[from] anyhow::Error),
    #[error("{0}")]
    IotaObjectResponse(IotaObjectResponseError),
}

async fn get_object_from_bcs<T: DeserializeOwned>(
    client: &IotaClient,
    object_id: ObjectID,
) -> Result<T, RpcError> {
    let object_response = client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await
        .map_err(|e| RpcError::Any(e.into()))?;

    if let Some(error) = object_response.error {
        return Err(RpcError::IotaObjectResponse(error));
    }

    Ok(deserialize_move_object_from_bcs::<T>(object_response)?)
}

fn deserialize_move_object_from_bcs<T: DeserializeOwned>(
    object_response: IotaObjectResponse,
) -> anyhow::Result<T> {
    object_response
        .into_object()?
        .bcs
        .ok_or_else(|| anyhow::anyhow!("missing bcs"))?
        .try_into_move()
        .ok_or_else(|| anyhow::anyhow!("invalid move type"))?
        .deserialize::<T>()
}

async fn get_coupons_package_address(client: &IotaClient) -> anyhow::Result<ObjectID> {
    let coupons_package_address = fetch_package_id_by_module_and_name(
        client,
        &Identifier::from_str("coupon_house")?,
        &Identifier::from_str("CouponsAuth")?,
    )
    .await?;

    Ok(coupons_package_address)
}

#[derive(Debug, Deserialize)]
struct Coupons {
    coupons: iota_names::registry::Table,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct CouponRange {
    pub from: u8,
    pub to: u8,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct CouponRules {
    pub length: Option<CouponRange>,
    pub available_claims: Option<u64>,
    pub user: Option<IotaAddress>,
    pub expiration: Option<u64>,
    pub years: Option<CouponRange>,
    pub can_stack: bool,
}

#[derive(Debug, Deserialize)]
struct Coupon {
    pub kind: u8,
    pub amount: u64,
    pub rules: CouponRules,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct CouponHouse {
    coupons: Coupons,
    version: u8,
    id: ObjectID,
}

impl CouponHouse {
    async fn new(iota_client: &IotaClient) -> anyhow::Result<CouponHouse> {
        let coupons_package_address = get_coupons_package_address(iota_client).await?;
        let iota_names_config = get_iota_names_config(iota_client).await?;
        let coupon_house_key = StructTag::from_str(&format!(
            "{}::iota_names::RegistryKey<{coupons_package_address}::coupon_house::CouponHouse>",
            iota_names_config.package_address,
        ))?;
        let layout = MoveTypeLayout::Struct(Box::new(MoveStructLayout {
            type_: coupon_house_key.clone(),
            fields: vec![MoveFieldLayout::new(
                Identifier::from_str("dummy_field")?,
                MoveTypeLayout::Bool,
            )],
        }));
        let object_id = iota_types::dynamic_field::derive_dynamic_field_id(
            iota_names_config.object_id,
            &TypeTag::Struct(Box::new(coupon_house_key)),
            &IotaJsonValue::new(serde_json::json!({ "dummy_field": false }))?
                .to_bcs_bytes(&layout)?,
        )?;

        let entry = get_object_from_bcs::<Field<DummyKey, CouponHouse>>(iota_client, object_id)
            .await
            .map_err(|e| anyhow::anyhow!("couldn't fetch coupon house: {e}"))?;

        Ok(entry.value)
    }

    async fn get_coupon(&self, name: &str, iota_client: &IotaClient) -> anyhow::Result<Coupon> {
        let mut hasher = blake2::Blake2b::<blake2::digest::consts::U32>::new();
        hasher.update(name);
        let hash = hasher.finalize().to_vec();
        let coupon_bytes = bcs::to_bytes(&hash).unwrap();

        let object_id = iota_types::dynamic_field::derive_dynamic_field_id(
            self.coupons.coupons.id,
            &TypeTag::Vector(Box::new(TypeTag::U8)),
            &coupon_bytes,
        )?;

        let entry = get_object_from_bcs::<Field<Vec<u8>, Coupon>>(iota_client, object_id)
            .await
            .map_err(|e| anyhow::anyhow!("couldn't fetch coupon: {e}"))?;

        Ok(entry.value)
    }

    async fn apply_coupon(&self, coupon: &Coupon, price: u64) -> anyhow::Result<u64> {
        Ok(match coupon.kind {
            0 => {
                let discount_amount = ((price as u128) * (coupon.amount as u128) / 100) as u64;
                price - discount_amount
            }
            1 => price.saturating_sub(coupon.amount),
            _ => bail!("undefined coupon kind"),
        })
    }

    async fn apply_coupons(
        &self,
        coupons: &[String],
        mut price: u64,
        iota_client: &IotaClient,
    ) -> anyhow::Result<u64> {
        for coupon_str in coupons {
            let coupon = self.get_coupon(coupon_str, iota_client).await?;

            if !coupon.rules.can_stack && coupons.len() > 1 {
                bail!("coupon '{coupon_str}' cannot stack with the other coupons provided");
            }

            price = self.apply_coupon(&coupon, price).await?;
        }

        Ok(price)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(
            "2015-02-18 23:16:09".parse::<Timestamp>().unwrap().0,
            1424301369000
        );
        assert_eq!(
            "2015-02-18 23:16:09 +0800".parse::<Timestamp>().unwrap().0,
            1424272569000
        );
        assert_eq!(
            "2015-02-18 23:16:09 -0100".parse::<Timestamp>().unwrap().0,
            1424304969000
        );
        assert_eq!(
            "2015-02-18 23:16:09.987".parse::<Timestamp>().unwrap().0,
            1424301369987
        );
        assert_eq!(
            "2015-02-18 23:16:09.123 -0100"
                .parse::<Timestamp>()
                .unwrap()
                .0,
            1424304969123
        );
        assert_eq!(
            "1424304969123".parse::<Timestamp>().unwrap().0,
            1424304969123
        );
    }
}
