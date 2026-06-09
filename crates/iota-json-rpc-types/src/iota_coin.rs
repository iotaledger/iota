// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_types::{
    balance::Supply,
    base_types::{ObjectDigest, ObjectRef, SequenceNumber, TransactionDigest},
    coin::CoinMetadata,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, DisplayFromStr, SerializeAs, serde_as};

use crate::{
    Page,
    iota_primitives::{
        Base58 as Base58Schema, ObjectId as ObjectIdSchema,
        SequenceNumberString as SequenceNumberStringSchema,
    },
};

pub type CoinPage = Page<Coin, ObjectId>;

#[serde_as]
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Supply")]
pub struct IotaSupply {
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub value: u64,
}

impl SerializeAs<Supply> for IotaSupply {
    fn serialize_as<S>(source: &Supply, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IotaSupply::from(source.clone()).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, Supply> for IotaSupply {
    fn deserialize_as<D>(deserializer: D) -> Result<Supply, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaSupply::deserialize(deserializer)?;
        Ok(Supply::from(schema))
    }
}

impl From<Supply> for IotaSupply {
    fn from(supply: Supply) -> Self {
        Self {
            value: supply.value,
        }
    }
}

impl From<IotaSupply> for Supply {
    fn from(schema: IotaSupply) -> Self {
        Self {
            value: schema.value,
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub coin_type: String,
    pub coin_object_count: usize,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
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
    #[serde_as(as = "ObjectIdSchema")]
    #[schemars(with = "ObjectIdSchema")]
    pub coin_object_id: ObjectId,
    #[serde_as(as = "SequenceNumberStringSchema")]
    #[schemars(with = "SequenceNumberStringSchema")]
    pub version: SequenceNumber,
    #[serde_as(as = "Base58Schema")]
    #[schemars(with = "Base58Schema")]
    pub digest: ObjectDigest,
    #[schemars(with = "String")]
    #[serde_as(as = "DisplayFromStr")]
    pub balance: u64,
    #[serde_as(as = "Base58Schema")]
    #[schemars(with = "Base58Schema")]
    pub previous_transaction: TransactionDigest,
}

impl Coin {
    pub fn object_ref(&self) -> ObjectRef {
        ObjectRef::new(self.coin_object_id, self.version, self.digest)
    }
}

#[serde_as]
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
    #[serde_as(as = "Option<ObjectIdSchema>")]
    #[schemars(with = "Option<ObjectIdSchema>")]
    pub id: Option<ObjectId>,
}

impl TryFrom<Object> for IotaCoinMetadata {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        let metadata: CoinMetadata = object.try_into()?;
        Ok(metadata.into())
    }
}

impl From<CoinMetadata> for IotaCoinMetadata {
    fn from(metadata: CoinMetadata) -> Self {
        let CoinMetadata {
            decimals,
            name,
            symbol,
            description,
            icon_url,
            id,
        } = metadata;
        Self {
            id: Some(*id.object_id()),
            decimals,
            name,
            symbol,
            description,
            icon_url,
        }
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
