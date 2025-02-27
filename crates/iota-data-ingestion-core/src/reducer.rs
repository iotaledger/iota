// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use async_trait::async_trait;
use futures::StreamExt;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, Worker,
    worker_pool::WorkerPoolStatus,
};

#[async_trait]
pub trait Reducer<Mapper: Worker>: Send + Sync {
    async fn commit(&self, batch: Vec<Mapper::Message>) -> Result<(), Mapper::Error>;

    fn should_close_batch(
        &self,
        _batch: &[Mapper::Message],
        next_item: Option<&Mapper::Message>,
    ) -> bool {
        next_item.is_none()
    }
}

/// Processes worker messages and manages batching through a reducer.
///
/// This function is the core of the reduction pipeline, handling message
/// batching, watermark tracking, and progress reporting. It maintains message
/// order by checkpoint sequence number and applies batching logic through the
/// provided reducer.
///
/// # Message Processing Flow
///
/// 1. Receives messages in chunks up to [`MAX_CHECKPOINTS_IN_PROGRESS`]
/// 2. Maintains message order using checkpoint sequence numbers
/// 3. Batches messages according to reducer's [`Reducer::should_close_batch`]
///    policy and after that its committed
/// 4. Progress is updated after each commit
/// 5. Reports progress back to the executor
///
/// # Shutdown Behavior
///
/// The function will gracefully exit when receiving a shutdown signal,
/// ensuring no data loss for processed messages.
pub(crate) async fn reduce<W: Worker>(
    task_name: String,
    mut current_checkpoint_number: CheckpointSequenceNumber,
    watermark_receiver: mpsc::Receiver<(CheckpointSequenceNumber, W::Message)>,
    executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
    reducer: Box<dyn Reducer<W>>,
) -> IngestionResult<()> {
    // convert to a stream of MAX_CHECKPOINTS_IN_PROGRESS size. This way, each
    // iteration of the loop will process all ready messages
    let mut stream =
        ReceiverStream::new(watermark_receiver).ready_chunks(MAX_CHECKPOINTS_IN_PROGRESS);
    // store unprocessed progress messages from workers.
    let mut unprocessed = HashMap::new();
    // buffer to accumulate results before passing them to the reducer.
    // The size of this batch is dynamically determined by the reducer's
    // `should_close_batch` method.
    let mut batch = vec![];
    // track the next unprocessed checkpoint number for reporting progress
    // after each chunk of messages is received from the stream.
    let mut progress_update = None;

    while let Some(update_batch) = stream.next().await {
        unprocessed.extend(update_batch.into_iter());
        // Process messages sequentially based on checkpoint sequence number.
        // This ensures in-order processing and maintains progress integrity.
        while let Some(message) = unprocessed.remove(&current_checkpoint_number) {
            // reducer is configured, collect messages in batch based on
            // `reducer.should_close_batch` policy, once a batch is collected it gets
            // committed and a new batch is created with the current message.
            if reducer.should_close_batch(&batch, Some(&message)) {
                reducer
                    .commit(std::mem::take(&mut batch))
                    .await
                    .map_err(|err| {
                        IngestionError::Reducer(format!("failed to commit batch: {err}"))
                    })?;
                batch = vec![message];
                progress_update = Some(current_checkpoint_number);
            } else {
                // Add message to existing batch since no commit needed
                batch.push(message);
            }
            current_checkpoint_number += 1;
        }
        // Handle final batch processing
        // Check if the final batch should be committed.
        // None parameter indicates no more messages available
        if reducer.should_close_batch(&batch, None) {
            reducer
                .commit(std::mem::take(&mut batch))
                .await
                .map_err(|err| IngestionError::Reducer(format!("failed to commit batch: {err}")))?;
            progress_update = Some(current_checkpoint_number);
        }
        // report progress update to executor
        if let Some(watermark) = progress_update {
            executor_progress_sender
                .send(WorkerPoolStatus::Running((task_name.clone(), watermark)))
                .await
                .map_err(|_| IngestionError::Channel("unable to send worker pool progress updates to executor, receiver half closed".into()))?;
            progress_update = None;
        }
    }
    Ok(())
}
