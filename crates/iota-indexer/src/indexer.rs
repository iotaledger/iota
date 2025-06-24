// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, env};

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::{
    DataIngestionMetrics, IndexerExecutor, ProgressStore, ReaderOptions, WorkerPool,
};
use iota_metrics::spawn_monitored_task;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::Registry;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    build_optimistic_json_rpc_server,
    config::{IngestionConfig, JsonRpcConfig, PruningOptions, SnapshotLagConfig},
    db::ConnectionPool,
    errors::IndexerError,
    handlers::{
        checkpoint_handler::new_handlers, objects_snapshot_handler::start_objects_snapshot_handler,
        pruner::Pruner,
    },
    indexer_reader::IndexerReader,
    metrics::IndexerMetrics,
    processors::processor_orchestrator::ProcessorOrchestrator,
    store::{IndexerAnalyticalStore, IndexerStore, PgIndexerStore},
};

pub struct Indexer;

impl Indexer {
    pub async fn start_writer(
        config: &IngestionConfig,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
    ) -> Result<(), IndexerError> {
        let snapshot_config = SnapshotLagConfig::default();
        Indexer::start_writer_with_config(
            config,
            store,
            metrics,
            snapshot_config,
            PruningOptions::default(),
            CancellationToken::new(),
        )
        .await
    }

    pub async fn start_writer_with_config(
        config: &IngestionConfig,
        store: PgIndexerStore,
        metrics: IndexerMetrics,
        snapshot_config: SnapshotLagConfig,
        pruning_options: PruningOptions,
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
        let (object_snapshot_worker, object_snapshot_watermark) = start_objects_snapshot_handler(
            store.clone(),
            metrics.clone(),
            snapshot_config,
            cancel.clone(),
        )
        .await?;

        if let Some(epochs_to_keep) = pruning_options.epochs_to_keep {
            info!(
                "Starting indexer pruner with epochs to keep: {}",
                epochs_to_keep
            );
            assert!(epochs_to_keep > 0, "Epochs to keep must be positive");
            let pruner: Pruner = Pruner::new(store.clone(), epochs_to_keep, metrics.clone())?;
            spawn_monitored_task!(pruner.start(CancellationToken::new()));
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
        executor
            .run(
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
            )
            .await?;
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
        let indexer_reader = IndexerReader::new(connection_pool);
        let handle =
            build_optimistic_json_rpc_server(store, registry, indexer_reader, config, metrics)
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
