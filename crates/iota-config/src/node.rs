// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use consensus_config::Parameters as ConsensusParameters;
use iota_keys::keypair_file::{read_authority_keypair_from_file, read_keypair_from_file};
use iota_types::{
    base_types::IotaAddress,
    committee::EpochId,
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthorityPublicKeyBytes, IotaKeyPair, KeypairTraits,
        NetworkKeyPair, get_key_pair_from_rng,
    },
    messages_checkpoint::CheckpointSequenceNumber,
    multiaddr::Multiaddr,
    supported_protocol_versions::{Chain, SupportedProtocolVersions},
    traffic_control::{PolicyConfig, RemoteFirewallConfig},
};
use once_cell::sync::OnceCell;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::info;

use crate::{
    Config, certificate_deny_config::CertificateDenyConfig, genesis,
    migration_tx_data::MigrationTxData, object_storage_config::ObjectStoreConfig, p2p::P2pConfig,
    transaction_deny_config::TransactionDenyConfig,
};

// Default max number of concurrent requests served
pub const DEFAULT_GRPC_CONCURRENCY_LIMIT: usize = 20000000000;

/// Default gas price of 1000 Nanos
pub const DEFAULT_VALIDATOR_GAS_PRICE: u64 = iota_types::transaction::DEFAULT_VALIDATOR_GAS_PRICE;

