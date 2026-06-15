// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, net::SocketAddr, path::PathBuf};

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use iota_names::config::IotaNamesConfig;
use iota_sdk_types::ObjectId;
use iota_types::base_types::IotaAddress;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tracing::warn;
use url::Url;

use crate::{
    backfill::BackfillKind, db::ConnectionPoolConfig, pruning::pruner::PrunableTable,
    types::IndexerResult,
};

#[derive(Parser, Clone, Debug)]
#[command(
    name = "IOTA indexer",
    about = "An off-fullnode service serving data from IOTA protocol"
)]
pub struct IndexerConfig {
    #[arg(long, alias = "db-url")]
    pub database_url: Option<Url>,

    #[command(flatten)]
    pub connection_pool_config: ConnectionPoolConfig,

    #[arg(long, default_value = "0.0.0.0:9184")]
    pub metrics_address: SocketAddr,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct IotaNamesOptions {
    #[arg(default_value_t = IotaNamesConfig::default().package_address)]
    #[arg(long = "iota-names-package-address")]
    pub package_address: IotaAddress,
    #[arg(default_value_t = IotaNamesConfig::default().object_id)]
    #[arg(long = "iota-names-object-id")]
    pub object_id: ObjectId,
    #[arg(default_value_t = IotaNamesConfig::default().payments_package_address)]
    #[arg(long = "iota-names-payments-package-address")]
    pub payments_package_address: IotaAddress,
    #[arg(default_value_t = IotaNamesConfig::default().registry_id)]
    #[arg(long = "iota-names-registry-id")]
    pub registry_id: ObjectId,
    #[arg(default_value_t = IotaNamesConfig::default().reverse_registry_id)]
    #[arg(long = "iota-names-reverse-registry-id")]
    pub reverse_registry_id: ObjectId,
}

impl From<IotaNamesOptions> for IotaNamesConfig {
    fn from(options: IotaNamesOptions) -> Self {
        let IotaNamesOptions {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        } = options;
        Self {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        }
    }
}

impl From<IotaNamesConfig> for IotaNamesOptions {
    fn from(config: IotaNamesConfig) -> Self {
        let IotaNamesConfig {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        } = config;
        Self {
            package_address,
            object_id,
            payments_package_address,
            registry_id,
            reverse_registry_id,
        }
    }
}

impl Default for IotaNamesOptions {
    fn default() -> Self {
        IotaNamesConfig::default().into()
    }
}

#[derive(Args, Debug, Clone)]
pub struct JsonRpcConfig {
    #[command(flatten)]
    pub iota_names_options: IotaNamesOptions,

    #[command(flatten)]
    pub historic_fallback_options: HistoricFallbackOptions,

    #[clap(long, default_value = "0.0.0.0:9000")]
    pub rpc_address: SocketAddr,

    #[clap(long)]
    pub rpc_client_url: String,
}

#[derive(Args, Debug, Clone)]
pub struct HistoricFallbackOptions {
    #[arg(
        long,
        help = "Experimental: REST KV store URL for historic fallback. Depends on the iota-rest-kv API which is still being finalized."
    )]
    pub fallback_kv_url: Option<Url>,

    #[arg(
        long,
        default_value_t = Self::DEFAULT_MULTI_FETCH_BATCH_SIZE,
        env = "FALLBACK_KV_MULTI_FETCH_BATCH_SIZE",
        help = "Experimental: Maximum number of keys per batch request to fallback KV store."
    )]
    pub fallback_kv_multi_fetch_batch_size: usize,

    #[arg(
        long,
        default_value_t = Self::DEFAULT_CONCURRENT_FETCHES,
        env = "FALLBACK_KV_CONCURRENT_FETCHES",
        help = "Experimental: Maximum number of concurrent batch requests to fallback KV store."
    )]
    pub fallback_kv_concurrent_fetches: usize,

    #[arg(
        long,
        default_value_t = Self::DEFAULT_CACHE_SIZE,
        env = "FALLBACK_KV_CACHE_SIZE",
        help = "Experimental: Cache size for historic fallback."
    )]
    pub fallback_kv_cache_size: u64,
}

