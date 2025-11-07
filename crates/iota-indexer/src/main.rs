// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::env;

use clap::{CommandFactory, FromArgMatches, Parser};
use iota_indexer::{
    backfill::runner::BackfillRunner,
    config::{Command, IndexerConfig, deprecated::OldIndexerConfig},
    db::{
        check_prunable_tables_valid, get_pool_connection, new_connection_pool, reset_database,
        setup_postgres::{check_db_migration_consistency, run_migrations},
    },
    errors::IndexerError,
    indexer::Indexer,
    metrics::{IndexerMetrics, spawn_connection_pool_metric_collector, start_prometheus_server},
    store::{PgIndexerAnalyticalStore, PgIndexerStore},
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    // NOTE: this is to print out tracing like info, warn & error.
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();
    warn!(
        "WARNING: IOTA indexer is still experimental and we expect occasional breaking changes that require backfills."
    );

    let old_conf = OldIndexerConfig::try_parse();

    let opts = match old_conf {
        Ok(old_conf) => old_conf.try_into()?,
        Err(_) => IndexerConfig::from_arg_matches_mut(
            &mut IndexerConfig::command().version(VERSION).get_matches(),
        )
        .unwrap_or_else(|e| e.exit()),
    };

    let (_registry_service, registry) = start_prometheus_server(opts.metrics_address)?;
    iota_metrics::init_metrics(&registry);
    let indexer_metrics = IndexerMetrics::new(&registry);

    if let Command::HelpDeprecated = opts.command {
        OldIndexerConfig::command().print_help().map_err(|e| {
            IndexerError::Generic(format!("failed printing deprecated CLI help: {e}"))
        })?;
        return Ok(());
    }

    let connection_pool = new_connection_pool(
        opts.database_url
            .ok_or(IndexerError::InvalidArgument(
                "--database-url argument is mandatory for this command".into(),
            ))?
            .as_str(),
        &opts.connection_pool_config,
    )?;
    spawn_connection_pool_metric_collector(indexer_metrics.clone(), connection_pool.clone());

    match opts.command {
        Command::Indexer {
            ingestion_config,
            snapshot_config,
            pruning_options,
            reset_db,
        } => {
            let retention_config = pruning_options.load_from_file()?;
            {
                // Make sure to run all migrations on startup, and also serve as a compatibility
                // check.
                let mut pool_conn = get_pool_connection(&connection_pool)?;
                if reset_db {
                    reset_database(&mut pool_conn)?;
                } else {
                    run_migrations(&mut pool_conn)?;
                }
                if retention_config.is_some() {
                    check_prunable_tables_valid(&mut pool_conn).await?;
                }
            }

            let store = PgIndexerStore::new(connection_pool, indexer_metrics.clone());
            Indexer::start_writer_with_config(
                &ingestion_config,
                store,
                indexer_metrics,
                snapshot_config,
                retention_config,
                pruning_options.optimistic_pruner_batch_size,
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

            let store = PgIndexerStore::new(connection_pool.clone(), indexer_metrics.clone());
            Indexer::start_reader(
                &json_rpc_config,
                store,
                &registry,
                connection_pool,
                indexer_metrics,
            )
            .await?;
        }
        Command::AnalyticalWorker => {
            let store = PgIndexerAnalyticalStore::new(connection_pool);
            return Indexer::start_analytical_worker(store, indexer_metrics.clone()).await;
        }
        Command::HelpDeprecated => unreachable!("this case is handled earlier"),
        Command::RunBackfill {
            start,
            end,
            runner_kind,
            backfill_config,
        } => {
            let total_range = start..=end;
            BackfillRunner::run(runner_kind, connection_pool, backfill_config, total_range).await?;
        }
    }

    Ok(())
}
