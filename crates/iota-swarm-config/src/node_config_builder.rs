// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{Ipv4Addr, SocketAddr},
    num::NonZeroUsize,
    path::PathBuf,
};

use anyhow::anyhow;
use fastcrypto::{
    encoding::{Encoding, Hex},
    traits::KeyPair,
};
use iota_config::{
    AUTHORITIES_DB_NAME, CONSENSUS_DB_NAME, ConsensusConfig, FULL_NODE_DB_PATH,
    IOTA_GENESIS_MIGRATION_TX_DATA_FILENAME, NodeConfig, local_ip_utils,
    node::{
        AuthorityKeyPairWithPath, AuthorityOverloadConfig, AuthorityStorePruningConfig,
        CheckpointExecutorConfig, ExecutionCacheConfig, ExpensiveSafetyCheckConfig, Genesis,
        GrpcApiConfig, KeyPairWithPath, RunWithRange, StateSnapshotConfig,
        default_enable_index_processing, default_end_of_epoch_broadcast_channel_capacity,
        default_full_checkpoint_contents_cache_size_mb,
    },
    p2p::{DiscoveryConfig, P2pConfig, SeedPeer, StateSyncConfig},
    transaction_deny_config::TransactionDenyConfig,
    verifier_signing_config::VerifierSigningConfig,
};
use iota_multiaddr::Multiaddr;
use iota_names::config::IotaNamesConfig;
use iota_protocol_config::Chain;
use iota_types::{
    crypto::{
        AuthorityKeyPair, AuthorityPublicKeyBytes, NetworkKeyPair, network_to_simple_keypair,
    },
    supported_protocol_versions::SupportedProtocolVersions,
    traffic_control::{PolicyConfig, RemoteFirewallConfig},
};

use crate::{
    genesis_config::{ValidatorGenesisConfig, ValidatorGenesisConfigBuilder},
    network_config::NetworkConfig,
};

/// This builder contains information that's not included in
/// ValidatorGenesisConfig for building a validator NodeConfig. It can be used
/// to build either a genesis validator or a new validator.
#[derive(Clone, Default)]
pub struct ValidatorConfigBuilder {
    config_directory: Option<PathBuf>,
    supported_protocol_versions: Option<SupportedProtocolVersions>,
    force_unpruned_checkpoints: bool,
    authority_overload_config: Option<AuthorityOverloadConfig>,
    transaction_deny_config: Option<TransactionDenyConfig>,
    execution_cache_config: Option<ExecutionCacheConfig>,
    data_ingestion_dir: Option<PathBuf>,
    policy_config: Option<PolicyConfig>,
    firewall_config: Option<RemoteFirewallConfig>,
    max_submit_position: Option<usize>,
    submit_delay_step_override_millis: Option<u64>,
    discovery_config: Option<DiscoveryConfig>,
    chain_override: Option<Chain>,
}

impl ValidatorConfigBuilder {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn with_chain_override(mut self, chain: Chain) -> Self {
        assert!(self.chain_override.is_none(), "Chain override already set");
        self.chain_override = Some(chain);
        self
    }

    pub fn with_config_directory(mut self, config_directory: PathBuf) -> Self {
        assert!(self.config_directory.is_none());
        self.config_directory = Some(config_directory);
        self
    }

    pub fn with_supported_protocol_versions(
        mut self,
        supported_protocol_versions: SupportedProtocolVersions,
    ) -> Self {
        assert!(self.supported_protocol_versions.is_none());
        self.supported_protocol_versions = Some(supported_protocol_versions);
        self
    }

    pub fn with_unpruned_checkpoints(mut self) -> Self {
        self.force_unpruned_checkpoints = true;
        self
    }

    pub fn with_authority_overload_config(mut self, config: AuthorityOverloadConfig) -> Self {
        self.authority_overload_config = Some(config);
        self
    }

    pub fn with_transaction_deny_config(mut self, config: TransactionDenyConfig) -> Self {
        self.transaction_deny_config = Some(config);
        self
    }