impl HistoricFallbackOptions {
    pub const DEFAULT_MULTI_FETCH_BATCH_SIZE: usize = 100;
    pub const DEFAULT_CONCURRENT_FETCHES: usize = 10;
    pub const DEFAULT_CACHE_SIZE: u64 = 100_000;
}

impl Default for HistoricFallbackOptions {
    fn default() -> Self {
        Self {
            fallback_kv_url: None,
            fallback_kv_multi_fetch_batch_size: Self::DEFAULT_MULTI_FETCH_BATCH_SIZE,
            fallback_kv_concurrent_fetches: Self::DEFAULT_CONCURRENT_FETCHES,
            fallback_kv_cache_size: Self::DEFAULT_CACHE_SIZE,
        }
    }
}

#[derive(Args, Debug, Default, Clone)]
#[group(required = true, multiple = true)]
pub struct IngestionSources {
    /// Ingest checkpoints from the given path.
    #[arg(long)]
    pub data_ingestion_path: Option<PathBuf>,

    /// Primary remote checkpoint source.
    ///
    /// Accepts either a fullnode gRPC URL (e.g. `http://0.0.0.0:50051`) or an
    /// S3-compatible object store URL hosting batched checkpoint files
    /// (e.g. `https://checkpoints.mainnet.iota.cafe/ingestion/historical`).
    ///
    /// When pointing to an object store, this provides complete checkpoint
    /// coverage from genesis. When pointing to a fullnode, checkpoint
    /// availability depends on the node's pruning configuration.
    #[arg(long)]
    pub remote_store_url: Option<Url>,

    /// Optional live checkpoint store for low-latency ingestion at the network
    /// tip.
    ///
    /// S3-compatible object store URL serving individual checkpoint files for
    /// the current epoch only
    /// (e.g. `https://checkpoints.mainnet.iota.cafe/ingestion/live`).
    ///
    /// Use alongside `--remote-store-url` pointing to a historical store for
    /// complete coverage with minimal tip latency.
    #[arg(long, requires = "remote_store_url")]
    pub live_checkpoints_store_url: Option<Url>,
}

#[derive(Args, Debug, Clone)]
pub struct IngestionConfig {
    #[clap(flatten)]
    pub sources: IngestionSources,

    #[arg(
        long,
        default_value_t = Self::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE,
        env = "DOWNLOAD_QUEUE_SIZE",
    )]
    pub checkpoint_download_queue_size: usize,

    #[arg(
        long,
        default_value_t = Self::DEFAULT_CHECKPOINT_DOWNLOAD_TIMEOUT,
        env = "INGESTION_READER_TIMEOUT_SECS",
    )]
    pub checkpoint_download_timeout: u64,

    /// Limit indexing parallelism on big checkpoints to avoid OOMing by
    /// limiting the total size of the checkpoint download queue.
    #[arg(
        long,
        default_value_t = Self::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE_BYTES,
        env = "CHECKPOINT_PROCESSING_BATCH_DATA_LIMIT",
    )]
    pub checkpoint_download_queue_size_bytes: usize,
}

impl IngestionConfig {
    pub const DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE: usize = 200;
    pub const DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE_BYTES: usize = 20_000_000;
    pub const DEFAULT_CHECKPOINT_DOWNLOAD_TIMEOUT: u64 = 20;
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            sources: Default::default(),
            checkpoint_download_queue_size: Self::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE,
            checkpoint_download_timeout: Self::DEFAULT_CHECKPOINT_DOWNLOAD_TIMEOUT,
            checkpoint_download_queue_size_bytes:
                Self::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE_BYTES,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct BackfillConfig {
    /// Maximum number of concurrent tasks to run.
    #[arg(
    long,
    default_value_t = Self::DEFAULT_MAX_CONCURRENCY,
    )]
    pub max_concurrency: usize,
    /// Size of the data chunks processed per task.
    #[arg(
    long,
    default_value_t = Self::DEFAULT_CHUNK_SIZE,
    )]
    pub chunk_size: usize,
}

