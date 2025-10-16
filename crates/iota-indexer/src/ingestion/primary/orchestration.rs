// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_data_ingestion_core::{DataIngestionMetrics, IndexerExecutor, WorkerPool};
use iota_metrics::get_metrics;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::Registry;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    ingestion::{
        common::orchestration::ShimIndexerProgressStore,
        primary::{persist::PrimaryWriter, prepare::PrimaryWorker},
    },
    metrics::IndexerMetrics,
    spawn_monitored_task,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    types::IndexerResult,
};

const CHECKPOINT_QUEUE_SIZE: usize = 100;

pub(crate) struct PrimaryPipeline {
    pub primary_executor: IndexerExecutor<ShimIndexerProgressStore>,
    pub primary_writer: PrimaryWriter,
    pub primary_watermark: CheckpointSequenceNumber,
}

impl PrimaryPipeline {
    pub async fn setup(
        state: PgIndexerStore,
        metrics: IndexerMetrics,
        checkpoint_download_queue_size: usize,
        cancel: CancellationToken,
    ) -> IndexerResult<PrimaryPipeline> {
        let primary_watermark = state
            .get_latest_checkpoint_sequence_number()
            .await
            .expect("failed to get latest tx checkpoint sequence number from DB")
            .map(|seq| seq + 1)
            .unwrap_or_default();
        let primary_progress_store =
            ShimIndexerProgressStore::new(vec![("primary".to_string(), primary_watermark)]);
        let mut primary_executor = IndexerExecutor::new(
            primary_progress_store,
            1,
            DataIngestionMetrics::new(&Registry::new()),
            cancel.child_token(),
        );
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
        let primary_worker_pool = WorkerPool::new(
            PrimaryWorker::new(metrics.clone(), indexed_checkpoint_sender),
            "primary".to_string(),
            checkpoint_download_queue_size,
            Default::default(),
        );
        let primary_writer = PrimaryWriter::new(state, metrics, indexed_checkpoint_receiver);
        primary_executor.register(primary_worker_pool).await?;
        Ok(PrimaryPipeline {
            primary_executor,
            primary_writer,
            primary_watermark,
        })
    }

    pub async fn run(
        self,
        data_ingestion_path: std::path::PathBuf,
        remote_store_url: Option<String>,
        reader_options: iota_data_ingestion_core::ReaderOptions,
        cancel: CancellationToken,
    ) -> IndexerResult<(
        JoinHandle<iota_data_ingestion_core::IngestionResult<impl std::fmt::Debug>>,
        JoinHandle<IndexerResult<()>>,
    )> {
        info!("Starting primary writer...");
        let primary_writer_task_handle = spawn_monitored_task!(start_primary_writer_task(
            self.primary_writer,
            self.primary_watermark,
            cancel.clone()
        ));

        info!("Starting primary executor...");
        let primary_executor_handle = tokio::spawn(self.primary_executor.run(
            data_ingestion_path,
            remote_store_url,
            vec![],
            reader_options,
        ));

        Ok((primary_executor_handle, primary_writer_task_handle))
    }
}

async fn start_primary_writer_task(
    mut primary_writer: PrimaryWriter,
    mut next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    use futures::StreamExt;

    info!("Indexer checkpoint commit task started...");
    let mut unprocessed = HashMap::new();
    let mut batch = vec![];

    while let Some(indexed_checkpoint_batch) = primary_writer.stream.next().await {
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
            if batch.len() == primary_writer.checkpoint_commit_batch_size || epoch.is_some() {
                primary_writer.commit_checkpoints(batch, epoch).await;
                batch = vec![];
            }
        }
        if !batch.is_empty() {
            primary_writer.commit_checkpoints(batch, None).await;
            batch = vec![];
        }
    }
    Ok(())
}
