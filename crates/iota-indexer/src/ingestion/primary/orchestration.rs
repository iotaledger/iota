// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anyhow::Context;
use iota_data_ingestion_core::{
    IndexerExecutor, WorkerPool,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};
use iota_metrics::get_metrics;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    ingestion::{
        common::orchestration::{ShimIndexerProgressStore, new_executor},
        primary::{persist::PrimaryWriter, prepare::PrimaryWorker},
    },
    metrics::IndexerMetrics,
    spawn_monitored_task,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    types::IndexerResult,
};

const CHECKPOINT_QUEUE_SIZE: usize = 100;

pub(crate) struct PrimaryPipeline {
    pub executor: IndexerExecutor<ShimIndexerProgressStore>,
    writer: PrimaryWriter,
    watermark: CheckpointSequenceNumber,
    cancel: CancellationToken,
}

impl PrimaryPipeline {
    pub async fn setup(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        checkpoint_download_queue_size: usize,
        cancel: CancellationToken,
    ) -> IndexerResult<PrimaryPipeline> {
        let watermark = state
            .get_latest_checkpoint_sequence_number()
            .await
            .expect("failed to get latest tx checkpoint sequence number from DB")
            .map(|seq| seq + 1)
            .unwrap_or_default();
        let mut executor = new_executor("primary".to_string(), watermark, cancel.clone());
        let checkpoint_queue_size = std::env::var("CHECKPOINT_QUEUE_SIZE")
            .unwrap_or(CHECKPOINT_QUEUE_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        let global_metrics = get_metrics().unwrap();
        let (indexed_checkpoint_sender, indexed_checkpoint_receiver) =
            iota_metrics::metered_channel::channel(
                checkpoint_queue_size,
                &global_metrics
                    .channel_inflight
                    .with_label_values(&["checkpoint_indexing"]),
            );
        let worker_pool = WorkerPool::new(
            PrimaryWorker::new(metrics.clone(), indexed_checkpoint_sender),
            "primary".to_string(),
            checkpoint_download_queue_size,
            Default::default(),
        );
        let writer = PrimaryWriter::new(state, metrics, indexed_checkpoint_receiver);
        executor.register(worker_pool).await?;
        Ok(PrimaryPipeline {
            executor,
            writer,
            watermark,
            cancel,
        })
    }

    pub async fn run(
        self,
        data_ingestion_path: Option<std::path::PathBuf>,
        remote_store_url: Option<RemoteUrl>,
        reader_options: iota_data_ingestion_core::ReaderOptions,
    ) -> JoinHandle<IndexerResult<()>> {
        let writer_cancel = self.cancel.clone();
        let cancel = self.cancel.clone();

        let handle = tokio::spawn(async move {
            info!("Starting primary writer...");
            let mut writer_handle = spawn_monitored_task!(start_writer_task(
                self.writer,
                self.watermark,
                writer_cancel
            ));

            info!("Starting primary executor...");
            let mut executor_handle =
                tokio::spawn(self.executor.run_with_config(CheckpointReaderConfig {
                    ingestion_path: data_ingestion_path,
                    remote_store_url,
                    reader_options,
                }));

            let mut executor_done = false;
            let mut writer_done = false;
            while !executor_done || !writer_done {
                tokio::select! {
                    result = &mut executor_handle, if !executor_done => {
                        result.context("failed to join primary executor")?.context("primary executor failed")?;
                        info!("Primary executor finished successfully");
                        executor_done = true;
                    },
                    result = &mut writer_handle, if !writer_done => {
                        result.context("failed to join primary writer")?.context("primary writer failed")?;
                        info!("Primary writer finished successfully");
                        writer_done = true;
                    }
                }
                cancel.cancel();
            }

            Ok(())
        });

        handle
    }
}

async fn start_writer_task(
    mut writer: PrimaryWriter,
    mut next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    use futures::StreamExt;

    info!("Indexer checkpoint commit task started...");
    let mut unprocessed = HashMap::new();
    let mut batch = vec![];

    while let Some(indexed_checkpoint_batch) = writer.stream.next().await {
        if cancel.is_cancelled() {
            break;
        }

        // split the batch into smaller batches per epoch to handle partitioning
        for checkpoint in indexed_checkpoint_batch {
            unprocessed.insert(checkpoint.checkpoint.sequence_number, checkpoint);
        }
        while let Some(checkpoint) = unprocessed.remove(&next_checkpoint_sequence_number) {
            let epoch = checkpoint.epoch.clone();
            batch.push(checkpoint);
            next_checkpoint_sequence_number += 1;
            // The batch will consist of contiguous checkpoints and at most one epoch
            // boundary at the end.
            if batch.len() == writer.checkpoint_commit_batch_size || epoch.is_some() {
                writer.commit_checkpoints(batch, epoch).await;
                batch = vec![];
            }
        }
        if !batch.is_empty() {
            writer.commit_checkpoints(batch, None).await;
            batch = vec![];
        }
    }
    Ok(())
}
