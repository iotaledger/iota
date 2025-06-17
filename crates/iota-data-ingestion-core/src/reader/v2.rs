// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future, num::NonZeroUsize, ops::Range, path::PathBuf, sync::Arc, time::Duration,
};

use backoff::backoff::Backoff;
use futures::{StreamExt, future::OptionFuture};
use iota_config::{
    node::ArchiveReaderConfig,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_metrics::spawn_monitored_task;
use iota_rest_api::CheckpointData;
use iota_storage::blob::Blob;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use object_store::{ObjectStore, path::Path as ObjectStorePath};
use tokio::{
    sync::{
        mpsc::{self, error::TryRecvError},
        oneshot,
    },
    task::JoinHandle,
    time::timeout,
};
use tracing::{debug, error, info};

use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, create_remote_store_client,
    history::reader::HistoricalReader,
    reader::{
        fetch::{
            CheckpointResult, ReadSource, fetch_from_full_node, fetch_from_object_store,
            read_local_files_with_retry_and_capacity_check, setup_directory_watcher,
        },
        v1::{DataLimiter, gc_processed_files},
    },
};

/// Available sources for checkpoint streams supported by the ingestion
/// framework.
///
/// This enum represents the different types of remote sources from which
/// checkpoint data can be fetched. Each variant corresponds to a supported
/// backend or combination of backends for checkpoint retrieval.
pub enum RemoteUrl {
    /// A REST API endpoint for checkpoint data.
    Rest(String),
    /// An object storage backend for checkpoint data.
    ObjectStore {
        /// The URL of the object store (e.g., S3 bucket, GCS bucket,
        /// HTTP/WebDAV Storage).
        object_store_url: String,
        /// Additional options for configuring the remote store as key-value
        /// pairs.
        remote_store_options: Vec<(String, String)>,
    },
    /// A hybrid source combining historical object store and optional live
    /// object store.
    HybridHistoricalStore {
        /// The URL of the historical object store.
        historical_url: String,
        /// The URL of the live object store.
        live_url: Option<String>,
    },
}

/// Represents a remote backend for checkpoint data retrieval.
///
/// This enum encapsulates the supported remote storage mechanisms that can be
/// used by the ingestion framework to fetch checkpoint data. Each variant
/// corresponds to a different type of remote source.
enum RemoteStore {
    ObjectStore(Box<dyn ObjectStore>),
    Rest(iota_rest_api::Client),
    HybridHistoricalStore {
        historical_store: HistoricalReader,
        live_store: Option<Box<dyn ObjectStore>>,
    },
}

/// Configuration options to control the behavior of a checkpoint
/// reader.
pub struct CheckpointReaderConfig {
    /// How often to check for new checkpoints, lower values mean faster
    /// detection but more CPU usage.
    ///
    /// Default: 100ms.
    pub tick_interval_ms: u64,
    /// Network request timeout, it applies to remote store operations.
    ///
    /// Default: 5 seconds.
    pub timeout_secs: u64,
    /// Number of maximum concurrent requests to the remote store. Increase it
    /// for backfills, higher values increase throughput but use more resources.
    ///
    /// Default: 10.
    pub batch_size: usize,
    /// Maximum memory (bytes) for batch checkpoint processing to prevent OOM
    /// errors. Zero indicates no limit.
    ///
    /// Default: 0.
    pub data_limit: usize,
    /// Local path for checkpoint ingestion. If not provided, checkpoints will
    /// be ingested from a temporary directory.
    pub ingestion_path: Option<PathBuf>,
    /// Remote source for checkpoint data stream.
    pub remote_store_url: Option<RemoteUrl>,
}

impl Default for CheckpointReaderConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 100,
            timeout_secs: 5,
            batch_size: 10,
            data_limit: 0,
            ingestion_path: None,
            remote_store_url: None,
        }
    }
}

struct CheckpointReader {
    path: PathBuf,
    tick_interval_ms: u64,
    current_checkpoint_number: CheckpointSequenceNumber,
    last_pruned_watermark: CheckpointSequenceNumber,
    checkpoint_tx: mpsc::Sender<Arc<CheckpointData>>,
    gc_signal_rx: mpsc::Receiver<CheckpointSequenceNumber>,
    remote_fetcher: Option<RemoteCheckpointFetcher>,
    remote_fetcher_receiver: Option<mpsc::Receiver<CheckpointResult>>,
    shutdown_rx: oneshot::Receiver<()>,
    data_limiter: DataLimiter,
}

