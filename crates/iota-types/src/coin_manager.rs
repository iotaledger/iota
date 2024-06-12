// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    coin::{CoinMetadata, TreasuryCap},
    id::UID,
};

/// The purpose of a CoinManager is to allow access to all
/// properties of a Coin on-chain from within a single shared object
/// This includes access to the total supply and metadata
/// In addition a optional maximum supply can be set and a custom
/// additional Metadata field can be added.
/// Holds all related objects to a Coin in a convenient shared function.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
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

/// The immutable version of CoinMetadata, used in case of migrating from frozen
/// objects to a `CoinManager` holding the metadata.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
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
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct CoinManagerTreasuryCap {
    /// The unique identifier of the object.
    pub id: UID,
}
