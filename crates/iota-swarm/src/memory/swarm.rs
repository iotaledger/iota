// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    net::SocketAddr,
    num::NonZeroUsize,
    ops,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use futures::future::try_join_all;
use iota_config::{
    ExecutionCacheConfig, IOTA_GENESIS_FILENAME, NodeConfig,
    node::{AuthorityOverloadConfig, GrpcApiConfig, RunWithRange},
    p2p::DiscoveryConfig,
    transaction_deny_config::TransactionDenyConfig,
};
use iota_macros::nondeterministic;
use iota_names::config::IotaNamesConfig;
use iota_node::IotaNodeHandle;
use iota_protocol_config::{Chain, ProtocolVersion};
use iota_swarm_config::{
    genesis_config::{AccountConfig, GenesisConfig, ValidatorGenesisConfig},
    network_config::NetworkConfig,
    network_config_builder::{
        CommitteeConfig, ConfigBuilder, GlobalStateHashV1EnabledConfig, ProtocolVersionsConfig,
        SupportedProtocolVersionsCallback,
    },
    node_config_builder::FullnodeConfigBuilder,
    node_config_override::{
        NodeConfigOverride, OverrideScope, apply_node_config_overrides,
        check_validator_override_scopes, overrides_for_fullnode, overrides_for_validator,
    },
};
use iota_types::{
    base_types::AuthorityName,
    object::Object,
    supported_protocol_versions::SupportedProtocolVersions,
    traffic_control::{PolicyConfig, RemoteFirewallConfig},
};
use rand::rngs::OsRng;
use tempfile::TempDir;
use tracing::info;

use super::Node;

pub struct SwarmBuilder<R = OsRng> {
    rng: R,
    // template: NodeConfig,
    dir: Option<PathBuf>,
    committee: CommitteeConfig,
    genesis_config: Option<GenesisConfig>,
    network_config: Option<NetworkConfig>,
    chain_override: Option<Chain>,
    additional_objects: Vec<Object>,
    fullnode_count: usize,
    fullnode_db_path: Option<PathBuf>,
    fullnode_rpc_port: Option<u16>,
    fullnode_rpc_addr: Option<SocketAddr>,
    supported_protocol_versions_config: ProtocolVersionsConfig,
    // Default to supported_protocol_versions_config, but can be overridden.
    fullnode_supported_protocol_versions_config: Option<ProtocolVersionsConfig>,
    num_unpruned_validators: Option<usize>,
    authority_overload_config: Option<AuthorityOverloadConfig>,
    transaction_deny_config: Option<TransactionDenyConfig>,
    execution_cache_config: Option<ExecutionCacheConfig>,
    data_ingestion_dir: Option<PathBuf>,
    fullnode_run_with_range: Option<RunWithRange>,
    validator_policy_config: Option<PolicyConfig>,
    fullnode_policy_config: Option<PolicyConfig>,
    fullnode_fw_config: Option<RemoteFirewallConfig>,
    max_submit_position: Option<usize>,
    submit_delay_step_override_millis: Option<u64>,
    global_state_hash_v1_enabled_config: GlobalStateHashV1EnabledConfig,
    disable_fullnode_pruning: bool,
    iota_names_config: Option<IotaNamesConfig>,
    fullnode_enable_grpc_api: bool,
    fullnode_grpc_api_config: Option<GrpcApiConfig>,
    disable_address_verification_cooldown: bool,
    deterministic_validator_port_base: Option<u16>,
    fullnode_genesis_config: Option<ValidatorGenesisConfig>,
    node_config_overrides: Vec<NodeConfigOverride>,
}

impl SwarmBuilder {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            rng: OsRng,
            dir: None,
            committee: CommitteeConfig::Size(NonZeroUsize::new(1).unwrap()),
            genesis_config: None,
            network_config: None,
            chain_override: None,
            additional_objects: vec![],
            fullnode_count: 0,
            fullnode_db_path: None,
            fullnode_rpc_port: None,
            fullnode_rpc_addr: None,
            supported_protocol_versions_config: ProtocolVersionsConfig::Default,
            fullnode_supported_protocol_versions_config: None,
            num_unpruned_validators: None,
            authority_overload_config: None,
            transaction_deny_config: None,
            execution_cache_config: None,
            data_ingestion_dir: None,
            fullnode_run_with_range: None,
            validator_policy_config: None,
            fullnode_policy_config: None,
            fullnode_fw_config: None,
            max_submit_position: None,
            submit_delay_step_override_millis: None,
            global_state_hash_v1_enabled_config: GlobalStateHashV1EnabledConfig::Global(true),
            disable_fullnode_pruning: false,
            iota_names_config: None,
            fullnode_enable_grpc_api: false,
            fullnode_grpc_api_config: None,
            disable_address_verification_cooldown: false,
            deterministic_validator_port_base: None,
            fullnode_genesis_config: None,
            node_config_overrides: vec![],
        }
    }
}