impl BackfillConfig {
    const DEFAULT_MAX_CONCURRENCY: usize = 10;
    const DEFAULT_CHUNK_SIZE: usize = 1000;
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    Indexer {
        #[command(flatten)]
        ingestion_config: IngestionConfig,
        #[command(flatten)]
        snapshot_config: SnapshotLagConfig,
        #[command(flatten)]
        pruning_options: PruningOptions,
        #[arg(long)]
        reset_db: bool,
    },
    JsonRpcService(JsonRpcConfig),
    AnalyticalWorker,
    /// Backfill DB tables for some ID range [start, end].
    /// The tool will automatically slice it into smaller ranges and for each
    /// range, it first makes a read query to the DB to get data needed for
    /// backfill if needed, which then can be processed and written back to
    /// the DB. To add a new backfill, add a new module and implement the
    /// `BackfillTask` trait.
    RunBackfill {
        /// Start of the range to backfill, inclusive.
        /// It can be a checkpoint number or an epoch or any other identifier
        /// that can be used to slice the backfill range.
        start: usize,
        /// End of the range to backfill, inclusive.
        end: usize,
        #[command(subcommand)]
        runner_kind: BackfillKind,
        #[command(flatten)]
        backfill_config: BackfillConfig,
    },
}

pub const DEFAULT_PRUNING_DELAY_MS: u64 = 2 * 60 * 60 * 1000; // 2 hours
pub const DEFAULT_PRUNING_BATCH_SIZE: u64 = 1000;

#[derive(Args, Debug, Clone)]
pub struct PruningOptions {
    /// DEPRECATED: will be removed in v1.28.0. Use `--pruning-config-path`
    /// pointing at a TOML retention config instead.
    #[arg(long, env = "EPOCHS_TO_KEEP")]
    pub epochs_to_keep: Option<u64>,
    /// Path to TOML file containing configuration for retention policies.
    #[arg(long)]
    pub pruning_config_path: Option<PathBuf>,
    /// Delay in milliseconds between a watermark's lower bound being advanced
    /// and the pruner acting on it. Lets in-flight reads complete or timeout
    /// before their data is pruned.
    #[arg(long, env = "PRUNING_DELAY_MS", default_value_t = DEFAULT_PRUNING_DELAY_MS)]
    pub pruning_delay_ms: u64,
    /// Upper bound on units (checkpoints, transactions, or global sequence
    /// numbers) pruned per chunk, and on rows deleted per statement for the
    /// `WithLimit` strategies. Must be > 0.
    #[arg(
        long,
        env = "PRUNING_BATCH_SIZE",
        default_value_t = DEFAULT_PRUNING_BATCH_SIZE,
        value_parser = clap::value_parser!(u64).range(1..),
    )]
    pub pruning_batch_size: u64,
    /// DEPRECATED: will be removed in v1.29.0. This parameter is no longer
    /// used. Optimistic transactions are now pruned by the unified pruner.
    #[arg(long, env = "OPTIMISTIC_PRUNER_BATCH_SIZE")]
    pub optimistic_pruner_batch_size: Option<u64>,
}

impl Default for PruningOptions {
    fn default() -> Self {
        Self {
            epochs_to_keep: None,
            pruning_config_path: None,
            pruning_delay_ms: DEFAULT_PRUNING_DELAY_MS,
            pruning_batch_size: DEFAULT_PRUNING_BATCH_SIZE,
            optimistic_pruner_batch_size: None,
        }
    }
}

/// Represents the default retention policy and overrides for prunable tables.
/// Instantiated only if `PruningOptions` is provided on indexer start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Default retention policy for all tables.
    pub epochs_to_keep: u64,
    /// A map of tables to their respective retention policies that will
    /// override the default. Prunable tables not named here will use the
    /// default retention policy.
    #[serde(default)]
    pub overrides: HashMap<PrunableTable, u64>,
}

