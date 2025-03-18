// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{Utc, prelude::DateTime};
use clap::Parser;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
};
use iota_sdk::wallet_context::WalletContext;
use iota_types::{
    TypeTag,
    base_types::{IotaAddress, ObjectID},
    collection_types::VecMap,
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

// Devnet values
const IOTA_NAMES_PACKAGE: &str =
    "0x20c890da38609db67e2713e6b33b4e4d3c6a8e9f620f9bb48f918d2337e31503";
const IOTA_NAMES_OBJECT_ID: &str =
    "0x55716ea4b9b7563537a1ef2705f1b06060b35f15f2ea00a20de29c547c319bef";
const REGISTRATION_PACKAGE: &str =
    "0x160581f35fb2a58a4964d513a96c70e0b64053a254936ae12b5f4d17087436f5";
const UTILS_PACKAGE: &str = "0xdea9e554fbee54e8dd0ac1d036d46047b5621b8f8739aa155258d656303af8cf";
const IOTA_FRAMEWORK: &str = "0x2";
const CLOCK_OBJECT_ID: &str = "0x6";
const REGISTRY_TABLE_ID: &str =
    "0xd48c0a882059036ca8c21dcc8d8bcaefc923aa678f225d3d515b79e3094e5616";
const REVERSE_REGISTRY_TABLE_ID: &str =
    "0x82139fa7c076816b67e2ff0927f2b30e4d6e2874a3a108649152a7b7d9eb25ac";

const MIN_LABEL_LEN: usize = 3;
const MAX_LABEL_LEN: usize = 63;

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
        /// The number of years to register the domain. Must be within [1-5]
        /// interval
        years: u8,
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

                let burn_function = if nft.domain.parent().is_some() {
                    "burn_expired_subname"
                } else {
                    "burn_expired"
                };
                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(UTILS_PACKAGE)?,
                        module: "direct_setup".to_owned(),
                        function: burn_function.to_owned(),
                        type_args: Default::default(),
                        args: vec![
                            IotaJsonValue::from_object_id(ObjectID::from_str(
                                IOTA_NAMES_OBJECT_ID,
                            )?),
                            IotaJsonValue::from_object_id(nft.id),
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
            Self::Register {
                domain,
                years,
                coin,
                opts,
            } => {
                anyhow::ensure!(
                    domain.len() == 2,
                    "domain to register must consist of two labels"
                );

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
                        "--move-call {REGISTRATION_PACKAGE}::register::register @{IOTA_NAMES_OBJECT_ID} '{domain_name}' {years} coins.0 @{CLOCK_OBJECT_ID}"
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

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(UTILS_PACKAGE)?,
                        module: "direct_setup".to_owned(),
                        function: "set_reverse_lookup".to_owned(),
                        type_args: Default::default(),
                        args: vec![
                            IotaJsonValue::from_object_id(ObjectID::from_str(
                                IOTA_NAMES_OBJECT_ID,
                            )?),
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
                let nft = get_owned_nft_by_name(&domain, context).await?.id;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(UTILS_PACKAGE)?,
                        module: "direct_setup".to_owned(),
                        function: "set_target_address".to_owned(),
                        type_args: Default::default(),
                        args: vec![
                            IotaJsonValue::from_object_id(ObjectID::from_str(
                                IOTA_NAMES_OBJECT_ID,
                            )?),
                            IotaJsonValue::from_object_id(nft),
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

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(UTILS_PACKAGE)?,
                        module: "direct_setup".to_owned(),
                        function: "set_user_data".to_owned(),
                        type_args: vec![],
                        args: vec![
                            IotaJsonValue::from_object_id(ObjectID::from_str(
                                IOTA_NAMES_OBJECT_ID,
                            )?),
                            IotaJsonValue::from_object_id(nft.id),
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
                let nft = get_owned_nft_by_name(&domain, context).await?.id;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(IOTA_FRAMEWORK)?,
                        module: "transfer".to_owned(),
                        function: "public_transfer".to_owned(),
                        type_args: vec![TypeTag::from_str(&format!(
                            "{IOTA_NAMES_PACKAGE}::iota_names_registration::IotaNamesRegistration"
                        ))?],
                        args: vec![
                            IotaJsonValue::from_object_id(nft),
                            IotaJsonValue::new(serde_json::to_value(address)?)?,
                        ],
                        gas_price: None,
                        opts,
                    }
                    .execute(context)
                    .await?,
                )
            }
            Self::UnsetReverseLookup { opts } => NameCommandResult::Client(
                IotaClientCommands::Call {
                    package: ObjectID::from_str(UTILS_PACKAGE)?,
                    module: "direct_setup".to_owned(),
                    function: "unset_reverse_lookup".to_owned(),
                    type_args: Default::default(),
                    args: vec![IotaJsonValue::from_object_id(ObjectID::from_str(
                        IOTA_NAMES_OBJECT_ID,
                    )?)],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?,
            ),
            Self::UnsetUserData { domain, key, opts } => {
                let nft = get_owned_nft_by_name(&domain, context).await?;

                NameCommandResult::Client(
                    IotaClientCommands::Call {
                        package: ObjectID::from_str(UTILS_PACKAGE)?,
                        module: "direct_setup".to_owned(),
                        function: "unset_user_data".to_owned(),
                        type_args: vec![],
                        args: vec![
                            IotaJsonValue::from_object_id(ObjectID::from_str(
                                IOTA_NAMES_OBJECT_ID,
                            )?),
                            IotaJsonValue::from_object_id(nft.id),
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
                        nft.id.to_string(),
                        nft.domain_name.clone(),
                        format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms),
                        nft.image_url.clone(),
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
    let address = get_identity_address(address.map(KeyIdentity::Address), context)?;
    let nft_type = StructTag::from_str(&format!(
        "{IOTA_NAMES_PACKAGE}::iota_names_registration::IotaNamesRegistration"
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
        if nft.domain_name == domain_name {
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
    let object_id = client
        .read_api()
        .get_dynamic_field_object(
            ObjectID::from_str(REGISTRY_TABLE_ID)?,
            DynamicFieldName {
                type_: TypeTag::Struct(Box::new(StructTag::from_str(&format!(
                    "{IOTA_NAMES_PACKAGE}::domain::Domain"
                ))?)),
                value: serde_json::json!(domain.labels),
            },
        )
        .await?
        .object_id()
        .map_err(|_| anyhow::anyhow!("name '{domain}' not found"))?;
    // TODO: merge with above when https://github.com/iotaledger/iota/issues/5807 is implemented
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

    let object_id = client
        .read_api()
        .get_dynamic_field_object(
            ObjectID::from_str(REVERSE_REGISTRY_TABLE_ID)?,
            DynamicFieldName {
                type_: TypeTag::Address,
                value: serde_json::Value::String(address.to_string()),
            },
        )
        .await?
        .object_id()?;
    // TODO: merge with above when https://github.com/iotaledger/iota/issues/5807 is implemented
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

#[derive(Debug, Serialize, Deserialize)]
pub struct IotaNamesRegistration {
    id: ObjectID,
    domain: Domain,
    domain_name: String,
    expiration_timestamp_ms: u64,
    image_url: String,
}

impl IotaNamesRegistration {
    pub fn expiration_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.expiration_timestamp_ms)
    }

    pub fn has_expired(&self) -> bool {
        self.expiration_time() <= SystemTime::now()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    // Labels of the domain name, in reverse order
    labels: Vec<String>,
}

impl Domain {
    fn parent(&self) -> Option<Self> {
        if self.len() > 2 {
            Some(Self {
                labels: self.labels.iter().take(self.len() - 1).cloned().collect(),
            })
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.labels.len()
    }

    fn label(&self, index: usize) -> Option<&String> {
        self.labels.get(index)
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let labels = self.labels.iter().rev().cloned().collect::<Vec<_>>();
        write!(f, "{}", labels.join("."))
    }
}

impl FromStr for Domain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const VALID_TLDS: &[&str] = &["iota"];
        let mut segments = s.split('.').collect::<Vec<_>>();
        anyhow::ensure!(segments.len() >= 2, "domain has too few labels");
        let tld = segments.pop().unwrap();
        anyhow::ensure!(VALID_TLDS.contains(&tld), "invalid TLD: {tld}");
        let mut labels = vec![tld.to_owned()];
        for segment in segments.into_iter().rev() {
            labels.push(parse_domain_label(segment)?);
        }
        Ok(Self { labels })
    }
}

fn parse_domain_label(label: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        label.len() >= MIN_LABEL_LEN && label.len() <= MAX_LABEL_LEN,
        "label length outside allowed range [{MIN_LABEL_LEN}..{MAX_LABEL_LEN}]: {}",
        label.len()
    );
    let regex = regex::Regex::new("^[a-zA-Z0-9][a-zA-Z0-9-]+[a-zA-Z0-9]$").unwrap();

    anyhow::ensure!(
        regex.is_match(label),
        "invalid characters in domain: {label}"
    );
    Ok(label.to_owned())
}

async fn fetch_pricing_config(context: &mut WalletContext) -> anyhow::Result<PricingConfig> {
    let client = context.get_client().await?;
    let iota_names_object_id = ObjectID::from_str(IOTA_NAMES_OBJECT_ID)?;
    let config_type = StructTag::from_str(&format!(
        "{IOTA_NAMES_PACKAGE}::iota_names::ConfigKey<{IOTA_NAMES_PACKAGE}::config::Config>"
    ))?;
    let df_name = DynamicFieldName {
        type_: TypeTag::Struct(Box::new(config_type)),
        value: serde_json::json!({ "dummy_field": false }),
    };
    let object_id = client
        .read_api()
        .get_dynamic_field_object(iota_names_object_id, df_name)
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
struct PricingConfig {
    three_char_price: u64,
    four_char_price: u64,
    five_plus_char_price: u64,
}

impl PricingConfig {
    pub fn get_price(&self, label: &str) -> anyhow::Result<u64> {
        Ok(match label.chars().count() {
            3 => self.three_char_price,
            4 => self.four_char_price,
            5..=MAX_LABEL_LEN => self.five_plus_char_price,
            _ => anyhow::bail!("invalid label length"),
        })
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

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct RegistryEntry {
    id: ObjectID,
    domain: Domain,
    name_record: NameRecord,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct NameRecord {
    nft_id: ObjectID,
    expiration_timestamp_ms: u64,
    target_address: Option<IotaAddress>,
    data: VecMap<String, String>,
}

#[expect(unused)]
#[derive(Debug, Deserialize)]
struct ReverseRegistryEntry {
    id: ObjectID,
    address: IotaAddress,
    domain: Domain,
}