impl<R> SwarmBuilder<R> {
    pub fn rng<N: rand::RngCore + rand::CryptoRng>(self, rng: N) -> SwarmBuilder<N> {
        SwarmBuilder {
            rng,
            dir: self.dir,
            committee: self.committee,
            genesis_config: self.genesis_config,
            network_config: self.network_config,
            chain_override: self.chain_override,
            additional_objects: self.additional_objects,
            fullnode_count: self.fullnode_count,
            fullnode_db_path: self.fullnode_db_path,
            fullnode_rpc_port: self.fullnode_rpc_port,
            fullnode_rpc_addr: self.fullnode_rpc_addr,
            supported_protocol_versions_config: self.supported_protocol_versions_config,
            fullnode_supported_protocol_versions_config: self
                .fullnode_supported_protocol_versions_config,
            num_unpruned_validators: self.num_unpruned_validators,
            authority_overload_config: self.authority_overload_config,
            transaction_deny_config: self.transaction_deny_config,
            execution_cache_config: self.execution_cache_config,
            data_ingestion_dir: self.data_ingestion_dir,
            fullnode_run_with_range: self.fullnode_run_with_range,
            validator_policy_config: self.validator_policy_config,
            fullnode_policy_config: self.fullnode_policy_config,
            fullnode_fw_config: self.fullnode_fw_config,
            max_submit_position: self.max_submit_position,
            submit_delay_step_override_millis: self.submit_delay_step_override_millis,
            global_state_hash_v1_enabled_config: self.global_state_hash_v1_enabled_config,
            disable_fullnode_pruning: self.disable_fullnode_pruning,
            iota_names_config: self.iota_names_config,
            fullnode_enable_grpc_api: self.fullnode_enable_grpc_api,
            fullnode_grpc_api_config: self.fullnode_grpc_api_config,
            disable_address_verification_cooldown: self.disable_address_verification_cooldown,
            deterministic_validator_port_base: self.deterministic_validator_port_base,
            fullnode_genesis_config: self.fullnode_genesis_config,
            node_config_overrides: self.node_config_overrides,
        }
    }