impl CheckpointReader {
    fn exceeds_capacity(&self, checkpoint_number: CheckpointSequenceNumber) -> bool {
        ((MAX_CHECKPOINTS_IN_PROGRESS as u64 + self.last_pruned_watermark) <= checkpoint_number)
            || self.data_limiter.exceeds()
    }

    fn should_fetch_from_remote(&self, checkpoints: &[Arc<CheckpointData>]) -> bool {
        self.remote_fetcher.is_some()
            && (checkpoints.is_empty()
                || checkpoints[0].checkpoint_summary.sequence_number
                    > self.current_checkpoint_number)
    }

    async fn remote_fetch(&mut self) -> Vec<Arc<CheckpointData>> {
        let mut checkpoints = vec![];

        let Some(remote_fetcher) = self.remote_fetcher.as_ref() else {
            return checkpoints;
        };

        if self.remote_fetcher_receiver.is_none() {
            self.remote_fetcher_receiver =
                Some(remote_fetcher.spawn_checkpoint_fetching_task(self.current_checkpoint_number));
        }

        while !self.exceeds_capacity(self.current_checkpoint_number + checkpoints.len() as u64) {
            match self.remote_fetcher_receiver.as_mut().unwrap().try_recv() {
                Ok(Ok((checkpoint, size))) => {
                    self.data_limiter.add(&checkpoint, size);
                    checkpoints.push(checkpoint);
                }
                Ok(Err(err)) => {
                    error!("remote reader transient error {err:?}");
                    self.remote_fetcher_receiver = None;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    error!("remote reader channel disconnect error");
                    self.remote_fetcher_receiver = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        checkpoints
    }

    async fn sync(&mut self) -> IngestionResult<()> {
        let mut read_source = ReadSource::Local;
        let mut checkpoints = read_local_files_with_retry_and_capacity_check(
            &self.path,
            self.current_checkpoint_number,
            |seq| self.exceeds_capacity(seq),
        )
        .await?;

        if self.should_fetch_from_remote(&checkpoints) {
            checkpoints = self.remote_fetch().await;
            read_source = ReadSource::Remote;
        } else {
            // cancel remote fetcher execution because local reader has made progress
            self.remote_fetcher_receiver.take();
        }

        info!(
            "Read from {read_source}. Current checkpoint number: {}, pruning watermark: {}, new updates: {:?}",
            self.current_checkpoint_number,
            self.last_pruned_watermark,
            checkpoints.len(),
        );
        for checkpoint in checkpoints {
            assert_eq!(
                checkpoint.checkpoint_summary.sequence_number,
                self.current_checkpoint_number
            );
            self.checkpoint_tx.send(checkpoint).await.map_err(|_| {
                IngestionError::Channel(
                    "unable to send checkpoint to executor, receiver half closed".to_owned(),
                )
            })?;
            self.current_checkpoint_number += 1;
        }

        Ok(())
    }

    async fn run(mut self) {
        let (_watcher, mut inotify_rx) = setup_directory_watcher(&self.path);

        gc_processed_files(
            &self.path,
            self.last_pruned_watermark,
            &mut self.last_pruned_watermark,
            &mut self.data_limiter,
        )
        .expect("Failed to clean the directory");

        loop {
            tokio::select! {
                _ = &mut self.shutdown_rx => break,
                Some(watermark) = self.gc_signal_rx.recv() => {
                    gc_processed_files(
                        &self.path,
                        watermark,
                        &mut self.last_pruned_watermark,
                        &mut self.data_limiter,
                    ).expect("Failed to clean the directory");
                }
                Ok(Some(_)) | Err(_) = timeout(Duration::from_millis(self.tick_interval_ms), inotify_rx.recv())  => {
                    self.sync().await.expect("Failed to read checkpoint files");
                }
            }
        }
    }
}

pub(crate) struct CheckpointReaderHandle {
    handle: JoinHandle<()>,
    shutdown_tx: oneshot::Sender<()>,
    gc_signal_tx: mpsc::Sender<CheckpointSequenceNumber>,
    checkpoint_rx: mpsc::Receiver<Arc<CheckpointData>>,
}

impl CheckpointReaderHandle {
    pub(crate) async fn new(
        starting_checkpoint_number: CheckpointSequenceNumber,
        config: CheckpointReaderConfig,
    ) -> IngestionResult<Self> {
        let (checkpoint_tx, checkpoint_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (gc_signal_tx, gc_signal_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let remote_fetcher: OptionFuture<_> = config
            .remote_store_url
            .map(|url| RemoteCheckpointFetcher::new(url, config.timeout_secs, config.batch_size))
            .into();

        let remote_fetcher = remote_fetcher.await.transpose()?;

        let path = match config.ingestion_path {
            Some(p) => p,
            None => tempfile::tempdir()?.into_path(),
        };

        let reader = CheckpointReader {
            path,
            tick_interval_ms: config.tick_interval_ms,
            current_checkpoint_number: starting_checkpoint_number,
            last_pruned_watermark: starting_checkpoint_number,
            checkpoint_tx,
            gc_signal_rx,
            remote_fetcher,
            remote_fetcher_receiver: None,
            shutdown_rx,
            data_limiter: DataLimiter::new(config.data_limit),
        };

        let handle = spawn_monitored_task!(reader.run());

        Ok(Self {
            handle,
            gc_signal_tx,
            shutdown_tx,
            checkpoint_rx,
        })
    }

    /// Read downloaded checkpoints from the queue.
    pub(crate) async fn checkpoint(&mut self) -> Option<Arc<CheckpointData>> {
        self.checkpoint_rx.recv().await
    }

    /// Sends a garbage collection (GC) signal to the checkpoint reader.
    ///
    /// Transmits a watermark to the checkpoint reader, indicating that all
    /// checkpoints below this watermark can be safely pruned or cleaned up.
    /// The signal is sent over an internal channel to the checkpoint reader
    /// task.
    pub(crate) async fn send_gc_signal(
        &self,
        watermark: CheckpointSequenceNumber,
    ) -> IngestionResult<()> {
        self.gc_signal_tx.send(watermark).await.map_err(|_| {
            IngestionError::Channel(
                "unable to send GC operation to checkpoint reader, receiver half closed".into(),
            )
        })
    }

    pub(crate) async fn shutdown(self) -> IngestionResult<()> {
        _ = self.shutdown_tx.send(());
        self.handle.await.map_err(|err| IngestionError::Shutdown {
            component: "CheckpointReader".into(),
            msg: err.to_string(),
        })
    }
}

/// Encapsulates the logic required to retrieve checkpoint data
/// from various remote backends. It manages batching, retry logic, and
/// streaming of checkpoint data to consumers via channels.
struct RemoteCheckpointFetcher {
    store: Arc<RemoteStore>,
    batch_size: usize,
}

impl RemoteCheckpointFetcher {
    async fn new(
        remote_url: RemoteUrl,
        timeout_secs: u64,
        batch_size: usize,
    ) -> IngestionResult<Self> {
        let store = match remote_url {
            RemoteUrl::Rest(url) => RemoteStore::Rest(iota_rest_api::Client::new(url)),
            RemoteUrl::ObjectStore {
                object_store_url,
                remote_store_options,
            } => {
                let object_store = create_remote_store_client(
                    object_store_url,
                    remote_store_options,
                    timeout_secs,
                )?;
                RemoteStore::ObjectStore(object_store)
            }
            RemoteUrl::HybridHistoricalStore {
                historical_url,
                live_url,
            } => {
                let historical_store = Self::historical_reader(historical_url, batch_size).await?;

                let live_store = live_url
                    .map(|url| create_remote_store_client(url, Default::default(), timeout_secs))
                    .transpose()?;

                RemoteStore::HybridHistoricalStore {
                    historical_store,
                    live_store,
                }
            }
        };

        Ok(Self {
            store: Arc::new(store),
            batch_size,
        })
    }

    /// Creates a new historical reader.
    async fn historical_reader(
        url: String,
        batch_size: usize,
    ) -> IngestionResult<HistoricalReader> {
        let config = ArchiveReaderConfig {
            download_concurrency: NonZeroUsize::new(batch_size)
                .expect("batch size must be greater than zero"),
            remote_store_config: ObjectStoreConfig {
                object_store: Some(ObjectStoreType::S3),
                object_store_connection_limit: 20,
                aws_endpoint: Some(url),
                aws_virtual_hosted_style_request: true,
                no_sign_request: true,
                ..Default::default()
            },
            use_for_pruning_watermark: false,
        };
        HistoricalReader::new(config)
            .inspect_err(|e| tracing::error!("Unable to instantiate historical reader: {e}"))
    }

    /// Fetch a single checkpoint from the live object store.
    async fn fetch_from_live_object_store(
        live: &dyn ObjectStore,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> CheckpointResult {
        let path = ObjectStorePath::from(format!("ingestion/live/{checkpoint_number}.chk"));
        let response = live.get(&path).await?;
        let bytes = response.bytes().await?;
        Ok((
            Blob::from_bytes::<Arc<CheckpointData>>(&bytes)
                .map_err(|err| IngestionError::DeserializeCheckpoint(err.to_string()))?,
            bytes.len(),
        ))
    }

    /// Fetch a batch of checkpoints from the historical object store.
    async fn fetch_from_historical_object_store(
        historical_reader: &HistoricalReader,
        checkpoints: Range<CheckpointSequenceNumber>,
    ) -> IngestionResult<Vec<CheckpointResult>> {
        historical_reader.sync_manifest_once().await?;
        let latest_available_checkpoint = historical_reader.latest_available_checkpoint().await?;
        debug!("historical latest_available_checkpoint: {latest_available_checkpoint}");
        if checkpoints.end > latest_available_checkpoint {
            return Err(IngestionError::HistoryRead(
                "requested checkpoint range not available yet".into(),
            ));
        }
        Ok(historical_reader
            .iter_for_range(checkpoints)
            .await?
            .map(|checkpoint| {
                bcs::serialized_size(&checkpoint)
                    .map(|size| (Arc::new(checkpoint), size))
                    .map_err(Into::into)
            })
            .collect())
    }

    /// Streams checkpoints from a historical store directly to a channel, with
    /// live store fallback if provided.
    ///
    /// Fetch a range of checkpoints from the provided `historical_reader`. If
    /// the fetch from the historical reader succeeds, all checkpoints are
    /// sent to the provided channel sender. If the historical fetch fails
    /// and a `live_store` is provided, the function falls back to fetching
    /// each checkpoint from the live object store, retrying each fetch on
    /// transient errors.
    ///
    /// All fetch operations are wrapped in a retry mechanism using
    /// [`fetch_with_retry`] to handle transient errors robustly.
    async fn stream_with_retry_hybrid_historical_store(
        historical_reader: &HistoricalReader,
        live_store: Option<&dyn ObjectStore>,
        checkpoints: Range<CheckpointSequenceNumber>,
        sender: &mpsc::Sender<CheckpointResult>,
    ) -> IngestionResult<()> {
        if let Err(err) = Self::fetch_with_retry(|| async {
            match Self::fetch_from_historical_object_store(historical_reader, checkpoints.clone())
                .await
            {
                Ok(results) => {
                    for result in results {
                        Self::send_checkpoint_to_channel(result, sender).await?;
                    }
                    Ok(())
                }
                Err(err) => {
                    let Some(live) = live_store else {
                        return Err(err);
                    };
                    Self::stream_with_retry(checkpoints.clone(), sender, |checkpoint_number| {
                        Self::fetch_from_live_object_store(live, checkpoint_number)
                    })
                    .await
                }
            }
        })
        .await
        {
            return Self::send_checkpoint_to_channel(Err(err), sender).await;
        }

        Ok(())
    }

    async fn send_checkpoint_to_channel(
        result: CheckpointResult,
        sender: &mpsc::Sender<CheckpointResult>,
    ) -> IngestionResult<()> {
        sender.send(result).await.map_err(|_| {
            IngestionError::Channel(
                "unable to send new checkpoint to checkpoint reader, receiver half closed".into(),
            )
        })
    }

    /// Streams a batch of checkpoints to a channel, retrying fetches on
    /// transient errors.
    ///
    /// It takes an iterable of items (such as checkpoint sequence numbers), and
    /// for each item, attempts to fetch the corresponding checkpoint using
    /// the provided `fetch` function. Each fetch operation is automatically
    /// retried on transient errors using
    /// [`RemoteCheckpointReader::fetch_with_retry`]. Successfully fetched
    /// checkpoints are sent to the provided channel.
    ///
    /// The function processes the entire batch concurrently, buffering up to
    /// the batch size, and sends each result (success or error) to the
    /// channel in the order they complete.
    async fn stream_with_retry<T, F, Fut>(
        items: impl IntoIterator<Item = T>,
        sender: &mpsc::Sender<CheckpointResult>,
        fetch: F,
    ) -> IngestionResult<()>
    where
        F: Fn(T) -> Fut + Copy,
        Fut: Future<Output = CheckpointResult>,
        T: Copy,
    {
        let fetches = items
            .into_iter()
            .map(|item| async move { Self::fetch_with_retry(|| fetch(item)).await })
            .collect::<Vec<_>>();

        let batch_size = fetches.len();
        let mut stream = futures::stream::iter(fetches).buffered(batch_size);
        while let Some(result) = stream.next().await {
            Self::send_checkpoint_to_channel(result, sender).await?;
        }
        Ok(())
    }

    /// Attempts to fetch data asynchronously with automatic retry using
    /// exponential backoff.
    ///
    /// It repeatedly invokes the provided asynchronous `fetch`
    /// operation until it succeeds or the retry policy is exhausted. If the
    /// fetch operation returns an [`IngestionError::Channel`] error, the
    /// function returns immediately without retrying, as this indicates a
    /// non-recoverable channel closure. For all other errors, the function
    /// waits for the next backoff interval before retrying. If the maximum
    /// backoff duration is exceeded, the last error is returned.
    pub async fn fetch_with_retry<F, Fut, T>(mut fetch: F) -> IngestionResult<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = IngestionResult<T>>,
    {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.max_elapsed_time = Some(Duration::from_secs(60));
        backoff.initial_interval = Duration::from_millis(100);
        backoff.current_interval = backoff.initial_interval;
        backoff.multiplier = 1.0;

        loop {
            match fetch().await {
                Ok(data) => return Ok(data),
                Err(err) => {
                    if matches!(err, IngestionError::Channel(_)) {
                        return Err(err);
                    }
                    // once reached the tip of the network, the historical reader can take some time
                    // to issue a new checkpoint file, we reset the backoff only in this case.
                    if matches!(err, IngestionError::HistoryRead(ref e) if e.contains("requested checkpoint range not available yet"))
                        && backoff.next_backoff().is_none()
                    {
                        tracing::info!(
                            "Resetting backoff due to unavailable checkpoint range of the historical reader"
                        );
                        backoff.reset();
                    }

                    match backoff.next_backoff() {
                        Some(duration) => {
                            if !err.to_string().contains("404") {
                                debug!(
                                    "remote reader retry in {} ms. Error is {err:?}",
                                    duration.as_millis(),
                                );
                            }
                            tokio::time::sleep(duration).await
                        }
                        None => {
                            error!("remote reader failed after retries");
                            return Err(err);
                        }
                    }
                }
            }
        }
    }

    /// Fetch batch and stream results directly to channel
    async fn fetch_and_stream_batch_to_channel(
        store: Arc<RemoteStore>,
        range: Range<CheckpointSequenceNumber>,
        sender: &mpsc::Sender<CheckpointResult>,
    ) -> IngestionResult<()> {
        match store.as_ref() {
            RemoteStore::ObjectStore(store) => {
                Self::stream_with_retry(range, sender, |checkpoint_number| {
                    fetch_from_object_store(store, checkpoint_number)
                })
                .await
            }
            RemoteStore::Rest(client) => {
                Self::stream_with_retry(range, sender, |checkpoint_number| {
                    fetch_from_full_node(client, checkpoint_number)
                })
                .await
            }
            RemoteStore::HybridHistoricalStore {
                historical_store,
                live_store,
            } => {
                Self::stream_with_retry_hybrid_historical_store(
                    historical_store,
                    live_store.as_deref(),
                    range,
                    sender,
                )
                .await
            }
        }
    }

    /// Spawns a background task to fetch checkpoints from the remote store in
    /// batches, with automatic retry on transient errors.
    ///
    /// This function creates a monitored asynchronous task that iterates over
    /// checkpoint sequence numbers, starting from `start_checkpoint`, and
    /// fetches them in batches of size `batch_size`. For each batch, it
    /// retrieves the checkpoints from the remote store and streams them to
    /// the returned channel's receiver.
    ///
    /// Terminates gracefully if the channel receiver is dropped, indicating
    /// that no more data is needed.
    fn spawn_checkpoint_fetching_task(
        &self,
        start_checkpoint: CheckpointSequenceNumber,
    ) -> mpsc::Receiver<IngestionResult<(Arc<CheckpointData>, usize)>> {
        let batch_size = self.batch_size;
        let store = self.store.clone();
        let (checkpoint_tx, checkpoint_rx) = mpsc::channel(batch_size);
        spawn_monitored_task!(async move {
            for batch_start in (start_checkpoint..u64::MAX).step_by(batch_size) {
                let checkpoints = batch_start..batch_start + batch_size as u64;
                if Self::fetch_and_stream_batch_to_channel(
                    store.clone(),
                    checkpoints,
                    &checkpoint_tx,
                )
                .await
                .is_err()
                {
                    break;
                }
            }
        });

        checkpoint_rx
    }
}
