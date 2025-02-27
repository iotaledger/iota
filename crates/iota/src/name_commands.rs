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
};
use move_core_types::language_storage::StructTag;
use serde::Deserialize;
use tabled::{
    builder::Builder as TableBuilder,
    settings::{Style as TableStyle, style::HorizontalLine},
};

use crate::{
    client_commands::{IotaClientCommands, OptsWithGas},
    key_identity::{KeyIdentity, get_identity_address},
};

// Devnet values
const IOTA_NAMES_PACKAGE: &str =
    "0x20c890da38609db67e2713e6b33b4e4d3c6a8e9f620f9bb48f918d2337e31503";
const IOTA_NAMES_OBJECT_ID: &str =
    "0x55716ea4b9b7563537a1ef2705f1b06060b35f15f2ea00a20de29c547c319bef";
const UTILS_PACKAGE: &str = "0xdea9e554fbee54e8dd0ac1d036d46047b5621b8f8739aa155258d656303af8cf";
const IOTA_FRAMEWORK: &str = "0x2";
const CLOCK_OBJECT_ID: &str = "0x6";

const MIN_SEGMENT_LEN: usize = 3;
const MAX_SEGMENT_LEN: usize = 63;

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum NameCommand {
    /// Burn an expired IOTA-Names NFT
    Burn {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// List the names owned by the given address, or the active address.
    List { address: Option<IotaAddress> },
    /// Set the target address for a domain
    SetTargetAddress {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will point
        new_address: Option<IotaAddress>,
        #[command(flatten)]
        opts: OptsWithGas,
    },
    /// Transfer a registered name to another address via the owned NFT
    Transfer {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will be transferred
        address: IotaAddress,
        #[command(flatten)]
        opts: OptsWithGas,
    },
}

impl NameCommand {
    pub async fn execute(self, context: &mut WalletContext) -> Result<(), anyhow::Error> {
        match self {
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
                IotaClientCommands::Call {
                    package: ObjectID::from_str(UTILS_PACKAGE)?,
                    module: "direct_setup".to_owned(),
                    function: burn_function.to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(ObjectID::from_str(IOTA_NAMES_OBJECT_ID)?),
                        IotaJsonValue::from_object_id(nft.id),
                        IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?
                .print(true);
            }
            Self::List { address } => {
                let nfts = get_owned_nfts(address, context).await?;
                let mut table_builder = TableBuilder::default();

                table_builder.set_header(["id", "domain", "expiration", "image URL"]);

                for nft in nfts {
                    let expiration_datetime = DateTime::<Utc>::from(nft.expiration_time())
                        .format("%Y-%m-%d %H:%M:%S.%f UTC")
                        .to_string();

                    table_builder.push_record([
                        nft.id.to_string(),
                        nft.domain_name,
                        format!("{} ({expiration_datetime})", nft.expiration_timestamp_ms),
                        nft.image_url,
                    ]);
                }

                let mut table = table_builder.build();
                table.with(
                    tabled::settings::Style::rounded().horizontals([HorizontalLine::new(
                        1,
                        TableStyle::modern().get_horizontal(),
                    )]),
                );
                println!("{table}")
            }
            Self::SetTargetAddress {
                domain,
                new_address,
                opts,
            } => {
                let nft = get_owned_nft_by_name(&domain, context).await?.id;

                IotaClientCommands::Call {
                    package: ObjectID::from_str(UTILS_PACKAGE)?,
                    module: "direct_setup".to_owned(),
                    function: "set_target_address".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(ObjectID::from_str(IOTA_NAMES_OBJECT_ID)?),
                        IotaJsonValue::from_object_id(nft),
                        IotaJsonValue::new(serde_json::to_value(new_address)?)?,
                        IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID)?),
                    ],
                    gas_price: None,
                    opts,
                }
                .execute(context)
                .await?
                .print(true);
            }
            Self::Transfer {
                domain,
                address,
                opts,
            } => {
                let nft = get_owned_nft_by_name(&domain, context).await?.id;

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
                .await?
                .print(true);
            }
        }
        Ok(())
    }
}

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
    name: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<IotaNamesRegistration> {
    let name = name.to_string();

    for nft in get_owned_nfts(None, context).await? {
        if nft.domain_name == name {
            return Ok(nft);
        }
    }

    Err(anyhow::anyhow!(
        "no matching owned IotaNamesRegistration found for {name}"
    ))
}

#[derive(Debug, Deserialize)]
struct IotaNamesRegistration {
    pub id: ObjectID,
    pub domain: Domain,
    pub domain_name: String,
    pub expiration_timestamp_ms: u64,
    pub image_url: String,
}

impl IotaNamesRegistration {
    pub fn expiration_time(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.expiration_timestamp_ms)
    }

    pub fn has_expired(&self) -> bool {
        self.expiration_time() <= SystemTime::now()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Domain {
    // Segments of the domain name, in reverse order
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
        anyhow::ensure!(segments.len() >= 2, "domain has too few segments");
        let tld = segments.pop().unwrap();
        anyhow::ensure!(VALID_TLDS.contains(&tld), "invalid TLD: {tld}");
        let mut labels = vec![tld.to_owned()];
        for segment in segments.into_iter().rev() {
            labels.push(parse_domain_segment(segment)?);
        }
        Ok(Self { labels })
    }
}

fn parse_domain_segment(segment: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        segment.len() >= MIN_SEGMENT_LEN && segment.len() <= MAX_SEGMENT_LEN,
        "segment length outside allowed range [{MIN_SEGMENT_LEN}..{MAX_SEGMENT_LEN}]: {}",
        segment.len()
    );
    let regex = regex::Regex::new("^[a-zA-Z0-9_-]+$").unwrap();
    anyhow::ensure!(
        regex.is_match(segment),
        "invalid characters in domain: {segment}"
    );
    Ok(segment.to_owned())
}
