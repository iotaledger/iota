// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::{Base64, Encoding};
use iota_config::{Config, NodeConfig, genesis, node};
use iota_multiaddr::Multiaddr;
use iota_sdk_crypto::ToFromBytes as _;
use iota_types::{committee::CommitteeWithNetworkMetadata, crypto::AccountPrivateKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs, serde_as};

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
