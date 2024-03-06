// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_genesis_builder::config::{self, TokenAllocation, TokenDistributionScheduleBuilder};
use sui_macros::nondeterministic;
use sui_protocol_config::{ProtocolVersion, SupportedProtocolVersions};
use sui_sdk::sui_client_config::Config;
use sui_types::base_types::{AuthorityName, SuiAddress};
use sui_types::committee::{Committee, CommitteeWithNetworkMetadata};
use sui_types::crypto::{get_key_pair_from_rng, AccountKeyPair, PublicKey};
use sui_types::multiaddr::Multiaddr;
use sui_types::object::Object;

use crate::genesis_config::{
    AccountConfig, GenesisConfig, ValidatorGenesisConfigBuilder, DEFAULT_GAS_AMOUNT,
};
use crate::node_config::{AuthorityKeyPairWithPath, NodeConfig};

/// This is a config that is used for testing or local use as it contains the config and keys for
/// all validators
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub validator_configs: Vec<NodeConfig>,
    pub account_keys: Vec<AccountKeyPair>,
    pub genesis: config::Genesis,
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
}

pub enum CommitteeConfig {
    Size(NonZeroUsize),
    AccountKeys(Vec<AccountKeyPair>),
    /// Indicates that a committee should be deterministically generated, using the provided rng
    /// as a source of randomness as well as generating deterministic network port information.
    Deterministic((NonZeroUsize, Option<Vec<AccountKeyPair>>)),
}

pub type SupportedProtocolVersionsCallback = Arc<
    dyn Fn(
            usize,                 /* validator idx */
            Option<AuthorityName>, /* None for fullnode */
        ) -> SupportedProtocolVersions
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
pub enum ProtocolVersionsConfig {
    // use SYSTEM_DEFAULT
    Default,
    // Use one range for all validators.
    Global(SupportedProtocolVersions),
    // A closure that returns the versions for each validator.
    // TODO: This doesn't apply to fullnodes.
    PerValidator(SupportedProtocolVersionsCallback),
}

pub struct ConfigBuilder<R = OsRng> {
    rng: Option<R>,
    config_directory: PathBuf,
    committee: CommitteeConfig,
    genesis_config: Option<GenesisConfig>,
    reference_gas_price: Option<u64>,
    additional_objects: Vec<Object>,
}

impl ConfigBuilder {
    pub fn new<P: AsRef<Path>>(config_directory: P) -> Self {
        Self {
            rng: Some(OsRng),
            config_directory: config_directory.as_ref().into(),
            committee: CommitteeConfig::Size(NonZeroUsize::new(1).unwrap()),
            genesis_config: None,
            reference_gas_price: None,
            additional_objects: vec![],
        }
    }

    pub fn new_with_temp_dir() -> Self {
        Self::new(nondeterministic!(tempfile::tempdir().unwrap()).into_path())
    }
}

impl<R> ConfigBuilder<R> {
    pub fn committee(mut self, committee: CommitteeConfig) -> Self {
        self.committee = committee;
        self
    }

    pub fn committee_size(mut self, committee_size: NonZeroUsize) -> Self {
        self.committee = CommitteeConfig::Size(committee_size);
        self
    }

    pub fn deterministic_committee_size(mut self, committee_size: NonZeroUsize) -> Self {
        self.committee = CommitteeConfig::Deterministic((committee_size, None));
        self
    }

    pub fn deterministic_committee_validators(mut self, keys: Vec<AccountKeyPair>) -> Self {
        self.committee = CommitteeConfig::Deterministic((
            NonZeroUsize::new(keys.len()).expect("Validator keys should be non empty"),
            Some(keys),
        ));
        self
    }

    pub fn with_genesis_config(mut self, genesis_config: GenesisConfig) -> Self {
        assert!(self.genesis_config.is_none(), "Genesis config already set");
        self.genesis_config = Some(genesis_config);
        self
    }

    pub fn with_reference_gas_price(mut self, reference_gas_price: u64) -> Self {
        self.reference_gas_price = Some(reference_gas_price);
        self
    }

    pub fn with_accounts(mut self, accounts: Vec<AccountConfig>) -> Self {
        self.get_or_init_genesis_config().accounts = accounts;
        self
    }