/// Default commission rate of 2%
pub const DEFAULT_COMMISSION_RATE: u64 = 200;

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NodeConfig {
    /// The public key bytes corresponding to the private key that the validator
    /// holds to sign transactions.
    #[serde(default = "default_authority_key_pair")]
    pub authority_key_pair: AuthorityKeyPairWithPath,
    /// The public key bytes corresponding to the private key that the validator
    /// holds to sign consensus blocks.
    #[serde(default = "default_key_pair")]
    pub protocol_key_pair: KeyPairWithPath,
    #[serde(default = "default_key_pair")]
    pub account_key_pair: KeyPairWithPath,
    /// The public key bytes corresponding to the private key that the validator
    /// uses to establish TLS connections.
    #[serde(default = "default_key_pair")]
    pub network_key_pair: KeyPairWithPath,
    pub db_path: PathBuf,

    /// The network address for gRPC communication.
    ///
    /// Can be overwritten with args `listen-address` parameters.
    #[serde(default = "default_grpc_address")]
    pub network_address: Multiaddr,
    #[serde(default = "default_json_rpc_address")]
    pub json_rpc_address: SocketAddr,

    /// Flag to enable the REST API under `/api/v1`
    /// endpoint on the same interface as `json` `rpc` server.
    #[serde(default)]
    pub enable_rest_api: bool,

    /// The address for Prometheus metrics.
    #[serde(default = "default_metrics_address")]
    pub metrics_address: SocketAddr,

    /// The port for the admin interface that is
    /// run in the metrics separate runtime and provides access to
    /// admin node commands such as logging and tracing options.
    #[serde(default = "default_admin_interface_port")]
    pub admin_interface_port: u16,

    /// Configuration struct for the consensus.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus_config: Option<ConsensusConfig>,

    /// Flag to enable index processing for a full node.
    ///
    /// If set to true, node creates `IndexStore` for transaction
    /// data including ownership and balance information.
    #[serde(default = "default_enable_index_processing")]
    pub enable_index_processing: bool,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove_deprecated_tables: bool,

    // only allow websocket connections for jsonrpc traffic
    #[serde(default)]
    /// Determines the jsonrpc server type as either:
    /// - 'websocket' for a websocket based service (deprecated)
    /// - 'http' for an http based service
    /// - 'both' for both a websocket and http based service (deprecated)
    pub jsonrpc_server_type: Option<ServerType>,

    /// Flag to enable gRPC load shedding to manage and
    /// mitigate overload conditions by shedding excess
    /// load with `LoadShedLayer` middleware.
    #[serde(default)]
    pub grpc_load_shed: Option<bool>,

    #[serde(default = "default_concurrency_limit")]
    pub grpc_concurrency_limit: Option<usize>,

    /// Configuration struct for P2P.
    #[serde(default)]
    pub p2p_config: P2pConfig,

    /// Contains genesis location that might be `InPlace`
    /// for reading all genesis data to memory or `InFile`,
    /// and `OnceCell` pointer to a genesis struct.
    pub genesis: Genesis,

    /// Contains the path where to find the migration blob.
    pub migration_tx_data_path: Option<PathBuf>,

    /// Configuration for pruning of the authority store, to define when
    /// an old data is removed from the storage space.
    #[serde(default = "default_authority_store_pruning_config")]
    pub authority_store_pruning_config: AuthorityStorePruningConfig,

    /// Size of the broadcast channel used for notifying other systems of end of
    /// epoch.
    ///
    /// If unspecified, this will default to `128`.
    #[serde(default = "default_end_of_epoch_broadcast_channel_capacity")]
    pub end_of_epoch_broadcast_channel_capacity: usize,

    /// Configuration for the checkpoint executor for limiting
    /// the number of checkpoints to execute concurrently,
    /// and to allow for checkpoint post-processing.
    #[serde(default)]
    pub checkpoint_executor_config: CheckpointExecutorConfig,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsConfig>,

    /// In a `iota-node` binary, this is set to
    /// SupportedProtocolVersions::SYSTEM_DEFAULT in iota-node/src/main.rs.
    /// It is present in the config so that it can be changed by tests in
    /// order to test protocol upgrades.
    #[serde(skip)]
    pub supported_protocol_versions: Option<SupportedProtocolVersions>,

    /// Configuration to manage database checkpoints,
    /// including whether to perform checkpoints at the end of an epoch,
    /// the path for storing checkpoints, and other related settings.
    #[serde(default)]
    pub db_checkpoint_config: DBCheckpointConfig,

    /// Defines a threshold for an object size above which object
    /// is stored separately as `IndirectObject`. Used in `AuthorityStore`.
    #[serde(default)]
    pub indirect_objects_threshold: usize,

    /// Configuration for enabling/disabling expensive safety checks.
    #[serde(default)]
    pub expensive_safety_check_config: ExpensiveSafetyCheckConfig,

    /// Configuration to specify rules for denying transactions
    /// based on `objectsIDs`, `addresses`, or enable/disable many
    /// features such as publishing new packages or using shared objects.
    #[serde(default)]
    pub transaction_deny_config: TransactionDenyConfig,

    /// Config used to deny execution for certificate digests
    /// know for crashing or hanging validator nodes.
    ///
    /// Should be used for a fast temporary fixes and
    /// removed once the issue is fixed.
    #[serde(default)]
    pub certificate_deny_config: CertificateDenyConfig,

    /// Used to determine how state debug information is dumped
    /// when a node forks.
    #[serde(default)]
    pub state_debug_dump_config: StateDebugDumpConfig,

    /// Configuration for writing state archive. If `ObjectStorage`
    /// config is provided, `ArchiveWriter` will be created
    /// for checkpoints archival.
    #[serde(default)]
    pub state_archive_write_config: StateArchiveConfig,

    #[serde(default)]
    pub state_archive_read_config: Vec<StateArchiveConfig>,

    /// Determines if snapshot should be uploaded to the remote storage.
    #[serde(default)]
    pub state_snapshot_write_config: StateSnapshotConfig,

    #[serde(default)]
    pub indexer_max_subscriptions: Option<usize>,

    #[serde(default = "default_transaction_kv_store_config")]
    pub transaction_kv_store_read_config: TransactionKeyValueStoreReadConfig,

    // TODO: write config seem to be unused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_kv_store_write_config: Option<TransactionKeyValueStoreWriteConfig>,

    #[serde(default = "default_jwk_fetch_interval_seconds")]
    pub jwk_fetch_interval_seconds: u64,

    #[serde(default = "default_zklogin_oauth_providers")]
    pub zklogin_oauth_providers: BTreeMap<Chain, BTreeSet<String>>,

    /// Configuration for defining thresholds and settings
    /// for managing system overload conditions in a node.
    #[serde(default = "default_authority_overload_config")]
    pub authority_overload_config: AuthorityOverloadConfig,

    /// Specifies the ending epoch for a node for debugging purposes.
    ///
    ///  Ignored if set by config, can be configured only by cli arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_with_range: Option<RunWithRange>,

    // For killswitch use None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_config: Option<PolicyConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_config: Option<RemoteFirewallConfig>,

    #[serde(default)]
    pub execution_cache: ExecutionCacheConfig,

    #[serde(default = "bool_true")]
    pub enable_validator_tx_finalizer: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionCacheConfig {
    #[default]
    PassthroughCache,
    WritebackCache {
        max_cache_size: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerType {
    WebSocket,
    Http,
    Both,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct TransactionKeyValueStoreReadConfig {
    pub base_url: String,
}

fn default_jwk_fetch_interval_seconds() -> u64 {
    3600
}

pub fn default_zklogin_oauth_providers() -> BTreeMap<Chain, BTreeSet<String>> {
    let mut map = BTreeMap::new();

    // providers that are available on devnet only.
    let experimental_providers = BTreeSet::from([
        "Google".to_string(),
        "Facebook".to_string(),
        "Twitch".to_string(),
        "Kakao".to_string(),
        "Apple".to_string(),
        "Slack".to_string(),
        "TestIssuer".to_string(),
        "Microsoft".to_string(),
        "KarrierOne".to_string(),
        "Credenza3".to_string(),
    ]);

    // providers that are available for mainnet and testnet.
    let providers = BTreeSet::from([
        "Google".to_string(),
        "Facebook".to_string(),
        "Twitch".to_string(),
        "Apple".to_string(),
        "KarrierOne".to_string(),
        "Credenza3".to_string(),
    ]);
    map.insert(Chain::Mainnet, providers.clone());
    map.insert(Chain::Testnet, providers);
    map.insert(Chain::Unknown, experimental_providers);
    map
}

fn default_transaction_kv_store_config() -> TransactionKeyValueStoreReadConfig {
    TransactionKeyValueStoreReadConfig {
        base_url: "https://transactions.iota.cafe/".to_string(),
    }
}

fn default_authority_store_pruning_config() -> AuthorityStorePruningConfig {
    AuthorityStorePruningConfig::default()
}

pub fn default_enable_index_processing() -> bool {
    true
}

fn default_grpc_address() -> Multiaddr {
    "/ip4/0.0.0.0/tcp/8080".parse().unwrap()
}
fn default_authority_key_pair() -> AuthorityKeyPairWithPath {
    AuthorityKeyPairWithPath::new(get_key_pair_from_rng::<AuthorityKeyPair, _>(&mut OsRng).1)
}

fn default_key_pair() -> KeyPairWithPath {
    KeyPairWithPath::new(
        get_key_pair_from_rng::<AccountKeyPair, _>(&mut OsRng)
            .1
            .into(),
    )
}

fn default_metrics_address() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9184)
}

pub fn default_admin_interface_port() -> u16 {
    1337
}

pub fn default_json_rpc_address() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9000)
}

