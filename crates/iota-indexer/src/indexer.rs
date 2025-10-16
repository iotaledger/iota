// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::env;

use anyhow::{Context, Result};
use iota_data_ingestion_core::ReaderOptions;
use iota_metrics::spawn_monitored_task;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    build_json_rpc_server,
    config::{IngestionConfig, JsonRpcConfig, RetentionConfig, SnapshotLagConfig},
    db::ConnectionPool,
    errors::IndexerError,
    ingestion::{
        primary::orchestration::PrimaryPipeline, snapshot::orchestration::SnapshotPipeline,
    },
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
    pruning::{optimistic_pruner::OptimisticPruner, pruner::Pruner},
    read::IndexerReader,
    store::{IndexerAnalyticalStore, IndexerStore, PgIndexerStore},
};

pub struct Indexer;

impl Indexer {
    pub async fn start_writer_with_config(
        config: &IngestionConfig,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
        snapshot_config: SnapshotLagConfig,
        retention_config: Option<RetentionConfig>,
        optimistic_pruner_batch_size: Option<u64>,
        cancel: CancellationToken,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Writer (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );

        info!("IOTA Indexer Writer config: {config:?}",);
        let extra_reader_options = ReaderOptions {
            batch_size: config.checkpoint_download_queue_size,
            timeout_secs: config.checkpoint_download_timeout,
            data_limit: config.checkpoint_download_queue_size_bytes,
            ..Default::default()
        };
        let data_ingestion_path = config
            .sources
            .data_ingestion_path
            .clone()
            .unwrap_or(tempfile::tempdir().unwrap().keep());
        let remote_store_url = config
            .sources
            .remote_store_url
            .as_ref()
            .map(|url| url.as_str().to_owned());

        if let Some(retention_config) = retention_config {
            let pruner = Pruner::new(store.clone(), retention_config, metrics.clone())?;
            let cancel_clone = cancel.clone();
            spawn_monitored_task!(pruner.start(cancel_clone));
        }

        if let Some(optimistic_pruner_batch_size) = optimistic_pruner_batch_size {
            info!("Starting indexer optimistic tables pruner");
            let optimistic_pruner = OptimisticPruner::new(
                store.clone(),
                optimistic_pruner_batch_size,
                metrics.clone(),
            )?;
            let cancellation_token_for_optimistic_pruner = cancel.child_token();
            spawn_monitored_task!(
                optimistic_pruner.start(cancellation_token_for_optimistic_pruner)
            );
        }

        // If we already have chain identifier indexed (i.e. the first checkpoint has
        // been indexed), then we persist protocol configs for protocol versions
        // not yet in the db. Otherwise, we would do the persisting in
        // `commit_checkpoint` while the first cp is being indexed.
        if let Some(chain_id) = IndexerStore::get_chain_identifier(&store).await? {
            store.persist_protocol_configs_and_feature_flags(chain_id)?;
        }

        let mut primary_pipeline = PrimaryPipeline::setup(
            store.clone(),
            metrics.clone(),
            config.checkpoint_download_queue_size,
            cancel.clone(),
        )
        .await?;

        // data_ingestion_path can only feed data to one executor,
        // but if we have remote_store_url we can use many executors
        let use_separate_executors = remote_store_url.is_some();
        let snapshot_pipeline = if use_separate_executors {
            // SnapshotPipeline::setup will create a separate executor
            SnapshotPipeline::setup(
                store.clone(),
                metrics.clone(),
                snapshot_config,
                config.checkpoint_download_queue_size,
                cancel.clone(),
            )
            .await?
        } else {
            warn!(
                "Sharing the same executor between Primary and Snapshot pipelines due to not \
                 provided --remote-store-url argument. Limited possibilities for Snapshot lag \
                 config. This may be deprecated in the future."
            );
            SnapshotPipeline::setup_with_shared_executor(
                store.clone(),
                metrics.clone(),
                snapshot_config,
                config.checkpoint_download_queue_size,
                &mut primary_pipeline.primary_executor,
            )
            .await?
        };

        info!("Starting data ingestion executor...");
        let (mut primary_executor_handle, mut primary_writer_handle) = primary_pipeline
            .run(
                data_ingestion_path.clone(),
                remote_store_url.clone(),
                extra_reader_options.clone(),
                cancel.clone(),
            )
            .await?;

        // Wait for max committable checkpoint > 0 before starting snapshot executor
        // Also monitor primary_executor_handle - if it finishes, no point in waiting
        let (mut snapshot_executor_handle, mut snapshot_persist_task_handle) = tokio::select! {
            snapshot_pipeline_with_snapshottable_data = snapshot_pipeline.wait_for_snapshottable_data(cancel.clone()) => {
                snapshot_pipeline_with_snapshottable_data?.run(
                    remote_store_url,
                    extra_reader_options,
                    cancel.clone(),
                ).await?
            },
            result = &mut primary_executor_handle => {
                result.context("failed to join primary executor")?.context("primary executor failed")?;
                return Ok(());
            }
        };

        let mut primary_executor_done = false;
        let mut primary_writer_done = false;
        let mut snapshot_executor_done = false;
        let mut snapshot_persist_task_done = false;
        while !primary_executor_done
            || !primary_writer_done
            || !snapshot_executor_done
            || !snapshot_persist_task_done
        {
            tokio::select! {
                result = &mut primary_executor_handle, if !primary_executor_done => {
                    result.context("failed to join primary executor")?.context("primary executor failed")?;
                    info!("Primary executor finished successfully");
                    primary_executor_done = true;
                },
                result = &mut primary_writer_handle, if !primary_writer_done => {
                    result.context("failed to join primary writer")?.context("primary writer failed")?;
                    info!("Primary writer finished successfully");
                    primary_writer_done = true;
                },
                result = &mut snapshot_executor_handle, if !snapshot_executor_done => {
                    result.context("failed to join snapshot executor")?.context("snapshot executor failed")?;
                    info!("Snapshot executor finished successfully");
                    snapshot_executor_done = true;
                },
                result = &mut snapshot_persist_task_handle, if !snapshot_persist_task_done => {
                    result.context("failed to join snapshot persist task")?.context("snapshot persist task failed")?;
                    info!("Snapshot persist task finished successfully");
                    snapshot_persist_task_done = true;
                }
            }
            cancel.cancel();
        }

        Ok(())
    }

    pub async fn start_reader(
        config: &JsonRpcConfig,
        store: PgIndexerStore,
        registry: &Registry,
        connection_pool: ConnectionPool,
        metrics: IndexerMetrics,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Reader (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );
        let read = IndexerReader::new(connection_pool);
        let handle = build_json_rpc_server(store, registry, read, config, metrics)
            .await
            .expect("json rpc server should not run into errors upon start.");
        tokio::spawn(async move { handle.stopped().await })
            .await
            .expect("rpc server task failed");

        Ok(())
    }
    pub async fn start_analytical_worker<
        S: IndexerAnalyticalStore + Clone + Send + Sync + 'static,
    >(
        store: S,
        metrics: IndexerMetrics,
    ) -> Result<(), IndexerError> {
        info!(
            "IOTA Indexer Analytical Worker (version {:?}) started...",
            env!("CARGO_PKG_VERSION")
        );
        let mut processor_orchestrator = ProcessorOrchestrator::new(store, metrics);
        processor_orchestrator.run_forever().await;
        Ok(())
    }
}
