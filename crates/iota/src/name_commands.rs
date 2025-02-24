// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use clap::Parser;
use iota_json::IotaJsonValue;
use iota_json_rpc_types::{
    IotaData, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery,
};
use iota_sdk::wallet_context::WalletContext;
use iota_types::base_types::{IotaAddress, ObjectID};
use move_core_types::language_storage::StructTag;
use serde::Deserialize;

use crate::{
    client_commands::{IotaClientCommands, OptsWithGas},
    key_identity::get_identity_address,
};

const IOTA_NAMES_PACKAGE: &str = "0x0";
const IOTA_NAMES_OBJECT_ID: &str = "0x0";
const UTILS_PACKAGE: &str = "0x0";
const CLOCK_OBJECT_ID: &str = "0x6";

const MIN_SEGMENT_LEN: usize = 3;
const MAX_SEGMENT_LEN: usize = 63;

#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum NameCommand {
    /// Set the target address for a domain
    SetTargetAddress {
        /// The full name of the domain. Ex. my-domain.iota
        domain: Domain,
        /// The address to which the domain will point
        new_address: Option<IotaAddress>,
        #[command(flatten)]
        opts: OptsWithGas,
    },
}

impl NameCommand {
    pub async fn execute(self, context: &mut WalletContext) -> Result<(), anyhow::Error> {
        match self {
            Self::SetTargetAddress {
                domain,
                new_address,
                opts,
            } => {
                let nft = get_owned_nft_by_name(&domain, context).await?.0;
                IotaClientCommands::Call {
                    package: ObjectID::from_str(UTILS_PACKAGE).unwrap(),
                    module: "direct_setup".to_owned(),
                    function: "set_target_address".to_owned(),
                    type_args: Default::default(),
                    args: vec![
                        IotaJsonValue::from_object_id(
                            ObjectID::from_str(IOTA_NAMES_OBJECT_ID).unwrap(),
                        ),
                        IotaJsonValue::from_object_id(nft),
                        IotaJsonValue::new(serde_json::to_value(new_address)?)?,
                        IotaJsonValue::from_object_id(ObjectID::from_str(CLOCK_OBJECT_ID).unwrap()),
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

async fn get_owned_nft_by_name(
    name: &Domain,
    context: &mut WalletContext,
) -> anyhow::Result<(ObjectID, IotaNamesRegistration)> {
    let name = name.to_string();
    let client = context.get_client().await?;
    let address = get_identity_address(None, context)?;
    let nft_type = StructTag::from_str(&format!(
        "{IOTA_NAMES_PACKAGE}::iota_names_registration::IotaNamesRegistration"
    ))?;
    let mut cursor = None;
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
            if nft.domain_name == name {
                return Ok((data.object_id, nft));
            }
        }

        if response.has_next_page {
            cursor = response.next_cursor;
        } else {
            break;
        }
    }
    Err(anyhow::anyhow!(
        "no matching owned IotaNamesRegistration found for {name}"
    ))
}

#[derive(Deserialize)]
#[expect(unused)]
struct IotaNamesRegistration {
    pub domain: Domain,
    pub domain_name: String,
    pub expiration_timestamp_ms: u64,
    pub image_url: String,
}

#[derive(Deserialize, Clone)]
pub struct Domain {
    // Segments of the domain name, in reverse order
    labels: Vec<String>,
}

impl Domain {
    #[expect(unused)]
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
