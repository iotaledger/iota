// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use backoff::{ExponentialBackoff, backoff::Backoff};
use futures::StreamExt;
use iota_metrics::spawn_monitored_task;
use iota_rest_api::CheckpointData;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    IngestionError, IngestionResult, Reducer, Worker, executor::MAX_CHECKPOINTS_IN_PROGRESS,
    reducer::reduce, util::reset_backoff,
};

type TaskName = String;
type WorkerID = usize;

/// Represents the possible message types a [`WorkerPool`] can communicate with
/// external components.
#[derive(Debug, Clone)]
pub enum WorkerPoolStatus {
    /// Message with information (e.g. `(<task-name>,
    /// checkpoint_sequence_number)`) about the ingestion progress.
    Running((TaskName, CheckpointSequenceNumber)),
    /// Message with information (e.g. `<task-name>`) about shutdown status.
    Shutdown(String),
}

/// Represents the possible message types a [`Worker`] can communicate with
/// external components
#[derive(Debug, Clone, Copy)]
enum WorkerStatus<M> {
    /// Message with information (e.g. `(<worker-id>`,
    /// `checkpoint_sequence_number`, [`Worker::Message`]) about the ingestion
    /// progress.
    Running((WorkerID, CheckpointSequenceNumber, M)),
    /// Message with information (e.g. `<worker-id>`) about shutdown status.
    Shutdown(WorkerID),
}

