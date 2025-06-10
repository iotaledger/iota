// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, net::SocketAddr, time::Duration};

use clap::{CommandFactory, FromArgMatches, Parser};
use iota_indexer::{
    config::{
        Command, IngestionConfig, IngestionSources, JsonRpcConfig, PruningOptions,
        SnapshotLagConfig, deprecated::OldIndexerConfig,
    },
    db::{
        ConnectionPoolConfig, get_pool_connection, new_connection_pool, reset_database,
        setup_postgres::{check_db_migration_consistency, run_migrations},
    },
    errors::IndexerError,
    indexer::Indexer,
    metrics::{IndexerMetrics, spawn_connection_pool_metric_collector, start_prometheus_server},
    store::{PgIndexerAnalyticalStore, PgIndexerStore},
};
use secrecy::ExposeSecret;
use tokio_util::sync::CancellationToken;
use tracing::warn;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

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

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    // NOTE: this is to print out tracing like info, warn & error.
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    warn!(
        "WARNING: IOTA indexer is still experimental and we expect occasional breaking changes that require backfills."
    );

    let opts = match iota_indexer::config::IndexerConfig::try_parse() {
        Ok(opts) => opts,
        Err(e) => {
            e.print()
                .map_err(|e| IndexerError::Generic(format!("Failed writing clap error: {e}")))?;
            warn!(
                "Parsing arguments using new CLI failed. Falling back to the old CLI, note that this will be deprecated in the future."
            );
            let mut old_conf = OldIndexerConfig::from_arg_matches_mut(
                &mut OldIndexerConfig::command().version(VERSION).get_matches(),
            )
            .map_err(|e| {
                IndexerError::Generic(format!("Failed parsing arguments using old CLI: {e}"))
            })?;

            // TODO: Explore other options as in upstream.
            // For the moment we only use the fullnode for fetching checkpoints
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

            iota_indexer::config::IndexerConfig {
                database_url: db_url
                    .map_err(|e| {
                        IndexerError::PgPoolConnection(format!(
                            "Failed parsing database url with error {e:?}"
                        ))
                    })?
                    .expose_secret()
                    .parse()
                    .expect("Database URL should be correct"),
                connection_pool_config: pool_config_from_env(),
                metrics_address,
                command,
            }
        }
    };

    let (_registry_service, registry) = start_prometheus_server(opts.metrics_address)?;
    iota_metrics::init_metrics(&registry);
    let indexer_metrics = IndexerMetrics::new(&registry);

    let connection_pool =
        new_connection_pool(opts.database_url.as_str(), &opts.connection_pool_config)?;
    spawn_connection_pool_metric_collector(indexer_metrics.clone(), connection_pool.clone());

    match opts.command {
        Command::Indexer {
            ingestion_config,
            snapshot_config,
            pruning_options,
            reset_db,
        } => {
            {
                // Make sure to run all migrations on startup, and also serve as a compatibility
                // check.
                let mut pool_conn = get_pool_connection(&connection_pool)?;
                if reset_db {
                    reset_database(&mut pool_conn)?;
                } else {
                    run_migrations(&mut pool_conn)?;
                }
            }

            let store = PgIndexerStore::new(connection_pool, indexer_metrics.clone());
            Indexer::start_writer_with_config(
                &ingestion_config,
                store,
                indexer_metrics,
                snapshot_config,
                pruning_options,
                CancellationToken::new(),
            )
            .await?;
        }
        Command::JsonRpcService(json_rpc_config) => {
            {
                // Run compatibility check
                let mut pool_conn = get_pool_connection(&connection_pool)?;
                check_db_migration_consistency(&mut pool_conn)?;
            }

            Indexer::start_reader(&json_rpc_config, &registry, connection_pool).await?;
        }
        Command::AnalyticalWorker => {
            let store = PgIndexerAnalyticalStore::new(connection_pool);
            return Indexer::start_analytical_worker(store, indexer_metrics.clone()).await;
        }
    }
    Ok(())
}
