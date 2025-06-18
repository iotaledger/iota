// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::Future;
use iota_metrics::spawn_monitored_task;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use prometheus::Registry;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    DataIngestionMetrics, IndexerExecutor as IndexerExecutorV1, IngestionError, IngestionResult,
    Worker,
    progress_store::{ExecutorProgress, ProgressStore, ShimProgressStore},
    reader::v2::{CheckpointReader, CheckpointReaderConfig},
    worker_pool::{WorkerPool, WorkerPoolStatus},
};

/// The Executor of the main ingestion pipeline process.
///
/// This struct orchestrates the execution of multiple worker pools, handling
/// checkpoint distribution, progress tracking, and shutdown. It utilizes
/// [`ProgressStore`] for persisting checkpoint progress and provides metrics
/// for monitoring the indexing process.
///
/// # Example
/// ```rust,no_run
/// use std::{path::PathBuf, sync::Arc};
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{
///     DataIngestionMetrics, FileProgressStore, IngestionError, Worker, WorkerPool,
///     executor::v2::IndexerExecutor, reader::v2::CheckpointReaderConfig,
/// };
/// use iota_types::full_checkpoint_content::CheckpointData;
/// use prometheus::Registry;
/// use tokio_util::sync::CancellationToken;
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
///     let config = CheckpointReaderConfig {
///         ingestion_path: Some(PathBuf::from("./chk".to_string())),
///         ..Default::default()
///     };
///     let mut executor = IndexerExecutor::new(
///         progress_store,
///         1, // number of registered WorkerPools.
///         DataIngestionMetrics::new(&Registry::new()),
///         CancellationToken::new(),
///     );
///     // register a worker pool with 5 workers to process checkpoints in parallel
///     let worker_pool = WorkerPool::new(
///         CustomWorker,
///         "local_reader".to_string(),
///         concurrency,
///         Default::default(),
///     );
///     // register the worker pool to the executor.
///     executor.register(worker_pool).await.unwrap();
///     // run the ingestion pipeline.
///     executor.run(config).await.unwrap();
/// }
/// ```
pub struct IndexerExecutor<P> {
    inner: IndexerExecutorV1<P>,
}

impl<P: ProgressStore> IndexerExecutor<P> {
    pub fn new(
        progress_store: P,
        number_of_jobs: usize,
        metrics: DataIngestionMetrics,
        token: CancellationToken,
    ) -> Self {
        Self {
            inner: IndexerExecutorV1::new(progress_store, number_of_jobs, metrics, token),
        }
    }

    /// Registers new worker pool in executor.
    pub async fn register<W: Worker + 'static>(
        &mut self,
        pool: WorkerPool<W>,
    ) -> IngestionResult<()> {
        self.inner.register(pool).await
    }

    pub async fn update_watermark(
        &mut self,
        task_name: String,
        watermark: CheckpointSequenceNumber,
    ) -> IngestionResult<()> {
        self.inner.update_watermark(task_name, watermark).await
    }

    pub async fn read_watermark(
        &mut self,
        task_name: String,
    ) -> IngestionResult<CheckpointSequenceNumber> {
        self.inner.read_watermark(task_name).await
    }

    /// Main executor loop.
    ///
    /// # Error
    ///
    /// Returns an [`IngestionError::EmptyWorkerPool`] if no worker pool was
    /// registered.
    pub async fn run(
        mut self,
        config: CheckpointReaderConfig,
    ) -> IngestionResult<ExecutorProgress> {
        let mut reader_checkpoint_number = self.inner.progress_store.min_watermark()?;

        let mut checkpoint_reader = CheckpointReader::new(reader_checkpoint_number, config).await?;

        let worker_pools = std::mem::take(&mut self.inner.pools)
            .into_iter()
            .map(|pool| spawn_monitored_task!(pool))
            .collect::<Vec<JoinHandle<()>>>();

        let mut worker_pools_shutdown_signals = vec![];

        loop {
            tokio::select! {
                Some(worker_pool_progress_msg) = self.inner.pool_status_receiver.recv() => {
                    match worker_pool_progress_msg {
                        WorkerPoolStatus::Running((task_name, watermark)) => {
                            self.inner.progress_store.save(task_name.clone(), watermark).await.map_err(|err| IngestionError::ProgressStore(err.to_string()))?;
                            let seq_number = self.inner.progress_store.min_watermark()?;
                            if seq_number > reader_checkpoint_number {
                                checkpoint_reader.send_gc_signal(seq_number).await?;
                                reader_checkpoint_number = seq_number;
                            }
                            self.inner.metrics.data_ingestion_checkpoint.with_label_values(&[&task_name]).set(watermark as i64);
                        }
                        WorkerPoolStatus::Shutdown(worker_pool_name) => {
                            // Track worker pools that have initiated shutdown.
                            worker_pools_shutdown_signals.push(worker_pool_name);
                        }
                    }
                }
                // Only process new checkpoints while system is running (token not cancelled).
                // The guard prevents accepting new work during shutdown while allowing existing work to complete for other branches.
                Some(checkpoint) = checkpoint_reader.checkpoint(), if !self.inner.token.is_cancelled() => {
                    for sender in &self.inner.pool_senders {
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
            // process.
            if worker_pools_shutdown_signals.len() == self.inner.pool_senders.len() {
                break components_graceful_shutdown(worker_pools, checkpoint_reader).await?;
            }
        }

        Ok(self.inner.progress_store.stats())
    }
}

/// Start the graceful shutdown of remaining components.
///
/// - Awaits all worker pool handles.
/// - Await checkpoint reader shutdown.
async fn components_graceful_shutdown(
    worker_pools: Vec<JoinHandle<()>>,
    checkpoint_reader: CheckpointReader,
) -> IngestionResult<()> {
    for worker_pool in worker_pools {
        worker_pool.await.map_err(|err| IngestionError::Shutdown {
            component: "Worker Pool".into(),
            msg: err.to_string(),
        })?;
    }
    checkpoint_reader.shutdown().await?;
    Ok(())
}

/// Sets up a single workflow for data ingestion.
///
/// This function initializes an [`IndexerExecutor`] with a single worker pool,
/// using a [`ShimProgressStore`] initialized with the provided
/// `initial_checkpoint_number`. It then returns a future that runs the executor
/// and a [`CancellationToken`] for graceful shutdown.
///
/// # Example
/// ```rust,no_run
/// use std::sync::Arc;
///
/// use async_trait::async_trait;
/// use iota_data_ingestion_core::{
///     IngestionError, Worker,
///     executor::v2::setup_single_workflow,
///     reader::v2::{CheckpointReaderConfig, RemoteUrl},
/// };
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
///     let config = CheckpointReaderConfig {
///         remote_store_url: Some(RemoteUrl::Rest("http://127.0.0.1:9000/api/v1".into())),
///         ..Default::default()
///     };
///     let (executor, _) = setup_single_workflow(
///         CustomWorker,
///         0, // initial checkpoint number.
///         5, // concurrency.
///         config,
///     )
///     .await
///     .unwrap();
///     executor.await.unwrap();
/// }
/// ```
pub async fn setup_single_workflow<W: Worker + 'static>(
    worker: W,
    initial_checkpoint_number: CheckpointSequenceNumber,
    concurrency: usize,
    config: CheckpointReaderConfig,
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

    Ok((executor.run(config), token))
}
