// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, pin::Pin, sync::Arc};

use futures::Future;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    committee::EpochId, full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
};
use prometheus::Registry;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    DataIngestionMetrics, IngestionError, IngestionResult, ReaderOptions, Worker,
    progress_store::{ExecutorProgress, ProgressStore, ProgressStoreWrapper, ShimProgressStore},
    reader::{
        v1::CheckpointReader as CheckpointReaderV1,
        v2::{CheckpointReader as CheckpointReaderV2, CheckpointReaderConfig},
    },
    worker_pool::{WorkerPool, WorkerPoolStatus},
};

pub const MAX_CHECKPOINTS_IN_PROGRESS: usize = 10000;

/// Callback function invoked for each incoming checkpoint to determine the
/// shutdown action if it exceeds the ingestion limit.
type ShutdownCallback = Box<dyn Fn(&CheckpointData) -> ShutdownAction + Send>;

/// Determines the shutdown action when a checkpoint reaches the ingestion
/// limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShutdownAction {
    /// Include the current checkpoint in the ingestion process, then initiate
    /// the graceful shutdown process.
    IncludeAndShutdown,
    /// Exclude the current checkpoint from ingestion and immediately initiate
    /// the graceful shutdown process.
    ExcludeAndShutdown,
    /// Continue processing the current checkpoint without shutting down.
    Continue,
}

/// Common policies for upper limit checkpoint ingestion by the framework.
///
/// Once the limit is reached, the framework will start the graceful
/// shutdown process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IngestionLimit {
    /// Last checkpoint sequence number to process.
    ///
    /// After processing this checkpoint the framework will start the graceful
    /// shutdown process.
    MaxCheckpoint(CheckpointSequenceNumber),
    /// Last checkpoint to process based on the given epoch.
    ///
    /// After processing this checkpoint the framework will start the graceful
    /// shutdown process.
    EndOfEpoch(EpochId),
}

impl IngestionLimit {
    /// Evaluates whether the given checkpoint triggers a shutdown action based
    /// on the ingestion limit.
    fn matches(&self, checkpoint: &CheckpointData) -> ShutdownAction {
        match self {
            IngestionLimit::MaxCheckpoint(max) => {
                if &checkpoint.checkpoint_summary.sequence_number > max {
                    return ShutdownAction::ExcludeAndShutdown;
                }
                ShutdownAction::Continue
            }
            IngestionLimit::EndOfEpoch(max) => {
                if &checkpoint.checkpoint_summary.epoch > max {
                    return ShutdownAction::ExcludeAndShutdown;
                }
                ShutdownAction::Continue
            }
        }
    }
}

/// Represents a common interface for checkpoint readers.
///
/// It manages the old checkpoint reader implementation for backwards
/// compatibility and the new one.
enum CheckpointReader {
    /// The old checkpoint reader implementation.
    V1 {
        checkpoint_recv: mpsc::Receiver<Arc<CheckpointData>>,
        gc_sender: mpsc::Sender<CheckpointSequenceNumber>,
        exit_sender: oneshot::Sender<()>,
        handle: JoinHandle<IngestionResult<()>>,
    },
    /// The new checkpoint reader implementation.
    V2(CheckpointReaderV2),
}

impl CheckpointReader {
    /// Gets the next checkpoint from the reader.
    async fn get_checkpoint(&mut self) -> Option<Arc<CheckpointData>> {
        match self {
            Self::V1 {
                checkpoint_recv, ..
            } => checkpoint_recv.recv().await,
            Self::V2(reader) => reader.checkpoint().await,
        }
    }

    /// Sends a GC signal to the reader.
    async fn send_gc_signal(
        &mut self,
        seq_number: CheckpointSequenceNumber,
    ) -> IngestionResult<()> {
        match self {
            Self::V1 { gc_sender, .. } => gc_sender.send(seq_number).await.map_err(|_| {
                IngestionError::Channel(
                    "unable to send GC operation to checkpoint reader, receiver half closed".into(),
                )
            }),
            Self::V2(reader) => reader.send_gc_signal(seq_number).await,
        }
    }

