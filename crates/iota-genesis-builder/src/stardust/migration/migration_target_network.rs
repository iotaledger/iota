// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Display, str::FromStr};

use fastcrypto::hash::HashFunction;
use iota_types::{crypto::DefaultHash, digests::TransactionDigest};

const MAINNET: &str = "mainnet";
const TESTNET: &str = "testnet";

/// The target network of the migration.
///
/// Different variants of this enum will result in different digests of the
/// objects generated in the migration.
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationTargetNetwork {
    Mainnet,
    Testnet,
}

impl MigrationTargetNetwork {
    /// Returns the [`TransactionDigest`] for the migration to the target
    /// network in `self`.
    pub fn migration_transaction_digest(&self) -> TransactionDigest {
        let hash_input = match self {
            MigrationTargetNetwork::Mainnet => b"stardust-migration-mainnet",
            MigrationTargetNetwork::Testnet => b"stardust-migration-testnet",
        };

        let mut hasher = DefaultHash::default();
        hasher.update(hash_input);
        let hash = hasher.finalize();

        TransactionDigest::new(hash.into())
    }
}

impl FromStr for MigrationTargetNetwork {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            MAINNET => Ok(MigrationTargetNetwork::Mainnet),
            TESTNET => Ok(MigrationTargetNetwork::Testnet),
            other => anyhow::bail!("unknown target network name: {other}"),
        }
    }
}

impl Display for MigrationTargetNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationTargetNetwork::Mainnet => f.write_str(MAINNET),
            MigrationTargetNetwork::Testnet => f.write_str(TESTNET),
        }
    }
}
