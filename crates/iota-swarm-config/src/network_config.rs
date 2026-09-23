// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use fastcrypto::encoding::{Base64, Encoding};
use iota_config::{Config, IOTA_NETWORK_CONFIG, NodeConfig, genesis, node};
use iota_multiaddr::Multiaddr;
use iota_sdk_crypto::ToFromBytes as _;
use iota_types::{committee::CommitteeWithNetworkMetadata, crypto::AccountPrivateKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs, serde_as};

use crate::{genesis_config::GenesisConfig, node_config_builder::ValidatorConfigBuilder};

/// Serializes an account key as a base64 string of the raw 32-byte ed25519
/// private key.
struct AccountPrivateKeyBase64;

impl SerializeAs<AccountPrivateKey> for AccountPrivateKeyBase64 {
    fn serialize_as<S: Serializer>(key: &AccountPrivateKey, s: S) -> Result<S::Ok, S::Error> {
        Base64::encode(key.to_bytes()).serialize(s)
    }
}

impl<'de> DeserializeAs<'de, AccountPrivateKey> for AccountPrivateKeyBase64 {
    fn deserialize_as<D: Deserializer<'de>>(d: D) -> Result<AccountPrivateKey, D::Error> {
        let bytes = Base64::decode(&String::deserialize(d)?).map_err(serde::de::Error::custom)?;
        AccountPrivateKey::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

/// This is a config that is used for testing or local use as it contains the
/// config and keys for all validators
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub validator_configs: Vec<NodeConfig>,
    #[serde_as(as = "Vec<AccountPrivateKeyBase64>")]
    pub account_keys: Vec<AccountPrivateKey>,
    pub genesis: genesis::Genesis,
}

impl Config for NetworkConfig {}

impl NetworkConfig {
    pub fn validator_configs(&self) -> &[NodeConfig] {
        &self.validator_configs
    }

    pub fn net_addresses(&self) -> Vec<Multiaddr> {
        self.genesis
            .committee_with_network()
            .validators()
            .values()
            .map(|(_, n)| n.network_address.clone())
            .collect()
    }

    pub fn committee_with_network(&self) -> CommitteeWithNetworkMetadata {
        self.genesis.committee_with_network()
    }

    pub fn into_validator_configs(self) -> Vec<NodeConfig> {
        self.validator_configs
    }

    /// Retrieve genesis information that might be present in the configured
    /// validators.
    pub fn get_validator_genesis(&self) -> Option<&node::Genesis> {
        self.validator_configs
            .first()
            .as_ref()
            .map(|validator| &validator.genesis)
    }
}

/// What `iota-localnet` writes to `network.yaml`: everything the node configs
/// of a local network are derived from, and the version of the format they are
/// written in.
///
/// The genesis blob is not derived from this. It is persisted beside it and
/// only read.
#[serde_as]
#[derive(Deserialize, Serialize)]
pub struct PersistedNetworkConfig {
    /// The version of this file's format. A file without one predates the
    /// field and cannot be read.
    pub version: u32,
    pub genesis_config: GenesisConfig,
    #[serde_as(as = "Vec<AccountPrivateKeyBase64>")]
    pub account_keys: Vec<AccountPrivateKey>,
}

impl Config for PersistedNetworkConfig {}

/// Reads only the format version, so that a file this build cannot read is
/// rejected before its other fields are.
#[derive(Deserialize)]
struct NetworkConfigFormatVersion {
    #[serde(default)]
    version: Option<u32>,
}

impl PersistedNetworkConfig {
    /// The format version this build writes and reads.
    pub const VERSION: u32 = 1;