    /// Set the directory that should be used by the Swarm for any on-disk data.
    ///
    /// If a directory is provided, it will not be cleaned up when the Swarm is
    /// dropped.
    ///
    /// Defaults to using a temporary directory that will be cleaned up when the
    /// Swarm is dropped.
    pub fn dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.dir = Some(dir.into());
        self
    }

    /// Set the committee size (the number of validators in the validator set).
    ///
    /// Defaults to 1.
    pub fn committee_size(mut self, committee_size: NonZeroUsize) -> Self {
        self.committee = CommitteeConfig::Size(committee_size);
        self
    }

    pub fn with_validators(mut self, validators: Vec<ValidatorGenesisConfig>) -> Self {
        self.committee = CommitteeConfig::Validators(validators);
        self
    }

    /// Lay the generated validators out as
    /// [`ConfigBuilder::with_deterministic_ports`] describes.
    ///
    /// Has no effect when the validators come from a network config or from
    /// `with_validators`.
    pub fn with_deterministic_validator_ports(mut self, port_base: u16) -> Self {
        self.deterministic_validator_port_base = Some(port_base);
        self
    }

    /// Take the first fullnode's key pairs and addresses from
    /// `fullnode_genesis_config` instead of generating them. This gives it the
    /// same config, and the same db path, on every build.
    ///
    /// Further fullnodes keep generated key pairs and addresses, since an
    /// address can only be used once.
    pub fn with_fullnode_genesis_config(
        mut self,
        fullnode_genesis_config: ValidatorGenesisConfig,
    ) -> Self {
        self.fullnode_genesis_config = Some(fullnode_genesis_config);
        self
    }

    pub fn with_genesis_config(mut self, genesis_config: GenesisConfig) -> Self {
        assert!(self.network_config.is_none() && self.genesis_config.is_none());
        self.genesis_config = Some(genesis_config);
        self
    }

    pub fn with_chain_override(mut self, chain: Chain) -> Self {
        assert!(self.chain_override.is_none());
        self.chain_override = Some(chain);
        self
    }

    pub fn with_num_unpruned_validators(mut self, n: usize) -> Self {
        assert!(self.network_config.is_none());
        self.num_unpruned_validators = Some(n);
        self
    }

    pub fn with_network_config(mut self, network_config: NetworkConfig) -> Self {
        assert!(self.network_config.is_none() && self.genesis_config.is_none());
        self.network_config = Some(network_config);
        self
    }

    pub fn with_accounts(mut self, accounts: Vec<AccountConfig>) -> Self {
        self.get_or_init_genesis_config().accounts = accounts;
        self
    }

    pub fn with_objects<I: IntoIterator<Item = Object>>(mut self, objects: I) -> Self {
        self.additional_objects.extend(objects);
        self
    }

    pub fn with_fullnode_count(mut self, fullnode_count: usize) -> Self {
        self.fullnode_count = fullnode_count;
        self
    }

    pub fn with_fullnode_db_path(mut self, fullnode_db_path: PathBuf) -> Self {
        self.fullnode_db_path = Some(fullnode_db_path);
        self
    }

    pub fn with_fullnode_rpc_port(mut self, fullnode_rpc_port: u16) -> Self {
        assert!(self.fullnode_rpc_addr.is_none());
        self.fullnode_rpc_port = Some(fullnode_rpc_port);
        self
    }

    pub fn with_fullnode_rpc_addr(mut self, fullnode_rpc_addr: SocketAddr) -> Self {
        assert!(self.fullnode_rpc_port.is_none());
        self.fullnode_rpc_addr = Some(fullnode_rpc_addr);
        self
    }

    pub fn with_epoch_duration_ms(mut self, epoch_duration_ms: u64) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .epoch_duration_ms = epoch_duration_ms;
        self
    }

    pub fn with_protocol_version(mut self, v: ProtocolVersion) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .protocol_version = v;
        self
    }

    pub fn with_supported_protocol_versions(mut self, c: SupportedProtocolVersions) -> Self {
        self.supported_protocol_versions_config = ProtocolVersionsConfig::Global(c);
        self
    }

    pub fn with_supported_protocol_version_callback(
        mut self,
        func: SupportedProtocolVersionsCallback,
    ) -> Self {
        self.supported_protocol_versions_config = ProtocolVersionsConfig::PerValidator(func);
        self
    }

    pub fn with_supported_protocol_versions_config(mut self, c: ProtocolVersionsConfig) -> Self {
        self.supported_protocol_versions_config = c;
        self
    }

    pub fn with_global_state_hash_v1_enabled_config(
        mut self,
        c: GlobalStateHashV1EnabledConfig,
    ) -> Self {
        self.global_state_hash_v1_enabled_config = c;
        self
    }

    pub fn with_fullnode_supported_protocol_versions_config(
        mut self,
        c: ProtocolVersionsConfig,
    ) -> Self {
        self.fullnode_supported_protocol_versions_config = Some(c);
        self
    }

    pub fn with_authority_overload_config(
        mut self,
        authority_overload_config: AuthorityOverloadConfig,
    ) -> Self {
        assert!(self.network_config.is_none());
        self.authority_overload_config = Some(authority_overload_config);
        self
    }

    pub fn with_transaction_deny_config(
        mut self,
        transaction_deny_config: TransactionDenyConfig,
    ) -> Self {
        assert!(self.network_config.is_none());
        self.transaction_deny_config = Some(transaction_deny_config);
        self
    }

    pub fn with_execution_cache_config(
        mut self,
        execution_cache_config: ExecutionCacheConfig,
    ) -> Self {
        self.execution_cache_config = Some(execution_cache_config);
        self
    }

    pub fn with_data_ingestion_dir(mut self, path: PathBuf) -> Self {
        self.data_ingestion_dir = Some(path);
        self
    }

    pub fn with_fullnode_run_with_range(mut self, run_with_range: Option<RunWithRange>) -> Self {
        if let Some(run_with_range) = run_with_range {
            self.fullnode_run_with_range = Some(run_with_range);
        }
        self
    }

    /// Set the traffic control policy of every validator, whether the
    /// committee is generated here or taken from a network config.
    pub fn with_validator_policy_config(mut self, config: Option<PolicyConfig>) -> Self {
        self.validator_policy_config = config;
        self
    }

    pub fn with_fullnode_policy_config(mut self, config: Option<PolicyConfig>) -> Self {
        self.fullnode_policy_config = config;
        self
    }

    pub fn with_fullnode_fw_config(mut self, config: Option<RemoteFirewallConfig>) -> Self {
        self.fullnode_fw_config = config;
        self
    }

    pub fn with_fullnode_enable_grpc_api(mut self, enable: bool) -> Self {
        self.fullnode_enable_grpc_api = enable;
        self
    }

    pub fn with_fullnode_grpc_api_config(mut self, config: GrpcApiConfig) -> Self {
        self.fullnode_grpc_api_config = Some(config);
        self
    }

    fn get_or_init_genesis_config(&mut self) -> &mut GenesisConfig {
        if self.genesis_config.is_none() {
            assert!(self.network_config.is_none());
            self.genesis_config = Some(GenesisConfig::for_local_testing());
        }
        self.genesis_config.as_mut().unwrap()
    }

    pub fn with_max_submit_position(mut self, max_submit_position: usize) -> Self {
        self.max_submit_position = Some(max_submit_position);
        self
    }

    pub fn with_disable_fullnode_pruning(mut self) -> Self {
        self.disable_fullnode_pruning = true;
        self
    }

    pub fn with_submit_delay_step_override_millis(
        mut self,
        submit_delay_step_override_millis: u64,
    ) -> Self {
        self.submit_delay_step_override_millis = Some(submit_delay_step_override_millis);
        self
    }

    pub fn with_iota_names_config(mut self, iota_names_config: IotaNamesConfig) -> Self {
        self.iota_names_config = Some(iota_names_config);
        self
    }

    /// Disable address verification cooldown for test environments where nodes
    /// frequently restart. This prevents nodes from being blocked from
    /// reconnecting after crashes/restarts.
    pub fn with_disabled_address_verification_cooldown(mut self) -> Self {
        self.disable_address_verification_cooldown = true;
        self
    }

    /// Set overrides applied to every node config this builder produces, in
    /// the given order, after all other configuration. Nodes spawned on the
    /// built [`Swarm`] later get them too, except `validator-<N>` scoped
    /// overrides, which refer to positions in the initial network config.
    pub fn with_node_config_overrides(
        mut self,
        node_config_overrides: Vec<NodeConfigOverride>,
    ) -> Self {
        self.node_config_overrides = node_config_overrides;
        self
    }
}