    /// Shuts down the reader.
    async fn shutdown(self) -> IngestionResult<()> {
        match self {
            Self::V1 {
                exit_sender,
                handle,
                ..
            } => {
                _ = exit_sender.send(());
                handle.await.map_err(|err| IngestionError::Shutdown {
                    component: "checkpoint reader".into(),
                    msg: err.to_string(),
                })?
            }
            Self::V2(reader) => reader.shutdown().await,
        }
    }
}

/// The Executor of the main ingestion pipeline process.
///
/// This struct orchestrates the execution of multiple worker pools, handling
/// checkpoint distribution, progress tracking, and shutdown. It utilizes
/// [`ProgressStore`] for persisting checkpoint progress and provides metrics
/// for monitoring the indexing process.
///
/// # Example
/// ```rust,no_run
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{
///     DataIngestionMetrics, FileProgressStore, IndexerExecutor, IngestionError, ReaderOptions,
///     Worker, WorkerPool,
/// };
/// use iota_types::full_checkpoint_content::CheckpointData;
/// use prometheus::Registry;
/// use tokio_util::sync::CancellationToken;
/// use std::{path::PathBuf, sync::Arc};
///
/// struct CustomWorker;
///
/// #[async_trait]
/// impl Worker for CustomWorker {
///     type Message = ();
///     type Error = IngestionError;
///
///     async fn process_checkpoint(
///         &self,
///         checkpoint: Arc<CheckpointData>,
///     ) -> Result<Self::Message, Self::Error> {
///         // custom processing logic.
///         println!(
///             "Processing Local checkpoint: {}",
///             checkpoint.checkpoint_summary.to_string()
///         );
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let concurrency = 5;
///     let progress_store = FileProgressStore::new("progress.json").await.unwrap();
///     let mut executor = IndexerExecutor::new(
///         progress_store,
///         1, // number of registered WorkerPools.
///         DataIngestionMetrics::new(&Registry::new()),
///         CancellationToken::new(),
///     );
///     // register a worker pool with 5 workers to process checkpoints in parallel
///     let worker_pool = WorkerPool::new(CustomWorker, "local_reader".to_string(), concurrency, Default::default());
///     // register the worker pool to the executor.
///     executor.register(worker_pool).await.unwrap();
///     // run the ingestion pipeline.
///     executor
///         .run(
///             PathBuf::from("./chk".to_string()), // path to a local directory where checkpoints are stored.
///             None,
///             vec![],                   // optional remote store access options.
///             ReaderOptions::default(), // remote_read_batch_size.
///         )
///         .await
///         .unwrap();
/// }
/// ```
pub struct IndexerExecutor<P> {
    pools: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
    pool_senders: Vec<mpsc::Sender<Arc<CheckpointData>>>,
    progress_store: ProgressStoreWrapper<P>,
    pool_status_sender: mpsc::Sender<WorkerPoolStatus>,
    pool_status_receiver: mpsc::Receiver<WorkerPoolStatus>,
    metrics: DataIngestionMetrics,
    token: CancellationToken,
    shutdown_callback: Option<ShutdownCallback>,
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
            shutdown_callback: None,
        }
    }

    /// Registers new worker pool in executor.
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

    /// Registers a predicate callback that determines when the ingestion
    /// process should stop.
    ///
    /// This function `f` will be called for every **incoming checkpoint**
    /// before it’s sent to the worker pool.
    ///
    /// Based on the returned [`ShutdownAction`] the executor will evaluate
    /// whether to continue or stop the ingestion process by initiating the
    /// graceful shutdown process.
    ///
    /// Once a shutdown action is triggered, the executor will stop sending new
    /// checkpoints and will wait for all previously sent checkpoints to be
    /// processed by workers before initiating graceful shutdown process.
    ///
    /// Note:
    ///
    /// Calling this method after
    /// [`with_ingestion_limit`](Self::with_ingestion_limit) replaces the
    /// earlier predicate, and vice versa. They are not cumulative.
    pub fn shutdown_when<F>(&mut self, f: F)
    where
        F: Fn(&CheckpointData) -> ShutdownAction + Send + 'static,
    {
        self.shutdown_callback = Some(Box::new(f));
    }

    /// Adds an upper‑limit policy that determines when the ingestion
    /// process should stop.
    ///
    /// This is a convenience method, it internally uses
    /// [`shutdown_when`](Self::shutdown_when) by registering a predicate
    /// derived from the provided [`IngestionLimit`].
    ///
    /// Note:
    ///
    /// Calling this method after [`shutdown_when`](Self::shutdown_when)
    /// replaces the earlier predicate, and vice versa. They are not cumulative.
    pub fn with_ingestion_limit(&mut self, limit: IngestionLimit) {
        self.shutdown_when(move |checkpoint| limit.matches(checkpoint));
    }

    pub async fn update_watermark(
        &mut self,
        task_name: String,
        watermark: CheckpointSequenceNumber,
    ) -> IngestionResult<()> {
        self.progress_store.save(task_name, watermark).await
    }
    pub async fn read_watermark(
        &mut self,
        task_name: String,
    ) -> IngestionResult<CheckpointSequenceNumber> {
        self.progress_store.load(task_name).await
    }

    /// Main executor loop.
    ///
    /// # Error
    ///
    /// Returns an [`IngestionError::EmptyWorkerPool`] if no worker pool was
    /// registered.
    pub async fn run(
        mut self,
        path: PathBuf,
        remote_store_url: Option<String>,
        remote_store_options: Vec<(String, String)>,
        reader_options: ReaderOptions,
    ) -> IngestionResult<ExecutorProgress> {
        let reader_checkpoint_number = self.progress_store.min_watermark()?;
        let (checkpoint_reader, checkpoint_recv, gc_sender, exit_sender) =
            CheckpointReaderV1::initialize(
                path,
                reader_checkpoint_number,
                remote_store_url,
                remote_store_options,
                reader_options,
            );

        let handle = spawn_monitored_task!(checkpoint_reader.run());

        self.run_executor_loop(
            reader_checkpoint_number,
            CheckpointReader::V1 {
                checkpoint_recv,
                gc_sender,
                exit_sender,
                handle,
            },
        )
        .await
    }

    /// Alternative main executor loop. Uses the new iteration of the
    /// `CheckpointReader` supporting syncing checkpoints from hybrid historical
    /// store.
    ///
    /// # Error
    ///
    /// Returns an [`IngestionError::EmptyWorkerPool`] if no worker pool was
    /// registered.
    pub async fn run_with_config(
        mut self,
        config: CheckpointReaderConfig,
    ) -> IngestionResult<ExecutorProgress> {
        let reader_checkpoint_number = self.progress_store.min_watermark()?;
        let checkpoint_reader = CheckpointReaderV2::new(reader_checkpoint_number, config).await?;

        self.run_executor_loop(
            reader_checkpoint_number,
            CheckpointReader::V2(checkpoint_reader),
        )
        .await
    }

    /// Common execution logic
    async fn run_executor_loop(
        &mut self,
        mut reader_checkpoint_number: u64,
        mut checkpoint_reader: CheckpointReader,
    ) -> IngestionResult<ExecutorProgress> {
        let worker_pools = std::mem::take(&mut self.pools)
            .into_iter()
            .map(|pool| spawn_monitored_task!(pool))
            .collect::<Vec<JoinHandle<()>>>();

        let mut worker_pools_shutdown_signals = vec![];
        let mut checkpoint_limit_reached = None;

        loop {
            // the min watermark represents the lowest watermark that
            // has been processed by any worker pool. This guarantees that
            // all worker pools have processed the checkpoint before the
            // shutdown process starts.
            if checkpoint_limit_reached.is_some_and(|ch_seq_num| {
                self.progress_store
                    .min_watermark()
                    .map(|watermark| watermark > ch_seq_num)
                    .unwrap_or_default()
            }) {
                info!(
                    "checkpoint upper limit reached: last checkpoint was processed, shutdown process started"
                );
                self.token.cancel();
            }

            tokio::select! {
                Some(worker_pool_progress_msg) = self.pool_status_receiver.recv() => {
                    match worker_pool_progress_msg {
                        WorkerPoolStatus::Running((task_name, watermark)) => {
                            self.progress_store.save(task_name.clone(), watermark).await
                                .map_err(|err| IngestionError::ProgressStore(err.to_string()))?;
                            let seq_number = self.progress_store.min_watermark()?;
                            if seq_number > reader_checkpoint_number {
                                checkpoint_reader.send_gc_signal(seq_number).await?;
                                reader_checkpoint_number = seq_number;
                            }
                            self.metrics.data_ingestion_checkpoint
                                .with_label_values(&[&task_name])
                                .set(watermark as i64);
                        }
                        WorkerPoolStatus::Shutdown(worker_pool_name) => {
                            worker_pools_shutdown_signals.push(worker_pool_name);
                        }
                    }
                }
                Some(checkpoint) = checkpoint_reader.get_checkpoint(), if !self.token.is_cancelled() => {
                    // once upper limit reached skip sending new checkpoints to workers.
                    if self.should_shutdown(&checkpoint, &mut checkpoint_limit_reached) {
                        continue;
                    }

                    for sender in &self.pool_senders {
                        sender.send(checkpoint.clone()).await.map_err(|_| {
                            IngestionError::Channel(
                                "unable to send new checkpoint to worker pool, receiver half closed".to_owned(),
                            )
                        })?;
                    }
                }
            }

            if worker_pools_shutdown_signals.len() == self.pool_senders.len() {
                // Shutdown worker pools
                for worker_pool in worker_pools {
                    worker_pool.await.map_err(|err| IngestionError::Shutdown {
                        component: "worker pool".into(),
                        msg: err.to_string(),
                    })?;
                }
                // Shutdown checkpoint reader
                checkpoint_reader.shutdown().await?;
                break;
            }
        }

        Ok(self.progress_store.stats())
    }

    /// Check if the current ingestion limit has been reached.
    ///
    /// Returns `true` if the ingestion limit has been reached.
    /// If no ingestion limit is present or it has not been reached yet, the
    /// function returns `false`.
    fn should_shutdown(
        &mut self,
        checkpoint: &CheckpointData,
        checkpoint_limit_reached: &mut Option<CheckpointSequenceNumber>,
    ) -> bool {
        if checkpoint_limit_reached.is_some() {
            return true;
        }

        let Some(shutdown_action) = self
            .shutdown_callback
            .as_ref()
            .map(|matches| matches(checkpoint))
        else {
            return false;
        };

        match shutdown_action {
            ShutdownAction::IncludeAndShutdown => {
                checkpoint_limit_reached
                    .get_or_insert(checkpoint.checkpoint_summary.sequence_number);
                false
            }
            ShutdownAction::ExcludeAndShutdown => {
                checkpoint_limit_reached.get_or_insert(
                    checkpoint
                        .checkpoint_summary
                        .sequence_number
                        .saturating_sub(1),
                );
                true
            }
            ShutdownAction::Continue => false,
        }
    }
}