    pub fn with_chain_start_timestamp_ms(mut self, chain_start_timestamp_ms: u64) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .chain_start_timestamp_ms = chain_start_timestamp_ms;
        self
    }

    pub fn with_epoch_duration(mut self, epoch_duration_ms: u64) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .epoch_duration_ms = epoch_duration_ms;
        self
    }

    pub fn with_protocol_version(mut self, protocol_version: ProtocolVersion) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .protocol_version = protocol_version;
        self
    }

    pub fn rng<N: rand::RngCore + rand::CryptoRng>(self, rng: N) -> ConfigBuilder<N> {
        ConfigBuilder {
            rng: Some(rng),
            config_directory: self.config_directory,
            committee: self.committee,
            genesis_config: self.genesis_config,
            reference_gas_price: self.reference_gas_price,
            additional_objects: self.additional_objects,
        }
    }

    fn get_or_init_genesis_config(&mut self) -> &mut GenesisConfig {
        if self.genesis_config.is_none() {
            self.genesis_config = Some(GenesisConfig::for_local_testing());
        }
        self.genesis_config.as_mut().unwrap()
    }
}

impl<R: rand::RngCore + rand::CryptoRng> ConfigBuilder<R> {
    //TODO right now we always randomize ports, we may want to have a default port configuration
    pub fn build(self) -> NetworkConfig {
        let committee = self.committee;

        let mut rng = self.rng.unwrap();
        let validators = match committee {
            CommitteeConfig::Size(size) => {
                // We always get fixed protocol keys from this function (which is isolated from
                // external test randomness because it uses a fixed seed). Necessary because some
                // tests call `make_tx_certs_and_signed_effects`, which locally forges a cert using
                // this same committee.
                let (_, keys) = Committee::new_simple_test_committee_of_size(size.into());

                keys.into_iter()
                    .map(|authority_key| {
                        let mut builder = ValidatorGenesisConfigBuilder::new()
                            .with_protocol_key_pair(authority_key);
                        if let Some(rgp) = self.reference_gas_price {
                            builder = builder.with_gas_price(rgp);
                        }
                        builder.build(&mut rng)
                    })
                    .collect::<Vec<_>>()
            }

            CommitteeConfig::AccountKeys(keys) => {
                // See above re fixed protocol keys
                let (_, protocol_keys) = Committee::new_simple_test_committee_of_size(keys.len());
                keys.into_iter()
                    .zip(protocol_keys)
                    .map(|(account_key, protocol_key)| {
                        let mut builder = ValidatorGenesisConfigBuilder::new()
                            .with_protocol_key_pair(protocol_key)
                            .with_account_key_pair(account_key);
                        if let Some(rgp) = self.reference_gas_price {
                            builder = builder.with_gas_price(rgp);
                        }
                        builder.build(&mut rng)
                    })
                    .collect::<Vec<_>>()
            }
            CommitteeConfig::Deterministic((size, keys)) => {
                // If no keys are provided, generate them.
                let keys = keys.unwrap_or(
                    (0..size.get())
                        .map(|_| get_key_pair_from_rng(&mut rng).1)
                        .collect(),
                );

                let mut configs = vec![];
                for (i, key) in keys.into_iter().enumerate() {
                    let port_offset = 8000 + i * 10;
                    let mut builder = ValidatorGenesisConfigBuilder::new()
                        .with_ip("127.0.0.1".to_owned())
                        .with_account_key_pair(key)
                        .with_deterministic_ports(port_offset as u16);
                    if let Some(rgp) = self.reference_gas_price {
                        builder = builder.with_gas_price(rgp);
                    }
                    configs.push(builder.build(&mut rng));
                }
                configs
            }
        };

        let genesis_config = self
            .genesis_config
            .unwrap_or_else(GenesisConfig::for_local_testing);

        let (account_keys, allocations) = genesis_config.generate_accounts(&mut rng).unwrap();

        let token_distribution_schedule = {
            let mut builder = TokenDistributionScheduleBuilder::new();
            for allocation in allocations {
                builder.add_allocation(allocation);
            }
            // Add allocations for each validator
            for validator in &validators {
                let account_key: PublicKey = validator.account_key_pair.public();
                let address = SuiAddress::from(&account_key);
                // Give each validator some gas so they can pay for their transactions.
                let gas_coin = TokenAllocation {
                    recipient_address: address,
                    amount_mist: DEFAULT_GAS_AMOUNT,
                    staked_with_validator: None,
                };
                let stake = TokenAllocation {
                    recipient_address: address,
                    amount_mist: validator.stake,
                    staked_with_validator: Some(address),
                };
                builder.add_allocation(gas_coin);
                builder.add_allocation(stake);
            }
            builder.build()
        };

        let genesis = {
            let mut builder = sui_genesis_builder::Builder::new()
                .with_parameters(genesis_config.parameters)
                .add_objects(self.additional_objects);

            for (i, validator) in validators.iter().enumerate() {
                let name = validator
                    .name
                    .clone()
                    .unwrap_or(format!("validator-{i}").to_string());
                let validator_info = validator.to_validator_info(name);
                builder =
                    builder.add_validator(validator_info.info, validator_info.proof_of_possession);
            }

            builder = builder.with_token_distribution_schedule(token_distribution_schedule);

            for validator in &validators {
                builder = builder.add_validator_signature(&validator.key_pair);
            }

            builder.build()
        };

        let validator_configs = validators
            .into_iter()
            .enumerate()
            .map(|(_, validator)| NodeConfig {
                protocol_key_pair: AuthorityKeyPairWithPath::new(validator.key_pair),
            })
            .collect();
        NetworkConfig {
            validator_configs,
            genesis,
            account_keys,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::network_config;
    use crate::node_config::Genesis;

    #[test]
    fn serialize_genesis_config_in_place() {
        let dir = tempfile::TempDir::new().unwrap();
        let network_config = network_config::ConfigBuilder::new(&dir).build();
        let genesis = network_config.genesis;

        let g = Genesis::new(genesis);

        let mut s = serde_yaml::to_string(&g).unwrap();
        let loaded_genesis: Genesis = serde_yaml::from_str(&s).unwrap();
        loaded_genesis
            .genesis()
            .unwrap()
            .checkpoint_contents()
            .digest(); // cache digest before comparing.
        assert_eq!(g, loaded_genesis);

        // If both in-place and file location are provided, prefer the in-place variant
        s.push_str("\ngenesis-file-location: path/to/file");
        let loaded_genesis: Genesis = serde_yaml::from_str(&s).unwrap();
        loaded_genesis
            .genesis()
            .unwrap()
            .checkpoint_contents()
            .digest(); // cache digest before comparing.
        assert_eq!(g, loaded_genesis);
    }

    #[test]
    fn load_genesis_config_from_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let genesis_config = Genesis::new_from_file(file.path());

        let dir = tempfile::TempDir::new().unwrap();
        let network_config = network_config::ConfigBuilder::new(&dir).build();
        let genesis = network_config.genesis;
        genesis.save(file.path()).unwrap();

        let loaded_genesis = genesis_config.genesis().unwrap();
        loaded_genesis.checkpoint_contents().digest(); // cache digest before comparing.
        assert_eq!(&genesis, loaded_genesis);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::sync::Arc;
    use sui_genesis_builder::config::Genesis;
    use sui_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use sui_types::epoch_data::EpochData;
    use sui_types::gas::SuiGasStatus;
    use sui_types::in_memory_storage::InMemoryStorage;
    use sui_types::metrics::LimitsMetrics;
    use sui_types::sui_system_state::SuiSystemStateTrait;
    use sui_types::transaction::CheckedInputObjects;

    use crate::network_config;

    #[test]
    fn roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let network_config = network_config::ConfigBuilder::new(&dir).build();
        let genesis = network_config.genesis;

        let s = serde_yaml::to_string(&genesis).unwrap();
        let from_s: Genesis = serde_yaml::from_str(&s).unwrap();
        // cache the digest so that the comparison succeeds.
        from_s.checkpoint_contents().digest();
        assert_eq!(genesis, from_s);
    }

    #[test]
    fn genesis_transaction() {
        let builder = network_config::ConfigBuilder::new_with_temp_dir();
        let network_config = builder.build();
        let genesis = network_config.genesis;
        let protocol_version = ProtocolVersion::new(genesis.sui_system_object().protocol_version());
        let protocol_config = ProtocolConfig::get_for_version(protocol_version, Chain::Unknown);

        let genesis_transaction = genesis.transaction().clone();

        let genesis_digest = *genesis_transaction.digest();

        let silent = true;
        let executor = sui_execution::executor(&protocol_config, silent, None)
            .expect("Creating an executor should not fail here");

        // Use a throwaway metrics registry for genesis transaction execution.
        let registry = prometheus::Registry::new();
        let metrics = Arc::new(LimitsMetrics::new(&registry));
        let expensive_checks = false;
        let certificate_deny_set = HashSet::new();
        let epoch = EpochData::new_test();
        let transaction_data = &genesis_transaction.data().intent_message().value;
        let (kind, signer, _) = transaction_data.execution_parts();
        let input_objects = CheckedInputObjects::new_for_genesis(vec![]);

        let (_inner_temp_store, _, effects, _execution_error) = executor
            .execute_transaction_to_effects(
                &InMemoryStorage::new(Vec::new()),
                &protocol_config,
                metrics,
                expensive_checks,
                &certificate_deny_set,
                &epoch.epoch_id(),
                epoch.epoch_start_timestamp(),
                input_objects,
                vec![],
                SuiGasStatus::new_unmetered(),
                kind,
                signer,
                genesis_digest,
            );

        assert_eq!(&effects, genesis.effects());
    }
}