    pub fn with_execution_cache_config(mut self, config: ExecutionCacheConfig) -> Self {
        self.execution_cache_config = Some(config);
        self
    }

    pub fn with_data_ingestion_dir(mut self, path: PathBuf) -> Self {
        self.data_ingestion_dir = Some(path);
        self
    }

    pub fn with_policy_config(mut self, config: Option<PolicyConfig>) -> Self {
        self.policy_config = config;
        self
    }

    pub fn with_firewall_config(mut self, config: Option<RemoteFirewallConfig>) -> Self {
        self.firewall_config = config;
        self
    }

    pub fn with_max_submit_position(mut self, max_submit_position: usize) -> Self {
        self.max_submit_position = Some(max_submit_position);
        self
    }

    pub fn with_submit_delay_step_override_millis(
        mut self,
        submit_delay_step_override_millis: u64,
    ) -> Self {
        self.submit_delay_step_override_millis = Some(submit_delay_step_override_millis);
        self
    }

    pub fn with_discovery_config(mut self, discovery_config: DiscoveryConfig) -> Self {
        self.discovery_config = Some(discovery_config);
        self
    }

    pub fn build_without_genesis(self, validator: ValidatorGenesisConfig) -> NodeConfig {
        let key_path = get_key_path(&validator.authority_key_pair);
        let config_directory = self
            .config_directory
            .unwrap_or_else(|| iota_common::tempdir().keep());
        let migration_tx_data_path =
            Some(config_directory.join(IOTA_GENESIS_MIGRATION_TX_DATA_FILENAME));
        let db_path = config_directory
            .join(AUTHORITIES_DB_NAME)
            .join(key_path.clone());
        let network_address = validator.network_address;
        let consensus_db_path = config_directory.join(CONSENSUS_DB_NAME).join(key_path);
        let localhost = local_ip_utils::localhost_for_testing();
        let consensus_config = ConsensusConfig {
            db_path: consensus_db_path,
            db_retention_epochs: None,
            db_pruner_period_secs: None,
            max_pending_transactions: None,
            max_submit_position: self.max_submit_position,
            submit_delay_step_override_millis: self.submit_delay_step_override_millis,
            parameters: Default::default(),
            graduated_load_shedding_soft_limit_pct: Default::default(),
        };

        let p2p_config = P2pConfig {
            listen_address: validator.p2p_listen_address.unwrap_or_else(|| {
                validator
                    .p2p_address
                    .udp_multiaddr_to_listen_address()
                    .unwrap()
            }),
            external_address: Some(validator.p2p_address),
            // Set a shorter timeout for checkpoint content download in tests, since
            // checkpoint pruning also happens much faster, and network is local.
            state_sync: Some(StateSyncConfig {
                checkpoint_content_timeout_ms: Some(10_000),
                ..Default::default()
            }),
            // Use discovery config if provided
            discovery: self.discovery_config,
            ..Default::default()
        };

        let mut pruning_config = AuthorityStorePruningConfig::default();
        if self.force_unpruned_checkpoints {
            pruning_config.set_num_epochs_to_retain_for_checkpoints(None);
        }
        let pruning_config = pruning_config;
        let checkpoint_executor_config = CheckpointExecutorConfig {
            data_ingestion_dir: self.data_ingestion_dir,
            ..Default::default()
        };

        NodeConfig {
            authority_key_pair: AuthorityKeyPairWithPath::new(validator.authority_key_pair),
            network_key_pair: KeyPairWithPath::new(network_to_simple_keypair(
                &validator.network_key_pair,
            )),
            account_key_pair: KeyPairWithPath::new(validator.account_key_pair),
            protocol_key_pair: KeyPairWithPath::new(network_to_simple_keypair(
                &validator.protocol_key_pair,
            )),
            db_path,
            network_address,
            metrics_address: validator.metrics_address,
            admin_interface_address: validator.admin_interface_address,
            json_rpc_address: local_ip_utils::new_tcp_address_for_testing(&localhost)
                .to_socket_addr()
                .unwrap(),
            consensus_config: Some(consensus_config),
            enable_index_processing: default_enable_index_processing(),
            genesis: Genesis::new_empty(),
            migration_tx_data_path,
            grpc_load_shed: None,
            // Effectively unlimited: tests and benchmarks must not be
            // throttled.
            grpc_concurrency_limit_per_core: NonZeroUsize::new(500_000_000).unwrap(),
            p2p_config,
            authority_store_pruning_config: pruning_config,
            end_of_epoch_broadcast_channel_capacity:
                default_end_of_epoch_broadcast_channel_capacity(),
            checkpoint_executor_config,
            metrics: None,
            supported_protocol_versions: self.supported_protocol_versions,
            // By default, expensive checks will be enabled in debug build, but not in release
            // build.
            expensive_safety_check_config: ExpensiveSafetyCheckConfig::default(),
            transaction_deny_config: self.transaction_deny_config.unwrap_or_default(),
            certificate_deny_config: Default::default(),
            state_debug_dump_config: Default::default(),
            checkpoint_archive_config: None,
            state_snapshot_write_config: StateSnapshotConfig::default(),
            indexer_max_subscriptions: Default::default(),
            transaction_kv_store_read_config: Default::default(),
            transaction_kv_store_write_config: None,
            authority_overload_config: self.authority_overload_config.unwrap_or_default(),
            execution_cache_config: self.execution_cache_config.unwrap_or_default(),
            full_checkpoint_contents_cache_size_mb: default_full_checkpoint_contents_cache_size_mb(
            ),
            run_with_range: None,
            jsonrpc_server_type: None,
            policy_config: self.policy_config,
            firewall_config: self.firewall_config,
            enable_validator_tx_finalizer: true,
            enable_soft_locking: true,
            verifier_signing_config: VerifierSigningConfig::default(),
            enable_db_write_stall: None,
            iota_names_config: None,
            enable_grpc_api: false,
            grpc_api_config: None,
            chain_override_for_testing: self.chain_override,
            validator_client_monitor_config: None,
        }
    }

