// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::ingestion::common::persist::CHECKPOINT_COMMIT_BATCH_SIZE;
use crate::ingestion::primary::prepare::CheckpointHandler;
use crate::{
    errors::IndexerError,
    ingestion::primary::persist::{CheckpointDataToCommit, commit_checkpoints},
    metrics::IndexerMetrics,
    store::PgIndexerStore,
    types::IndexerResult,
};

use iota_metrics::{get_metrics, spawn_monitored_task};
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
use tracing::info;
const CHECKPOINT_QUEUE_SIZE: usize = 100;

pub async fn new_handlers(
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> Result<CheckpointHandler, IndexerError> {
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

    let metrics_clone = metrics.clone();
    spawn_monitored_task!(start_tx_checkpoint_commit_task(
        state,
        metrics_clone,
        indexed_checkpoint_receiver,
        next_checkpoint_sequence_number,
        cancel.clone()
    ));
    Ok(CheckpointHandler::new(metrics, indexed_checkpoint_sender))
}

pub(crate) async fn start_tx_checkpoint_commit_task(
    state: PgIndexerStore,
    metrics: IndexerMetrics,
    tx_indexing_receiver: iota_metrics::metered_channel::Receiver<CheckpointDataToCommit>,
    mut next_checkpoint_sequence_number: CheckpointSequenceNumber,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    use futures::StreamExt;

    info!("Indexer checkpoint commit task started...");
    let checkpoint_commit_batch_size = std::env::var("CHECKPOINT_COMMIT_BATCH_SIZE")
        .unwrap_or(CHECKPOINT_COMMIT_BATCH_SIZE.to_string())
        .parse::<usize>()
        .unwrap();
    info!("Using checkpoint commit batch size {checkpoint_commit_batch_size}");

    let mut stream = iota_metrics::metered_channel::ReceiverStream::new(tx_indexing_receiver)
        .ready_chunks(checkpoint_commit_batch_size);

    let mut unprocessed = HashMap::new();
    let mut batch = vec![];

    while let Some(indexed_checkpoint_batch) = stream.next().await {
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
            if batch.len() == checkpoint_commit_batch_size || epoch.is_some() {
                commit_checkpoints(&state, batch, epoch, &metrics).await;
                batch = vec![];
            }
        }
        if !batch.is_empty() {
            commit_checkpoints(&state, batch, None, &metrics).await;
            batch = vec![];
        }
    }
    Ok(())
}