/// A pool of [`Worker`]'s that process checkpoints concurrently.
///
/// This struct manages a collection of workers that process checkpoints in
/// parallel. It handles checkpoint distribution, progress tracking, and
/// graceful shutdown. It can optionally use a [`Reducer`] to aggregate and
/// process worker [`Messages`](Worker::Message).
///
/// # Examples
/// ## Direct Processing (Without Batching)
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{Worker, WorkerPool};
/// use iota_types::full_checkpoint_content::{CheckpointData, CheckpointTransaction};
/// #
/// # struct DatabaseClient;
/// #
/// # impl DatabaseClient {
/// #     pub fn new() -> Self {
/// #         Self
/// #     }
/// #
/// #     pub async fn store_transaction(&self,
/// #         _transactions: &CheckpointTransaction,
/// #     ) -> Result<(), DatabaseError> {
/// #         Ok(())
/// #     }
/// # }
/// #
/// # #[derive(Debug, Clone)]
/// # struct DatabaseError;
/// #
/// # impl std::fmt::Display for DatabaseError {
/// #     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
/// #         write!(f, "database error")
/// #     }
/// # }
/// #
/// # fn extract_transaction(checkpoint: &CheckpointData) -> CheckpointTransaction {
/// #     checkpoint.transactions.first().unwrap().clone()
/// # }
///
/// struct DirectProcessor {
///     // generic Database client.
///     client: Arc<DatabaseClient>,
/// }
///
/// #[async_trait]
/// impl Worker for DirectProcessor {
///     type Message = ();
///     type Error = DatabaseError;
///
///     async fn process_checkpoint(
///         &self,
///         checkpoint: Arc<CheckpointData>,
///     ) -> Result<Self::Message, Self::Error> {
///         // extract a particulat transaction we care about.
///         let tx: CheckpointTransaction = extract_transaction(checkpoint.as_ref());
///         // store the transaction in our database of choice.
///         self.client.store_transaction(&tx).await?;
///         Ok(())
///     }
/// }
///
/// // configure worker pool for direct processing.
/// let processor = DirectProcessor {
///     client: Arc::new(DatabaseClient::new()),
/// };
/// let pool = WorkerPool::new(processor, "direct_processing".into(), 5, Default::default());
/// ```
///
/// ## Batch Processing (With Reducer)
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{Reducer, Worker, WorkerPool};
/// use iota_types::full_checkpoint_content::{CheckpointData, CheckpointTransaction};
/// # struct DatabaseClient;
/// #
/// # impl DatabaseClient {
/// #     pub fn new() -> Self {
/// #         Self
/// #     }
/// #
/// #     pub async fn store_transactions_batch(&self,
/// #         _transactions: &Vec<CheckpointTransaction>,
/// #     ) -> Result<(), DatabaseError> {
/// #         Ok(())
/// #     }
/// # }
/// #
/// # #[derive(Debug, Clone)]
/// # struct DatabaseError;
/// #
/// # impl std::fmt::Display for DatabaseError {
/// #     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
/// #         write!(f, "database error")
/// #     }
/// # }
///
/// // worker that accumulates transactions for batch processing.
/// struct BatchProcessor;
///
/// #[async_trait]
/// impl Worker for BatchProcessor {
///     type Message = Vec<CheckpointTransaction>;
///     type Error = DatabaseError;
///
///     async fn process_checkpoint(
///         &self,
///         checkpoint: Arc<CheckpointData>,
///     ) -> Result<Self::Message, Self::Error> {
///         // collect all checkpoint transactions for batch processing.
///         Ok(checkpoint.transactions.clone())
///     }
/// }
///
/// // batch reducer for efficient storage.
/// struct TransactionBatchReducer {
///     batch_size: usize,
///     // generic Database client.
///     client: Arc<DatabaseClient>,
/// }
///
/// #[async_trait]
/// impl Reducer<BatchProcessor> for TransactionBatchReducer {
///     async fn commit(&self, batch: &[Vec<CheckpointTransaction>]) -> Result<(), DatabaseError> {
///         let flattened: Vec<CheckpointTransaction> = batch.iter().flatten().cloned().collect();
///         // store the transaction batch in the database of choice.
///         self.client.store_transactions_batch(&flattened).await?;
///         Ok(())
///     }
///
///     fn should_close_batch(
///         &self,
///         batch: &[Vec<CheckpointTransaction>],
///         _: Option<&Vec<CheckpointTransaction>>,
///     ) -> bool {
///         batch.iter().map(|b| b.len()).sum::<usize>() >= self.batch_size
///     }
/// }
///
/// // configure worker pool with batch processing.
/// let processor = BatchProcessor;
/// let reducer = TransactionBatchReducer {
///     batch_size: 1000,
///     client: Arc::new(DatabaseClient::new()),
/// };
/// let pool = WorkerPool::new_with_reducer(
///     processor,
///     "batch_processing".into(),
///     5,
///     Default::default(),
///     reducer,
/// );
/// ```
pub struct WorkerPool<W: Worker> {
    /// An unique name of the WorkerPool task.
    pub task_name: String,
    /// How many instances of the current [`Worker`] to create, more workers are
    /// created more checkpoints they can process in parallel.
    concurrency: usize,
    /// The actual [`Worker`] instance itself.
    worker: Arc<W>,
    /// The reducer instance, responsible for batch processing.
    reducer: Option<Box<dyn Reducer<W>>>,
    backoff: Arc<ExponentialBackoff>,
}