    pub fn build(
        self,
        validator: ValidatorGenesisConfig,
        genesis: iota_config::genesis::Genesis,
    ) -> NodeConfig {
        let mut config = self.build_without_genesis(validator);
        config.genesis = iota_config::node::Genesis::new(genesis);
        config
    }

    pub fn build_new_validator<R: rand::RngCore + rand::CryptoRng>(
        self,
        rng: &mut R,
        network_config: &NetworkConfig,
    ) -> NodeConfig {
        let validator_config = ValidatorGenesisConfigBuilder::new().build(rng);
        self.build(validator_config, network_config.genesis.clone())
    }
}

#[derive(Clone, Debug, Default)]
pub struct FullnodeConfigBuilder {
    config_directory: Option<PathBuf>,
    // port for json rpc api
    rpc_port: Option<u16>,
    rpc_addr: Option<SocketAddr>,
    supported_protocol_versions: Option<SupportedProtocolVersions>,
    expensive_safety_check_config: Option<ExpensiveSafetyCheckConfig>,
    db_path: Option<PathBuf>,
    network_address: Option<Multiaddr>,
    json_rpc_address: Option<SocketAddr>,
    metrics_address: Option<SocketAddr>,
    admin_interface_address: Option<SocketAddr>,
    genesis: Option<Genesis>,
    p2p_external_address: Option<Multiaddr>,
    p2p_listen_address: Option<SocketAddr>,
    network_key_pair: Option<KeyPairWithPath>,
    run_with_range: Option<RunWithRange>,
    policy_config: Option<PolicyConfig>,
    fw_config: Option<RemoteFirewallConfig>,
    data_ingestion_dir: Option<PathBuf>,
    disable_pruning: bool,
    iota_names_config: Option<IotaNamesConfig>,
    enable_grpc_api: bool,
    grpc_api_config: Option<GrpcApiConfig>,
    discovery_config: Option<DiscoveryConfig>,
    chain_override: Option<Chain>,
    deterministic_port_base: Option<u16>,
}

