// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_data_ingestion_core::{
    DataIngestionMetrics, IndexerExecutor, IngestionResult, ReaderOptions, WorkerPool,
};
use iota_metrics::get_metrics;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::Registry;
use tokio::{
    task::JoinHandle,
    time::{Duration, sleep},
};
use tracing::info;

use crate::{
    CancellationToken, PgIndexerStore,
    config::SnapshotLagConfig,
    ingestion::{
        common::{orchestration::ShimIndexerProgressStore, persist::Writer},
        primary::persist::TransactionObjectChangesToCommit,
        snapshot::{persist::ObjectSnapshotWriter, prepare::ObjectsSnapshotWorker},
    },
    metrics::IndexerMetrics,
    spawn_monitored_task,
    types::IndexerResult,
};

const OBJECT_SNAPSHOT_CHANNEL_CAPACITY: usize = 600;

pub(crate) struct SnapshotPipeline {
    pub executor: Option<IndexerExecutor<ShimIndexerProgressStore>>,
    pub writer: ObjectSnapshotWriter,
    pub receiver: iota_metrics::metered_channel::Receiver<(u64, TransactionObjectChangesToCommit)>,
}

impl SnapshotPipeline {
    pub async fn setup(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        lag_config: SnapshotLagConfig,
        checkpoint_download_queue_size: usize,
        cancel: CancellationToken,
    ) -> IndexerResult<SnapshotPipeline> {
        let writer = ObjectSnapshotWriter::new(state.clone(), metrics.clone(), lag_config);

        let watermark = writer.get_watermark_hi().await?.unwrap_or_default();
        let mut executor = IndexerExecutor::new(
            ShimIndexerProgressStore::new(Default::default()),
            1,
            DataIngestionMetrics::new(&Registry::new()),
            cancel.child_token(),
        );
        let receiver = Self::register_on_executor(
            &mut executor,
            metrics.clone(),
            checkpoint_download_queue_size,
            watermark,
        )
        .await?;
        Ok(SnapshotPipeline {
            executor: Some(executor),
            writer,
            receiver,
        })
    }

    pub async fn setup_with_shared_executor(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        lag_config: SnapshotLagConfig,
        checkpoint_download_queue_size: usize,
        executor: &mut IndexerExecutor<ShimIndexerProgressStore>,
    ) -> IndexerResult<SnapshotPipeline> {
        let writer = ObjectSnapshotWriter::new(state.clone(), metrics.clone(), lag_config);

        let watermark = writer.get_watermark_hi().await?.unwrap_or_default();
        let receiver = Self::register_on_executor(
            executor,
            metrics.clone(),
            checkpoint_download_queue_size,
            watermark,
        )
        .await?;

        Ok(SnapshotPipeline {
            executor: None,
            writer,
            receiver,
        })
    }

    async fn register_on_executor(
        executor: &mut IndexerExecutor<ShimIndexerProgressStore>,
        metrics: IndexerMetrics,
        checkpoint_download_queue_size: usize,
        watermark: CheckpointSequenceNumber,
    ) -> IndexerResult<
        iota_metrics::metered_channel::Receiver<(u64, TransactionObjectChangesToCommit)>,
    > {
        let global_metrics = get_metrics().unwrap();
        let (sender, receiver) = iota_metrics::metered_channel::channel(
            OBJECT_SNAPSHOT_CHANNEL_CAPACITY,
            &global_metrics
                .channel_inflight
                .with_label_values(&["objects_snapshot_handler_checkpoint_data"]),
        );

        let worker_pool = WorkerPool::new(
            ObjectsSnapshotWorker::new(sender, metrics),
            "object_snapshot".to_string(),
            checkpoint_download_queue_size,
            Default::default(),
        );
        executor
            .update_watermark("object_snapshot".to_string(), watermark)
            .await?;
        executor.register(worker_pool).await?;
        Ok(receiver)
    }

    fn spawn_writer_task(
        writer: ObjectSnapshotWriter,
        receiver: iota_metrics::metered_channel::Receiver<(u64, TransactionObjectChangesToCommit)>,
        cancel: CancellationToken,
    ) -> JoinHandle<IndexerResult<()>> {
        spawn_monitored_task!(writer.persist_sequentially(receiver, cancel))
    }

    pub async fn wait_for_snapshottable_data(
        self,
        cancel: CancellationToken,
    ) -> IndexerResult<Self> {
        info!("Waiting for data for the Snapshot Pipeline");
        loop {
            match self.writer.get_max_committable_checkpoint().await {
                Ok(max_committable) if max_committable > 0 => {
                    info!(
                        "Max committable checkpoint is {max_committable}, snapshottable data present",
                    );
                    break;
                }
                Ok(max_committable) => {
                    info!(
                        "Max committable checkpoint is {max_committable}, waiting for snapshottable data",
                    );
                    sleep(Duration::from_secs(1)).await;
                }
                Err(e) => {
                    info!("Error getting max committable checkpoint: {e}, waiting",);
                    sleep(Duration::from_secs(1)).await;
                }
            }

            if cancel.is_cancelled() {
                return Err(crate::errors::IndexerError::Generic(
                    "cancelled while waiting for snapshottable data".to_string(),
                ));
            }
        }
        Ok(self)
    }

    pub async fn run(
        self,
        remote_store_url: Option<String>,
        reader_options: ReaderOptions,
        cancel: CancellationToken,
    ) -> IndexerResult<(
        JoinHandle<IngestionResult<impl std::fmt::Debug>>,
        JoinHandle<IndexerResult<()>>,
    )> {
        info!("Starting snapshot writer");
        let persist_task_handle =
            Self::spawn_writer_task(self.writer.clone(), self.receiver, cancel.clone());
        let dummy_ingestion_path = tempfile::tempdir().unwrap().keep();
        let executor_handle = if let Some(executor) = self.executor {
            info!("Starting snapshot executor");
            tokio::spawn(executor.run(
                dummy_ingestion_path,
                remote_store_url,
                vec![],
                reader_options,
            ))
        } else {
            info!("Using shared executor - skipping creation of snapshot executor");
            // Create a dummy executor handle that only completes when cancelled
            tokio::spawn(async move {
                cancel.cancelled().await;
                Ok(HashMap::new())
            })
        };

        Ok((executor_handle, persist_task_handle))
    }
}