pub fn default_concurrency_limit() -> Option<usize> {
    Some(DEFAULT_GRPC_CONCURRENCY_LIMIT)
}

pub fn default_end_of_epoch_broadcast_channel_capacity() -> usize {
    128
}

pub fn bool_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

impl Config for NodeConfig {}

impl NodeConfig {
    pub fn authority_key_pair(&self) -> &AuthorityKeyPair {
        self.authority_key_pair.authority_keypair()
    }

    pub fn protocol_key_pair(&self) -> &NetworkKeyPair {
        match self.protocol_key_pair.keypair() {
            IotaKeyPair::Ed25519(kp) => kp,
            other => panic!(
                "invalid keypair type: {:?}, only Ed25519 is allowed for protocol key",
                other
            ),
        }
    }

    pub fn network_key_pair(&self) -> &NetworkKeyPair {
        match self.network_key_pair.keypair() {
            IotaKeyPair::Ed25519(kp) => kp,
            other => panic!(
                "invalid keypair type: {:?}, only Ed25519 is allowed for network key",
                other
            ),
        }
    }

    pub fn authority_public_key(&self) -> AuthorityPublicKeyBytes {
        self.authority_key_pair().public().into()
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.join("live")
    }

    pub fn db_checkpoint_path(&self) -> PathBuf {
        self.db_path.join("db_checkpoints")
    }

    pub fn archive_path(&self) -> PathBuf {
        self.db_path.join("archive")
    }

    pub fn snapshot_path(&self) -> PathBuf {
        self.db_path.join("snapshot")
    }

    pub fn network_address(&self) -> &Multiaddr {
        &self.network_address
    }

    pub fn consensus_config(&self) -> Option<&ConsensusConfig> {
        self.consensus_config.as_ref()
    }

    pub fn genesis(&self) -> Result<&genesis::Genesis> {
        self.genesis.genesis()
    }

    pub fn load_migration_tx_data(&self) -> Result<MigrationTxData> {
        let Some(location) = &self.migration_tx_data_path else {
            anyhow::bail!("no file location set");
        };

        // Load from file
        let migration_tx_data = MigrationTxData::load(location)?;

        // Validate migration content in order to avoid corrupted or malicious data
        migration_tx_data.validate_from_genesis(self.genesis.genesis()?)?;
        Ok(migration_tx_data)
    }

    pub fn iota_address(&self) -> IotaAddress {
        (&self.account_key_pair.keypair().public()).into()
    }

    pub fn archive_reader_config(&self) -> Vec<ArchiveReaderConfig> {
        self.state_archive_read_config
            .iter()
            .flat_map(|config| {
                config
                    .object_store_config
                    .as_ref()
                    .map(|remote_store_config| ArchiveReaderConfig {
                        remote_store_config: remote_store_config.clone(),
                        download_concurrency: NonZeroUsize::new(config.concurrency)
                            .unwrap_or(NonZeroUsize::new(5).unwrap()),
                        use_for_pruning_watermark: config.use_for_pruning_watermark,
                    })
            })
            .collect()
    }

