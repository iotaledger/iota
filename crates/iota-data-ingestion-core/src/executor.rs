// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, pin::Pin};

use futures::Future;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use prometheus::Registry;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, IngestionError, IngestionResult, ReaderOptions, Worker,
    progress_store::{ExecutorProgress, ProgressStore, ProgressStoreWrapper, ShimProgressStore},
    reader::CheckpointReader,
    worker_pool::{WorkerPool, WorkerPoolStatus},
};

pub const MAX_CHECKPOINTS_IN_PROGRESS: usize = 10000;

pub struct IndexerExecutor<P> {
    pools: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
    pool_senders: Vec<mpsc::Sender<CheckpointData>>,
    progress_store: ProgressStoreWrapper<P>,
    pool_status_sender: mpsc::Sender<WorkerPoolStatus>,
    pool_status_receiver: mpsc::Receiver<WorkerPoolStatus>,
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
        let (pool_status_sender, pool_status_receiver) =
            mpsc::channel(number_of_jobs * MAX_CHECKPOINTS_IN_PROGRESS);
        Self {
            pools: vec![],
            pool_senders: vec![],
            progress_store: ProgressStoreWrapper::new(progress_store),
            pool_status_sender,
            pool_status_receiver,
            metrics,
            token,
        }
    }

    /// Registers new worker pool in executor
    pub async fn register<W: Worker + 'static>(
        &mut self,
        pool: WorkerPool<W>,
    ) -> IngestionResult<()> {
        let checkpoint_number = self.progress_store.load(pool.task_name.clone()).await?;
        let (sender, receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        self.pools.push(Box::pin(pool.run(
            checkpoint_number,
            receiver,
            self.pool_status_sender.clone(),
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
    ) -> IngestionResult<ExecutorProgress> {
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
                Some(worker_pool_progress_msg) = self.pool_status_receiver.recv() => {
                    match worker_pool_progress_msg {
                        WorkerPoolStatus::Running((task_name, sequence_number)) => {
                            self.progress_store.save(task_name.clone(), sequence_number).await.map_err(|err| IngestionError::ProgressStore(err.to_string()))?;
                            let seq_number = self.progress_store.min_watermark()?;
                            if seq_number > reader_checkpoint_number {
                                gc_sender.send(seq_number).await.map_err(|_| {
                                    IngestionError::Channel(
                                        "unable to send GC operation to checkpoint reader, receiver half closed"
                                            .to_owned(),
                                    )
                                })?;
                                reader_checkpoint_number = seq_number;
                            }
                            self.metrics.data_ingestion_checkpoint.with_label_values(&[&task_name]).set(sequence_number as i64);
                        }
                        WorkerPoolStatus::Shutdown(worker_pool_name) => {
                            // Track worker pools that have initiated shutdown
                            worker_pools_shutdown_signals.push(worker_pool_name);
                        }
                    }
                }
                // Only process new checkpoints while system is running (token not cancelled).
                // The guard prevents accepting new work during shutdown while allowing existing work to complete for other branches.
                Some(checkpoint) = checkpoint_recv.recv(), if !self.token.is_cancelled() => {
                    for sender in &self.pool_senders {
                        sender.send(checkpoint.clone()).await.map_err(|_| {
                            IngestionError::Channel(
                                "unable to send new checkpoint to worker pool, receiver half closed"
                                    .to_owned(),
                            )
                        })?;
                    }
                }
            }

            // Once all workers pools have signaled completion, start the graceful shutdown
            // process
            if worker_pools_shutdown_signals.len() == self.pool_senders.len() {
                break components_graceful_shutdown(
                    worker_pools,
                    exit_sender,
                    checkpoint_reader_handle,
                )
                .await?;
            }
        }

        Ok(self.progress_store.stats())
    }
}

/// Start the graceful shutdown of remaining components
///
/// - Awaits all worker pool handles
/// - Send shutdown signal to checkpoint reader actor
/// - Await checkpoint reader handle
async fn components_graceful_shutdown(
    worker_pools: Vec<JoinHandle<()>>,
    exit_sender: oneshot::Sender<()>,
    checkpoint_reader_handle: JoinHandle<IngestionResult<()>>,
) -> IngestionResult<()> {
    for worker_pool in worker_pools {
        worker_pool.await.map_err(|err| IngestionError::Shutdown {
            component: "Worker Pool".into(),
            msg: err.to_string(),
        })?;
    }
    _ = exit_sender.send(());
    checkpoint_reader_handle
        .await
        .map_err(|err| IngestionError::Shutdown {
            component: "Checkpoint Reader".into(),
            msg: err.to_string(),
        })??;
    Ok(())
}

pub async fn setup_single_workflow<W: Worker + 'static>(
    worker: W,
    remote_store_url: String,
    initial_checkpoint_number: CheckpointSequenceNumber,
    concurrency: usize,
    reader_options: Option<ReaderOptions>,
) -> IngestionResult<(
    impl Future<Output = IngestionResult<ExecutorProgress>>,
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
