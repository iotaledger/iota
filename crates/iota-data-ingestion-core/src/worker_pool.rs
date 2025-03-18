// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    fmt::Debug,
    sync::Arc,
    time::Instant,
};

use futures::StreamExt;
use iota_metrics::spawn_monitored_task;
use iota_rest_api::CheckpointData;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    IngestionError, IngestionResult, Reducer, Worker, executor::MAX_CHECKPOINTS_IN_PROGRESS,
    reducer::reduce,
};

type TaskName = String;
type WorkerID = usize;

/// Represents the possible message types a [`WorkerPool`] can communicate with
/// external components
#[derive(Debug, Clone)]
pub enum WorkerPoolStatus {
    /// Message with information (e.g. `(<task-name>,
    /// checkpoint_sequence_number)`) about the ingestion progress.
    Running((TaskName, CheckpointSequenceNumber)),
    /// Message with information (e.g. `<task-name>`) about shutdown status
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
    /// Message with information (e.g. `<worker-id>`) about shutdown status
    Shutdown(WorkerID),
}

pub struct WorkerPool<W: Worker> {
    pub task_name: String,
    concurrency: usize,
    worker: Arc<W>,
    reducer: Option<Box<dyn Reducer<W>>>,
}

impl<W: Worker + 'static> WorkerPool<W> {
    pub fn new(worker: W, task_name: String, concurrency: usize) -> Self {
        Self {
            task_name,
            concurrency,
            worker: Arc::new(worker),
            reducer: None,
        }
    }

    pub fn new_with_reducer<R>(worker: W, task_name: String, concurrency: usize, reducer: R) -> Self
    where
        R: Reducer<W> + 'static,
    {
        Self {
            task_name,
            concurrency,
            worker: Arc::new(worker),
            reducer: Some(Box::new(reducer)),
        }
    }

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
        // mian loop
        let (progress_sender, mut progress_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        // This channel will be used to send Workers progress data from WorkerPool to
        // watermark tracking task
        let (watermark_sender, watermark_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let mut idle: BTreeSet<_> = (0..self.concurrency).collect();
        let mut checkpoints = VecDeque::new();
        let mut workers_shutdown_signals = vec![];
        let (workers, workers_join_handles) = self.spawn_workers(progress_sender, token.clone());
        // Spawn a task that tracks checkpoint processing progress. The task:
        // - Receives (checkpoint_number, message) pairs from workers
        // - Maintains checkpoint sequence order
        // - Reports progress either:
        //   * After processing each chunk (simple tracking)
        //   * After committing batches (with reducer)
        let watermark_handle = self.spawn_watermark_tracking(
            watermark,
            watermark_receiver,
            pool_status_sender.clone(),
        );
        // main worker pool loop
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
                            // further checkpoints will be sent to the workers
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
                            // Track workers that have initiated shutdown
                            workers_shutdown_signals.push(worker_id);
                        }
                    }
                }
                // Adding an if guard to this branch ensure that no checkpoints
                // will be sent to workers once the token has been cancelled
                Some(checkpoint) = checkpoint_receiver.recv(), if !token.is_cancelled() => {
                    let sequence_number = checkpoint.checkpoint_summary.sequence_number;
                    if sequence_number < watermark {
                        continue;
                    }
                    self.worker
                        .preprocess_hook(&checkpoint)
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
            // process
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
    /// in parallel
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
                            let backoff = backoff::ExponentialBackoff::default();
                            let status = backoff::future::retry(backoff, || async {
                               let processed_checkpoint_result = worker
                                    .process_checkpoint(&checkpoint)
                                    .await
                                    .map_err(|err| {
                                        let err = IngestionError::CheckpointProcessing(err.to_string());
                                        info!("transient worker execution error {err:?} for checkpoint {sequence_number}");
                                        backoff::Error::transient(err)
                                    });
                                if processed_checkpoint_result.is_err() && token.is_cancelled() {
                                    return Ok(WorkerStatus::Shutdown(worker_id))
                                }
                                processed_checkpoint_result
                                    .map(|message| WorkerStatus::Running((worker_id, sequence_number, message)))
                            })
                            .await
                            .expect("checkpoint processing failed for checkpoint");

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
    ) -> JoinHandle<Result<(), IngestionError>> {
        let task_name = self.task_name.clone();
        if let Some(reducer) = self.reducer.take() {
            return spawn_monitored_task!(reduce::<W>(
                task_name,
                watermark,
                watermark_receiver,
                executor_progress_sender,
                reducer,
            ));
        };
        spawn_monitored_task!(simple_watermark_tracking::<W>(
            task_name,
            watermark,
            watermark_receiver,
            executor_progress_sender
        ))
    }

    /// Start the workers graceful shutdown
    ///
    /// - Awaits all worker handles
    /// - Awaits the reducer handle
    /// - Send `WorkerPoolStatus::Shutdown(<task-name>)` message notifying
    ///   external components that Worker Pool has been shutdown
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
        // watermark tracker task will exit its loop
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
/// 1. Receiving batches of progress status from workers
/// 2. Processing them in sequence order
/// 3. Reporting progress to the executor after each chunk from the stream
async fn simple_watermark_tracking<W: Worker>(
    task_name: String,
    mut current_checkpoint_number: CheckpointSequenceNumber,
    watermark_receiver: mpsc::Receiver<(CheckpointSequenceNumber, W::Message)>,
    executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
) -> IngestionResult<()> {
    // convert to a stream of MAX_CHECKPOINTS_IN_PROGRESS size. This way, each
    // iteration of the loop will process all ready messages
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
        // report progress update to executor
        if let Some(watermark) = progress_update.take() {
            executor_progress_sender
                .send(WorkerPoolStatus::Running((task_name.clone(), watermark)))
                .await
                .map_err(|_| IngestionError::Channel("unable to send worker pool progress updates to executor, receiver half closed".into()))?;
        }
    }
    Ok(())
}