/// Sets up a single workflow for data ingestion.
///
/// This function initializes an [`IndexerExecutor`] with a single worker pool,
/// using a [`ShimProgressStore`] initialized with the provided
/// `initial_checkpoint_number`. It then returns a future that runs the executor
/// and a [`CancellationToken`] for graceful shutdown.
///
/// # Docs
/// For more info please check the [custom indexer docs](https://docs.iota.org/developer/advanced/custom-indexer).
///
/// # Example
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{IngestionError, Worker, setup_single_workflow};
/// use iota_types::full_checkpoint_content::CheckpointData;
///
/// struct CustomWorker;
///
/// #[async_trait]
/// impl Worker for CustomWorker {
///     type Message = ();
///     type Error = IngestionError;
///
///     async fn process_checkpoint(
///         &self,
///         checkpoint: Arc<CheckpointData>,
///     ) -> Result<Self::Message, Self::Error> {
///         // custom processing logic.
///         println!(
///             "Processing checkpoint: {}",
///             checkpoint.checkpoint_summary.to_string()
///         );
///         Ok(())
///     }
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let (executor, _) = setup_single_workflow(
///         CustomWorker,
///         "http://127.0.0.1:50051".to_string(), // fullnode gRPC API
///         0,                                    // initial checkpoint number.
///         5,                                    // concurrency.
///         None,                                 // extra reader options.
///     )
///     .await
///     .unwrap();
///     executor.await.unwrap();
/// }
/// ```
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
    let worker_pool = WorkerPool::new(
        worker,
        "workflow".to_string(),
        concurrency,
        Default::default(),
    );
    executor.register(worker_pool).await?;
    Ok((
        executor.run(
            tempfile::tempdir()?.keep(),
            Some(remote_store_url),
            vec![],
            reader_options.unwrap_or_default(),
        ),
        token,
    ))
}