impl FullnodeConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_chain_override(mut self, chain: Chain) -> Self {
        assert!(self.chain_override.is_none(), "Chain override already set");
        self.chain_override = Some(chain);
        self
    }

    pub fn with_config_directory(mut self, config_directory: PathBuf) -> Self {
        self.config_directory = Some(config_directory);
        self
    }

    pub fn with_rpc_port(mut self, port: u16) -> Self {
        assert!(self.rpc_addr.is_none() && self.rpc_port.is_none());
        self.rpc_port = Some(port);
        self
    }

    pub fn with_rpc_addr(mut self, addr: impl Into<SocketAddr>) -> Self {
        assert!(self.rpc_addr.is_none() && self.rpc_port.is_none());
        self.rpc_addr = Some(addr.into());
        self
    }

    pub fn with_supported_protocol_versions(mut self, versions: SupportedProtocolVersions) -> Self {
        self.supported_protocol_versions = Some(versions);
        self
    }

    pub fn with_disable_pruning(mut self, disable_pruning: bool) -> Self {
        self.disable_pruning = disable_pruning;
        self
    }

    pub fn with_expensive_safety_check_config(
        mut self,
        expensive_safety_check_config: ExpensiveSafetyCheckConfig,
    ) -> Self {
        self.expensive_safety_check_config = Some(expensive_safety_check_config);
        self
    }

    pub fn with_db_path(mut self, db_path: PathBuf) -> Self {
        self.db_path = Some(db_path);
        self
    }

    pub fn with_network_address(mut self, network_address: Multiaddr) -> Self {
        self.network_address = Some(network_address);
        self
    }

    pub fn with_json_rpc_address(mut self, json_rpc_address: impl Into<SocketAddr>) -> Self {
        self.json_rpc_address = Some(json_rpc_address.into());
        self
    }

    pub fn with_metrics_address(mut self, metrics_address: impl Into<SocketAddr>) -> Self {
        self.metrics_address = Some(metrics_address.into());
        self
    }

    pub fn with_admin_interface_address(
        mut self,
        admin_interface_address: Option<impl Into<SocketAddr>>,
    ) -> Self {
        self.admin_interface_address = admin_interface_address.map(|addr| addr.into());
        self
    }

    pub fn with_genesis(mut self, genesis: Genesis) -> Self {
        self.genesis = Some(genesis);
        self
    }

    pub fn with_p2p_external_address(mut self, p2p_external_address: Multiaddr) -> Self {
        self.p2p_external_address = Some(p2p_external_address);
        self
    }

    pub fn with_p2p_listen_address(mut self, p2p_listen_address: impl Into<SocketAddr>) -> Self {
        self.p2p_listen_address = Some(p2p_listen_address.into());
        self
    }

    pub fn with_network_key_pair(mut self, network_key_pair: Option<NetworkKeyPair>) -> Self {
        if let Some(network_key_pair) = network_key_pair {
            self.network_key_pair = Some(KeyPairWithPath::new(network_to_simple_keypair(
                &network_key_pair,
            )));
        }
        self
    }

    pub fn with_run_with_range(mut self, run_with_range: Option<RunWithRange>) -> Self {
        if let Some(run_with_range) = run_with_range {
            self.run_with_range = Some(run_with_range);
        }
        self
    }

    pub fn with_policy_config(mut self, config: Option<PolicyConfig>) -> Self {
        self.policy_config = config;
        self
    }

    pub fn with_fw_config(mut self, config: Option<RemoteFirewallConfig>) -> Self {
        self.fw_config = config;
        self
    }

    pub fn with_data_ingestion_dir(mut self, path: Option<PathBuf>) -> Self {
        self.data_ingestion_dir = path;
        self
    }

    pub fn with_iota_names_config(mut self, config: Option<IotaNamesConfig>) -> Self {
        self.iota_names_config = config;
        self
    }

    pub fn with_enable_grpc_api(mut self, enable_grpc_api: bool) -> Self {
        self.enable_grpc_api = enable_grpc_api;
        self
    }

    pub fn with_grpc_api_config(mut self, config: GrpcApiConfig) -> Self {
        self.grpc_api_config = Some(config);
        self
    }

    pub fn with_discovery_config(mut self, discovery_config: DiscoveryConfig) -> Self {
        self.discovery_config = Some(discovery_config);
        self
    }

    /// Give the fullnode fixed addresses on 127.0.0.1 instead of
    /// currently-free ports: `port_base` for the metrics endpoint,
    /// `port_base + 1` for the admin interface and `port_base + 2` for p2p.
    ///
    /// Addresses set explicitly on this builder still win.
    pub fn with_deterministic_ports(mut self, port_base: u16) -> Self {
        self.deterministic_port_base = Some(port_base);
        self
    }

    /// Build the fullnode config against the given validator configs.
    ///
    /// # Panics
    ///
    /// Panics if [`Self::try_build_from_parts`] returns an error.
    pub fn build_from_parts<R: rand::RngCore + rand::CryptoRng>(
        self,
        rng: &mut R,
        validator_configs: &[NodeConfig],
        genesis: iota_config::node::Genesis,
    ) -> NodeConfig {
        self.try_build_from_parts(rng, validator_configs, genesis)
            .unwrap_or_else(|err| panic!("{err:#}"))
    }

    /// Build the fullnode config against the given validator configs.
    ///
    /// Fails if a validator config has no `p2p-config.external-address`,
    /// which the fullnode's seed peers are derived from.
    ///
    /// # Panics
    ///
    /// Panics on failures the config cannot be built without: creating the
    /// temporary config directory, allocating a local port, or parsing a
    /// generated network address.
    pub fn try_build_from_parts<R: rand::RngCore + rand::CryptoRng>(
        self,
        rng: &mut R,
        validator_configs: &[NodeConfig],
        genesis: iota_config::node::Genesis,
    ) -> anyhow::Result<NodeConfig> {
        // Take advantage of ValidatorGenesisConfigBuilder to build the keypairs and
        // addresses, even though this is a fullnode.
        let validator_config = ValidatorGenesisConfigBuilder::new().build(rng);
        let ip = validator_config
            .network_address
            .to_socket_addr()
            .unwrap()
            .ip()
            .to_string();

        let key_path = get_key_path(&validator_config.authority_key_pair);
        let config_directory = self
            .config_directory
            .unwrap_or_else(|| iota_common::tempdir().keep());

        let migration_tx_data_path =
            Some(config_directory.join(IOTA_GENESIS_MIGRATION_TX_DATA_FILENAME));

        let localhost = local_ip_utils::localhost_for_testing();
        let deterministic_port = |offset: u16| {
            self.deterministic_port_base.map(|port_base| {
                port_base.checked_add(offset).unwrap_or_else(|| {
                    panic!("the fullnode port layout does not fit above port {port_base}")
                })
            })
        };
        let deterministic_metrics_address =
            deterministic_port(0).map(|port| SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
        let deterministic_admin_interface_address =
            deterministic_port(1).map(|port| SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
        let deterministic_p2p_address = deterministic_port(2).map(|port| {
            local_ip_utils::new_deterministic_udp_address_for_testing(&localhost, port)
        });

        let p2p_config = {
            let seed_peers = validator_configs
                .iter()
                .enumerate()
                .map(|(index, config)| {
                    let address = config.p2p_config.external_address.clone().ok_or_else(|| {
                        anyhow!(
                            "validator {index} has no `p2p-config.external-address`, which the \
                             fullnode needs to derive its seed peers: either its config never set \
                             one or it was cleared"
                        )
                    })?;
                    Ok(SeedPeer {
                        peer_id: Some(anemo::PeerId(
                            config.network_key_pair().public().0.to_bytes(),
                        )),
                        address,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            P2pConfig {
                listen_address: self
                    .p2p_listen_address
                    .or_else(|| {
                        deterministic_p2p_address
                            .as_ref()
                            .and_then(|address| address.udp_multiaddr_to_listen_address())
                    })
                    .unwrap_or_else(|| {
                        validator_config.p2p_listen_address.unwrap_or_else(|| {
                            validator_config
                                .p2p_address
                                .udp_multiaddr_to_listen_address()
                                .unwrap()
                        })
                    }),
                external_address: self
                    .p2p_external_address
                    .or(deterministic_p2p_address)
                    .or(Some(validator_config.p2p_address.clone())),
                seed_peers,
                // Set a shorter timeout for checkpoint content download in tests, since
                // checkpoint pruning also happens much faster, and network is local.
                state_sync: Some(StateSyncConfig {
                    checkpoint_content_timeout_ms: Some(10_000),
                    ..Default::default()
                }),
                // Use discovery config if provided
                discovery: self.discovery_config,
                ..Default::default()
            }
        };

        let json_rpc_address = self.json_rpc_address.unwrap_or_else(|| {
            self.rpc_addr.unwrap_or_else(|| {
                let rpc_port = self
                    .rpc_port
                    .unwrap_or_else(|| local_ip_utils::get_available_port(&ip));
                format!("{ip}:{rpc_port}").parse().unwrap()
            })
        });

        let grpc_api_config = self.grpc_api_config.or_else(|| {
            if self.enable_grpc_api {
                Some(GrpcApiConfig {
                    address: format!("{ip}:{}", local_ip_utils::get_available_port(&ip))
                        .parse()
                        .unwrap(),
                    ..Default::default()
                })
            } else {
                None
            }
        });

        let checkpoint_executor_config = CheckpointExecutorConfig {
            data_ingestion_dir: self.data_ingestion_dir,
            ..Default::default()
        };

        let mut pruning_config = AuthorityStorePruningConfig::default();
        if self.disable_pruning {
            pruning_config.set_num_epochs_to_retain_for_checkpoints(None);
            pruning_config.set_num_epochs_to_retain(u64::MAX);
        };

        Ok(NodeConfig {
            authority_key_pair: AuthorityKeyPairWithPath::new(validator_config.authority_key_pair),
            account_key_pair: KeyPairWithPath::new(validator_config.account_key_pair),
            protocol_key_pair: KeyPairWithPath::new(network_to_simple_keypair(
                &validator_config.protocol_key_pair,
            )),
            network_key_pair: self.network_key_pair.unwrap_or(KeyPairWithPath::new(
                network_to_simple_keypair(&validator_config.network_key_pair),
            )),
            db_path: self
                .db_path
                .unwrap_or(config_directory.join(FULL_NODE_DB_PATH).join(key_path)),
            network_address: self
                .network_address
                .unwrap_or(validator_config.network_address),
            metrics_address: self
                .metrics_address
                .or(deterministic_metrics_address)
                .unwrap_or_else(local_ip_utils::new_local_tcp_socket_for_testing),
            admin_interface_address: self
                .admin_interface_address
                .or(deterministic_admin_interface_address)
                .unwrap_or_else(local_ip_utils::new_local_tcp_socket_for_testing),
            json_rpc_address,
            consensus_config: None,
            enable_index_processing: default_enable_index_processing(),
            genesis,
            migration_tx_data_path,
            grpc_load_shed: None,
            // Effectively unlimited: tests and benchmarks must not be
            // throttled.
            grpc_concurrency_limit_per_core: NonZeroUsize::new(500_000_000).unwrap(),
            p2p_config,
            authority_store_pruning_config: pruning_config,
            end_of_epoch_broadcast_channel_capacity:
                default_end_of_epoch_broadcast_channel_capacity(),
            checkpoint_executor_config,
            metrics: None,
            supported_protocol_versions: self.supported_protocol_versions,
            expensive_safety_check_config: self
                .expensive_safety_check_config
                .unwrap_or_else(ExpensiveSafetyCheckConfig::new_enable_all),
            transaction_deny_config: Default::default(),
            certificate_deny_config: Default::default(),
            state_debug_dump_config: Default::default(),
            checkpoint_archive_config: None,
            state_snapshot_write_config: StateSnapshotConfig::default(),
            indexer_max_subscriptions: Default::default(),
            transaction_kv_store_read_config: Default::default(),
            transaction_kv_store_write_config: Default::default(),
            authority_overload_config: Default::default(),
            run_with_range: self.run_with_range,
            jsonrpc_server_type: None,
            policy_config: self.policy_config,
            firewall_config: self.fw_config,
            execution_cache_config: ExecutionCacheConfig::default(),
            full_checkpoint_contents_cache_size_mb: default_full_checkpoint_contents_cache_size_mb(
            ),
            // This is a validator specific feature.
            enable_validator_tx_finalizer: false,
            // No effect on a fullnode (soft-locking runs only in the validator
            // submit path); kept at the default so the config mirrors production.
            enable_soft_locking: true,
            verifier_signing_config: VerifierSigningConfig::default(),
            enable_db_write_stall: None,
            iota_names_config: self.iota_names_config,
            enable_grpc_api: self.enable_grpc_api,
            grpc_api_config,
            chain_override_for_testing: self.chain_override,
            validator_client_monitor_config: None,
        })
    }

    /// Build the fullnode config against the given network config.
    ///
    /// # Panics
    ///
    /// Panics if [`Self::try_build`] returns an error.
    pub fn build<R: rand::RngCore + rand::CryptoRng>(
        self,
        rng: &mut R,
        network_config: &NetworkConfig,
    ) -> NodeConfig {
        self.try_build(rng, network_config)
            .unwrap_or_else(|err| panic!("{err:#}"))
    }

    /// Build the fullnode config against the given network config.
    ///
    /// Fails if a validator config has no `p2p-config.external-address`,
    /// which the fullnode's seed peers are derived from.
    ///
    /// # Panics
    ///
    /// Panics on failures the config cannot be built without: creating the
    /// temporary config directory, allocating a local port, or parsing a
    /// generated network address.
    pub fn try_build<R: rand::RngCore + rand::CryptoRng>(
        self,
        rng: &mut R,
        network_config: &NetworkConfig,
    ) -> anyhow::Result<NodeConfig> {
        let genesis = self
            .genesis
            .as_ref()
            .or_else(|| network_config.get_validator_genesis())
            .cloned()
            .unwrap_or_else(|| iota_config::node::Genesis::new(network_config.genesis.clone()));
        self.try_build_from_parts(rng, network_config.validator_configs(), genesis)
    }
}

/// Given a validator keypair, return a path that can be used to identify the
/// validator.
fn get_key_path(key_pair: &AuthorityKeyPair) -> String {
    let public_key: AuthorityPublicKeyBytes = key_pair.public().into();
    let mut key_path = Hex::encode(public_key);
    // 12 is rather arbitrary here but it's a nice balance between being short and
    // being unique.
    key_path.truncate(12);
    key_path
}

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;

    use super::{FullnodeConfigBuilder, Genesis};

    #[test]
    fn deterministic_ports_fill_the_three_slots_of_a_fullnode() {
        let config = FullnodeConfigBuilder::new()
            .with_deterministic_ports(9184)
            .build_from_parts(&mut OsRng, &[], Genesis::new_empty());

        assert_eq!(config.metrics_address.to_string(), "127.0.0.1:9184");
        assert_eq!(config.admin_interface_address.to_string(), "127.0.0.1:9185");
        assert_eq!(
            config.p2p_config.external_address.unwrap().to_string(),
            "/ip4/127.0.0.1/udp/9186/http"
        );
        assert_eq!(
            config.p2p_config.listen_address.to_string(),
            "127.0.0.1:9186"
        );
    }

    #[test]
    fn an_address_set_on_the_builder_wins_over_the_deterministic_ports() {
        let config = FullnodeConfigBuilder::new()
            .with_deterministic_ports(9184)
            .with_admin_interface_address(Some(([127, 0, 0, 1], 1337)))
            .build_from_parts(&mut OsRng, &[], Genesis::new_empty());

        assert_eq!(config.admin_interface_address.to_string(), "127.0.0.1:1337");
        assert_eq!(config.metrics_address.to_string(), "127.0.0.1:9184");
    }
}
