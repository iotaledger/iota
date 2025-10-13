// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, env, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, IndexerExecutor, ProgressStore, ReaderOptions, WorkerPool,
};
use iota_metrics::spawn_monitored_task;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Timeout for waiting for tasks to shutdown gracefully after cancellation
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

use crate::{
    build_json_rpc_server,
    config::{IngestionConfig, JsonRpcConfig, RetentionConfig, SnapshotLagConfig},
    db::ConnectionPool,
    errors::IndexerError,
    handlers::{
        checkpoint_handler::new_handlers, objects_snapshot_handler::start_objects_snapshot_handler,
        optimistic_pruner::OptimisticPruner, pruner::Pruner,
    },
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
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

        let primary_watermark = store
            .get_latest_checkpoint_sequence_number()
            .await
            .expect("Failed to get latest tx checkpoint sequence number from DB")
            .map(|seq| seq + 1)
            .unwrap_or_default();
        let extra_reader_options = ReaderOptions {
            batch_size: config.checkpoint_download_queue_size,
            timeout_secs: config.checkpoint_download_timeout,
            data_limit: config.checkpoint_download_queue_size_bytes,
            ..Default::default()
        };

        // Start objects snapshot processor, which is a separate pipeline with its
        // ingestion pipeline.
        let (object_snapshot_worker, object_snapshot_watermark, mut object_snapshot_task_handle) =
            start_objects_snapshot_handler(
                store.clone(),
                metrics.clone(),
                snapshot_config,
                cancel.clone(),
            )
            .await?;

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

        let mut executor = IndexerExecutor::new(
            ShimIndexerProgressStore::new(vec![
                ("primary".to_string(), primary_watermark),
                ("object_snapshot".to_string(), object_snapshot_watermark),
            ]),
            1,
            DataIngestionMetrics::new(&Registry::new()),
            cancel.child_token(),
        );
        let worker = new_handlers(store, metrics, primary_watermark, cancel.clone()).await?;
        let worker_pool = WorkerPool::new(
            worker,
            "primary".to_string(),
            config.checkpoint_download_queue_size,
            Default::default(),
        );

        executor.register(worker_pool).await?;

        let worker_pool = WorkerPool::new(
            object_snapshot_worker,
            "object_snapshot".to_string(),
            config.checkpoint_download_queue_size,
            Default::default(),
        );
        executor.register(worker_pool).await?;
        info!("Starting data ingestion executor...");
        let mut executor_handle = tokio::spawn(
            executor.run(
                config
                    .sources
                    .data_ingestion_path
                    .clone()
                    .unwrap_or(tempfile::tempdir().unwrap().keep()),
                config
                    .sources
                    .remote_store_url
                    .as_ref()
                    .map(|url| url.as_str().to_owned()),
                vec![],
                extra_reader_options,
            ),
        );

        tokio::select! {
            executor_result = &mut executor_handle => {
                // Executor completed first - cancel snapshot task and check result
                cancel.cancel();
                let snapshot_result = tokio::time::timeout(
                    SHUTDOWN_TIMEOUT,
                    object_snapshot_task_handle
                ).await
                .context("timeout waiting for snapshot task to shutdown");
                executor_result.context("failed to join data ingestion executor")?.context("data ingestion executor failed")?;
                snapshot_result?.context("failed to join snapshot task during shutdown")?.context("snapshot task failed during shutdown")?;
            },
            snapshot_result = &mut object_snapshot_task_handle => {
                // Snapshot task completed first - cancel executor and check result
                cancel.cancel();
                let executor_result = tokio::time::timeout(
                    SHUTDOWN_TIMEOUT,
                    executor_handle
                ).await
                .context("timeout waiting for executor to shutdown");
                snapshot_result.context("failed to join snapshot task")?.context("snapshot task failed")?;
                executor_result?.context("failed to join data ingestion executor during shutdown")?.context("data ingestion executor failed during shutdown")?;
            }
        };

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
            .expect("Json rpc server should not run into errors upon start.");
        tokio::spawn(async move { handle.stopped().await })
            .await
            .expect("Rpc server task failed");

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

struct ShimIndexerProgressStore {
    watermarks: HashMap<String, CheckpointSequenceNumber>,
}

impl ShimIndexerProgressStore {
    fn new(watermarks: Vec<(String, CheckpointSequenceNumber)>) -> Self {
        Self {
            watermarks: watermarks.into_iter().collect(),
        }
    }
}

#[async_trait]
impl ProgressStore for ShimIndexerProgressStore {
    type Error = IndexerError;

    async fn load(&mut self, task_name: String) -> Result<CheckpointSequenceNumber, Self::Error> {
        Ok(*self.watermarks.get(&task_name).expect("missing watermark"))
    }

    async fn save(&mut self, _: String, _: CheckpointSequenceNumber) -> Result<(), Self::Error> {
        Ok(())
    }
}
