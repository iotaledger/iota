// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use iota_data_ingestion_core::WorkerPool;
use iota_metrics::get_metrics;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    errors::IndexerError,
    ingestion::primary::{persist::PrimaryWriter, prepare::PrimaryWorker},
    metrics::IndexerMetrics,
    store::PgIndexerStore,
    types::IndexerResult,
};
const CHECKPOINT_QUEUE_SIZE: usize = 100;

pub async fn setup_primary(
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    checkpoint_download_queue_size: usize,
) -> Result<(WorkerPool<PrimaryWorker>, PrimaryWriter), IndexerError> {
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

    Ok((
        worker_pool,
        PrimaryWriter::new(state, metrics, indexed_checkpoint_receiver),
    ))
}

pub(crate) async fn start_primary_writer_task(
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
