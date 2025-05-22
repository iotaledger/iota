// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use chrono::{Utc, prelude::DateTime};
use clap::Parser;
use iota_graphql_rpc_client::simple_client::{GraphqlQueryVariable, SimpleClient};
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockResponse,
};
use iota_names::{
    IotaNamesNft, IotaNamesRegistration, SubdomainRegistration,
    config::IotaNamesConfig,
    domain::Domain,
    registry::{NameRecord, RegistryEntry, ReverseRegistryEntry},
};
use iota_protocol_config::Chain;
use iota_sdk::{IotaClient, PagedFn, wallet_context::WalletContext};
use iota_types::{
    IOTA_CLOCK_OBJECT_ID, IOTA_FRAMEWORK_PACKAGE_ID, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID},
    coin::Coin,
    collection_types::{Entry, LinkedTable, LinkedTableNode, VecMap},
    digests::{ChainIdentifier, TransactionDigest},
};
use move_core_types::{
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
    client_commands::{IotaClientCommandResult, IotaClientCommands, OptsWithGas},
    client_ptb::ptb::PTB,
    key_identity::{KeyIdentity, get_identity_address},
};

/// The overbid must be at least of 1 IOTA, which is 10^9 NANOs
const MIN_OVERBID: u64 = 1_000_000_000;

