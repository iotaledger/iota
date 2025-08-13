// Copyright 2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::string::String;
use core::borrow::Borrow;

use packable::{Packable, prefix::StringPrefix};

use super::address::Hrp;
use crate::types::block::{
    ConvertTo, Error, PROTOCOL_VERSION, helper::network_name_to_id, output::RentStructure,
};

/// Defines the parameters of the protocol.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Packable)]
#[packable(unpack_error = Error)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ProtocolParameters {
    // The version of the protocol running.
    #[cfg_attr(feature = "serde", serde(rename = "version"))]
    protocol_version: u8,
    // The human friendly name of the network.
    #[packable(unpack_error_with = |err| Error::InvalidNetworkName(err.into_item_err()))]
    #[cfg_attr(feature = "serde", serde(with = "crate::utils::serde::string_prefix"))]
    network_name: StringPrefix<u8>,
    // The HRP prefix used for Bech32 addresses in the network.
    bech32_hrp: Hrp,
    // The minimum pow score of the network.
    min_pow_score: u32,
    // The below max depth parameter of the network.
    below_max_depth: u8,
    // The rent structure used by given node/network.
    rent_structure: RentStructure,
    // TokenSupply defines the current token supply on the network.
    #[cfg_attr(feature = "serde", serde(with = "crate::utils::serde::string"))]
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
        // PANIC: These values are known to be correct.
        Self::new(
            PROTOCOL_VERSION,
            String::from("shimmer"),
            "smr",
            1500,
            15,
            RentStructure::default(),
            1_813_620_509_061_365,
        )
        .unwrap()
    }
}

impl ProtocolParameters {
    /// Creates a new [`ProtocolParameters`].
    pub fn new(
        protocol_version: u8,
        network_name: String,
        bech32_hrp: impl ConvertTo<Hrp>,
        min_pow_score: u32,
        below_max_depth: u8,
        rent_structure: RentStructure,
        token_supply: u64,
    ) -> Result<Self, Error> {
        Ok(Self {
            protocol_version,
            network_name: <StringPrefix<u8>>::try_from(network_name)
                .map_err(Error::InvalidStringPrefix)?,
            bech32_hrp: bech32_hrp.convert()?,
            min_pow_score,
            below_max_depth,
            rent_structure,
            token_supply,
        })
    }

    /// Returns the protocol version of the [`ProtocolParameters`].
    pub fn protocol_version(&self) -> u8 {
        self.protocol_version
    }

    /// Returns the network name of the [`ProtocolParameters`].
    pub fn network_name(&self) -> &str {
        &self.network_name
    }

    /// Returns the network ID of the [`ProtocolParameters`].
    pub fn network_id(&self) -> u64 {
        network_name_to_id(&self.network_name)
    }

    /// Returns the bech32 HRP of the [`ProtocolParameters`].
    pub fn bech32_hrp(&self) -> &Hrp {
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

    /// Returns the rent structure of the [`ProtocolParameters`].
    pub fn rent_structure(&self) -> &RentStructure {
        &self.rent_structure
    }

    /// Returns the token supply of the [`ProtocolParameters`].
    pub fn token_supply(&self) -> u64 {
        self.token_supply
    }
}

/// Returns a [`ProtocolParameters`] for testing purposes.
#[cfg(any(test, feature = "rand"))]
pub fn protocol_parameters() -> ProtocolParameters {
    ProtocolParameters::new(
        2,
        String::from("testnet"),
        "rms",
        1500,
        15,
        crate::types::block::output::RentStructure::new(500, 10, 1),
        1_813_620_509_061_365,
    )
    .unwrap()
}