impl<R: rand::RngCore + rand::CryptoRng> SwarmBuilder<R> {
    /// Create the configured Swarm.
    ///
    /// # Panics
    ///
    /// Panics if [`SwarmBuilder::try_build`] returns an error.
    pub fn build(self) -> Swarm {
        self.try_build().unwrap_or_else(|err| panic!("{err:#}"))
    }

    /// Create the configured Swarm.
    ///
    /// # Errors
    ///
    /// - A `validator-<N>` override names a validator the network does not
    ///   have.
    /// - An override fails to apply to a built config.
    /// - The network has a fullnode and a validator config has no
    ///   `p2p-config.external-address`.
    ///
    /// # Panics
    ///
    /// Panics on failures the swarm cannot run without: creating its temporary
    /// directory, saving the genesis blob, parsing a generated network address,
    /// and building the genesis (e.g. on invalid genesis parameters or a
    /// validator below the minimum stake).
    pub fn try_build(mut self) -> Result<Swarm> {
        let mut fullnode_genesis_config = self.fullnode_genesis_config.take();
        let dir = if let Some(dir) = self.dir {
            SwarmDirectory::Persistent(dir)
        } else {
            SwarmDirectory::new_temporary()
        };

        let ingest_data = self.data_ingestion_dir.clone();

        let mut network_config = self.network_config.unwrap_or_else(|| {
            let mut config_builder = ConfigBuilder::new(dir.as_ref());

            if let Some(genesis_config) = self.genesis_config {
                config_builder = config_builder.with_genesis_config(genesis_config);
            }

            if let Some(chain_override) = self.chain_override {
                config_builder = config_builder.with_chain_override(chain_override);
            }

            if let Some(num_unpruned_validators) = self.num_unpruned_validators {
                config_builder =
                    config_builder.with_num_unpruned_validators(num_unpruned_validators);
            }

            if let Some(authority_overload_config) = self.authority_overload_config {
                config_builder =
                    config_builder.with_authority_overload_config(authority_overload_config);
            }

            if let Some(transaction_deny_config) = self.transaction_deny_config {
                config_builder =
                    config_builder.with_transaction_deny_config(transaction_deny_config);
            }

            if let Some(execution_cache_config) = self.execution_cache_config {
                config_builder = config_builder.with_execution_cache_config(execution_cache_config);
            }

            if let Some(path) = self.data_ingestion_dir {
                config_builder = config_builder.with_data_ingestion_dir(path);
            }

            if let Some(port_base) = self.deterministic_validator_port_base {
                config_builder = config_builder.with_deterministic_ports(port_base);
            }

            if let Some(max_submit_position) = self.max_submit_position {
                config_builder = config_builder.with_max_submit_position(max_submit_position);
            }

            if let Some(submit_delay_step_override_millis) = self.submit_delay_step_override_millis
            {
                config_builder = config_builder
                    .with_submit_delay_step_override_millis(submit_delay_step_override_millis);
            }

            let mut network_config = config_builder
                .committee(self.committee)
                .rng(self.rng)
                .with_objects(self.additional_objects)
                .with_empty_validator_genesis()
                .with_supported_protocol_versions_config(
                    self.supported_protocol_versions_config.clone(),
                )
                .with_global_state_hash_v1_enabled_config(
                    self.global_state_hash_v1_enabled_config.clone(),
                )
                .build();
            // Populate validator genesis by pointing to the blob
            let genesis_path = dir.join(IOTA_GENESIS_FILENAME);
            network_config
                .genesis
                .save(&genesis_path)
                .expect("genesis should be saved successfully");
            for validator in &mut network_config.validator_configs {
                validator.genesis = iota_config::node::Genesis::new_from_file(&genesis_path);
            }
            network_config
        });

        if let Some(policy_config) = self.validator_policy_config {
            for validator in &mut network_config.validator_configs {
                validator.policy_config = Some(policy_config.clone());
            }
        }

        if self.disable_address_verification_cooldown {
            for validator in &mut network_config.validator_configs {
                if let Some(ref mut discovery_config) = validator.p2p_config.discovery {
                    discovery_config.address_verification_failure_cooldown_sec = Some(0);
                } else {
                    validator.p2p_config.discovery = Some(DiscoveryConfig {
                        address_verification_failure_cooldown_sec: Some(0),
                        ..Default::default()
                    });
                }
            }
        }

        check_validator_override_scopes(
            &self.node_config_overrides,
            network_config.validator_configs.len(),
        )?;
        for (index, validator) in network_config.validator_configs.iter_mut().enumerate() {
            apply_node_config_overrides(
                overrides_for_validator(&self.node_config_overrides, index),
                validator,
            )
            .with_context(|| {
                format!("failed to apply node config overrides to validator {index}")
            })?;
        }

        let mut nodes: HashMap<_, _> = network_config
            .validator_configs()
            .iter()
            .map(|config| {
                info!(
                    "SwarmBuilder configuring validator with name {}",
                    config.authority_public_key()
                );
                (config.authority_public_key(), Node::new(config.to_owned()))
            })
            .collect();

        let mut fullnode_config_builder = FullnodeConfigBuilder::new()
            .with_config_directory(dir.as_ref().into())
            .with_run_with_range(self.fullnode_run_with_range)
            .with_policy_config(self.fullnode_policy_config)
            .with_data_ingestion_dir(ingest_data)
            .with_fw_config(self.fullnode_fw_config)
            .with_disable_pruning(self.disable_fullnode_pruning)
            .with_iota_names_config(self.iota_names_config);
        if let Some(fullnode_db_path) = self.fullnode_db_path {
            fullnode_config_builder = fullnode_config_builder.with_db_path(fullnode_db_path);
        }

        if self.disable_address_verification_cooldown {
            let discovery_config = DiscoveryConfig {
                address_verification_failure_cooldown_sec: Some(0),
                ..Default::default()
            };

            fullnode_config_builder =
                fullnode_config_builder.with_discovery_config(discovery_config);
        }

        if let Some(chain) = self.chain_override {
            fullnode_config_builder = fullnode_config_builder.with_chain_override(chain);
        }

        if let Some(spvc) = &self.fullnode_supported_protocol_versions_config {
            let supported_versions = match spvc {
                ProtocolVersionsConfig::Default => SupportedProtocolVersions::SYSTEM_DEFAULT,
                ProtocolVersionsConfig::Global(v) => *v,
                ProtocolVersionsConfig::PerValidator(func) => func(0, None),
            };
            fullnode_config_builder =
                fullnode_config_builder.with_supported_protocol_versions(supported_versions);
        }

        // Add gRPC config wiring
        fullnode_config_builder =
            fullnode_config_builder.with_enable_grpc_api(self.fullnode_enable_grpc_api);
        if let Some(grpc_config) = &self.fullnode_grpc_api_config {
            fullnode_config_builder =
                fullnode_config_builder.with_grpc_api_config(grpc_config.clone());
        }

        for idx in 0..self.fullnode_count {
            let mut builder = fullnode_config_builder.clone();
            // Only the first fullnode is used as the rpc fullnode, and only it
            // takes the given genesis config: an address can only be used once.
            let genesis_config = if idx == 0 {
                if let Some(rpc_addr) = self.fullnode_rpc_addr {
                    builder = builder.with_rpc_addr(rpc_addr);
                }
                if let Some(rpc_port) = self.fullnode_rpc_port {
                    builder = builder.with_rpc_port(rpc_port);
                }
                fullnode_genesis_config.take()
            } else {
                None
            };
            let mut config = match genesis_config {
                Some(genesis_config) => {
                    builder.try_build_with_genesis_config(genesis_config, &network_config)
                }
                None => builder.try_build(&mut OsRng, &network_config),
            }
            .context("failed to build the fullnode config")?;
            apply_node_config_overrides(
                overrides_for_fullnode(&self.node_config_overrides),
                &mut config,
            )
            .with_context(|| format!("failed to apply node config overrides to fullnode {idx}"))?;
            info!(
                "SwarmBuilder configuring full node with name {}",
                config.authority_public_key()
            );
            nodes.insert(config.authority_public_key(), Node::new(config));
        }
        Ok(Swarm {
            dir,
            network_config,
            nodes,
            fullnode_config_builder,
            node_config_overrides: self.node_config_overrides,
        })
    }
}