/// Tool to register and manage domains and subdomains
#[derive(Parser)]
pub enum NameCommand {
    /// Auction related operations, like bidding and claiming
    #[command(subcommand)]
    Auction(AuctionCommand),
    /// Check the availability of a domain and return its price if available.
    /// Subdomains are always free to register by the parent domain owner.
    Availability { domain: Domain },
    /// Burn an expired IOTA-Names NFT
    Burn {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Get user data by its key
    GetUserData {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// A key representing data in the table. If not provided then all
        /// records will be returned.
        key: Option<String>,
    },
    /// List the domains owned by the given address, or the active address
    List { address: Option<IotaAddress> },
    /// Lookup the address of a domain
    Lookup { domain: Domain },
    /// Register a domain
    Register {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        coin: Option<ObjectID>,
        /// The address or alias to which the domain will point. If the flag is
        /// specified without a value, the current active address will be used.
        #[arg(long, require_equals = true, default_missing_value = "", num_args = 0..=1)]
        set_target_address: Option<String>,
        /// Set the reverse lookup for the domain. This will fail if the
        /// `set-target-address` flag is provided with an address other than the
        /// sender or if no target address is set.
        #[arg(long)]
        set_reverse_lookup: bool,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Renew an existing domain. Cost is the domain price * years.
    Renew {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The number of years to renew the domain. Must be within [1-5]
        /// interval.
        years: u8,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        coin: Option<ObjectID>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Lookup a domain by its address if reverse lookup was set
    ReverseLookup {
        /// The address for which to look up its domain. Defaults to the active
        /// address.
        address: Option<IotaAddress>,
    },
    /// Set the reverse lookup of the domain to the transaction sender address
    SetReverseLookup {
        /// Domain for which to set the reverse lookup
        domain: Domain,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Set the target address for a domain
    SetTargetAddress {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will point
        new_address: Option<IotaAddress>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Set arbitrary keyed user data
    SetUserData {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The key representing the data in the table
        key: String,
        /// The value in the table
        value: String,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Commands for managing subdomains
    #[command(subcommand)]
    Subdomain(SubdomainCommand),
    /// Transfer a registered domain to another address via the owned NFT
    Transfer {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will be transferred
        address: IotaAddress,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Unset reverse lookup
    UnsetReverseLookup {
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Unset keyed user data
    UnsetUserData {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The key representing the data in the table
        key: String,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
}

impl NameCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<NameCommandResult, anyhow::Error> {
        let iota_client = context.get_client().await?;

        Ok(match self {
            Self::Auction(cmd) => cmd.execute(context).await?,
            Self::Availability { domain } => {
                let domain_str = domain.to_string();

                let price = if iota_client
                    .read_api()
                    .iota_names_lookup(&domain_str)
                    .await?
                    .is_some()
                {
                    None
                } else {
                    Some(if domain.is_subdomain() {
                        0
                    } else {
                        fetch_pricing_config(&iota_client)
                            .await?
                            .get_price(domain.label(1).unwrap())?
                    })
                };

                NameCommandResult::Availability {
                    domain: domain_str,
                    price,
                }
            }
            Self::Burn {
                domain,
                verbose,
                opts,
            } => {
                let nft = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context).await?;

                if !nft.has_expired() {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();
                    return Err(anyhow::anyhow!(
                        "NFT for {domain} has not expired yet: {expiration_datetime}"
                    ));
                }

                let burn_function = if nft.domain().parent().is_some() {
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
                    gas_price: None,
                    opts,
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
            Self::GetUserData { domain, key } => {
                let entry = get_registry_entry(&domain, &iota_client).await?;

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
                let mut nfts = get_owned_nfts::<IotaNamesRegistration>(address, context).await?;
                let subdomain_nfts =
                    get_owned_nfts::<SubdomainRegistration>(address, context).await?;
                nfts.extend(subdomain_nfts.into_iter().map(|nft| nft.into_inner()));
                NameCommandResult::List(nfts)
            }
            Self::Lookup { domain } => {
                let entry = get_registry_entry(&domain, &iota_client).await?;
                NameCommandResult::Lookup {
                    domain,
                    target_address: entry.name_record.target_address,
                }
            }
            Self::Register {
                domain,
                coin,
                set_target_address,
                set_reverse_lookup,
                verbose,
                mut opts,
            } => {
                anyhow::ensure!(
                    domain.num_labels() == 2,
                    "domain to register must consist of two labels"
                );
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let label = domain.label(1).unwrap();
                let price = fetch_pricing_config(&iota_client).await?.get_price(label)?;
                let domain_name = domain.to_string();
                let coin =
                    select_coin_for_payment(domain_name.as_str(), coin, price, context).await?;
                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_string(),
                    "--assign sender".to_string(),
                    format!("--split-coins @{coin} [{price}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {}::payment::init_registration @{} '{domain_name}'",
                        iota_names_config.package_address, iota_names_config.object_id
                    ),
                    "--assign payment_intent".to_string(),
                    format!(
                        "--move-call {}::payments::handle_base_payment <{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA> @{} payment_intent coins.0",
                        iota_names_config.payments_package_address, iota_names_config.object_id
                    ),
                    "--assign receipt".to_string(),
                    format!(
                        "--move-call {}::payment::register receipt @{} @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id
                    ),
                    "--assign nft".to_string(),
                ];
                if let Some(identity) = &set_target_address {
                    let identity = (!identity.is_empty())
                        .then(|| identity.parse::<KeyIdentity>())
                        .transpose()?;
                    let address = get_identity_address(identity, context)?;
                    if set_reverse_lookup && address != context.active_address()? {
                        bail!("cannot set reverse lookup if target address is not the sender");
                    }
                    args.push(format!(
                        "--move-call {}::controller::set_target_address @{} nft some(@{address}) @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ));
                }
                if set_reverse_lookup {
                    if set_target_address.is_none() {
                        bail!("cannot set reverse lookup without first setting the target address");
                    }
                    args.push(format!(
                        "--move-call {}::controller::set_reverse_lookup @{} '{domain}'",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ));
                }
                args.push("--transfer-objects [nft] sender".to_string());
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());
                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Register {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Renew {
                domain,
                years,
                coin,
                verbose,
                mut opts,
            } => {
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let label = domain.label(1).unwrap();
                let price = fetch_renewal_config(context)
                    .await?
                    .pricing
                    .get_price(label)?
                    * years as u64;
                let domain_name = domain.to_string();
                let coin =
                    select_coin_for_payment(domain_name.as_str(), coin, price, context).await?;
                let nft_id = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                    .await?
                    .id();
                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_string(),
                    "--assign sender".to_string(),
                    format!("--split-coins @{coin} [{price}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {}::payment::init_renewal @{} @{nft_id} {years}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ),
                    "--assign renewal_intent".to_string(),
                    format!(
                        "--move-call {}::payments::handle_base_payment <{IOTA_FRAMEWORK_PACKAGE_ID}::iota::IOTA> @{} renewal_intent coins.0",
                        iota_names_config.payments_package_address, iota_names_config.object_id
                    ),
                    "--assign receipt".to_string(),
                    format!(
                        "--move-call {}::payment::renew receipt @{} @{nft_id} @{IOTA_CLOCK_OBJECT_ID}",
                        iota_names_config.package_address, iota_names_config.object_id,
                    ),
                ];
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());

                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Renew {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::ReverseLookup { address } => {
                let address = get_identity_address(address.map(KeyIdentity::Address), context)?;
                let entry = get_reverse_registry_entry(address, &iota_client).await?;

                NameCommandResult::ReverseLookup {
                    address,
                    domain: entry.map(|e| e.domain),
                }
            }
            Self::SetReverseLookup {
                domain,
                verbose,
                opts,
            } => {
                // Check ownership of the name off-chain to avoid potentially wasting gas
                get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "set_reverse_lookup".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::new(serde_json::to_value(domain.to_string())?)?,
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    let Some(entry) = get_reverse_registry_entry(
                        get_identity_address(None, context)?,
                        &iota_client,
                    )
                    .await?
                    else {
                        return Ok(NameCommandResult::CommandResult(
                            IotaClientCommandResult::TransactionBlock(res),
                        ));
                    };
                    Ok(NameCommandResult::SetReverseLookup {
                        entry,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::SetTargetAddress {
                domain,
                new_address,
                verbose,
                opts,
            } => {
                let nft_id = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                    .await?
                    .id();
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "set_target_address".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft_id),
                        IotaJsonValue::new(serde_json::to_value(
                            new_address.into_iter().collect::<Vec<_>>(),
                        )?)?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    let entry = get_registry_entry(&domain, &iota_client).await?;
                    Ok(NameCommandResult::SetTargetAddress {
                        entry,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::SetUserData {
                domain,
                key,
                value,
                verbose,
                opts,
            } => {
                let nft = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "set_user_data".to_owned(),
                    type_args: vec![],
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::Value::String(key.clone()))?,
                        IotaJsonValue::new(serde_json::Value::String(value.clone()))?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::SetUserData {
                        key,
                        value,
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Subdomain(subdomain_command) => subdomain_command.execute(context).await?,
            Self::Transfer {
                domain,
                address,
                verbose,
                opts,
            } => {
                let nft_id = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                    .await?
                    .id();
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: IOTA_FRAMEWORK_PACKAGE_ID,
                    module: "transfer".to_owned(),
                    function: "public_transfer".to_owned(),
                    type_args: vec![TypeTag::from_str(&format!(
                        "{}::iota_names_registration::IotaNamesRegistration",
                        iota_names_config.package_address
                    ))?],
                    args: vec![
                        IotaJsonValue::from_object_id(nft_id),
                        IotaJsonValue::new(serde_json::to_value(address)?)?,
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::Transfer {
                        domain,
                        to: address,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UnsetReverseLookup { verbose, opts } => {
                let iota_names_config = get_iota_names_config(&iota_client).await?;
                let address = get_identity_address(None, context)?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "unset_reverse_lookup".to_owned(),
                    type_args: Default::default(),
                    args: vec![IotaJsonValue::from_object_id(iota_names_config.object_id)],
                    gas_price: None,
                    opts,
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
            Self::UnsetUserData {
                domain,
                key,
                verbose,
                opts,
            } => {
                let nft = get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context).await?;
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let res = IotaClientCommands::Call {
                    package: iota_names_config.package_address.into(),
                    module: "controller".to_owned(),
                    function: "unset_user_data".to_owned(),
                    type_args: vec![],
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(serde_json::Value::String(key.clone()))?,
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::UnsetUserData {
                        key,
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
        })
    }
}

/// Commands related to the auction system
#[derive(Parser)]
pub enum AuctionCommand {
    /// Place a new bid
    Bid {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The bid amount. Must be at least one IOTA more than the last highest
        /// bid. Defaults to the minimum possible bid.
        #[arg(long)]
        amount: Option<u64>,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        #[arg(long)]
        coin: Option<ObjectID>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Claim the name if the auction winner is the sender
    Claim {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Get metadata of an auction
    Metadata { domain: Domain },
    /// Start an auction, if it's not started yet, and make the first bid
    Start {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The initial bid amount. Must be at least the minimum cost of the
        /// domain. Defaults to the minimum.
        #[arg(long)]
        amount: Option<u64>,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        #[arg(long)]
        coin: Option<ObjectID>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
}

impl AuctionCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<NameCommandResult, anyhow::Error> {
        let iota_client = context.get_client().await?;
        let graphql_client = SimpleClient::new(
            context
                .active_env()?
                .graphql()
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("missing graphql url in IotaEnv"))?,
        );

        Ok(match self {
            Self::Bid {
                domain,
                amount,
                coin,
                verbose,
                mut opts,
            } => {
                let auction_package_address = get_auction_package_address(&iota_client).await?;
                let auction_house_id =
                    get_auction_house_id(auction_package_address, &graphql_client).await?;
                let auction_house =
                    get_object_from_bcs::<AuctionHouse>(&iota_client, auction_house_id).await?;

                let auction = auction_house.get_auction(&domain, &iota_client).await?;
                let min_price = auction.current_bid.value() + MIN_OVERBID;
                let amount = amount.unwrap_or(min_price);
                anyhow::ensure!(
                    amount >= min_price,
                    "bid amount must be at least {min_price} for this domain"
                );
                let coin =
                    select_coin_for_payment(&domain.to_string(), coin, amount, context).await?;

                let mut args = vec![
                    format!("--split-coins @{coin} [{amount}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {auction_package_address}::auction::place_bid @{} '{}' coins.0 @{IOTA_CLOCK_OBJECT_ID}",
                        auction_house_id,
                        domain.to_string(),
                    ),
                ];
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());

                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::AuctionBid {
                        auction: get_auction_house(&iota_client, &graphql_client)
                            .await?
                            .get_auction(&domain, &iota_client)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Claim {
                domain,
                verbose,
                mut opts,
            } => {
                let auction_package_address = get_auction_package_address(&iota_client).await?;
                let auction_house_id =
                    get_auction_house_id(auction_package_address, &graphql_client).await?;

                let mut args = vec![
                    "--move-call iota::tx_context::sender".to_string(),
                    "--assign sender".to_string(),
                    format!(
                        "--move-call {auction_package_address}::auction::claim @{auction_house_id} '{domain}' @{IOTA_CLOCK_OBJECT_ID}",
                    ),
                    "--assign nft".to_string(),
                    "--transfer-objects [nft] sender".to_string(),
                ];
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());

                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::AuctionClaim {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<IotaNamesRegistration>(&domain, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::Metadata { domain } => NameCommandResult::AuctionMetadata(
                get_auction_house(&iota_client, &graphql_client)
                    .await?
                    .get_auction(&domain, &iota_client)
                    .await?,
            ),
            Self::Start {
                domain,
                amount,
                coin,
                verbose,
                mut opts,
            } => {
                let auction_package_address = get_auction_package_address(&iota_client).await?;
                let auction_house_id =
                    get_auction_house_id(auction_package_address, &graphql_client).await?;

                let min_price = fetch_pricing_config(&iota_client)
                    .await?
                    .get_price(domain.label(1).unwrap())?;
                let amount = amount.unwrap_or(min_price);
                anyhow::ensure!(
                    amount >= min_price,
                    "bid amount must be at least {min_price} for this domain"
                );
                let coin =
                    select_coin_for_payment(&domain.to_string(), coin, amount, context).await?;

                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let mut args = vec![
                    format!("--split-coins @{coin} [{amount}]"),
                    "--assign coins".to_string(),
                    format!(
                        "--move-call {auction_package_address}::auction::start_auction_and_place_bid @{} @{} '{}' coins.0 @{IOTA_CLOCK_OBJECT_ID}",
                        auction_house_id,
                        iota_names_config.object_id,
                        domain.to_string(),
                    ),
                ];
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());

                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::AuctionStart {
                        auction: get_auction_house(&iota_client, &graphql_client)
                            .await?
                            .get_auction(&domain, &iota_client)
                            .await?,
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
pub enum SubdomainCommand {
    /// Register a new leaf subdomain, which can only be managed by the parent's
    /// NFT
    RegisterLeaf {
        /// The full name of the subdomain. Ex. my-subdomain.my-domain.iota
        domain: Domain,
        /// The address to which the subdomain will point. Defaults to the
        /// active address.
        target_address: Option<IotaAddress>,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Register a new node subdomain, which will create an NFT for management
    RegisterNode {
        /// The full name of the subdomain. Ex. my-subdomain.my-domain.iota
        domain: Domain,
        /// Expiration timestamp in one of the following formats:
        ///  - YYYY-MM-DD HH:MM:SS +0000 (Ex. 2015-02-18 23:16:09 -0500)
        ///  - YYYY-MM-DD HH:MM:SS.MMM +0000 (Ex. 2015-02-18 23:16:09.123 -0500)
        ///  - unix timestamp (Ex. 1424297769000)
        ///
        /// Defaults to the parent's expiration
        #[arg(long, short = 'e', verbatim_doc_comment)]
        expiration_timestamp: Option<Timestamp>,
        /// Whether to allow further subdomain creation.
        #[arg(long, short = 'c')]
        allow_creation: bool,
        /// Whether to allow expiration time extension.
        #[arg(long, short = 't')]
        allow_time_extension: bool,
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Update the metadata flags for a subdomain
    UpdateMetadata {
        /// The full name of the subdomain. Ex. my-subdomain.my-domain.iota
        domain: Domain,
        /// Whether to allow further subdomain creation.
        #[arg(long, short = 'c')]
        allow_creation: std::primitive::bool, // https://github.com/clap-rs/clap/issues/4626
        /// Whether to allow expiration time extension.
        #[arg(long, short = 't')]
        allow_time_extension: std::primitive::bool, // https://github.com/clap-rs/clap/issues/4626
        // Whether to print detailed output.
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Extend the expiration of a subdomain
    ExtendExpiration {
        /// The full name of the subdomain. Ex. my-subdomain.my-domain.iota
        domain: Domain,
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
        opts: OptsWithGas,
    },
}

impl SubdomainCommand {
    pub async fn execute(self, context: &mut WalletContext) -> anyhow::Result<NameCommandResult> {
        let iota_client = context.get_client().await?;

        Ok(match self {
            Self::RegisterLeaf {
                domain,
                target_address,
                verbose,
                opts,
            } => {
                let Some(parent) = domain.parent() else {
                    bail!("invalid subdomain: {domain}");
                };

                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let parent = get_proxy_nft_by_name(&parent, context).await?;
                anyhow::ensure!(!parent.has_expired(), "parent NFT has expired");
                let package_id = parent.package_id(&iota_client).await?;
                let module_name = parent.module_name();

                let target_address = if let Some(target_address) = target_address {
                    target_address
                } else {
                    context.active_address().map_err(|_| {
                        anyhow::anyhow!("no active address or target-address specified")
                    })?
                };

                let res = IotaClientCommands::Call {
                    package: package_id,
                    module: module_name.to_owned(),
                    function: "new_leaf".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(parent.id()),
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                        IotaJsonValue::new(JsonValue::String(domain.to_string()))?,
                        IotaJsonValue::new(JsonValue::String(target_address.to_string()))?,
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::RegisterLeafSubdomain {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::RegisterNode {
                domain,
                expiration_timestamp,
                allow_creation,
                allow_time_extension,
                verbose,
                mut opts,
            } => {
                let Some(parent) = domain.parent() else {
                    bail!("invalid subdomain: {domain}");
                };

                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let parent = get_proxy_nft_by_name(&parent, context).await?;
                anyhow::ensure!(!parent.has_expired(), "parent NFT has expired");
                let package_id = parent.package_id(&iota_client).await?;
                let module_name = parent.module_name();

                let expiration_timestamp =
                    expiration_timestamp.unwrap_or(Timestamp(parent.expiration_timestamp_ms()));
                anyhow::ensure!(
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
                        '{domain}' {expiration_timestamp} {allow_creation} {allow_time_extension}",
                        iota_names_config.object_id
                    ),
                    "--assign nft".to_owned(),
                    "--transfer-objects [nft] sender".to_owned(),
                ];
                let display = std::mem::take(&mut opts.rest.display);
                args.extend(opts.into_args());
                let res = IotaClientCommands::PTB(PTB { args, display })
                    .execute(context)
                    .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::RegisterNodeSubdomain {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<SubdomainRegistration>(&domain, context)
                            .await?,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::UpdateMetadata {
                domain,
                allow_creation,
                allow_time_extension,
                verbose,
                opts,
            } => {
                let Some(parent) = domain.parent() else {
                    bail!("invalid subdomain: {domain}");
                };
                let iota_names_config = get_iota_names_config(&iota_client).await?;

                let parent = get_proxy_nft_by_name(&parent, context).await?;
                let package_id = parent.package_id(&iota_client).await?;
                let module_name = parent.module_name();

                let res = IotaClientCommands::Call {
                    package: package_id,
                    module: module_name.to_owned(),
                    function: "edit_setup".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(parent.id()),
                        IotaJsonValue::from_object_id(IOTA_CLOCK_OBJECT_ID),
                        IotaJsonValue::new(JsonValue::String(domain.to_string()))?,
                        IotaJsonValue::new(JsonValue::Bool(allow_creation))?,
                        IotaJsonValue::new(JsonValue::Bool(allow_time_extension))?,
                    ],
                    gas_price: None,
                    opts: opts.clone(),
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::UpdateMetadata {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        digest: res.digest,
                    })
                })
                .await?
            }
            Self::ExtendExpiration {
                domain,
                expiration_timestamp,
                verbose,
                opts,
            } => {
                let nft = get_owned_nft_by_name::<SubdomainRegistration>(&domain, context).await?;
                anyhow::ensure!(
                    expiration_timestamp.as_system_time() > nft.expiration_time(),
                    "new expiration time is not after old expiration: {}",
                    chrono::DateTime::<chrono::Utc>::from(nft.expiration_time())
                );
                let iota_names_config = get_iota_names_config(&iota_client).await?;
                let subdomains_package = fetch_package_id_by_module_and_name(
                    &iota_client,
                    &Identifier::from_str("subdomains")?,
                    &Identifier::from_str("SubdomainsAuth")?,
                )
                .await?;

                let res = IotaClientCommands::Call {
                    package: subdomains_package,
                    module: "subdomains".to_owned(),
                    function: "extend_expiration".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(iota_names_config.object_id),
                        IotaJsonValue::from_object_id(nft.id()),
                        IotaJsonValue::new(JsonValue::Number(expiration_timestamp.0.into()))?,
                    ],
                    gas_price: None,
                    opts: opts.clone(),
                }
                .execute(context)
                .await?;

                handle_transaction_result(res, verbose, async |res| {
                    Ok(NameCommandResult::ExtendExpiration {
                        record: get_registry_entry(&domain, &iota_client).await?.name_record,
                        nft: get_owned_nft_by_name::<SubdomainRegistration>(&domain, context)
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
    AuctionBid {
        auction: Auction,
        digest: TransactionDigest,
    },
    AuctionClaim {
        record: NameRecord,
        nft: IotaNamesRegistration,
        digest: TransactionDigest,
    },
    AuctionMetadata(Auction),
    AuctionStart {
        auction: Auction,
        digest: TransactionDigest,
    },
    Availability {
        domain: String,
        price: Option<u64>,
    },
    Burn {
        burned: IotaNamesRegistration,
        digest: TransactionDigest,
    },
    CommandResult(IotaClientCommandResult),
    ExtendExpiration {
        record: NameRecord,
        nft: SubdomainRegistration,
        digest: TransactionDigest,
    },
    List(Vec<IotaNamesRegistration>),
    Lookup {
        domain: Domain,
        target_address: Option<IotaAddress>,
    },
    Register {
        record: NameRecord,
        nft: IotaNamesRegistration,
        digest: TransactionDigest,
    },
    RegisterLeafSubdomain {
        record: NameRecord,
        digest: TransactionDigest,
    },
    RegisterNodeSubdomain {
        record: NameRecord,
        nft: SubdomainRegistration,
        digest: TransactionDigest,
    },
    Renew {
        record: NameRecord,
        nft: IotaNamesRegistration,
        digest: TransactionDigest,
    },
    ReverseLookup {
        address: IotaAddress,
        domain: Option<Domain>,
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
        domain: Domain,
        to: IotaAddress,
        digest: TransactionDigest,
    },
    UnsetReverseLookup {
        address: IotaAddress,
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
            Self::AuctionBid {
                auction,
                digest: transaction,
            } => {
                writeln!(f, "Successfully placed bid for {}", auction.domain)?;
                writeln!(f, "Auction status: {}", auction.status())?;
                format_auction(f, auction)?;
                writeln!(f, "\nNFT:")?;
                format_nft(f, &auction.nft)?;
                write!(f, "Transaction digest: {transaction}")
            }
            Self::AuctionClaim {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Successfully claimed {}", nft.domain_name())?;
                format_name_record(f, record)?;
                writeln!(f, "\nCreated NFT:")?;
                format_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::AuctionMetadata(auction) => {
                writeln!(f, "Auction status: {}", auction.status())?;
                format_auction(f, auction)
            }
            Self::AuctionStart {
                auction,
                digest: transaction,
            } => {
                writeln!(f, "Successfully started auction for {}", auction.domain)?;
                writeln!(f, "Auction status: {}", auction.status())?;
                format_auction(f, auction)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::Availability { domain, price } => match price {
                Some(price) => {
                    write!(f, "\"{domain}\" is available for {price} NANOs")
                }
                None => {
                    write!(f, "\"{domain}\" is not available")
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
                format_subdomain_nft(f, nft)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::List(nfts) => {
                let mut table_builder = TableBuilder::default();

                table_builder.set_header(["id", "domain", "expiration"]);

                for nft in nfts {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();

                    table_builder.push_record([
                        nft.id().to_string(),
                        nft.domain_name().to_owned(),
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
                domain,
                target_address,
            } => {
                if let Some(target_address) = target_address {
                    write!(f, "{target_address}")
                } else {
                    write!(f, "no target address found for '{domain}'")
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
            Self::RegisterLeafSubdomain {
                record,
                digest: transaction,
            } => {
                writeln!(f, "Registered record:")?;
                format_name_record(f, record)?;
                write!(f, "\nTransaction digest: {transaction}")
            }
            Self::RegisterNodeSubdomain {
                record,
                nft,
                digest: transaction,
            } => {
                writeln!(f, "Registered record:")?;
                format_name_record(f, record)?;
                writeln!(f, "\nCreated NFT:")?;
                format_subdomain_nft(f, nft)?;
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
            Self::ReverseLookup { address, domain } => {
                if let Some(domain) = domain {
                    write!(f, "{domain}")
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
                writeln!(f, "Successfully set target address for {}", entry.domain)?;
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
                domain,
                to,
                digest: transaction,
            } => {
                writeln!(f, "Successfully transferred {domain} to {to}")?;
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

fn format_auction(f: &mut std::fmt::Formatter, auction: &Auction) -> std::fmt::Result {
    let start_datetime = DateTime::<Utc>::from(auction.start_timestamp())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();
    let end_datetime = DateTime::<Utc>::from(auction.end_timestamp())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    let data = [
        ("Start", start_datetime),
        ("End", end_datetime),
        (
            "Current Bid",
            auction.current_bid.balance.value().to_string(),
        ),
        ("Current Bidder", auction.current_bidder.to_string()),
    ];
    let mut table_builder = Table::builder(data);
    table_builder.set_header(["field", "value"]);
    let mut table = table_builder.build();
    table.with(tabled::settings::Style::rounded());
    write!(f, "{table}")
}

fn format_registry_entry(f: &mut std::fmt::Formatter, entry: &RegistryEntry) -> std::fmt::Result {
    let data = [
        ("ID", entry.id.to_string()),
        ("Domain", entry.domain.to_string()),
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
        ("Domain", entry.domain.to_string()),
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

fn format_nft(f: &mut std::fmt::Formatter, nft: &IotaNamesRegistration) -> std::fmt::Result {
    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    let data = [
        ("ID", nft.id().to_string()),
        ("Domain", nft.domain_name().to_owned()),
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

fn format_subdomain_nft(
    f: &mut std::fmt::Formatter,
    nft: &SubdomainRegistration,
) -> std::fmt::Result {
    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
        .format("%Y-%m-%d %H:%M:%S.%f UTC")
        .to_string();

    let data = [
        ("ID", nft.id().to_string()),
        ("Domain", nft.domain_name().to_owned()),
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
    address: Option<IotaAddress>,
    context: &mut WalletContext,
) -> anyhow::Result<Vec<T>> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let address = get_identity_address(address.map(KeyIdentity::Address), context)?;
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
    domain: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<T> {
    let domain = domain.to_string();

    for nft in get_owned_nfts::<T>(None, context).await? {
        if nft.domain_name() == domain {
            return Ok(nft);
        }
    }

    Err(anyhow::anyhow!(
        "no matching owned {} found for {domain}",
        T::TYPE_NAME
    ))
}

async fn get_proxy_nft_by_name(
    domain: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<IotaNamesNftProxy> {
    Ok(if domain.is_sld() {
        IotaNamesNftProxy::Domain(get_owned_nft_by_name(domain, context).await?)
    } else {
        IotaNamesNftProxy::Subdomain(get_owned_nft_by_name(domain, context).await?)
    })
}

async fn get_registry_entry(domain: &Domain, client: &IotaClient) -> anyhow::Result<RegistryEntry> {
    let iota_names_config = get_iota_names_config(client).await?;
    let object_id = iota_names_config.record_field_id(domain);

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

    let entry = get_object_from_bcs::<PricingConfigEntry>(client, object_id).await?;

    Ok(entry.pricing_config)
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

    let entry = get_object_from_bcs::<RenewalConfigEntry>(&client, object_id).await?;

    Ok(entry.renewal_config)
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
            NameCommandResult::CommandResult(res)
        },
    )
}

pub enum IotaNamesNftProxy {
    Domain(IotaNamesRegistration),
    Subdomain(SubdomainRegistration),
}

macro_rules! def_enum_fns {
    ($($vis:vis fn $fn:ident(&self)$( -> $ret:ty)?;)+) => {
        $($vis fn $fn(&self)$( -> $ret)? {
            match self {
                IotaNamesNftProxy::Domain(nft) => nft.$fn(),
                IotaNamesNftProxy::Subdomain(nft) => nft.$fn(),
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

    async fn package_id(&self, client: &IotaClient) -> anyhow::Result<ObjectID> {
        Ok(match self {
            IotaNamesNftProxy::Domain(_) => {
                fetch_package_id_by_module_and_name(
                    client,
                    &Identifier::from_str("subdomains")?,
                    &Identifier::from_str("SubdomainsAuth")?,
                )
                .await?
            }
            IotaNamesNftProxy::Subdomain(_) => {
                fetch_package_id_by_module_and_name(
                    client,
                    &Identifier::from_str("subdomain_proxy")?,
                    &Identifier::from_str("SubdomainProxyAuth")?,
                )
                .await?
            }
        })
    }

    fn module_name(&self) -> &'static str {
        match self {
            IotaNamesNftProxy::Domain(_) => "subdomains",
            IotaNamesNftProxy::Subdomain(_) => "subdomain_proxy",
        }
    }
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct PricingConfigEntry {
    id: ObjectID,
    key: ConfigKey,
    pricing_config: PricingConfig,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct RenewalConfigEntry {
    id: ObjectID,
    key: ConfigKey,
    renewal_config: RenewalConfig,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct ConfigKey {
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
        for Entry::<Range, u64> { key, value } in &self.pricing.contents {
            if key.contains(label.chars().count() as u64) {
                return Ok(*value);
            }
        }
        bail!("no pricing config for label length")
    }
}

async fn select_coin_for_payment(
    domain_name: &str,
    coin: Option<ObjectID>,
    price: u64,
    context: &mut WalletContext,
) -> anyhow::Result<ObjectID> {
    Ok(match coin {
        Some(coin) => coin,
        None => {
            let gas_result = IotaClientCommands::Gas { address: None }
                .execute(context)
                .await?;
            let mut balance = 0;
            match gas_result {
                IotaClientCommandResult::Gas(coins) => {
                    for coin in coins {
                        if coin.value() >= price {
                            return Ok(*coin.id());
                        }
                        balance += coin.value();
                    }
                }
                _ => unreachable!(),
            }
            if balance > price {
                bail!("merge coins first to register/renew the domain '{domain_name}'");
            } else {
                bail!(
                    "insufficient balance {balance}/{price} to register/renew the domain '{domain_name}'"
                );
            }
        }
    })
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Auction {
    pub domain: Domain,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: u64,
    pub current_bidder: IotaAddress,
    pub current_bid: Coin,
    pub nft: IotaNamesRegistration,
}

impl Auction {
    fn start_timestamp(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.start_timestamp_ms)
    }

    fn end_timestamp(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.end_timestamp_ms)
    }

    fn is_active(&self) -> bool {
        SystemTime::now() <= self.end_timestamp()
    }

    fn status(&self) -> &str {
        if self.is_active() {
            "Active"
        } else {
            "Finished"
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuctionEntry {
    pub id: ObjectID,
    pub domain: Domain,
    pub node: LinkedTableNode<Domain, Auction>,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct AuctionHouse {
    id: ObjectID,
    balance: Balance,
    auctions: LinkedTable<Domain>,
}

impl AuctionHouse {
    async fn get_auction(&self, domain: &Domain, client: &IotaClient) -> anyhow::Result<Auction> {
        let iota_names_config = get_iota_names_config(client).await?;
        let domain_type_tag = Domain::type_(iota_names_config.package_address);
        let domain_bytes = bcs::to_bytes(domain).unwrap();

        let object_id = iota_types::dynamic_field::derive_dynamic_field_id(
            self.auctions.id,
            &TypeTag::Struct(Box::new(domain_type_tag)),
            &domain_bytes,
        )?;

        let auction_entry = get_object_from_bcs::<AuctionEntry>(client, object_id).await?;

        Ok(auction_entry.node.value)
    }
}

async fn get_auction_house(
    iota_client: &IotaClient,
    graphql_client: &SimpleClient,
) -> anyhow::Result<AuctionHouse> {
    let auction_package_address = get_auction_package_address(iota_client).await?;
    let auction_house_id = get_auction_house_id(auction_package_address, graphql_client).await?;
    get_object_from_bcs::<AuctionHouse>(iota_client, auction_house_id).await
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
    Err(anyhow::anyhow!(
        "failed to find package ID for {module_name}::{struct_name}"
    ))?
}

async fn get_auction_package_address(client: &IotaClient) -> anyhow::Result<ObjectID> {
    let auction_package_address = fetch_package_id_by_module_and_name(
        client,
        &Identifier::from_str("auction")?,
        &Identifier::from_str("AuctionAuth")?,
    )
    .await?;
    Ok(auction_package_address)
}

async fn get_auction_house_id(
    auction_package_id: ObjectID,
    client: &SimpleClient,
) -> anyhow::Result<ObjectID> {
    let variable = GraphqlQueryVariable {
        name: "type".to_string(),
        ty: "String".to_string(),
        value: serde_json::Value::String(format!("{auction_package_id}::auction::AuctionHouse")),
    };
    let query = r#"{
        objects(filter: {type: $type}) {
            edges {
                node {
                    address
                    asMoveObject {
                        contents {
                            json
                        }
                    }
                }
            }
        }
    }"#;
    let response = client
        .execute_to_graphql(query.to_string(), true, vec![variable], vec![])
        .await?;
    anyhow::ensure!(response.errors().is_empty(), "{:?}", response.errors());

    let response_body = response.response_body_json();
    let object_id_str = response_body["data"]["objects"]["edges"][0]["node"]["address"]
        .as_str()
        .ok_or(anyhow::anyhow!("missing AuctionHouse object"))?;
    let object_id = ObjectID::from_str(object_id_str)?;
    Ok(object_id)
}

async fn get_object_from_bcs<T: DeserializeOwned>(
    client: &IotaClient,
    object_id: ObjectID,
) -> anyhow::Result<T> {
    let object_response = client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await?;
    anyhow::ensure!(
        object_response.error.is_none(),
        "{:?}",
        object_response.error
    );

    deserialize_move_object_from_bcs::<T>(object_response)
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
