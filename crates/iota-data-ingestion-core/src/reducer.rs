// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use async_trait::async_trait;
use futures::StreamExt;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, Reducer, Worker,
    worker_pool::WorkerPoolStatus,
};

/// A wrapper type that adapts a [`Reducer`] implementation to use
/// [`IngestionError`] as its error type.
///
/// This wrapper is used internally by the worker pool to standardize error
/// handling across different reducer implementations. It converts the reducer's
/// original error type into [`IngestionError`].
pub(crate) struct ReducerWrapper<R>(R);

impl<R> ReducerWrapper<R> {
    /// Creates a new `ReducerWrapper` instance containing the provided reducer.
    pub(crate) fn new(reducer: R) -> Self {
        Self(reducer)
    }
}

#[async_trait]
impl<R, T> Reducer<T> for ReducerWrapper<R>
where
    R: Reducer<T>,
    R::Error: Debug + Display,
    T: Send + Sync + 'static,
{
    type Error = IngestionError;

    /// Delegates the commit operation to the wrapped reducer and converts its
    /// error type.
    async fn commit(&self, batch: Vec<T>) -> Result<(), Self::Error> {
        self.0
            .commit(batch)
            .await
            .map_err(|err| IngestionError::Reducer(format!("failed to commit batch: {err}")))
    }

    /// Delegates the batch closing decision to the wrapped reducer.
    fn should_close_batch(&self, batch: &[T], next_item: Option<&T>) -> bool {
        self.0.should_close_batch(batch, next_item)
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
///    logic
/// 4. Commits batches when appropriate
/// 5. Reports progress back to the executor
///
/// # Shutdown Behavior
///
/// The function will gracefully exit when receiving a shutdown signal,
/// ensuring no data loss for processed messages.
pub(crate) async fn reduce<W: Worker>(
    task_name: String,
    mut current_checkpoint_number: CheckpointSequenceNumber,
    progress_receiver: mpsc::Receiver<(CheckpointSequenceNumber, W::Message)>,
    executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
    reducer: Option<Box<dyn Reducer<W::Message, Error = IngestionError>>>,
    mut shutdown: oneshot::Receiver<()>,
) -> IngestionResult<()> {
    // convert to a stream of MAX_CHECKPOINTS_IN_PROGRESS size. This way, each
    // iteration of the loop will process all ready messages
    let mut stream =
        ReceiverStream::new(progress_receiver).ready_chunks(MAX_CHECKPOINTS_IN_PROGRESS);
    // store unprocessed progress messages from workers.
    let mut unprocessed = HashMap::new();
    // buffer to accumulate results before passing them to the reducer.
    // The size of this batch is dynamically determined by the reducer's
    // `should_close_batch` method.
    let mut batch = vec![];
    // track the latest processed checkpoint number for reporting progress
    // after each chunk of messages is received from the stream.
    let mut progress_update = None;

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            // get the available progress from workers as a chunks
            Some(update_batch) = stream.next() => {
                unprocessed.extend(update_batch.into_iter());
                // Process messages sequentially based on checkpoint sequence number.
                // This ensures in-order processing and maintains progress integrity.
                while let Some(message) = unprocessed.remove(&current_checkpoint_number) {
                    if let Some(ref reducer) = reducer {
                        // reducer is configured, collect messages in batch based on
                        // `reducer.should_close_batch` policy, once a batch is collected it gets
                        // committed and a new batch is created with the current message.
                        if reducer.should_close_batch(&batch, Some(&message)) {
                            reducer
                                .commit(std::mem::take(&mut batch))
                                .await?;
                            batch = vec![message];
                            progress_update = Some(current_checkpoint_number);
                        } else {
                            // Add message to existing batch since no commit needed
                            batch.push(message);
                        }
                    }
                    current_checkpoint_number += 1;
                }
                // Handle final batch processing
                match reducer {
                    Some(ref reducer) => {
                        // Check if the final batch should be committed.
                        // None parameter indicates no more messages available
                        if reducer.should_close_batch(&batch, None) {
                            reducer
                                .commit(std::mem::take(&mut batch))
                                .await?;
                            progress_update = Some(current_checkpoint_number);
                        }
                    }
                    None => progress_update = Some(current_checkpoint_number),
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
        }
    }
    Ok(())
}
