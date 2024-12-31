// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, pin::Pin};

use anyhow::Result;
use futures::Future;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use prometheus::Registry;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, ReaderOptions, Worker,
    progress_store::{ExecutorProgress, ProgressStore, ProgressStoreWrapper, ShimProgressStore},
    reader::CheckpointReader,
    worker_pool::WorkerPool,
};

pub const MAX_CHECKPOINTS_IN_PROGRESS: usize = 10000;

#[derive(Debug, Clone)]
pub enum WorkerPoolMsg {
    /// Send WorkerPool progress status to Executor main loop
    Progress((String, u64)),
    /// Signal WorkerPool graceful shutdown to Executor main loop
    ShutDown(String),
}

pub struct IndexerExecutor<P> {
    pools: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
    pool_senders: Vec<mpsc::Sender<CheckpointData>>,
    progress_store: ProgressStoreWrapper<P>,
    pool_progress_sender: mpsc::Sender<WorkerPoolMsg>,
    pool_progress_receiver: mpsc::Receiver<WorkerPoolMsg>,
    metrics: DataIngestionMetrics,
    token: CancellationToken,
}

impl<P: ProgressStore> IndexerExecutor<P> {
    pub fn new(
        progress_store: P,
        number_of_jobs: usize,
        metrics: DataIngestionMetrics,
        token: CancellationToken,
    ) -> Self {
        let (pool_progress_sender, pool_progress_receiver) =
            mpsc::channel(number_of_jobs * MAX_CHECKPOINTS_IN_PROGRESS);
        Self {
            pools: vec![],
            pool_senders: vec![],
            progress_store: ProgressStoreWrapper::new(progress_store),
            pool_progress_sender,
            pool_progress_receiver,
            metrics,
            token,
        }
    }

    /// Registers new worker pool in executor
    pub async fn register<W: Worker + 'static>(&mut self, pool: WorkerPool<W>) -> Result<()> {
        let checkpoint_number = self.progress_store.load(pool.task_name.clone()).await?;
        let (sender, receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        self.pools.push(Box::pin(pool.run(
            checkpoint_number,
            receiver,
            self.pool_progress_sender.clone(),
            self.token.child_token(),
        )));
        self.pool_senders.push(sender);
        Ok(())
    }

    /// Main executor loop
    pub async fn run(
        mut self,
        path: PathBuf,
        remote_store_url: Option<String>,
        remote_store_options: Vec<(String, String)>,
        reader_options: ReaderOptions,
    ) -> Result<ExecutorProgress> {
        let mut reader_checkpoint_number = self.progress_store.min_watermark()?;
        let (checkpoint_reader, mut checkpoint_recv, gc_sender, exit_sender) =
            CheckpointReader::initialize(
                path,
                reader_checkpoint_number,
                remote_store_url,
                remote_store_options,
                reader_options,
            );

        let checkpoint_reader_handle = spawn_monitored_task!(checkpoint_reader.run());

        let worker_pools = std::mem::take(&mut self.pools)
            .into_iter()
            .map(|pool| spawn_monitored_task!(pool))
            .collect::<Vec<JoinHandle<()>>>();

        let mut worker_pools_shutdown_signals = vec![];

        loop {
            tokio::select! {
                Some(worker_pool_progress_msg) = self.pool_progress_receiver.recv() => {
                    match worker_pool_progress_msg {
                        WorkerPoolMsg::Progress((task_name, sequence_number)) => {
                            self.progress_store.save(task_name.clone(), sequence_number).await?;
                            let seq_number = self.progress_store.min_watermark()?;
                            if seq_number > reader_checkpoint_number {
                                gc_sender.send(seq_number).await?;
                                reader_checkpoint_number = seq_number;
                            }
                            self.metrics.data_ingestion_checkpoint.with_label_values(&[&task_name]).set(sequence_number as i64);
                        }
                        // Manages the graceful shutdown sequence of the entire indexer system.
                        //
                        // The shutdown process follows these steps:
                        // 1. Token cancellation triggers:
                        //    a. Individual workers in each pool:
                        //       - Complete current checkpoint processing
                        //       - Send final progress updates
                        //       - Signal completion to their pool
                        //    b. Worker pools:
                        //       - Stop accepting new checkpoints
                        //       - Process remaining progress messages
                        //       - Wait for all their workers to finish
                        //       - Send ShutDown message to executor
                        //
                        // 2. Executor main loop:
                        //    - Continues processing Progress messages from pools
                        //    - Tracks pool shutdowns via ShutDown messages
                        //    - Once all pools report shutdown:
                        //      a. Awaits all worker pool join handles
                        //      b. Signals checkpoint reader to stop
                        //      c. Awaits checkpoint reader completion
                        //      d. Exits main loop
                        //
                        // This ensures hierarchical shutdown order:
                        // 1. Workers (in parallel within each pool)
                        // 2. Worker pools (in parallel)
                        // 3. Checkpoint reader
                        // 4. Executor main loop
                        //
                        // Guarantees:
                        // - No work is interrupted mid-processing
                        // - All progress is saved to storage
                        // - All messages are processed in order
                        // - All resources are properly cleaned up
                        WorkerPoolMsg::ShutDown(worker_pool_name) => {
                            // Track worker pools that have initiated shutdown
                            worker_pools_shutdown_signals.push(worker_pool_name);
                            // Once all workers pools have signaled completion, await their handles
                            // This ensures all workers have finished their final tasks
                            if worker_pools_shutdown_signals.len() == self.pool_senders.len() {
                                for worker in worker_pools {
                                    // Await the Worker actor completion
                                    worker.await?;
                                }
                                // Send shutdown signal to CheckpointReader Actor
                                _ = exit_sender.send(());
                                // Await the CheckpointReader actor completion
                                checkpoint_reader_handle.await??;
                                break;
                            }
                        }
                    }
                }
                // Only process new checkpoints while system is running (token not cancelled).
                // The guard prevents accepting new work during shutdown while allowing existing work to complete for other branches.
                Some(checkpoint) = checkpoint_recv.recv(), if !self.token.is_cancelled() => {
                    for sender in &self.pool_senders {
                        sender.send(checkpoint.clone()).await?;
                    }
                }
            }
        }

        Ok(self.progress_store.stats())
    }
}

pub async fn setup_single_workflow<W: Worker + 'static>(
    worker: W,
    remote_store_url: String,
    initial_checkpoint_number: CheckpointSequenceNumber,
    concurrency: usize,
    reader_options: Option<ReaderOptions>,
) -> Result<(
    impl Future<Output = Result<ExecutorProgress>>,
    CancellationToken,
)> {
    let metrics = DataIngestionMetrics::new(&Registry::new());
    let progress_store = ShimProgressStore(initial_checkpoint_number);
    let token = CancellationToken::new();
    let mut executor = IndexerExecutor::new(progress_store, 1, metrics, token.child_token());
    let worker_pool = WorkerPool::new(worker, "workflow".to_string(), concurrency);
    executor.register(worker_pool).await?;
    Ok((
        executor.run(
            tempfile::tempdir()?.into_path(),
            Some(remote_store_url),
            vec![],
            reader_options.unwrap_or_default(),
        ),
        token,
    ))
}
