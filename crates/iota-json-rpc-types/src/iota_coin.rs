// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{ObjectDigest, ObjectID, ObjectRef, SequenceNumber, TransactionDigest},
    coin::CoinMetadata,
    error::IotaError,
    iota_serde::{BigInt, SequenceNumber as AsSequenceNumber},
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::Page;

pub type CoinPage = Page<Coin, ObjectID>;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub coin_type: String,
    pub coin_object_count: usize,
    #[schemars(with = "BigInt<u128>")]
    #[serde_as(as = "BigInt<u128>")]
    pub total_balance: u128,
}

impl Balance {
    pub fn zero(coin_type: String) -> Self {
        Self {
            coin_type,
            coin_object_count: 0,
            total_balance: 0,
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Coin {
    pub coin_type: String,
    pub coin_object_id: ObjectID,
    #[schemars(with = "AsSequenceNumber")]
    #[serde_as(as = "AsSequenceNumber")]
    pub version: SequenceNumber,
    pub digest: ObjectDigest,
    #[schemars(with = "BigInt<u64>")]
    #[serde_as(as = "BigInt<u64>")]
    pub balance: u64,
    pub previous_transaction: TransactionDigest,
}

impl Coin {
    pub fn object_ref(&self) -> ObjectRef {
        (self.coin_object_id, self.version, self.digest)
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IotaCoinMetadata {
    /// Number of decimal places the coin uses.
    pub decimals: u8,
    /// Name for the token
    pub name: String,
    /// Symbol for the token
    pub symbol: String,
    /// Description of the token
    pub description: String,
    /// URL for the token logo
    pub icon_url: Option<String>,
    /// Object id for the CoinMetadata object
    pub id: Option<ObjectID>,
}

impl TryFrom<Object> for IotaCoinMetadata {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        let metadata: CoinMetadata = object.try_into()?;
        let CoinMetadata {
            decimals,
            name,
            symbol,
            description,
            icon_url,
            id,
        } = metadata;
        Ok(Self {
            id: Some(*id.object_id()),
            decimals,
            name,
            symbol,
            description,
            icon_url,
        })
    }
}

/// Provides a summary of the circulating IOTA supply.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IotaCirculatingSupply {
    /// Circulating supply in NANOS at the given timestamp.
    pub value: u64,
    /// Percentage of total supply that is currently circulating (range: 0.0 to
    /// 1.0).
    pub circulating_supply_percentage: f64,
    /// Timestamp (UTC) when the circulating supply was calculated.
    pub at_checkpoint: CheckpointSequenceNumber,
}
