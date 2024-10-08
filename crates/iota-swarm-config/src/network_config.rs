// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::{genesis, node, Config, NodeConfig};
use iota_types::{
    committee::CommitteeWithNetworkMetadata, crypto::AccountKeyPair, multiaddr::Multiaddr,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// This is a config that is used for testing or local use as it contains the
/// config and keys for all validators
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub validator_configs: Vec<NodeConfig>,
    pub account_keys: Vec<AccountKeyPair>,
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
            .network_metadata
            .into_values()
            .map(|n| n.network_address)
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
    pub account_keys: Vec<AccountKeyPair>,
    pub committee_with_network: CommitteeWithNetworkMetadata,
}

impl Config for NetworkConfigLight {}

impl NetworkConfigLight {
    pub fn new(
        validator_configs: Vec<NodeConfig>,
        account_keys: Vec<AccountKeyPair>,
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
            .network_metadata
            .clone()
            .into_values()
            .map(|n| n.network_address)
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
