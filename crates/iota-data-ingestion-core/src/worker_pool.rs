// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};

use iota_metrics::spawn_monitored_task;
use iota_rest_api::CheckpointData;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{IngestionError, Worker, executor::MAX_CHECKPOINTS_IN_PROGRESS};

type TaskName = String;
type WorkerID = usize;

pub struct WorkerPool<W: Worker> {
    pub task_name: String,
    concurrency: usize,
    worker: Arc<W>,
}

/// Represents the possible message types a WorkerPool can communicate with
/// external components
#[derive(Debug, Clone)]
pub enum WorkerPoolStatus {
    /// Message with information (e.g. `(<task-name>,
    /// checkpoint_sequence_number)`) about the ingestion progress.
    Running((TaskName, CheckpointSequenceNumber)),
    /// Message with information (e.g. `<task-name>`) about shutdown status
    Shutdown(String),
}

/// Represents the possible message types a Worker can communicate with external
/// components
#[derive(Debug, Clone, Copy)]
enum WorkerStatus {
    /// Message with information (e.g. `(<worker-id>,
    /// checkpoint_sequence_number,
    /// <saved-progress-store-checkpoint-sequence-number>)`) about the ingestion
    /// progress.
    Running(
        (
            WorkerID,
            CheckpointSequenceNumber,
            Option<CheckpointSequenceNumber>,
        ),
    ),
    /// Message with information (e.g. `<worker-id>`) about shutdown status
    Shutdown(WorkerID),
}

impl<W: Worker + 'static> WorkerPool<W> {
    pub fn new(worker: W, task_name: String, concurrency: usize) -> Self {
        Self {
            task_name,
            concurrency,
            worker: Arc::new(worker),
        }
    }
    pub async fn run(
        self,
        mut current_checkpoint_number: CheckpointSequenceNumber,
        mut checkpoint_receiver: mpsc::Receiver<Arc<CheckpointData>>,
        pool_status_sender: mpsc::Sender<WorkerPoolStatus>,
        token: CancellationToken,
    ) {
        info!(
            "Starting indexing pipeline {} with concurrency {}. Current watermark is {}.",
            self.task_name, self.concurrency, current_checkpoint_number
        );
        let mut updates = HashMap::new();

        let (progress_sender, mut progress_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let mut idle: BTreeSet<_> = (0..self.concurrency).collect();
        let mut checkpoints = VecDeque::new();
        let mut workers_shutdown_signals = vec![];

        let (workers, workers_join_handles) = self.spawn_workers(progress_sender, token.clone());

        // main worker pool loop
        loop {
            tokio::select! {
                Some(worker_progress_msg) = progress_receiver.recv() => {
                    match worker_progress_msg {
                        WorkerStatus::Running((worker_id, status_update, progress_watermark)) => {
                            idle.insert(worker_id);
                            updates.insert(status_update, progress_watermark);
                            if status_update == current_checkpoint_number {
                                let mut executor_status_update = None;
                                while let Some(progress_watermark) = updates.remove(&current_checkpoint_number) {
                                    if let Some(watermark) =  progress_watermark {
                                        executor_status_update = Some(watermark + 1);
                                    }
                                    current_checkpoint_number += 1;
                                }
                                if let Some(update) = executor_status_update {
                                    if pool_status_sender
                                        .send(WorkerPoolStatus::Running((self.task_name.clone(), update)))
                                        .await.is_err() {
                                            // The executor progress channel closing is a sign we need to
                                            // exit this loop.
                                            break;
                                        }
                                }
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
                    if sequence_number < current_checkpoint_number {
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
                    .workers_graceful_shutdown(workers_join_handles, pool_status_sender)
                    .await;
            }
        }
    }

    /// Spawn workers based on `self.concurrency` to process checkpoints
    /// in parallel
    fn spawn_workers(
        &self,
        progress_sender: mpsc::Sender<WorkerStatus>,
        token: CancellationToken,
    ) -> (Vec<mpsc::Sender<Arc<CheckpointData>>>, Vec<JoinHandle<()>>) {
        let mut worker_senders = vec![];
        let mut workers_join_handles = vec![];

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
                            backoff::future::retry(backoff, || async {
                                worker
                                    .clone()
                                    .process_checkpoint(&checkpoint)
                                    .await
                                    .map_err(|err| {
                                        let err = IngestionError::CheckpointProcessing(err.to_string());
                                        info!("transient worker execution error {:?} for checkpoint {}", err, sequence_number);
                                        backoff::Error::transient(err)
                                    })
                            })
                            .await
                            .expect("checkpoint processing failed for checkpoint");
                            info!("finished checkpoint processing {} for workflow {} in {:?}", sequence_number, task_name, start_time.elapsed());
                            if cloned_progress_sender.send(WorkerStatus::Running((worker_id, sequence_number, worker.save_progress(sequence_number).await))).await.is_err() {
                                // The progress channel closing is a sign we need to exit this loop.
                                break;
                            }
                        }
                    }
                }
            });
            // Keep all join handles to ensure all workers are terminated before exiting
            workers_join_handles.push(join_handle);
        }
        (worker_senders, workers_join_handles)
    }

    /// Start the workers graceful shutdown
    ///
    /// - Awaits all worker handles
    /// - Send `WorkerPoolStatus::Shutdown(<task-name>)` message notifying
    ///   external components that Worker Pool has been shutdown
    async fn workers_graceful_shutdown(
        &self,
        workers_join_handles: Vec<JoinHandle<()>>,
        executor_progress_sender: mpsc::Sender<WorkerPoolStatus>,
    ) {
        for worker in workers_join_handles {
            if let Err(err) = worker.await {
                tracing::error!("worker thread panicked: {err}");
            }
        }
        _ = executor_progress_sender
            .send(WorkerPoolStatus::Shutdown(self.task_name.clone()))
            .await;
        tracing::info!("Worker pool `{}` terminated gracefully", self.task_name);
    }
}
