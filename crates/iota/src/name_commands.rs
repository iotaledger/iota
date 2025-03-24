// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use chrono::{Utc, prelude::DateTime};
use clap::Parser;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
};
use iota_names::{
    IotaNamesRegistration,
    config::{CLOCK_OBJECT_ID, IOTA_FRAMEWORK, IotaNamesConfig},
    domain::Domain,
    registry::{RegistryEntry, ReverseRegistryEntry},
};
use iota_protocol_config::Chain;
use iota_sdk::{IotaClient, wallet_context::WalletContext};
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    collection_types::{Entry, VecMap},
    digests::ChainIdentifier,
    dynamic_field::DynamicFieldName,
};
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};
use tabled::{
    builder::Builder as TableBuilder,
    settings::{Style as TableStyle, style::HorizontalLine},
};

use crate::{
    PrintableResult,
    client_commands::{IotaClientCommandResult, IotaClientCommands, OptsWithGas},
    client_ptb::ptb::PTB,
    key_identity::{KeyIdentity, get_identity_address},
};

/// Tool to register and manage domains and subdomains
#[derive(Parser)]
pub enum NameCommand {
    /// Burn an expired IOTA-Names NFT
    Burn {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
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
    /// Register a domain
    Register {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The coin to use for payment. If not provided, selects the first coin
        /// with enough balance.
        coin: Option<ObjectID>,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Lookup the address of a domain
    Lookup { domain: Domain },
    // Lookup a domain by its address if reverse lookup was set
    ReverseLookup {
        /// The address for which to look up its domain. Defaults to the active
        /// address.
        address: Option<IotaAddress>,
    },
    /// Set the reverse lookup of the domain to the transaction sender address
    SetReverseLookup {
        /// Domain for which to set the reverse lookup
        domain: Domain,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Set the target address for a domain
    SetTargetAddress {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will point
        new_address: Option<IotaAddress>,
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
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Transfer a registered domain to another address via the owned NFT
    Transfer {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will be transferred
        address: IotaAddress,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Unset reverse lookup
    UnsetReverseLookup {
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Unset keyed user data
    UnsetUserData {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The key representing the data in the table
        key: String,
        #[command(flatten)]
        opts: OptsWithGas,
    },
}

impl NameCommand {
    pub async fn execute(
        self,
        context: &mut WalletContext,
    ) -> Result<NameCommandResult, anyhow::Error> {
        Ok(match self {
            Self::Burn { domain, opts } => {
                let nft = get_owned_nft_by_name(&domain, context).await?;

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
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: iota_names_config.package_address.into(),
                        module: "controller".to_owned(),
                        function: burn_function.to_owned(),
                        type_args: Default::default(),
                        args: vec![
                            IotaJsonValue::from_object_id(iota_names_config.object_id),
                            IotaJsonValue::from_object_id(*nft.id()),
                            IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                        ],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
            Self::GetUserData { domain, key } => {
                let entry = get_registry_entry(&domain, context).await?;

                if let Some(key) = key {
                    let Some(value) = entry
                        .name_record
                        .data
                        .contents
                        .into_iter()
                        .find(|entry| entry.key == key)
                    else {
                        anyhow::bail!("no value found for key `{key}`");
                    };
                    NameCommandResult::UserData(VecMap {
                        contents: vec![value],
                    })
                } else {
                    NameCommandResult::UserData(entry.name_record.data)
                }
            }
            Self::List { address } => {
                let nfts = get_owned_nfts(address, context).await?;
                NameCommandResult::List(nfts)
            }
            Self::Register { domain, coin, opts } => {
                anyhow::ensure!(
                    domain.depth() == 2,
                    "domain to register must consist of two labels"
                );
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                let label = domain.label(1).unwrap();
                let price = fetch_pricing_config(context).await?.get_price(label)?;
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
                        "--move-call {}::payments::handle_base_payment <0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA> @{} payment_intent coins.0",
                        iota_names_config.payment_package_address, iota_names_config.object_id
                    ),
                    "--assign receipt".to_string(),
                    format!(
                        "--move-call {}::payment::register receipt @{} @0x6",
                        iota_names_config.package_address, iota_names_config.object_id
                    ),
                    "--assign nft".to_string(),
                    "--transfer-objects [nft] sender".to_string(),
                ];
                opts.append_args(&mut args);

                NameCommandResult::Client(
                    IotaClientCommands::PTB(PTB { args })
                        .execute(context)
                        .await?,
                )
            }
            Self::Lookup { domain } => {
                let entry = get_registry_entry(&domain, context).await?;
                NameCommandResult::Lookup {
                    domain,
                    address: entry.name_record.target_address,
                }
            }
            Self::ReverseLookup { address } => {
                let address = get_identity_address(address.map(KeyIdentity::Address), context)?;
                let entry = get_reverse_registry_entry(address, context).await?;
                NameCommandResult::ReverseLookup(entry.domain)
            }
            Self::SetReverseLookup { domain, opts } => {
                // Check ownership of the name off-chain to avoid potentially wasting gas
                get_owned_nft_by_name(&domain, context).await?;
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
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
                    .await?,
                )
            }
            Self::SetTargetAddress {
                domain,
                new_address,
                opts,
            } => {
                let nft_id = *get_owned_nft_by_name(&domain, context).await?.id();
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
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
                            IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                        ],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
            Self::SetUserData {
                domain,
                key,
                value,
                opts,
            } => {
                let nft = get_owned_nft_by_name(&domain, context).await?;
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: iota_names_config.package_address.into(),
                        module: "controller".to_owned(),
                        function: "set_user_data".to_owned(),
                        type_args: vec![],
                        args: vec![
                            IotaJsonValue::from_object_id(iota_names_config.object_id),
                            IotaJsonValue::from_object_id(*nft.id()),
                            IotaJsonValue::new(serde_json::Value::String(key))?,
                            IotaJsonValue::new(serde_json::Value::String(value))?,
                            IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                        ],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
            Self::Transfer {
                domain,
                address,
                opts,
            } => {
                let nft_id = *get_owned_nft_by_name(&domain, context).await?.id();
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(IOTA_FRAMEWORK)?,
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
                    .await?,
                )
            }
            Self::UnsetReverseLookup { opts } => {
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: iota_names_config.package_address.into(),
                        module: "controller".to_owned(),
                        function: "unset_reverse_lookup".to_owned(),
                        type_args: Default::default(),
                        args: vec![IotaJsonValue::from_object_id(iota_names_config.object_id)],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
            Self::UnsetUserData { domain, key, opts } => {
                let nft = get_owned_nft_by_name(&domain, context).await?;
                let client = context.get_client().await?;
                let iota_names_config = get_iota_names_config(&client).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: iota_names_config.package_address.into(),
                        module: "controller".to_owned(),
                        function: "unset_user_data".to_owned(),
                        type_args: vec![],
                        args: vec![
                            IotaJsonValue::from_object_id(iota_names_config.object_id),
                            IotaJsonValue::from_object_id(*nft.id()),
                            IotaJsonValue::new(serde_json::Value::String(key))?,
                            IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                        ],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum NameCommandResult {
    Client(IotaClientCommandResult),
    Lookup {
        domain: Domain,
        address: Option<IotaAddress>,
    },
    ReverseLookup(Domain),
    UserData(VecMap<String, String>),
    List(Vec<IotaNamesRegistration>),
}

impl std::fmt::Display for NameCommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(client_result) => client_result.fmt(f),
            Self::Lookup { domain, address } => {
                if let Some(address) = address {
                    write!(f, "{address}")
                } else {
                    write!(f, "no target address found for '{domain}'")
                }
            }
            Self::ReverseLookup(domain) => domain.fmt(f),
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
            Self::List(nfts) => {
                let mut table_builder = TableBuilder::default();

                table_builder.set_header(["id", "domain", "expiration", "image URL"]);

                for nft in nfts {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();

                    table_builder.push_record([
                        nft.id().to_string(),
                        nft.domain_name().to_owned(),
                        format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms()),
                        nft.image_url().to_owned(),
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
        }
    }
}

impl std::fmt::Debug for NameCommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = crate::unwrap_err_to_string(|| Ok(serde_json::to_string_pretty(self)?));
        write!(f, "{s}")
    }
}

impl PrintableResult for NameCommandResult {}

async fn get_owned_nfts(
    address: Option<IotaAddress>,
    context: &mut WalletContext,
) -> anyhow::Result<Vec<IotaNamesRegistration>> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let address = get_identity_address(address.map(KeyIdentity::Address), context)?;
    let nft_type = StructTag::from_str(&format!(
        "{}::iota_names_registration::IotaNamesRegistration",
        iota_names_config.package_address
    ))?;
    let mut cursor = None;
    let mut nfts = Vec::new();

    loop {
        let response = client
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
            .await?;

        for res in response.data {
            let data = res.data.expect("missing object data");
            let nft = data
                .bcs
                .expect("missing bcs")
                .try_as_move()
                .expect("invalid move type")
                .deserialize::<IotaNamesRegistration>()?;
            nfts.push(nft);
        }

        if response.has_next_page {
            cursor = response.next_cursor;
        } else {
            break;
        }
    }

    Ok(nfts)
}

async fn get_owned_nft_by_name(
    domain: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<IotaNamesRegistration> {
    let domain_name = domain.to_string();

    for nft in get_owned_nfts(None, context).await? {
        if nft.domain_name() == domain_name {
            return Ok(nft);
        }
    }

    Err(anyhow::anyhow!(
        "no matching owned IotaNamesRegistration found for {domain_name}"
    ))
}

async fn get_registry_entry(
    domain: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<RegistryEntry> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let object_id = iota_names_config.record_field_id(domain);

    client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await?
        .into_object()?
        .bcs
        .expect("missing bcs")
        .try_into_move()
        .expect("invalid move type")
        .deserialize::<RegistryEntry>()
}

async fn get_reverse_registry_entry(
    address: IotaAddress,
    context: &mut WalletContext,
) -> anyhow::Result<ReverseRegistryEntry> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let object_id = iota_names_config.reverse_record_field_id(&address);

    client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await?
        .into_object()?
        .bcs
        .expect("missing bcs")
        .try_into_move()
        .expect("invalid move type")
        .deserialize::<ReverseRegistryEntry>()
}

async fn get_iota_names_config(client: &IotaClient) -> anyhow::Result<IotaNamesConfig> {
    let chain_identifier = client.read_api().get_chain_identifier().await?;
    let chain = ChainIdentifier::from_chain_short_id(&chain_identifier)
        .map(|c| c.chain())
        .unwrap_or(Chain::Unknown);
    Ok(IotaNamesConfig::from_chain(&chain))
}

async fn fetch_pricing_config(context: &mut WalletContext) -> anyhow::Result<PricingConfig> {
    let client = context.get_client().await?;
    let iota_names_config = get_iota_names_config(&client).await?;
    let config_type = StructTag::from_str(&format!(
        "{}::iota_names::ConfigKey<{}::pricing_config::PricingConfig>",
        iota_names_config.package_address, iota_names_config.package_address
    ))?;
    let df_name = DynamicFieldName {
        type_: TypeTag::Struct(Box::new(config_type)),
        value: serde_json::json!({ "dummy_field": false }),
    };
    // TODO compute ID locally
    let object_id = client
        .read_api()
        .get_dynamic_field_object(iota_names_config.object_id, df_name)
        .await?
        .object_id()?;
    let entry = client
        .read_api()
        .get_object_with_options(object_id, IotaObjectDataOptions::new().with_bcs())
        .await?
        .into_object()?
        .bcs
        .expect("missing bcs")
        .try_into_move()
        .expect("invalid move type")
        .deserialize::<PricingConfigEntry>()?;
    Ok(entry.pricing_config)
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

impl PricingConfig {
    pub fn get_price(&self, label: &str) -> anyhow::Result<u64> {
        for Entry::<Range, u64> { key, value } in &self.pricing.contents {
            if key.contains(label.chars().count() as u64) {
                return Ok(*value);
            }
        }
        anyhow::bail!("no pricing config for label length")
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
                anyhow::bail!("merge coins first to register the domain '{domain_name}'");
            } else {
                anyhow::bail!("insufficient balance to register the domain '{domain_name}'");
            }
        }
    })
}
