// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::Context;
use iota_data_ingestion_core::{
    IndexerExecutor, ReaderOptions, WorkerPool,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};
use iota_metrics::get_metrics;
use tokio::{
    task::JoinHandle,
    time::{Duration, sleep},
};
use tracing::info;

use crate::{
    CancellationToken, PgIndexerStore,
    config::SnapshotLagConfig,
    ingestion::{
        common::{
            orchestration::{ShimIndexerProgressStore, new_executor},
            persist::{CommitterWatermark, Writer},
        },
        primary::persist::TransactionObjectChangesToCommit,
        snapshot::{persist::ObjectSnapshotWriter, prepare::ObjectsSnapshotWorker},
    },
    metrics::IndexerMetrics,
    spawn_monitored_task,
    types::IndexerResult,
};

const OBJECT_SNAPSHOT_CHANNEL_CAPACITY: usize = 600;
const WAIT_FOR_SNAPSHOTTABLE_DATA_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct SnapshotPipelineBuilder {
    writer: ObjectSnapshotWriter,
    checkpoint_download_queue_size: usize,
    cancel: CancellationToken,
    metrics: IndexerMetrics,
    watermark: u64,
}

impl SnapshotPipelineBuilder {
    pub async fn new(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        lag_config: SnapshotLagConfig,
        checkpoint_download_queue_size: usize,
        cancel: CancellationToken,
    ) -> IndexerResult<SnapshotPipelineBuilder> {
        let writer = ObjectSnapshotWriter::new(state.clone(), metrics.clone(), lag_config);
        let watermark = writer.get_watermark_hi().await?.unwrap_or_default();
        Ok(SnapshotPipelineBuilder {
            writer,
            checkpoint_download_queue_size,
            cancel,
            metrics,
            watermark,
        })
    }

    pub async fn finalize_with_dedicated_executor(self) -> IndexerResult<SnapshotPipeline> {
        let mut executor = new_executor(
            "object_snapshot".to_string(),
            self.watermark,
            self.cancel.clone(),
        );
        let receiver = self.register_on_executor(&mut executor).await?;
        Ok(SnapshotPipeline {
            executor: Some(executor),
            writer: self.writer,
            receiver,
            cancel: self.cancel,
        })
    }

    pub async fn finalize_with_shared_executor(
        self,
        executor: &mut IndexerExecutor<ShimIndexerProgressStore>,
    ) -> IndexerResult<SnapshotPipeline> {
        executor
            .update_watermark("object_snapshot".to_string(), self.watermark)
            .await?;
        let receiver = self.register_on_executor(executor).await?;
        Ok(SnapshotPipeline {
            executor: None,
            writer: self.writer,
            receiver,
            cancel: self.cancel,
        })
    }

    async fn register_on_executor(
        &self,
        executor: &mut IndexerExecutor<ShimIndexerProgressStore>,
    ) -> IndexerResult<
        iota_metrics::metered_channel::Receiver<(
            CommitterWatermark,
            TransactionObjectChangesToCommit,
        )>,
    > {
        let global_metrics = get_metrics().unwrap();
        let (sender, receiver) = iota_metrics::metered_channel::channel(
            OBJECT_SNAPSHOT_CHANNEL_CAPACITY,
            &global_metrics
                .channel_inflight
                .with_label_values(&["objects_snapshot_handler_checkpoint_data"]),
        );

        let worker_pool = WorkerPool::new(
            ObjectsSnapshotWorker::new(sender, self.metrics.clone()),
            "object_snapshot".to_string(),
            self.checkpoint_download_queue_size,
            Default::default(),
        );
        executor.register(worker_pool).await?;
        Ok(receiver)
    }
}

pub(crate) struct SnapshotPipeline {
    executor: Option<IndexerExecutor<ShimIndexerProgressStore>>,
    writer: ObjectSnapshotWriter,
    receiver: iota_metrics::metered_channel::Receiver<(
        CommitterWatermark,
        TransactionObjectChangesToCommit,
    )>,
    cancel: CancellationToken,
}

impl SnapshotPipeline {
    fn spawn_writer_task(
        writer: ObjectSnapshotWriter,
        receiver: iota_metrics::metered_channel::Receiver<(
            CommitterWatermark,
            TransactionObjectChangesToCommit,
        )>,
        cancel: CancellationToken,
    ) -> JoinHandle<IndexerResult<()>> {
        spawn_monitored_task!(writer.persist_sequentially(receiver, cancel))
    }

    pub async fn run(
        self,
        remote_store_url: Option<RemoteUrl>,
        reader_options: ReaderOptions,
    ) -> JoinHandle<IndexerResult<()>> {
        let handle = tokio::spawn(async move {
            wait_for_initial_snapshot_lag(&self.writer, &self.cancel).await?;
            let cancel_clone = self.cancel.clone();

            info!("Starting snapshot writer");
            let mut persist_task_handle =
                Self::spawn_writer_task(self.writer.clone(), self.receiver, self.cancel.clone());
            let mut executor_handle = if let Some(executor) = self.executor {
                info!("Starting snapshot executor");
                tokio::spawn(executor.run_with_config(CheckpointReaderConfig {
                    ingestion_path: None, // internally it creates a tempdir.
                    remote_store_url,
                    reader_options,
                }))
            } else {
                info!("Using shared executor - skipping creation of snapshot executor");
                // Create a dummy executor handle that only completes when cancelled
                tokio::spawn(async move {
                    self.cancel.cancelled().await;
                    Ok(HashMap::new())
                })
            };

            let mut executor_done = false;
            let mut persist_done = false;
            while !executor_done || !persist_done {
                tokio::select! {
                    result = &mut executor_handle, if !executor_done => {
                        result.context("failed to join snapshot executor")?.context("snapshot executor failed")?;
                        info!("Snapshot executor finished successfully");
                        executor_done = true;
                    },
                    result = &mut persist_task_handle, if !persist_done => {
                        result.context("failed to join snapshot persist task")?.context("snapshot persist task failed")?;
                        info!("Snapshot persist task finished successfully");
                        persist_done = true;
                    }
                }
                cancel_clone.cancel();
            }

            Ok(())
        });

        handle
    }
}

/// Waits until initial snapshot lag is reached,
/// meaning that the snapshot pipeline is allowed to process the genesis
/// checkpoint.
async fn wait_for_initial_snapshot_lag(
    writer: &ObjectSnapshotWriter,
    cancel: &CancellationToken,
) -> IndexerResult<()> {
    info!("Waiting for data for the Snapshot Pipeline");
    loop {
        match writer.get_max_committable_checkpoint().await {
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
                sleep(WAIT_FOR_SNAPSHOTTABLE_DATA_POLL_INTERVAL).await;
            }
            Err(e) => {
                info!("Error getting max committable checkpoint: {e}, waiting",);
                sleep(WAIT_FOR_SNAPSHOTTABLE_DATA_POLL_INTERVAL).await;
            }
        }

        if cancel.is_cancelled() {
            return Err(crate::errors::IndexerError::Generic(
                "cancelled while waiting for snapshottable data".to_string(),
            ));
        }
    }
    Ok(())
}
