// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use iota_names::config::IotaNamesConfig;
use iota_types::base_types::{IotaAddress, ObjectID};
use url::Url;

use crate::db::ConnectionPoolConfig;

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
    pub object_id: ObjectID,
    #[arg(default_value_t = IotaNamesConfig::default().payments_package_address)]
    #[arg(long = "iota-names-payments-package-address")]
    pub payments_package_address: IotaAddress,
    #[arg(default_value_t = IotaNamesConfig::default().registry_id)]
    #[arg(long = "iota-names-registry-id")]
    pub registry_id: ObjectID,
    #[arg(default_value_t = IotaNamesConfig::default().reverse_registry_id)]
    #[arg(long = "iota-names-reverse-registry-id")]
    pub reverse_registry_id: ObjectID,
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

    #[clap(long, default_value = "0.0.0.0:9000")]
    pub rpc_address: SocketAddr,

    #[clap(long)]
    pub rpc_client_url: String,
}

#[derive(Args, Debug, Default, Clone)]
#[group(required = true, multiple = true)]
pub struct IngestionSources {
    #[arg(long)]
    pub data_ingestion_path: Option<PathBuf>,

    #[arg(long)]
    pub remote_store_url: Option<Url>,

    #[arg(long)]
    pub rpc_client_url: Option<Url>,
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
    /// Print help for the deprecated interface.
    HelpDeprecated,
}

#[derive(Args, Default, Debug, Clone)]
pub struct PruningOptions {
    #[arg(long, env = "EPOCHS_TO_KEEP")]
    pub epochs_to_keep: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub struct SnapshotLagConfig {
    #[arg(
        long = "objects-snapshot-min-checkpoint-lag",
        default_value_t = Self::DEFAULT_MIN_LAG,
        env = "OBJECTS_SNAPSHOT_MIN_CHECKPOINT_LAG",
    )]
    pub snapshot_min_lag: usize,

    #[arg(
        long = "objects-snapshot-sleep-duration",
        default_value_t = Self::DEFAULT_SLEEP_DURATION_SEC,
    )]
    pub sleep_duration: u64,
}

impl SnapshotLagConfig {
    pub const DEFAULT_MIN_LAG: usize = 300;
    pub const DEFAULT_SLEEP_DURATION_SEC: u64 = 5;
}

impl Default for SnapshotLagConfig {
    fn default() -> Self {
        SnapshotLagConfig {
            snapshot_min_lag: Self::DEFAULT_MIN_LAG,
            sleep_duration: Self::DEFAULT_SLEEP_DURATION_SEC,
        }
    }
}

pub mod deprecated {
    use std::{net::SocketAddr, path::PathBuf, time::Duration};

    use anyhow::bail;
    use clap::Parser;
    use secrecy::{ExposeSecret, Secret};
    use url::Url;

    use crate::{
        config::{
            Command, IndexerConfig, IngestionConfig, IngestionSources, IotaNamesOptions,
            JsonRpcConfig, PruningOptions, SnapshotLagConfig,
        },
        db::ConnectionPoolConfig,
        errors::IndexerError,
    };