impl PruningOptions {
    /// Loads default retention policy and overrides from file.
    pub fn load_from_file(&self) -> IndexerResult<Option<RetentionConfig>> {
        let Some(config_path) = self.pruning_config_path.as_ref() else {
            let Some(epochs_to_keep) = self.epochs_to_keep else {
                return Ok(None);
            };
            warn!(
                "using the deprecated --epochs-to-keep argument for pruning configuration. \
                 This argument will be removed in v1.28.0. \
                 Please use --pruning-config-path to specify a TOML configuration file instead."
            );
            return Ok(Some(RetentionConfig::new(
                epochs_to_keep,
                Default::default(),
            )));
        };

        if self.epochs_to_keep.is_some() {
            warn!(
                "the --epochs-to-keep argument will be ignored since --pruning-config-path is also provided. \
                 Note that --epochs-to-keep is deprecated and will be removed in v1.28.0."
            );
        };

        let contents = std::fs::read_to_string(config_path)
            .context("failed to read default retention policy and overrides from file")?;
        let retention_with_overrides = toml::de::from_str::<RetentionConfig>(&contents)
            .context("failed to parse into RetentionConfig struct")?;

        let default_retention = retention_with_overrides.epochs_to_keep;

        assert!(
            default_retention > 0,
            "default retention must be greater than 0"
        );
        assert!(
            retention_with_overrides
                .overrides
                .values()
                .all(|&policy| policy > 0),
            "all retention overrides must be greater than 0"
        );

        Ok(Some(retention_with_overrides))
    }
}

impl RetentionConfig {
    /// Creates a new `RetentionConfig` with the specified default retention and
    /// overrides.
    ///
    /// Call `finalize()` on the instance to update the `policies` field with
    /// the default retention policy for all tables that do not have an
    /// override specified.
    pub fn new(epochs_to_keep: u64, overrides: HashMap<PrunableTable, u64>) -> Self {
        Self {
            epochs_to_keep,
            overrides,
        }
    }

    /// Consumes the struct and produces a mapping of every prunable table
    /// and its retention policy.
    ///
    /// By default, every prunable table will have the default retention policy
    /// from `epochs_to_keep`, overridden by any entries in `overrides`.
    pub fn retention_policies(self) -> HashMap<PrunableTable, u64> {
        let RetentionConfig {
            epochs_to_keep,
            mut overrides,
        } = self;

        for table in PrunableTable::iter() {
            overrides.entry(table).or_insert(epochs_to_keep);
        }

        overrides
    }
}

#[derive(Args, Default, Debug, Clone)]
pub struct SnapshotLagConfig {
    /// DEPRECATED: will be removed in v1.31.0. The objects_snapshot pipeline
    /// has been removed. This flag is a no-op.
    #[arg(
        long = "objects-snapshot-min-checkpoint-lag",
        env = "OBJECTS_SNAPSHOT_MIN_CHECKPOINT_LAG"
    )]
    pub snapshot_min_lag: Option<usize>,

    /// DEPRECATED: will be removed in v1.31.0. The objects_snapshot pipeline
    /// has been removed. This flag is a no-op.
    #[arg(long = "objects-snapshot-sleep-duration")]
    pub sleep_duration: Option<u64>,
}

impl SnapshotLagConfig {
    /// Returns `true` if any deprecated flag was explicitly provided.
    pub fn is_set(&self) -> bool {
        self.snapshot_min_lag.is_some() || self.sleep_duration.is_some()
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use tap::Pipe;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::pruning::pruner::PrunableTable;

    fn parse_args<'a, T>(args: impl IntoIterator<Item = &'a str>) -> Result<T, clap::error::Error>
    where
        T: clap::Args + clap::FromArgMatches,
    {
        clap::Command::new("test")
            .no_binary_name(true)
            .pipe(T::augment_args)
            .try_get_matches_from(args)
            .and_then(|matches| T::from_arg_matches(&matches))
    }

