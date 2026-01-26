// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Protocol parameters.

use alloc::string::String;
use core::borrow::Borrow;

use packable::{Packable, prefix::StringPrefix};

use super::Error;

/// The current protocol version.
pub const PROTOCOL_VERSION: u8 = 2;

/// Protocol parameters for unpacking from Hornet snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Packable)]
#[packable(unpack_error = Error)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ProtocolParameters {
    /// The version of the protocol running.
    #[cfg_attr(feature = "serde", serde(rename = "version"))]
    protocol_version: u8,
    /// The human friendly name of the network.
    #[packable(unpack_error_with = |err| Error::InvalidNetworkName(err.into_item_err()))]
    network_name: StringPrefix<u8>,
    /// The HRP prefix used for Bech32 addresses in the network.
    #[packable(unpack_error_with = |err| Error::InvalidBech32Hrp(format!("{:?}", err.into_item_err())))]
    bech32_hrp: StringPrefix<u8>,
    /// The minimum pow score of the network.
    min_pow_score: u32,
    /// The below max depth parameter of the network.
    below_max_depth: u8,
    /// The byte cost for rent calculation.
    rent_byte_cost: u32,
    /// The byte factor for data fields in rent calculation.
    rent_byte_factor_data: u8,
    /// The byte factor for key fields in rent calculation.
    rent_byte_factor_key: u8,
    /// TokenSupply defines the current token supply on the network.
    token_supply: u64,
}

// This implementation is required to make [`ProtocolParameters`] a [`Packable`]
// visitor.
impl Borrow<()> for ProtocolParameters {
    fn borrow(&self) -> &() {
        &()
    }
}

impl Default for ProtocolParameters {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            network_name: StringPrefix::try_from(String::from("shimmer"))
                .expect("network name should be valid"),
            bech32_hrp: StringPrefix::try_from(String::from("smr"))
                .expect("bech32 hrp should be valid"),
            min_pow_score: 1500,
            below_max_depth: 15,
            rent_byte_cost: 100,
            rent_byte_factor_data: 1,
            rent_byte_factor_key: 10,
            token_supply: 1_813_620_509_061_365,
        }
    }
}

impl ProtocolParameters {
    /// Returns the protocol version of the [`ProtocolParameters`].
    pub fn protocol_version(&self) -> u8 {
        self.protocol_version
    }

    /// Returns the network name of the [`ProtocolParameters`].
    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    /// Returns the bech32 HRP of the [`ProtocolParameters`].
    pub fn bech32_hrp(&self) -> &str {
        &self.bech32_hrp
    }

    /// Returns the minimum PoW score of the [`ProtocolParameters`].
    pub fn min_pow_score(&self) -> u32 {
        self.min_pow_score
    }

    /// Returns the below max depth of the [`ProtocolParameters`].
    pub fn below_max_depth(&self) -> u8 {
        self.below_max_depth
    }

    /// Returns the rent byte cost of the [`ProtocolParameters`].
    pub fn rent_byte_cost(&self) -> u32 {
        self.rent_byte_cost
    }

    /// Returns the rent byte factor for data fields.
    pub fn rent_byte_factor_data(&self) -> u8 {
        self.rent_byte_factor_data
    }

    /// Returns the rent byte factor for key fields.
    pub fn rent_byte_factor_key(&self) -> u8 {
        self.rent_byte_factor_key
    }

    /// Returns the token supply of the [`ProtocolParameters`].
    pub fn token_supply(&self) -> u64 {
        self.token_supply
    }
}