/// A handle to an in-memory IOTA Network.
#[derive(Debug)]
pub struct Swarm {
    dir: SwarmDirectory,
    network_config: NetworkConfig,
    nodes: HashMap<AuthorityName, Node>,
    // Save a copy of the fullnode config builder to build future fullnodes.
    fullnode_config_builder: FullnodeConfigBuilder,
    // Applied to the configs of nodes spawned after the initial build too.
    node_config_overrides: Vec<NodeConfigOverride>,
}

impl Drop for Swarm {
    fn drop(&mut self) {
        self.nodes_iter_mut().for_each(|node| node.stop());
    }
}

impl Swarm {
    fn nodes_iter_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.values_mut()
    }

    /// Return a new Builder
    pub fn builder() -> SwarmBuilder {
        SwarmBuilder::new()
    }

    /// Start all nodes associated with this Swarm
    pub async fn launch(&mut self) -> Result<()> {
        try_join_all(self.nodes_iter_mut().map(|node| node.start())).await?;
        tracing::info!("Successfully launched Swarm");
        Ok(())
    }

    /// Return the path to the directory where this Swarm's on-disk data is
    /// kept.
    pub fn dir(&self) -> &Path {
        self.dir.as_ref()
    }

    /// Return a reference to this Swarm's `NetworkConfig`.
    pub fn config(&self) -> &NetworkConfig {
        &self.network_config
    }

    /// Return a mutable reference to this Swarm's `NetworkConfig`.
    // TODO: It's not ideal to mutate network config. We should consider removing
    // this.
    pub fn config_mut(&mut self) -> &mut NetworkConfig {
        &mut self.network_config
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    pub fn node(&self, name: &AuthorityName) -> Option<&Node> {
        self.nodes.get(name)
    }

    pub fn node_mut(&mut self, name: &AuthorityName) -> Option<&mut Node> {
        self.nodes.get_mut(name)
    }

    /// Return an iterator over shared references of all nodes that are set up
    /// as validators. This means that they have a consensus config. This
    /// however doesn't mean this validator is currently active (i.e. it's
    /// not necessarily in the validator set at the moment).
    pub fn validator_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .values()
            .filter(|node| node.config().is_validator())
    }

    pub fn validator_node_handles(&self) -> Vec<IotaNodeHandle> {
        self.validator_nodes()
            .map(|node| node.get_node_handle().unwrap())
            .collect()
    }

    /// Returns an iterator over all current active validators.
    pub fn active_validators(&self) -> impl Iterator<Item = &Node> {
        self.validator_nodes().filter(|node| {
            node.get_node_handle().is_some_and(|handle| {
                let state = handle.state();
                state.is_active_validator(&state.epoch_store_for_testing())
            })
        })
    }

    /// Returns an iterator over all current active validators.
    pub fn committee_validators(&self) -> impl Iterator<Item = &Node> {
        self.validator_nodes().filter(|node| {
            node.get_node_handle().is_some_and(|handle| {
                let state = handle.state();
                state.is_committee_validator(&state.epoch_store_for_testing())
            })
        })
    }

    /// Return an iterator over shared references of all Fullnodes.
    pub fn fullnodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .values()
            .filter(|node| !node.config().is_validator())
    }

    /// Start a node from `config` and add it to the swarm.
    ///
    /// The swarm's node config overrides are applied to the config first.
    ///
    /// # Panics
    ///
    /// Panics on an override that fails to apply and on a node that fails
    /// to start.
    pub async fn spawn_new_node(&mut self, mut config: NodeConfig) -> IotaNodeHandle {
        self.apply_node_config_overrides_for_spawn(&mut config);
        let name = config.authority_public_key();
        let node = Node::new(config);
        node.start().await.unwrap();
        let handle = node.get_node_handle().unwrap();
        self.nodes.insert(name, node);
        handle
    }

    /// Apply the swarm's overrides to the config of a node spawned after the
    /// initial build. `validator-<N>` scoped overrides refer to positions in
    /// the initial network config, so they are skipped here.
    ///
    /// # Panics
    ///
    /// Panics on an override that fails to apply.
    fn apply_node_config_overrides_for_spawn(&self, config: &mut NodeConfig) {
        let overrides: Vec<&NodeConfigOverride> = if config.is_validator() {
            self.node_config_overrides
                .iter()
                .filter(|config_override| {
                    matches!(
                        config_override.scope,
                        OverrideScope::All | OverrideScope::AllValidators
                    )
                })
                .collect()
        } else {
            overrides_for_fullnode(&self.node_config_overrides).collect()
        };
        apply_node_config_overrides(overrides, config).unwrap_or_else(|err| panic!("{err:#}"));
    }

    pub fn get_fullnode_config_builder(&self) -> FullnodeConfigBuilder {
        self.fullnode_config_builder.clone()
    }

    /// The node config overrides the swarm was built with.
    pub fn node_config_overrides(&self) -> &[NodeConfigOverride] {
        &self.node_config_overrides
    }
}

