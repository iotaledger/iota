// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Display, str::FromStr};

use fastcrypto::hash::HashFunction;
use iota_types::{crypto::DefaultHash, digests::TransactionDigest, stardust::coin_type::CoinType};

const MAINNET: &str = "mainnet";
const TESTNET: &str = "testnet";
const ALPHANET: &str = "alphanet";

/// The target network of the migration.
///
/// Different variants of this enum will result in different digests of the
/// objects generated in the migration.
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationTargetNetwork {
    Mainnet,
    Testnet(String),
    Alphanet(String),
}

impl MigrationTargetNetwork {
    /// Returns the [`TransactionDigest`] for the migration to the target
    /// network in `self`.
    pub fn migration_transaction_digest(&self, coin_type: &CoinType) -> TransactionDigest {
        let hash_input = format!("{coin_type}-stardust-migration-{self}");
        let mut hasher = DefaultHash::default();
        hasher.update(hash_input);
        let hash = hasher.finalize();

        TransactionDigest::new(hash.into())
    }
}

impl FromStr for MigrationTargetNetwork {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        if string == MAINNET {
            return Ok(MigrationTargetNetwork::Mainnet);
        }

        if string.starts_with(TESTNET) {
            return Ok(MigrationTargetNetwork::Testnet(
                string.chars().skip(TESTNET.len()).collect(),
            ));
        }

        if string.starts_with(ALPHANET) {
            return Ok(MigrationTargetNetwork::Alphanet(
                string.chars().skip(ALPHANET.len()).collect(),
            ));
        }

        anyhow::bail!(
            "unknown target network name '{string}': please provide the target network for which the snapshot is being generated ('{}', '{}' or '{}')",
            MigrationTargetNetwork::Mainnet,
            MigrationTargetNetwork::Testnet("(optional-string)".to_owned()),
            MigrationTargetNetwork::Alphanet("(optional-string)".to_owned()),
        )
    }
}

impl Display for MigrationTargetNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationTargetNetwork::Mainnet => f.write_str(MAINNET),
            MigrationTargetNetwork::Testnet(string) => {
                f.write_str(TESTNET)?;
                f.write_str(string)
            }
            MigrationTargetNetwork::Alphanet(string) => {
                f.write_str(ALPHANET)?;
                f.write_str(string)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::stardust::migration::MigrationTargetNetwork;

    #[test]
    fn to_and_from_string() {
        let ok_test_inputs = [
            "mainnet",
            "testnet",
            "alphanet",
            "testnet1",
            "alphanetOther",
        ];

        for test_input in ok_test_inputs {
            assert_eq!(
                MigrationTargetNetwork::from_str(test_input)
                    .unwrap()
                    .to_string(),
                test_input
            )
        }
    }

    #[test]
    fn erroneous_input() {
        assert!(MigrationTargetNetwork::from_str("shimmer").is_err());
    }
}