    pub fn jsonrpc_server_type(&self) -> ServerType {
        self.jsonrpc_server_type.unwrap_or(ServerType::Http)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ConsensusProtocol {
    #[serde(rename = "mysticeti")]
    Mysticeti,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConsensusConfig {
    // Base consensus DB path for all epochs.
    pub db_path: PathBuf,

    // The number of epochs for which to retain the consensus DBs. Setting it to 0 will make a
    // consensus DB getting dropped as soon as system is switched to a new epoch.
    pub db_retention_epochs: Option<u64>,

    // Pruner will run on every epoch change but it will also check periodically on every
    // `db_pruner_period_secs` seconds to see if there are any epoch DBs to remove.
    pub db_pruner_period_secs: Option<u64>,

    /// Maximum number of pending transactions to submit to consensus, including
    /// those in submission wait.
    ///
    /// Default to 20_000 inflight limit, assuming 20_000 txn tps * 1 sec
    /// consensus latency.
    pub max_pending_transactions: Option<usize>,

    /// When defined caps the calculated submission position to the
    /// max_submit_position.
    ///
    /// Even if the is elected to submit from a higher
    /// position than this, it will "reset" to the max_submit_position.
    pub max_submit_position: Option<usize>,

    /// The submit delay step to consensus defined in milliseconds.
    ///
    /// When provided it will override the current back off logic otherwise the
    /// default backoff logic will be applied based on consensus latency
    /// estimates.
    pub submit_delay_step_override_millis: Option<u64>,

    pub parameters: Option<ConsensusParameters>,
}

impl ConsensusConfig {
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn max_pending_transactions(&self) -> usize {
        self.max_pending_transactions.unwrap_or(20_000)
    }

    pub fn submit_delay_step_override(&self) -> Option<Duration> {
        self.submit_delay_step_override_millis
            .map(Duration::from_millis)
    }

    pub fn db_retention_epochs(&self) -> u64 {
        self.db_retention_epochs.unwrap_or(0)
    }

    pub fn db_pruner_period(&self) -> Duration {
        // Default to 1 hour
        self.db_pruner_period_secs
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(3_600))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CheckpointExecutorConfig {
    /// Upper bound on the number of checkpoints that can be concurrently
    /// executed.
    ///
    /// If unspecified, this will default to `200`
    #[serde(default = "default_checkpoint_execution_max_concurrency")]
    pub checkpoint_execution_max_concurrency: usize,

    /// Number of seconds to wait for effects of a batch of transactions
    /// before logging a warning. Note that we will continue to retry
    /// indefinitely.
    ///
    /// If unspecified, this will default to `10`.
    #[serde(default = "default_local_execution_timeout_sec")]
    pub local_execution_timeout_sec: u64,

    /// Optional directory used for data ingestion pipeline.
    ///
    /// When specified, each executed checkpoint will be saved in a local
    /// directory for post-processing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_ingestion_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExpensiveSafetyCheckConfig {
    /// If enabled, at epoch boundary, we will check that the storage
    /// fund balance is always identical to the sum of the storage
    /// rebate of all live objects, and that the total IOTA in the network
    /// remains the same.
    #[serde(default)]
    enable_epoch_iota_conservation_check: bool,

    /// If enabled, we will check that the total IOTA in all input objects of a
    /// tx (both the Move part and the storage rebate) matches the total IOTA
    /// in all output objects of the tx + gas fees.
    #[serde(default)]
    enable_deep_per_tx_iota_conservation_check: bool,

    /// Disable epoch IOTA conservation check even when we are running in debug
    /// mode.
    #[serde(default)]
    force_disable_epoch_iota_conservation_check: bool,

    /// If enabled, at epoch boundary, we will check that the accumulated
    /// live object state matches the end of epoch root state digest.
    #[serde(default)]
    enable_state_consistency_check: bool,

    /// Disable state consistency check even when we are running in debug mode.
    #[serde(default)]
    force_disable_state_consistency_check: bool,

    #[serde(default)]
    enable_secondary_index_checks: bool,
    // TODO: Add more expensive checks here
}

impl ExpensiveSafetyCheckConfig {
    pub fn new_enable_all() -> Self {
        Self {
            enable_epoch_iota_conservation_check: true,
            enable_deep_per_tx_iota_conservation_check: true,
            force_disable_epoch_iota_conservation_check: false,
            enable_state_consistency_check: true,
            force_disable_state_consistency_check: false,
            enable_secondary_index_checks: false, // Disable by default for now
        }
    }

    pub fn new_disable_all() -> Self {
        Self {
            enable_epoch_iota_conservation_check: false,
            enable_deep_per_tx_iota_conservation_check: false,
            force_disable_epoch_iota_conservation_check: true,
            enable_state_consistency_check: false,
            force_disable_state_consistency_check: true,
            enable_secondary_index_checks: false,
        }
    }

    pub fn force_disable_epoch_iota_conservation_check(&mut self) {
        self.force_disable_epoch_iota_conservation_check = true;
    }

    pub fn enable_epoch_iota_conservation_check(&self) -> bool {
        (self.enable_epoch_iota_conservation_check || cfg!(debug_assertions))
            && !self.force_disable_epoch_iota_conservation_check
    }

    pub fn force_disable_state_consistency_check(&mut self) {
        self.force_disable_state_consistency_check = true;
    }

    pub fn enable_state_consistency_check(&self) -> bool {
        (self.enable_state_consistency_check || cfg!(debug_assertions))
            && !self.force_disable_state_consistency_check
    }

    pub fn enable_deep_per_tx_iota_conservation_check(&self) -> bool {
        self.enable_deep_per_tx_iota_conservation_check || cfg!(debug_assertions)
    }

    pub fn enable_secondary_index_checks(&self) -> bool {
        self.enable_secondary_index_checks
    }
}

fn default_checkpoint_execution_max_concurrency() -> usize {
    200
}

fn default_local_execution_timeout_sec() -> u64 {
    30
}

impl Default for CheckpointExecutorConfig {
    fn default() -> Self {
        Self {
            checkpoint_execution_max_concurrency: default_checkpoint_execution_max_concurrency(),
            local_execution_timeout_sec: default_local_execution_timeout_sec(),
            data_ingestion_dir: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthorityStorePruningConfig {
    /// number of the latest epoch dbs to retain
    #[serde(default = "default_num_latest_epoch_dbs_to_retain")]
    pub num_latest_epoch_dbs_to_retain: usize,
    /// time interval used by the pruner to determine whether there are any
    /// epoch DBs to remove
    #[serde(default = "default_epoch_db_pruning_period_secs")]
    pub epoch_db_pruning_period_secs: u64,
    /// number of epochs to keep the latest version of objects for.
    /// Note that a zero value corresponds to an aggressive pruner.
    /// This mode is experimental and needs to be used with caution.
    /// Use `u64::MAX` to disable the pruner for the objects.
    #[serde(default)]
    pub num_epochs_to_retain: u64,
    /// pruner's runtime interval used for aggressive mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pruning_run_delay_seconds: Option<u64>,
    /// maximum number of checkpoints in the pruning batch. Can be adjusted to
    /// increase performance
    #[serde(default = "default_max_checkpoints_in_batch")]
    pub max_checkpoints_in_batch: usize,
    /// maximum number of transaction in the pruning batch
    #[serde(default = "default_max_transactions_in_batch")]
    pub max_transactions_in_batch: usize,
    /// enables periodic background compaction for old SST files whose last
    /// modified time is older than `periodic_compaction_threshold_days`
    /// days. That ensures that all sst files eventually go through the
    /// compaction process
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodic_compaction_threshold_days: Option<usize>,
    /// number of epochs to keep the latest version of transactions and effects
    /// for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_epochs_to_retain_for_checkpoints: Option<u64>,
    #[serde(default = "default_smoothing", skip_serializing_if = "is_true")]
    pub smooth: bool,
}

fn default_num_latest_epoch_dbs_to_retain() -> usize {
    3
}

fn default_epoch_db_pruning_period_secs() -> u64 {
    3600
}

fn default_max_transactions_in_batch() -> usize {
    1000
}

fn default_max_checkpoints_in_batch() -> usize {
    10
}

fn default_smoothing() -> bool {
    cfg!(not(test))
}

impl Default for AuthorityStorePruningConfig {
    fn default() -> Self {
        Self {
            num_latest_epoch_dbs_to_retain: default_num_latest_epoch_dbs_to_retain(),
            epoch_db_pruning_period_secs: default_epoch_db_pruning_period_secs(),
            num_epochs_to_retain: 0,
            pruning_run_delay_seconds: if cfg!(msim) { Some(2) } else { None },
            max_checkpoints_in_batch: default_max_checkpoints_in_batch(),
            max_transactions_in_batch: default_max_transactions_in_batch(),
            periodic_compaction_threshold_days: None,
            num_epochs_to_retain_for_checkpoints: if cfg!(msim) { Some(2) } else { None },
            smooth: true,
        }
    }
}

impl AuthorityStorePruningConfig {
    pub fn set_num_epochs_to_retain_for_checkpoints(&mut self, num_epochs_to_retain: Option<u64>) {
        self.num_epochs_to_retain_for_checkpoints = num_epochs_to_retain;
    }

    pub fn num_epochs_to_retain_for_checkpoints(&self) -> Option<u64> {
        self.num_epochs_to_retain_for_checkpoints
            // if n less than 2, coerce to 2 and log
            .map(|n| {
                if n < 2 {
                    info!("num_epochs_to_retain_for_checkpoints must be at least 2, rounding up from {}", n);
                    2
                } else {
                    n
                }
            })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_interval_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_url: Option<String>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct DBCheckpointConfig {
    #[serde(default)]
    pub perform_db_checkpoints_at_epoch_end: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoint_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_store_config: Option<ObjectStoreConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perform_index_db_checkpoints_at_epoch_end: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prune_and_compact_before_upload: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ArchiveReaderConfig {
    pub remote_store_config: ObjectStoreConfig,
    pub download_concurrency: NonZeroUsize,
    pub use_for_pruning_watermark: bool,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StateArchiveConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_store_config: Option<ObjectStoreConfig>,
    pub concurrency: usize,
    pub use_for_pruning_watermark: bool,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StateSnapshotConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_store_config: Option<ObjectStoreConfig>,
    pub concurrency: usize,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TransactionKeyValueStoreWriteConfig {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_region: String,
    pub table_name: String,
    pub bucket_name: String,
    pub concurrency: usize,
}

/// Configuration for the threshold(s) at which we consider the system
/// to be overloaded. When one of the threshold is passed, the node may
/// stop processing new transactions and/or certificates until the congestion
/// resolves.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuthorityOverloadConfig {
    #[serde(default = "default_max_txn_age_in_queue")]
    pub max_txn_age_in_queue: Duration,

    // The interval of checking overload signal.
    #[serde(default = "default_overload_monitor_interval")]
    pub overload_monitor_interval: Duration,

    // The execution queueing latency when entering load shedding mode.
    #[serde(default = "default_execution_queue_latency_soft_limit")]
    pub execution_queue_latency_soft_limit: Duration,

    // The execution queueing latency when entering aggressive load shedding mode.
    #[serde(default = "default_execution_queue_latency_hard_limit")]
    pub execution_queue_latency_hard_limit: Duration,

    // The maximum percentage of transactions to shed in load shedding mode.
    #[serde(default = "default_max_load_shedding_percentage")]
    pub max_load_shedding_percentage: u32,

    // When in aggressive load shedding mode, the minimum percentage of
    // transactions to shed.
    #[serde(default = "default_min_load_shedding_percentage_above_hard_limit")]
    pub min_load_shedding_percentage_above_hard_limit: u32,

    // If transaction ready rate is below this rate, we consider the validator
    // is well under used, and will not enter load shedding mode.
    #[serde(default = "default_safe_transaction_ready_rate")]
    pub safe_transaction_ready_rate: u32,

    // When set to true, transaction signing may be rejected when the validator
    // is overloaded.
    #[serde(default = "default_check_system_overload_at_signing")]
    pub check_system_overload_at_signing: bool,

    // When set to true, transaction execution may be rejected when the validator
    // is overloaded.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check_system_overload_at_execution: bool,

    // Reject a transaction if transaction manager queue length is above this threshold.
    // 100_000 = 10k TPS * 5s resident time in transaction manager (pending + executing) * 2.
    #[serde(default = "default_max_transaction_manager_queue_length")]
    pub max_transaction_manager_queue_length: usize,

    // Reject a transaction if the number of pending transactions depending on the object
    // is above the threshold.
    #[serde(default = "default_max_transaction_manager_per_object_queue_length")]
    pub max_transaction_manager_per_object_queue_length: usize,
}

fn default_max_txn_age_in_queue() -> Duration {
    Duration::from_secs(1)
}

fn default_overload_monitor_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_execution_queue_latency_soft_limit() -> Duration {
    Duration::from_secs(1)
}

fn default_execution_queue_latency_hard_limit() -> Duration {
    Duration::from_secs(10)
}

fn default_max_load_shedding_percentage() -> u32 {
    95
}

fn default_min_load_shedding_percentage_above_hard_limit() -> u32 {
    50
}

fn default_safe_transaction_ready_rate() -> u32 {
    100
}

fn default_check_system_overload_at_signing() -> bool {
    true
}

fn default_max_transaction_manager_queue_length() -> usize {
    100_000
}

fn default_max_transaction_manager_per_object_queue_length() -> usize {
    100
}

impl Default for AuthorityOverloadConfig {
    fn default() -> Self {
        Self {
            max_txn_age_in_queue: default_max_txn_age_in_queue(),
            overload_monitor_interval: default_overload_monitor_interval(),
            execution_queue_latency_soft_limit: default_execution_queue_latency_soft_limit(),
            execution_queue_latency_hard_limit: default_execution_queue_latency_hard_limit(),
            max_load_shedding_percentage: default_max_load_shedding_percentage(),
            min_load_shedding_percentage_above_hard_limit:
                default_min_load_shedding_percentage_above_hard_limit(),
            safe_transaction_ready_rate: default_safe_transaction_ready_rate(),
            check_system_overload_at_signing: true,
            check_system_overload_at_execution: false,
            max_transaction_manager_queue_length: default_max_transaction_manager_queue_length(),
            max_transaction_manager_per_object_queue_length:
                default_max_transaction_manager_per_object_queue_length(),
        }
    }
}

fn default_authority_overload_config() -> AuthorityOverloadConfig {
    AuthorityOverloadConfig::default()
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq)]
pub struct Genesis {
    #[serde(flatten)]
    location: Option<GenesisLocation>,

    #[serde(skip)]
    genesis: once_cell::sync::OnceCell<genesis::Genesis>,
}

impl Genesis {
    pub fn new(genesis: genesis::Genesis) -> Self {
        Self {
            location: Some(GenesisLocation::InPlace { genesis }),
            genesis: Default::default(),
        }
    }

    pub fn new_from_file<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            location: Some(GenesisLocation::File {
                genesis_file_location: path.into(),
            }),
            genesis: Default::default(),
        }
    }

    pub fn new_empty() -> Self {
        Self {
            location: None,
            genesis: Default::default(),
        }
    }

    pub fn genesis(&self) -> Result<&genesis::Genesis> {
        match &self.location {
            Some(GenesisLocation::InPlace { genesis }) => Ok(genesis),
            Some(GenesisLocation::File {
                genesis_file_location,
            }) => self
                .genesis
                .get_or_try_init(|| genesis::Genesis::load(genesis_file_location)),
            None => anyhow::bail!("no genesis location set"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq)]
#[serde(untagged)]
enum GenesisLocation {
    InPlace {
        genesis: genesis::Genesis,
    },
    File {
        #[serde(rename = "genesis-file-location")]
        genesis_file_location: PathBuf,
    },
}

/// Wrapper struct for IotaKeyPair that can be deserialized from a file path.
/// Used by network, worker, and account keypair.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyPairWithPath {
    #[serde(flatten)]
    location: KeyPairLocation,

    #[serde(skip)]
    keypair: OnceCell<Arc<IotaKeyPair>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq)]
#[serde_as]
#[serde(untagged)]
enum KeyPairLocation {
    InPlace {
        #[serde_as(as = "Arc<KeyPairBase64>")]
        value: Arc<IotaKeyPair>,
    },
    File {
        #[serde(rename = "path")]
        path: PathBuf,
    },
}

impl KeyPairWithPath {
    pub fn new(kp: IotaKeyPair) -> Self {
        let cell: OnceCell<Arc<IotaKeyPair>> = OnceCell::new();
        let arc_kp = Arc::new(kp);
        // OK to unwrap panic because authority should not start without all keypairs
        // loaded.
        cell.set(arc_kp.clone()).expect("failed to set keypair");
        Self {
            location: KeyPairLocation::InPlace { value: arc_kp },
            keypair: cell,
        }
    }

    pub fn new_from_path(path: PathBuf) -> Self {
        let cell: OnceCell<Arc<IotaKeyPair>> = OnceCell::new();
        // OK to unwrap panic because authority should not start without all keypairs
        // loaded.
        cell.set(Arc::new(read_keypair_from_file(&path).unwrap_or_else(
            |e| panic!("invalid keypair file at path {:?}: {e}", &path),
        )))
        .expect("failed to set keypair");
        Self {
            location: KeyPairLocation::File { path },
            keypair: cell,
        }
    }

    pub fn keypair(&self) -> &IotaKeyPair {
        self.keypair
            .get_or_init(|| match &self.location {
                KeyPairLocation::InPlace { value } => value.clone(),
                KeyPairLocation::File { path } => {
                    // OK to unwrap panic because authority should not start without all keypairs
                    // loaded.
                    Arc::new(
                        read_keypair_from_file(path).unwrap_or_else(|e| {
                            panic!("invalid keypair file at path {:?}: {e}", path)
                        }),
                    )
                }
            })
            .as_ref()
    }
}

/// Wrapper struct for AuthorityKeyPair that can be deserialized from a file
/// path.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AuthorityKeyPairWithPath {
    #[serde(flatten)]
    location: AuthorityKeyPairLocation,

    #[serde(skip)]
    keypair: OnceCell<Arc<AuthorityKeyPair>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Eq)]
#[serde_as]
#[serde(untagged)]
enum AuthorityKeyPairLocation {
    InPlace { value: Arc<AuthorityKeyPair> },
    File { path: PathBuf },
}

impl AuthorityKeyPairWithPath {
    pub fn new(kp: AuthorityKeyPair) -> Self {
        let cell: OnceCell<Arc<AuthorityKeyPair>> = OnceCell::new();
        let arc_kp = Arc::new(kp);
        // OK to unwrap panic because authority should not start without all keypairs
        // loaded.
        cell.set(arc_kp.clone())
            .expect("failed to set authority keypair");
        Self {
            location: AuthorityKeyPairLocation::InPlace { value: arc_kp },
            keypair: cell,
        }
    }

    pub fn new_from_path(path: PathBuf) -> Self {
        let cell: OnceCell<Arc<AuthorityKeyPair>> = OnceCell::new();
        // OK to unwrap panic because authority should not start without all keypairs
        // loaded.
        cell.set(Arc::new(
            read_authority_keypair_from_file(&path)
                .unwrap_or_else(|_| panic!("invalid authority keypair file at path {:?}", &path)),
        ))
        .expect("failed to set authority keypair");
        Self {
            location: AuthorityKeyPairLocation::File { path },
            keypair: cell,
        }
    }

    pub fn authority_keypair(&self) -> &AuthorityKeyPair {
        self.keypair
            .get_or_init(|| match &self.location {
                AuthorityKeyPairLocation::InPlace { value } => value.clone(),
                AuthorityKeyPairLocation::File { path } => {
                    // OK to unwrap panic because authority should not start without all keypairs
                    // loaded.
                    Arc::new(
                        read_authority_keypair_from_file(path).unwrap_or_else(|_| {
                            panic!("invalid authority keypair file {:?}", &path)
                        }),
                    )
                }
            })
            .as_ref()
    }
}

/// Configurations which determine how we dump state debug info.
/// Debug info is dumped when a node forks.
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct StateDebugDumpConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dump_file_directory: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fastcrypto::traits::KeyPair;
    use iota_keys::keypair_file::{write_authority_keypair_to_file, write_keypair_to_file};
    use iota_types::crypto::{
        AuthorityKeyPair, IotaKeyPair, NetworkKeyPair, get_key_pair_from_rng,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use super::Genesis;
    use crate::NodeConfig;

    #[test]
    fn serialize_genesis_from_file() {
        let g = Genesis::new_from_file("path/to/file");

        let s = serde_yaml::to_string(&g).unwrap();
        assert_eq!("---\ngenesis-file-location: path/to/file\n", s);
        let loaded_genesis: Genesis = serde_yaml::from_str(&s).unwrap();
        assert_eq!(g, loaded_genesis);
    }

    #[test]
    fn fullnode_template() {
        const TEMPLATE: &str = include_str!("../data/fullnode-template.yaml");

        let _template: NodeConfig = serde_yaml::from_str(TEMPLATE).unwrap();
    }

    #[test]
    fn load_key_pairs_to_node_config() {
        let authority_key_pair: AuthorityKeyPair =
            get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1;
        let protocol_key_pair: NetworkKeyPair =
            get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1;
        let network_key_pair: NetworkKeyPair =
            get_key_pair_from_rng(&mut StdRng::from_seed([0; 32])).1;

        write_authority_keypair_to_file(&authority_key_pair, PathBuf::from("authority.key"))
            .unwrap();
        write_keypair_to_file(
            &IotaKeyPair::Ed25519(protocol_key_pair.copy()),
            PathBuf::from("protocol.key"),
        )
        .unwrap();
        write_keypair_to_file(
            &IotaKeyPair::Ed25519(network_key_pair.copy()),
            PathBuf::from("network.key"),
        )
        .unwrap();

        const TEMPLATE: &str = include_str!("../data/fullnode-template-with-path.yaml");
        let template: NodeConfig = serde_yaml::from_str(TEMPLATE).unwrap();
        assert_eq!(
            template.authority_key_pair().public(),
            authority_key_pair.public()
        );
        assert_eq!(
            template.network_key_pair().public(),
            network_key_pair.public()
        );
        assert_eq!(
            template.protocol_key_pair().public(),
            protocol_key_pair.public()
        );
    }
}

// RunWithRange is used to specify the ending epoch/checkpoint to process.
// this is intended for use with disaster recovery debugging and verification
// workflows, never in normal operations
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum RunWithRange {
    Epoch(EpochId),
    Checkpoint(CheckpointSequenceNumber),
}

impl RunWithRange {
    // is epoch_id > RunWithRange::Epoch
    pub fn is_epoch_gt(&self, epoch_id: EpochId) -> bool {
        matches!(self, RunWithRange::Epoch(e) if epoch_id > *e)
    }

    pub fn matches_checkpoint(&self, seq_num: CheckpointSequenceNumber) -> bool {
        matches!(self, RunWithRange::Checkpoint(seq) if *seq == seq_num)
    }
}