#[derive(Debug)]
enum SwarmDirectory {
    Persistent(PathBuf),
    Temporary(TempDir),
}

impl SwarmDirectory {
    fn new_temporary() -> Self {
        SwarmDirectory::Temporary(nondeterministic!(TempDir::new().unwrap()))
    }
}

impl ops::Deref for SwarmDirectory {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        match self {
            SwarmDirectory::Persistent(dir) => dir.deref(),
            SwarmDirectory::Temporary(dir) => dir.path(),
        }
    }
}

impl AsRef<Path> for SwarmDirectory {
    fn as_ref(&self) -> &Path {
        match self {
            SwarmDirectory::Persistent(dir) => dir.as_ref(),
            SwarmDirectory::Temporary(dir) => dir.as_ref(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::BTreeSet, num::NonZeroUsize};

    use iota_swarm_config::{
        genesis_config::ValidatorGenesisConfigBuilder,
        network_config::NetworkConfig,
        network_config_builder::ConfigBuilder,
        node_config_override::{NodeConfigOverride, apply_node_config_overrides},
    };
    use iota_types::traffic_control::PolicyConfig;

    use super::Swarm;

    #[test]
    fn the_validator_policy_config_applies_before_the_overrides() {
        let policy_config = PolicyConfig {
            channel_capacity: 4242,
            ..PolicyConfig::default()
        };
        let swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(2).unwrap())
            .with_validator_policy_config(Some(policy_config))
            .with_node_config_overrides(vec!["validator-0:policy-config=".parse().unwrap()])
            .build();

        let validators = swarm.config().validator_configs();
        assert!(validators[0].policy_config.is_none());
        assert_eq!(
            validators[1]
                .policy_config
                .as_ref()
                .unwrap()
                .channel_capacity,
            4242
        );
    }

    #[test]
    fn node_config_overrides() {
        let swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(2).unwrap())
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                "fullnode:authority-store-pruning-config.num-epochs-to-retain=18446744073709551615"
                    .parse()
                    .unwrap(),
                "validator-0:authority-store-pruning-config.num-epochs-to-retain=5"
                    .parse()
                    .unwrap(),
                "validator:enable-soft-locking=false".parse().unwrap(),
            ])
            .build();

        let validators = swarm.config().validator_configs();
        assert_eq!(
            validators[0]
                .authority_store_pruning_config
                .num_epochs_to_retain,
            5
        );
        assert_eq!(
            validators[1]
                .authority_store_pruning_config
                .num_epochs_to_retain,
            0
        );
        assert!(validators.iter().all(|config| !config.enable_soft_locking));

        let fullnode = swarm.fullnodes().next().unwrap();
        assert_eq!(
            fullnode
                .config()
                .authority_store_pruning_config
                .num_epochs_to_retain,
            u64::MAX
        );
        assert!(fullnode.config().enable_soft_locking);
    }

    #[test]
    fn node_config_overrides_apply_to_late_spawned_nodes() {
        let swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(2).unwrap())
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                "fullnode:authority-store-pruning-config.num-epochs-to-retain=18446744073709551615"
                    .parse()
                    .unwrap(),
                "validator:enable-soft-locking=false".parse().unwrap(),
                "validator-0:enable-index-processing=false".parse().unwrap(),
            ])
            .build();

        let mut config = swarm
            .get_fullnode_config_builder()
            .build(&mut rand::rngs::OsRng, swarm.config());
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            0
        );
        swarm.apply_node_config_overrides_for_spawn(&mut config);
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            u64::MAX
        );
        // Validator-scoped overrides do not apply to a fullnode.
        assert!(config.enable_soft_locking);

        // A validator respawned from its own config: the batch it was built
        // with applies again unchanged.
        let mut config = swarm.config().validator_configs()[1].clone();
        let num_epochs_to_retain = config.authority_store_pruning_config.num_epochs_to_retain;
        assert!(!config.enable_soft_locking);
        swarm.apply_node_config_overrides_for_spawn(&mut config);
        assert!(!config.enable_soft_locking);
        // The `validator-0` and fullnode scopes leave this validator alone.
        assert!(config.enable_index_processing);
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            num_epochs_to_retain
        );
    }

    #[test]
    fn try_build_rejects_a_consensus_override_that_reaches_the_fullnode() {
        // `all:` applies cleanly to the validators and must still fail on
        // the fullnode, which has no consensus config.
        let err = Swarm::builder()
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                "all:consensus-config.db-retention-epochs=2"
                    .parse()
                    .unwrap(),
            ])
            .try_build()
            .unwrap_err();
        let err = format!("{err:#}");
        assert!(
            err.contains("all:consensus-config.db-retention-epochs"),
            "{err}"
        );
        assert!(err.contains("on a fullnode"), "{err}");
    }

    #[test]
    fn the_fullnodes_own_addresses_are_overridable() {
        // A fullnode is not a committee member, so the addresses that are
        // genesis data on a validator are ordinary config on it. The seed
        // peers it derives from the validators are unaffected.
        let swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(1).unwrap())
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                "fullnode:p2p-config.external-address='/ip4/127.0.0.1/udp/19186'"
                    .parse()
                    .unwrap(),
            ])
            .try_build()
            .unwrap();
        let fullnode = swarm.fullnodes().next().unwrap();
        let config = fullnode.config();
        assert_eq!(
            config
                .p2p_config
                .external_address
                .as_ref()
                .unwrap()
                .to_string(),
            "/ip4/127.0.0.1/udp/19186"
        );
        assert_eq!(
            config.p2p_config.seed_peers[0].address,
            swarm.config().validator_configs()[0]
                .p2p_config
                .external_address
                .clone()
                .unwrap()
        );
    }

    /// A network config whose validator 0 carries a firewall section, which
    /// its peers do not. Returns the config and the temporary directory it
    /// must outlive.
    fn network_config_with_a_firewall_on_validator_0(
        committee_size: usize,
    ) -> (NetworkConfig, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let mut network_config = ConfigBuilder::new(dir.path())
            .committee_size(NonZeroUsize::new(committee_size).unwrap())
            .build();
        let overrides: Vec<NodeConfigOverride> = [
            "policy-config={}",
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut network_config.validator_configs[0]).unwrap();
        (network_config, dir)
    }

    #[test]
    fn validator_override_failures_name_the_validator() {
        // A `validator:` scope carries no index, so only the error context
        // can say which validator rejected the override.
        let (network_config, _dir) = network_config_with_a_firewall_on_validator_0(2);

        let err = Swarm::builder()
            .with_network_config(network_config)
            .with_node_config_overrides(vec![
                "validator:firewall-config.destination-port=65001"
                    .parse()
                    .unwrap(),
            ])
            .try_build()
            .unwrap_err();
        // Validator 0 has the section, validator 1 does not. The dotted
        // edit therefore leaves its required fields unset on validator 1.
        let err = format!("{err:#}");
        assert!(err.contains("validator 1"), "{err}");
        assert!(err.contains("remote-fw-url"), "{err}");
    }

    #[test]
    fn try_build_rejects_an_out_of_range_validator_scope_for_a_supplied_network_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let network_config = ConfigBuilder::new(dir.path())
            .committee_size(NonZeroUsize::new(1).unwrap())
            .build();
        let err = Swarm::builder()
            .with_network_config(network_config)
            .with_node_config_overrides(vec![
                "validator-1:enable-soft-locking=false".parse().unwrap(),
            ])
            .try_build()
            .unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("validator-1:enable-soft-locking"), "{err}");
        assert!(err.contains("only 1 validator"), "{err}");
    }

    #[test]
    fn validator_scopes_may_set_the_consensus_config() {
        let swarm = Swarm::builder()
            .with_node_config_overrides(vec![
                "validator:consensus-config.db-retention-epochs=2"
                    .parse()
                    .unwrap(),
                "validator-0:consensus-config.db-pruner-period-secs=60"
                    .parse()
                    .unwrap(),
            ])
            .try_build()
            .unwrap();
        let consensus_config = swarm.config().validator_configs()[0]
            .consensus_config
            .as_ref()
            .unwrap();
        assert_eq!(consensus_config.db_retention_epochs, Some(2));
        assert_eq!(consensus_config.db_pruner_period_secs, Some(60));
    }

    #[test]
    fn overrides_apply_to_a_supplied_network_config() {
        // The localnet feeds a network config loaded from disk. Overrides
        // apply to those configs, not to freshly generated ones.
        let (network_config, _dir) = network_config_with_a_firewall_on_validator_0(1);

        let swarm = Swarm::builder()
            .with_network_config(network_config)
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                "validator:firewall-config.destination-port=65001"
                    .parse()
                    .unwrap(),
                "fullnode:enable-index-processing=false".parse().unwrap(),
            ])
            .try_build()
            .unwrap();
        assert_eq!(
            swarm.config().validator_configs()[0]
                .firewall_config
                .as_ref()
                .unwrap()
                .destination_port,
            65001
        );
        let fullnode = swarm.fullnodes().next().unwrap();
        assert!(!fullnode.config().enable_index_processing);
    }

    #[test]
    fn try_build_rejects_a_fullnode_override_no_node_could_start_with() {
        let err = Swarm::builder()
            .committee_size(NonZeroUsize::new(1).unwrap())
            .with_fullnode_count(1)
            .with_node_config_overrides(vec![
                // A snapshot store without a backend: the node refuses to
                // start with it.
                "fullnode:state-snapshot-write-config.object-store-config.directory=/tmp/snapshots"
                    .parse()
                    .unwrap(),
            ])
            .try_build()
            .unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("storage backend"), "{err}");
    }

    #[test]
    fn try_build_fails_when_a_validator_has_no_p2p_external_address() {
        // The fullnode derives its seed peers from the validators' external
        // addresses.
        let dir = tempfile::TempDir::new().unwrap();
        let mut network_config = ConfigBuilder::new(dir.path())
            .committee_size(NonZeroUsize::new(1).unwrap())
            .build();
        network_config.validator_configs[0]
            .p2p_config
            .external_address = None;

        let err = Swarm::builder()
            .with_network_config(network_config)
            .with_fullnode_count(1)
            .try_build()
            .unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("validator 0"), "{err}");
        assert!(err.contains("seed peers"), "{err}");
    }

    #[tokio::test]
    async fn launch() {
        telemetry_subscribers::init_for_testing();
        let mut swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(4).unwrap())
            .with_fullnode_count(1)
            .build();

        swarm.launch().await.unwrap();

        for validator in swarm.validator_nodes() {
            validator.health_check(true).await.unwrap();
        }

        for fullnode in swarm.fullnodes() {
            fullnode.health_check(false).await.unwrap();
        }

        println!("hello");
    }

    #[test]
    fn deterministic_ports_reach_the_node_configs() {
        let swarm = Swarm::builder()
            .committee_size(NonZeroUsize::new(2).unwrap())
            .with_deterministic_validator_ports(9200)
            .build();

        let validator_ports = swarm
            .validator_nodes()
            .map(|validator| {
                validator
                    .config()
                    .network_address
                    .to_socket_addr()
                    .unwrap()
                    .port()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(validator_ports, BTreeSet::from([9200, 9210]));
    }

    #[test]
    fn the_first_fullnode_takes_the_given_genesis_config() {
        let mut fullnode_genesis_config = ValidatorGenesisConfigBuilder::new()
            .with_ip("127.0.0.1".to_owned())
            .build(&mut rand::rngs::OsRng);
        fullnode_genesis_config.metrics_address = ([127, 0, 0, 1], 19184).into();
        fullnode_genesis_config.admin_interface_address = ([127, 0, 0, 1], 19185).into();
        fullnode_genesis_config.p2p_address = "/ip4/127.0.0.1/udp/19186/http".parse().unwrap();
        let db_path_of = |swarm: &Swarm| swarm.fullnodes().next().unwrap().config().db_path.clone();

        let swarm = Swarm::builder()
            .with_fullnode_count(1)
            .with_fullnode_genesis_config(fullnode_genesis_config.copy_with_private_keys())
            .build();

        {
            let fullnode = swarm.fullnodes().next().unwrap().config();
            assert_eq!(fullnode.metrics_address.to_string(), "127.0.0.1:19184");
            assert_eq!(
                fullnode.admin_interface_address.to_string(),
                "127.0.0.1:19185"
            );
            assert_eq!(
                fullnode.p2p_config.listen_address.to_string(),
                "127.0.0.1:19186"
            );
        }

        // The same entry gives the fullnode the same db path in a second
        // network, which is what lets a persisted network reuse its database.
        let same_swarm = Swarm::builder()
            .with_fullnode_count(1)
            .with_fullnode_genesis_config(fullnode_genesis_config)
            .build();
        assert_eq!(
            db_path_of(&swarm).file_name(),
            db_path_of(&same_swarm).file_name()
        );
    }
}