    #[derive(Parser, Clone, Debug)]
    #[command(
        name = "IOTA indexer",
        about = "An off-fullnode service serving data from IOTA protocol"
    )]
    pub struct OldIndexerConfig {
        #[arg(long)]
        pub db_url: Option<Secret<String>>,
        #[arg(long)]
        pub db_user_name: Option<String>,
        #[arg(long)]
        pub db_password: Option<Secret<String>>,
        #[arg(long)]
        pub db_host: Option<String>,
        #[arg(long)]
        pub db_port: Option<u16>,
        #[arg(long)]
        pub db_name: Option<String>,
        #[arg(long, default_value = "http://0.0.0.0:9000", global = true)]
        pub rpc_client_url: String,
        #[arg(long, default_value = Some("http://0.0.0.0:9000/api/v1"), global = true)]
        pub remote_store_url: Option<String>,
        #[arg(long, default_value = "0.0.0.0", global = true)]
        pub client_metric_host: String,
        #[arg(long, default_value = "9184", global = true)]
        pub client_metric_port: u16,
        #[arg(long, default_value = "0.0.0.0", global = true)]
        pub rpc_server_url: String,
        #[arg(long, default_value = "9000", global = true)]
        pub rpc_server_port: u16,
        #[arg(long)]
        pub reset_db: bool,
        #[arg(long)]
        pub fullnode_sync_worker: bool,
        #[arg(long)]
        pub rpc_server_worker: bool,
        #[arg(long)]
        pub data_ingestion_path: Option<PathBuf>,
        #[arg(long)]
        pub analytical_worker: bool,
        #[command(flatten)]
        pub iota_names_options: IotaNamesOptions,
    }

    impl OldIndexerConfig {
        /// returns connection url without the db name
        pub fn base_connection_url(&self) -> anyhow::Result<String, anyhow::Error> {
            let url_secret = self.get_db_url()?;
            let url_str = url_secret.expose_secret();
            let url = Url::parse(url_str).expect("Failed to parse URL");
            Ok(format!(
                "{}://{}:{}@{}:{}/",
                url.scheme(),
                url.username(),
                url.password().unwrap_or_default(),
                url.host_str().unwrap_or_default(),
                url.port().unwrap_or_default()
            ))
        }

        pub fn get_db_url(&self) -> anyhow::Result<Secret<String>, anyhow::Error> {
            match (
                &self.db_url,
                &self.db_user_name,
                &self.db_password,
                &self.db_host,
                &self.db_port,
                &self.db_name,
            ) {
                (Some(db_url), _, _, _, _, _) => Ok(db_url.clone()),
                (
                    None,
                    Some(db_user_name),
                    Some(db_password),
                    Some(db_host),
                    Some(db_port),
                    Some(db_name),
                ) => Ok(secrecy::Secret::new(format!(
                    "postgres://{}:{}@{}:{}/{}",
                    db_user_name,
                    db_password.expose_secret(),
                    db_host,
                    db_port,
                    db_name
                ))),
                _ => bail!(
                    "Invalid db connection config, either db_url or (db_user_name, db_password, db_host, db_port, db_name) must be provided"
                ),
            }
        }
    }

    impl Default for OldIndexerConfig {
        fn default() -> Self {
            Self {
                db_url: Some(secrecy::Secret::new(
                    "postgres://postgres:postgrespw@localhost:5432/iota_indexer".to_string(),
                )),
                db_user_name: None,
                db_password: None,
                db_host: None,
                db_port: None,
                db_name: None,
                rpc_client_url: "http://127.0.0.1:9000".to_string(),
                remote_store_url: Some("http://127.0.0.1:9000/api/v1".to_string()),
                client_metric_host: "0.0.0.0".to_string(),
                client_metric_port: 9184,
                rpc_server_url: "0.0.0.0".to_string(),
                rpc_server_port: 9000,
                reset_db: false,
                fullnode_sync_worker: true,
                rpc_server_worker: true,
                data_ingestion_path: None,
                analytical_worker: false,
                iota_names_options: IotaNamesOptions::default(),
            }
        }
    }

    fn pool_config_from_env() -> ConnectionPoolConfig {
        let db_pool_size = std::env::var("DB_POOL_SIZE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(ConnectionPoolConfig::DEFAULT_POOL_SIZE);
        let conn_timeout_secs = std::env::var("DB_CONNECTION_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(ConnectionPoolConfig::DEFAULT_CONNECTION_TIMEOUT);
        let statement_timeout_secs = std::env::var("DB_STATEMENT_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(ConnectionPoolConfig::DEFAULT_STATEMENT_TIMEOUT);

        ConnectionPoolConfig {
            pool_size: db_pool_size,
            connection_timeout: Duration::from_secs(conn_timeout_secs),
            statement_timeout: Duration::from_secs(statement_timeout_secs),
        }
    }

    impl TryFrom<OldIndexerConfig> for IndexerConfig {
        type Error = IndexerError;
        fn try_from(mut old_conf: OldIndexerConfig) -> Result<Self, Self::Error> {
            old_conf.remote_store_url = Some(format!("{}/api/v1", old_conf.rpc_client_url));

            let db_url = old_conf.get_db_url();

            // NOTE: this parses the input host addr and port number for socket addr,
            // so unwrap() is safe here.
            let metrics_address = format!(
                "{}:{}",
                old_conf.client_metric_host, old_conf.client_metric_port
            )
            .parse()
            .unwrap();

            let download_queue_size = std::env::var("DOWNLOAD_QUEUE_SIZE")
                .unwrap_or_else(|_| {
                    IngestionConfig::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE.to_string()
                })
                .parse::<usize>()
                .expect("Invalid DOWNLOAD_QUEUE_SIZE");
            let ingestion_reader_timeout_secs = std::env::var("INGESTION_READER_TIMEOUT_SECS")
                .unwrap_or_else(|_| {
                    IngestionConfig::DEFAULT_CHECKPOINT_DOWNLOAD_TIMEOUT.to_string()
                })
                .parse::<u64>()
                .expect("Invalid INGESTION_READER_TIMEOUT_SECS");
            let data_limit = std::env::var("CHECKPOINT_PROCESSING_BATCH_DATA_LIMIT")
                .unwrap_or(
                    IngestionConfig::DEFAULT_CHECKPOINT_DOWNLOAD_QUEUE_SIZE_BYTES.to_string(),
                )
                .parse::<usize>()
                .unwrap();

            let snapshot_min_lag = std::env::var("OBJECTS_SNAPSHOT_MIN_CHECKPOINT_LAG")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(SnapshotLagConfig::DEFAULT_MIN_LAG);

            let rpc_client_url_parsed = old_conf
                .rpc_client_url
                .parse()
                .expect("RPC Client url should be valid");

            let command = if old_conf.analytical_worker {
                Command::AnalyticalWorker
            } else if old_conf.rpc_server_worker {
                Command::JsonRpcService(JsonRpcConfig {
                    iota_names_options: old_conf.iota_names_options,
                    rpc_address: SocketAddr::new(
                        old_conf
                            .rpc_server_url
                            .as_str()
                            .parse()
                            .expect("RPC Server url should be valid"),
                        old_conf.rpc_server_port,
                    ),
                    rpc_client_url: old_conf.rpc_client_url,
                })
            } else if old_conf.fullnode_sync_worker {
                Command::Indexer {
                    ingestion_config: IngestionConfig {
                        sources: IngestionSources {
                            data_ingestion_path: old_conf.data_ingestion_path,
                            remote_store_url: old_conf.remote_store_url.map(|url| {
                                url.parse().expect("Remote Store URL should be correct")
                            }),
                            rpc_client_url: Some(rpc_client_url_parsed),
                        },
                        checkpoint_download_queue_size: download_queue_size,
                        checkpoint_download_timeout: ingestion_reader_timeout_secs,
                        checkpoint_download_queue_size_bytes: data_limit,
                    },
                    snapshot_config: SnapshotLagConfig {
                        snapshot_min_lag,
                        sleep_duration: SnapshotLagConfig::DEFAULT_SLEEP_DURATION_SEC,
                    },
                    pruning_options: PruningOptions {
                        epochs_to_keep: std::env::var("EPOCHS_TO_KEEP")
                            .map(|s| s.parse::<u64>().ok())
                            .unwrap_or_else(|_e| None),
                    },
                    reset_db: old_conf.reset_db,
                }
            } else {
                return Err(IndexerError::InvalidArgument(
                    "Worker type argument not specified".into(),
                ));
            };

            Ok(IndexerConfig {
                database_url: Some(
                    db_url
                        .map_err(|e| {
                            IndexerError::PgPoolConnection(format!(
                                "Failed parsing database url with error {e:?}"
                            ))
                        })?
                        .expose_secret()
                        .parse()
                        .expect("Database URL should be correct"),
                ),
                connection_pool_config: pool_config_from_env(),
                metrics_address,
                command,
            })
        }
    }
}

#[cfg(test)]
mod test {
    use tap::Pipe;

    use super::*;

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
        parse_args::<IngestionSources>(["--rpc-client-url=http://example.com"]).unwrap();

        parse_args::<IngestionSources>([
            "--data-ingestion-path=/tmp/foo",
            "--remote-store-url=http://example.com",
            "--rpc-client-url=http://example.com",
        ])
        .unwrap();

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
        ])
        .unwrap();

        // fullnode rpc url must be present
        parse_args::<JsonRpcConfig>([]).unwrap_err();
    }
}