impl<W: Worker + 'static> WorkerPool<W> {
    /// Creates a new `WorkerPool` without a reducer.
    pub fn new(
        worker: W,
        task_name: String,
        concurrency: usize,
        backoff: ExponentialBackoff,
    ) -> Self {
        Self {
            task_name,
            concurrency,
            worker: Arc::new(worker),
            reducer: None,
            backoff: Arc::new(backoff),
        }
    }

    /// Creates a new `WorkerPool` with a reducer.
    pub fn new_with_reducer<R>(
        worker: W,
        task_name: String,
        concurrency: usize,
        backoff: ExponentialBackoff,
        reducer: R,
    ) -> Self
    where
        R: Reducer<W> + 'static,
    {
        Self {
            task_name,
            concurrency,
            worker: Arc::new(worker),
            reducer: Some(Box::new(reducer)),
            backoff: Arc::new(backoff),
        }
    }

    /// Runs the worker pool main logic.
    pub async fn run(
        mut self,
        watermark: CheckpointSequenceNumber,
        mut checkpoint_receiver: mpsc::Receiver<Arc<CheckpointData>>,
        pool_status_sender: mpsc::Sender<WorkerPoolStatus>,
        token: CancellationToken,
    ) {
        info!(
            "Starting indexing pipeline {} with concurrency {}. Current watermark is {watermark}.",
            self.task_name, self.concurrency
        );
        // This channel will be used to send progress data from Workers to WorkerPool
        // mian loop.
        let (progress_sender, mut progress_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        // This channel will be used to send Workers progress data from WorkerPool to
        // watermark tracking task.
        let (watermark_sender, watermark_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let mut idle: BTreeSet<_> = (0..self.concurrency).collect();
        let mut checkpoints = VecDeque::new();
        let mut workers_shutdown_signals = vec![];
        let (workers, workers_join_handles) = self.spawn_workers(progress_sender, token.clone());
        // Spawn a task that tracks checkpoint processing progress. The task:
        // - Receives (checkpoint_number, message) pairs from workers.
        // - Maintains checkpoint sequence order.
        // - Reports progress either:
        //   * After processing each chunk (simple tracking).
        //   * After committing batches (with reducer).
        let watermark_handle = self.spawn_watermark_tracking(
            watermark,
            watermark_receiver,
            pool_status_sender.clone(),
            token.clone(),
        );
        // main worker pool loop.
        loop {
            tokio::select! {
                Some(worker_progress_msg) = progress_receiver.recv() => {
                    match worker_progress_msg {
                        WorkerStatus::Running((worker_id, checkpoint_number, message)) => {
                            idle.insert(worker_id);
                            if watermark_sender.send((checkpoint_number, message)).await.is_err() {
                                break;
                            }
                            // By checking if token was not cancelled we ensure that no
                            // further checkpoints will be sent to the workers.
                            while !token.is_cancelled() && !checkpoints.is_empty() && !idle.is_empty() {
                                let checkpoint = checkpoints.pop_front().unwrap();
                                let worker_id = idle.pop_first().unwrap();
                                if workers[worker_id].send(checkpoint).await.is_err() {
                                    // The worker channel closing is a sign we need to exit this loop.
                                    break;
                                }
                            }
                        }
                        WorkerStatus::Shutdown(worker_id) => {
                            // Track workers that have initiated shutdown.
                            workers_shutdown_signals.push(worker_id);
                        }
                    }
                }
                // Adding an if guard to this branch ensure that no checkpoints
                // will be sent to workers once the token has been cancelled.
                Some(checkpoint) = checkpoint_receiver.recv(), if !token.is_cancelled() => {
                    let sequence_number = checkpoint.checkpoint_summary.sequence_number;
                    if sequence_number < watermark {
                        continue;
                    }
                    self.worker
                        .preprocess_hook(checkpoint.clone())
                        .map_err(|err| IngestionError::CheckpointHookProcessing(err.to_string()))
                        .expect("failed to preprocess task");
                    if idle.is_empty() {
                        checkpoints.push_back(checkpoint);
                    } else {
                        let worker_id = idle.pop_first().unwrap();
                        if workers[worker_id].send(checkpoint).await.is_err() {
                            // The worker channel closing is a sign we need to exit this loop.
                            break;
                        };
                    }
                }
            }
            // Once all workers have signaled completion, start the graceful shutdown
            // process.
            if workers_shutdown_signals.len() == self.concurrency {
                break self
                    .workers_graceful_shutdown(
                        workers_join_handles,
                        watermark_handle,
                        pool_status_sender,
                        watermark_sender,
                    )
                    .await;
            }
        }
    }

    /// Spawn workers based on `self.concurrency` to process checkpoints
    /// in parallel.
    fn spawn_workers(
        &self,
        progress_sender: mpsc::Sender<WorkerStatus<W::Message>>,
        token: CancellationToken,
    ) -> (Vec<mpsc::Sender<Arc<CheckpointData>>>, Vec<JoinHandle<()>>) {
        let mut worker_senders = Vec::with_capacity(self.concurrency);
        let mut workers_join_handles = Vec::with_capacity(self.concurrency);

        for worker_id in 0..self.concurrency {
            let (worker_sender, mut worker_recv) =
                mpsc::channel::<Arc<CheckpointData>>(MAX_CHECKPOINTS_IN_PROGRESS);
            let cloned_progress_sender = progress_sender.clone();
            let task_name = self.task_name.clone();
            worker_senders.push(worker_sender);

            let token = token.clone();

            let worker = self.worker.clone();
            let backoff = self.backoff.clone();
            let join_handle = spawn_monitored_task!(async move {
                loop {
                    tokio::select! {
                        // Once token is cancelled, notify worker's shutdown to the main loop
                        _ = token.cancelled() => {
                            _ = cloned_progress_sender.send(WorkerStatus::Shutdown(worker_id)).await;
                            break
                        },
                        Some(checkpoint) = worker_recv.recv() => {
                            let sequence_number = checkpoint.checkpoint_summary.sequence_number;
                            info!("received checkpoint for processing {} for workflow {}", sequence_number, task_name);
                            let start_time = Instant::now();
                            let status = Self::process_checkpoint_with_retry(worker_id, &worker, checkpoint, reset_backoff(&backoff), &token).await;
                            let trigger_shutdown = matches!(status, WorkerStatus::Shutdown(_));
                            if cloned_progress_sender.send(status).await.is_err() || trigger_shutdown {
                                break;
                            }
                            info!(
                                "finished checkpoint processing {sequence_number} for workflow {task_name} in {:?}",
                                start_time.elapsed()
                            );
                        }
                    }
                }
            });
            // Keep all join handles to ensure all workers are terminated before exiting
            workers_join_handles.push(join_handle);
        }
        (worker_senders, workers_join_handles)
    }

    /// Attempts to process a checkpoint with exponential backoff retries on
    /// failure.
    ///
    /// This function repeatedly calls the
    /// [`process_checkpoint`](Worker::process_checkpoint) method of the
    /// provided [`Worker`] until either:
    /// - The checkpoint processing succeeds, returning `WorkerStatus::Running`
    ///   with the processed message.
    /// - A cancellation signal is received via the [`CancellationToken`],
    ///   returning `WorkerStatus::Shutdown(<worker-id>)`.
    /// - All retry attempts are exhausted within backoff's maximum elapsed
    ///   time, causing a panic.
    ///
    /// # Retry Mechanism:
    /// - Uses [`ExponentialBackoff`](backoff::ExponentialBackoff) to introduce
    ///   increasing delays between retry attempts.
    /// - Checks for cancellation both before and after each processing attempt.
    /// - If a cancellation signal is received during a backoff delay, the
    ///   function exits immediately with `WorkerStatus::Shutdown(<worker-id>)`.
    ///
    /// # Panics:
    /// - If all retry attempts are exhausted within the backoff's maximum
    ///   elapsed time, indicating a persistent failure.
    async fn process_checkpoint_with_retry(
        worker_id: WorkerID,
        worker: &W,
        checkpoint: Arc<CheckpointData>,
        mut backoff: ExponentialBackoff,
        token: &CancellationToken,
    ) -> WorkerStatus<W::Message> {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        loop {
            // check for cancellation before attempting processing.
            if token.is_cancelled() {
                return WorkerStatus::Shutdown(worker_id);
            }
            // attempt to process checkpoint.
            match worker.process_checkpoint(checkpoint.clone()).await {
                Ok(message) => return WorkerStatus::Running((worker_id, sequence_number, message)),
                Err(err) => {
                    let err = IngestionError::CheckpointProcessing(err.to_string());
                    warn!(
                        "transient worker execution error {err:?} for checkpoint {sequence_number}"
                    );
                    // check for cancellation after failed processing.
                    if token.is_cancelled() {
                        return WorkerStatus::Shutdown(worker_id);
                    }
                }
            }
            // get next backoff duration or panic if max retries exceeded.
            let duration = backoff
                .next_backoff()
                .expect("max elapsed time exceeded: checkpoint processing failed for checkpoint");
            // if cancellation occurs during backoff wait, exit early with Shutdown.
            // Otherwise (if timeout expires), continue with the next retry attempt.
            if tokio::time::timeout(duration, token.cancelled())
                .await
                .is_ok()
            {
                return WorkerStatus::Shutdown(worker_id);
            }
        }
    }

    /// Spawns a task that tracks the progress of checkpoint processing,
    /// optionally with message reduction.
    ///
    /// This function spawns one of two types of tracking tasks:
    ///
    /// 1. Simple Watermark Tracking (when reducer = None):
    ///    - Reports watermark after processing each chunk.
    ///
    /// 2. Batch Processing (when reducer = Some):
    ///    - Reports progress only after successful batch commits.
    ///    - A batch is committed based on
    ///      [`should_close_batch`](Reducer::should_close_batch) policy.
    fn spawn_watermark_tracking(
        &mut self,
        watermark: CheckpointSequenceNumber,
        watermark_receiver: mpsc::Receiver<(CheckpointSequenceNumber, W::Message)>,
        executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
        token: CancellationToken,
    ) -> JoinHandle<Result<(), IngestionError>> {
        let task_name = self.task_name.clone();
        let backoff = self.backoff.clone();
        if let Some(reducer) = self.reducer.take() {
            return spawn_monitored_task!(reduce::<W>(
                task_name,
                watermark,
                watermark_receiver,
                executor_progress_sender,
                reducer,
                backoff,
                token
            ));
        };
        spawn_monitored_task!(simple_watermark_tracking::<W>(
            task_name,
            watermark,
            watermark_receiver,
            executor_progress_sender
        ))
    }

    /// Start the workers graceful shutdown.
    ///
    /// - Awaits all worker handles.
    /// - Awaits the reducer handle.
    /// - Send `WorkerPoolStatus::Shutdown(<task-name>)` message notifying
    ///   external components that Worker Pool has been shutdown.
    async fn workers_graceful_shutdown(
        &self,
        workers_join_handles: Vec<JoinHandle<()>>,
        watermark_handle: JoinHandle<Result<(), IngestionError>>,
        executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
        watermark_sender: mpsc::Sender<(u64, <W as Worker>::Message)>,
    ) {
        for worker in workers_join_handles {
            _ = worker
                .await
                .inspect_err(|err| tracing::error!("worker task panicked: {err}"));
        }
        // by dropping the sender we make sure that the stream will be closed and the
        // watermark tracker task will exit its loop.
        drop(watermark_sender);
        _ = watermark_handle
            .await
            .inspect_err(|err| tracing::error!("watermark task panicked: {err}"));
        _ = executor_progress_sender
            .send(WorkerPoolStatus::Shutdown(self.task_name.clone()))
            .await;
        tracing::info!("Worker pool `{}` terminated gracefully", self.task_name);
    }
}

/// Tracks checkpoint progress without reduction logic.
///
/// This function maintains a watermark of processed checkpoints by worker:
/// 1. Receiving batches of progress status from workers.
/// 2. Processing them in sequence order.
/// 3. Reporting progress to the executor after each chunk from the stream.
async fn simple_watermark_tracking<W: Worker>(
    task_name: String,
    mut current_checkpoint_number: CheckpointSequenceNumber,
    watermark_receiver: mpsc::Receiver<(CheckpointSequenceNumber, W::Message)>,
    executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
) -> IngestionResult<()> {
    // convert to a stream of MAX_CHECKPOINTS_IN_PROGRESS size. This way, each
    // iteration of the loop will process all ready messages.
    let mut stream =
        ReceiverStream::new(watermark_receiver).ready_chunks(MAX_CHECKPOINTS_IN_PROGRESS);
    // store unprocessed progress messages from workers.
    let mut unprocessed = HashMap::new();
    // track the next unprocessed checkpoint number for reporting progress
    // after each chunk of messages is received from the stream.
    let mut progress_update = None;

    while let Some(update_batch) = stream.next().await {
        unprocessed.extend(update_batch.into_iter());
        // Process messages sequentially based on checkpoint sequence number.
        // This ensures in-order processing and maintains progress integrity.
        while unprocessed.remove(&current_checkpoint_number).is_some() {
            current_checkpoint_number += 1;
            progress_update = Some(current_checkpoint_number);
        }
        // report progress update to executor.
        if let Some(watermark) = progress_update.take() {
            executor_progress_sender
                .send(WorkerPoolStatus::Running((task_name.clone(), watermark)))
                .await
                .map_err(|_| IngestionError::Channel("unable to send worker pool progress updates to executor, receiver half closed".into()))?;
        }
    }
    Ok(())
}