    /// Read the network config from `network.yaml` in `config_directory`.
    ///
    /// # Errors
    ///
    /// - The file is missing or unreadable.
    /// - It was written in a format version this build does not read, which
    ///   includes every file written before the version field existed.
    pub fn read(config_directory: &Path) -> Result<Self> {
        let path = config_directory.join(IOTA_NETWORK_CONFIG);
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot open the IOTA network config file at {path:?}"))?;
        let format_version: NetworkConfigFormatVersion = serde_yaml::from_str(&contents)
            .with_context(|| format!("cannot read the IOTA network config file at {path:?}"))?;
        if format_version.version != Some(Self::VERSION) {
            // `genesis --force` deletes the directory, which is the wrong
            // advice for a file a newer build wrote.
            if format_version.version > Some(Self::VERSION) {
                bail!(
                    "the configuration in {} was created by a newer version of iota-localnet and \
                     cannot be read. Update iota-localnet.",
                    config_directory.display()
                );
            }
            bail!(
                "the configuration in {} was created by an older version of iota-localnet and \
                 cannot be read. Re-create it with `iota-localnet genesis --force`.",
                config_directory.display()
            );
        }
        serde_yaml::from_str(&contents)
            .with_context(|| format!("cannot read the IOTA network config file at {path:?}"))
    }

    /// Derive the node config of every validator of this network, attaching
    /// `genesis` to each rather than building a genesis from the genesis
    /// config.
    ///
    /// # Errors
    ///
    /// - The network has no validator.
    /// - `genesis` cannot be read.
    pub fn into_network_config(
        self,
        config_directory: &Path,
        genesis: node::Genesis,
    ) -> Result<NetworkConfig> {
        let validators = self
            .genesis_config
            .validator_config_info
            .unwrap_or_default();
        ensure!(
            !validators.is_empty(),
            "the IOTA network config must contain at least one validator"
        );
        let validator_configs = validators
            .into_iter()
            .map(|validator| {
                let mut config = ValidatorConfigBuilder::new()
                    .with_config_directory(config_directory.to_path_buf())
                    .build_without_genesis(validator);
                config.genesis = genesis.clone();
                config
            })
            .collect();
        Ok(NetworkConfig {
            validator_configs,
            account_keys: self.account_keys,
            genesis: genesis.genesis()?.clone(),
        })
    }
}

/// This is the light version of [`NetworkConfig`] that does not
/// contain the entire [`genesis::Genesis`].
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkConfigLight {
    pub validator_configs: Vec<NodeConfig>,
    #[serde_as(as = "Vec<AccountPrivateKeyBase64>")]
    pub account_keys: Vec<AccountPrivateKey>,
    pub committee_with_network: CommitteeWithNetworkMetadata,
}

impl Config for NetworkConfigLight {}

impl NetworkConfigLight {
    pub fn new(
        validator_configs: Vec<NodeConfig>,
        account_keys: Vec<AccountPrivateKey>,
        genesis: &genesis::Genesis,
    ) -> Self {
        Self {
            validator_configs,
            account_keys,
            committee_with_network: genesis.committee_with_network(),
        }
    }

    pub fn validator_configs(&self) -> &[NodeConfig] {
        &self.validator_configs
    }

    pub fn net_addresses(&self) -> Vec<Multiaddr> {
        self.committee_with_network
            .validators()
            .values()
            .map(|(_, n)| n.network_address.clone())
            .collect()
    }

    pub fn into_validator_configs(self) -> Vec<NodeConfig> {
        self.validator_configs
    }

    /// Retrieve genesis information that might be present in the configured
    /// validators.
    pub fn get_validator_genesis(&self) -> Option<&node::Genesis> {
        self.validator_configs
            .first()
            .as_ref()
            .map(|validator| &validator.genesis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `network.yaml` a hand edit left without validators is refused here,
    /// rather than deeper in the launch of a network that has no committee.
    #[test]
    fn a_persisted_config_without_validators_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let persisted = PersistedNetworkConfig {
            version: PersistedNetworkConfig::VERSION,
            genesis_config: GenesisConfig::for_local_testing(),
            account_keys: vec![],
        };

        let err = persisted
            .into_network_config(directory.path(), node::Genesis::new_empty())
            .unwrap_err();

        assert!(err.to_string().contains("at least one validator"), "{err}");
    }
}