    #[test]
    fn name_service() {
        parse_args::<IotaNamesOptions>(["--iota-names-registry-id=0x1"]).unwrap();
        parse_args::<IotaNamesOptions>([
            "--iota-names-package-address",
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        ])
        .unwrap();
        parse_args::<IotaNamesOptions>(["--iota-names-reverse-registry-id=0x1"]).unwrap();
        parse_args::<IotaNamesOptions>([
            "--iota-names-registry-id=0x1",
            "--iota-names-package-address",
            "0x0000000000000000000000000000000000000000000000000000000000000002",
            "--iota-names-reverse-registry-id=0x3",
        ])
        .unwrap();
        parse_args::<IotaNamesOptions>([]).unwrap();
    }

    #[test]
    fn ingestion_sources() {
        parse_args::<IngestionSources>(["--data-ingestion-path=/tmp/foo"]).unwrap();
        parse_args::<IngestionSources>(["--remote-store-url=http://example.com"]).unwrap();

        parse_args::<IngestionSources>([
            "--data-ingestion-path=/tmp/foo",
            "--remote-store-url=http://example.com",
        ])
        .unwrap();

        // live-checkpoints-store-url can be provided if remote-store-url is also
        // provided
        parse_args::<IngestionSources>([
            "--remote-store-url=http://example.com",
            "--live-checkpoints-store-url=http://example.com",
        ])
        .unwrap();

        // live-checkpoints-store-url can't be provided if remote-store-url is not
        // provided
        parse_args::<IngestionSources>(["--live-checkpoints-store-url=http://example.com"])
            .unwrap_err();

        // At least one must be present
        parse_args::<IngestionSources>([]).unwrap_err();
    }

    #[test]
    fn json_rpc_config() {
        parse_args::<JsonRpcConfig>(["--rpc-client-url=http://example.com"]).unwrap();

        // Can include name service options and bind address
        parse_args::<JsonRpcConfig>([
            "--rpc-address=127.0.0.1:8080",
            "--rpc-client-url=http://example.com",
            "--fallback-kv-url=http://example.com/restkv/",
        ])
        .unwrap();

        // fullnode rpc url must be present
        parse_args::<JsonRpcConfig>([]).unwrap_err();
    }

    #[test]
    fn pruning_options_with_overrides() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let toml_content = r#"
        epochs_to_keep = 5
        [overrides]
        tx_senders = 10
        transactions = 20
        "#;
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        let temp_path: PathBuf = temp_file.path().to_path_buf();
        let pruning_options = PruningOptions {
            pruning_config_path: Some(temp_path),
            ..Default::default()
        };
        let retention_config = pruning_options.load_from_file().unwrap().unwrap();

        // Assert the parsed values
        assert_eq!(retention_config.epochs_to_keep, 5);
        assert_eq!(
            retention_config
                .overrides
                .get(&PrunableTable::TxSenders)
                .copied(),
            Some(10)
        );
        assert_eq!(
            retention_config
                .overrides
                .get(&PrunableTable::Transactions)
                .copied(),
            Some(20)
        );
        assert_eq!(retention_config.overrides.len(), 2);

        let retention_policies = retention_config.retention_policies();

        for table in PrunableTable::iter() {
            let Some(retention) = retention_policies.get(&table).copied() else {
                panic!("expected a retention policy for table {table:?}");
            };

            match table {
                PrunableTable::TxSenders => assert_eq!(retention, 10),
                PrunableTable::Transactions => assert_eq!(retention, 20),
                _ => assert_eq!(retention, 5),
            };
        }
    }

    #[test]
    fn test_invalid_pruning_config_file() {
        let toml_str = r#"
        epochs_to_keep = 5
        [overrides]
        tx_senders = 10
        transactions = 20
        invalid_table = 30
        "#;

        let result = toml::from_str::<RetentionConfig>(toml_str);
        assert!(result.is_err(), "expected an error, but parsing succeeded");

        if let Err(e) = result {
            assert!(
                e.to_string().contains("unknown variant `invalid_table`"),
                "error message doesn't mention the invalid table"
            );
        }
    }
}
