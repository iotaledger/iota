// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::Identifier;
use serde::{Deserialize, Serialize};

use crate::{
    IotaAddress, StructTag,
    coin::{CoinMetadata, TreasuryCap},
    error::IotaError,
    id::UID,
    object::{Data, Object},
};

pub const COIN_MANAGER_TREASURY_CAP_STRUCT_NAME: Identifier =
    Identifier::from_static("CoinManagerTreasuryCap");

/// The purpose of a CoinManager is to allow access to all
/// properties of a Coin on-chain from within a single shared object
/// This includes access to the total supply and metadata
/// In addition a optional maximum supply can be set and a custom
/// additional Metadata field can be added.
/// Holds all related objects to a Coin in a convenient shared function.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct CoinManager {
    /// The unique identifier of the object.
    pub id: UID,
    /// The original TreasuryCap object as returned by `create_currency`
    pub treasury_cap: TreasuryCap,
    /// Metadata object, original one from the `coin` module, if available
    pub metadata: Option<CoinMetadata>,
    /// Immutable Metadata object, only to be used as a last resort if the
    /// original metadata is frozen
    pub immutable_metadata: Option<ImmutableCoinMetadata>,
    /// Optional maximum supply, if set you can't mint more as this number - can
    /// only be set once
    pub maximum_supply: Option<u64>,
    /// Flag indicating if the supply is considered immutable (TreasuryCap is
    /// exchanged for this)
    pub supply_immutable: bool,
    /// Flag indicating if the metadata is considered immutable (MetadataCap is
    /// exchanged for this)
    pub metadata_immutable: bool,
}

impl CoinManager {
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize CoinManager object: {err}"),
        })
    }
}

/// The immutable version of CoinMetadata, used in case of migrating from frozen
/// objects to a `CoinManager` holding the metadata.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ImmutableCoinMetadata {
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
}

/// Like `TreasuryCap`, but for dealing with `TreasuryCap` inside `CoinManager`
/// objects
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct CoinManagerTreasuryCap {
    /// The unique identifier of the object.
    pub id: UID,
}

impl CoinManagerTreasuryCap {
    pub fn is_coin_manager_treasury_cap(object_type: &StructTag) -> bool {
        object_type.address() == IotaAddress::FRAMEWORK
            && object_type.module() == &Identifier::COIN_MANAGER_MODULE
            && object_type.name() == &COIN_MANAGER_TREASURY_CAP_STRUCT_NAME
    }
}

impl TryFrom<Object> for CoinManager {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        TryFrom::try_from(&object)
    }
}

impl TryFrom<&Object> for CoinManager {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        if let Data::Struct(o) = &object.data {
            if o.struct_tag().is_coin_manager() {
                return CoinManager::from_bcs_bytes(o.contents());
            }
        }

        Err(IotaError::Type {
            error: format!("Object type is not a CoinManager: {object:?}"),
        })
    }
}
