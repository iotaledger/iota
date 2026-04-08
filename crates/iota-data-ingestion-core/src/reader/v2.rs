// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use backoff::backoff::Backoff;
use futures::{StreamExt, TryStreamExt};
use iota_config::{
    node::ArchiveReaderConfig,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_grpc_client::Client as GrpcClient;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use tap::Pipe;
use tokio::{
    sync::mpsc::{self},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

#[cfg(not(target_os = "macos"))]
use crate::reader::fetch::init_watcher;
use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, create_remote_store_client,
    history::reader::HistoricalReader,
    reader::{
        fetch::{
            GRPC_MAX_DECODING_MESSAGE_SIZE_BYTES, LocalRead, ReadSource, fetch_from_object_store,
        },
        v1::{DataLimiter, ReaderOptions},
    },
};

/// Available sources for checkpoint streams supported by the ingestion
/// framework.
///
/// This enum represents the different types of remote sources from which
/// checkpoint data can be fetched. Each variant corresponds to a supported
/// backend or combination of backends for checkpoint retrieval.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RemoteUrl {
    /// The URL to the Fullnode server that exposes
    /// checkpoint data streaming through gRPC.
    ///
    /// # Example
    /// ```text
    /// "http://127.0.0.1:50051"
    /// ```
    Fullnode(String),
    /// A hybrid source combining historical object store and optional live
    /// object store.
    HybridHistoricalStore {
        /// The URL path to the historical object store that contains `*.chk`,
        /// `*.sum` & `MANIFEST` files.
        ///
        /// # Example
        /// ```text
        /// "https://checkpoints.mainnet.iota.cafe/ingestion/historical"
        /// ```
        historical_url: String,
        /// The URL path to the live object store that contains `*.chk`
        /// checkpoint files.
        ///
        /// # Example
        /// ```text
        /// "https://checkpoints.mainnet.iota.cafe/ingestion/live"
        /// ```
        live_url: Option<String>,
    },
}

/// Represents a remote backend for checkpoint data retrieval.
///
/// This enum encapsulates the supported remote storage mechanisms that can be
/// used by the ingestion framework to fetch checkpoint data. Each variant
/// corresponds to a different type of remote source.
enum RemoteStore {
    Fullnode(GrpcClient),
    HybridHistoricalStore {
        historical: HistoricalReader,
        live: Option<Box<dyn ObjectStore>>,
    },
}

impl RemoteStore {
    async fn new(
        remote_url: RemoteUrl,
        batch_size: usize,
        timeout_secs: u64,
    ) -> IngestionResult<Self> {
        let store = match remote_url {
            RemoteUrl::Fullnode(ref url) => {
                let grpc_client = GrpcClient::connect(url).await.map(|client| {
                    client.with_max_decoding_message_size(GRPC_MAX_DECODING_MESSAGE_SIZE_BYTES)
                })?;
                RemoteStore::Fullnode(grpc_client)
            }
            RemoteUrl::HybridHistoricalStore {
                historical_url,
                live_url,
            } => {
                let config = ArchiveReaderConfig {
                    download_concurrency: NonZeroUsize::new(batch_size)
                        .expect("batch size must be greater than zero"),
                    remote_store_config: ObjectStoreConfig {
                        object_store: Some(ObjectStoreType::S3),
                        object_store_connection_limit: 20,
                        aws_endpoint: Some(historical_url),
                        aws_virtual_hosted_style_request: true,
                        no_sign_request: true,
                        ..Default::default()
                    },
                    use_for_pruning_watermark: false,
                };
                let historical = HistoricalReader::new(config)
                    .inspect_err(|e| error!("unable to instantiate historical reader: {e}"))?;

                let live = live_url
                    .map(|url| create_remote_store_client(url, Default::default(), timeout_secs))
                    .transpose()?;

                RemoteStore::HybridHistoricalStore { historical, live }
            }
        };
        Ok(store)
    }
}

/// Configuration options to control the behavior of a checkpoint
/// reader.
#[derive(Default, Clone)]
pub struct CheckpointReaderConfig {
    /// Config the checkpoint reader behavior for downloading new checkpoints.
    pub reader_options: ReaderOptions,
    /// Local path for checkpoint ingestion. If not provided, checkpoints will
    /// be ingested from a temporary directory.
    pub ingestion_path: Option<PathBuf>,
    /// Remote source for checkpoint data stream.
    pub remote_store_url: Option<RemoteUrl>,
}

/// Internal actor responsible for reading and streaming checkpoints.
///
/// `CheckpointReaderActor` is the core background task that manages the logic
/// for fetching, batching, and streaming checkpoint data from local or remote
/// sources. It handles checkpoint discovery, garbage collection signals, and
/// coordinates with remote fetchers as needed.
///
/// This struct is intended to be run as an asynchronous task and is not
/// typically interacted with directly. Instead, users should use
/// [`CheckpointReader`], which provides a safe and ergonomic API for
/// interacting with the running actor, such as receiving checkpoints, sending
/// GC signals, or triggering shutdown.
///
/// # Responsibilities
/// - Periodically scans for new checkpoints from configured sources.
/// - Streams checkpoints to consumers via channels.
/// - Handles garbage collection signals to prune processed checkpoints.
/// - Coordinates with remote fetchers for batch downloads and retries.
///
/// # Usage
/// Users should not construct or manage `CheckpointReader` directly. Instead,
/// use [`CheckpointReader::new`] to spawn the actor and obtain a handle
/// for interaction.
struct CheckpointReaderActor {
    /// Filesystem path to the local checkpoint directory.
    path: PathBuf,
    /// Start fetch from the current checkpoint sequence.
    current_checkpoint_number: CheckpointSequenceNumber,
    /// Keeps tracks the last processed checkpoint sequence number, used to
    /// delete checkpoint files from ingestion path.
    last_pruned_watermark: CheckpointSequenceNumber,
    /// Channel for sending checkpoints to WorkerPools.
    checkpoint_tx: mpsc::Sender<Arc<CheckpointData>>,
    /// Sends a garbage collection (GC) signal to prune checkpoint files below
    /// the specified watermark.
    gc_signal_rx: mpsc::Receiver<CheckpointSequenceNumber>,
    /// Remote checkpoint reader for fetching checkpoints from the network.
    remote_store: Option<Arc<RemoteStore>>,
    /// Shutdown signal for the actor.
    token: CancellationToken,
    /// Configures the behavior of the checkpoint reader.
    reader_options: ReaderOptions,
    /// Limit the amount of downloaded checkpoints held in memory to avoid OOM.
    data_limiter: DataLimiter,
}

impl LocalRead for CheckpointReaderActor {
    fn exceeds_capacity(&self, checkpoint_number: CheckpointSequenceNumber) -> bool {
        ((MAX_CHECKPOINTS_IN_PROGRESS as u64 + self.last_pruned_watermark) <= checkpoint_number)
            || self.data_limiter.exceeds()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn current_checkpoint_number(&self) -> CheckpointSequenceNumber {
        self.current_checkpoint_number
    }

    fn update_last_pruned_watermark(&mut self, watermark: CheckpointSequenceNumber) {
        self.last_pruned_watermark = watermark;
    }
}

impl CheckpointReaderActor {
    fn should_fetch_from_remote(&self, checkpoints: &[Arc<CheckpointData>]) -> bool {
        self.remote_store.is_some()
            && (checkpoints.is_empty()
                || self.is_checkpoint_ahead(&checkpoints[0], self.current_checkpoint_number))
    }

    /// Fetches checkpoints from the historical object store and streams them to
    /// a channel.
    async fn relay_from_historical(
        &mut self,
        historical_reader: &HistoricalReader,
    ) -> IngestionResult<()> {
        // Only sync the manifest when needed to avoid unnecessary network calls.
        // If the requested checkpoint is beyond what's currently available in our
        // cached manifest, we need to refresh it to check for newer checkpoints.
        if self.current_checkpoint_number > historical_reader.latest_available_checkpoint().await? {
            timeout(
                Duration::from_secs(self.reader_options.timeout_secs),
                historical_reader.sync_manifest_once(),
            )
            .await
            .map_err(|_| {
                IngestionError::HistoryRead("reading manifest exceeded the timeout".into())
            })??;

            // Verify the requested checkpoint is now available after the manifest refresh.
            // If it's still not available, the checkpoint hasn't been published yet.
            if self.current_checkpoint_number
                > historical_reader.latest_available_checkpoint().await?
            {
                return Err(IngestionError::CheckpointNotAvailableYet);
            }
        }

        let manifest = historical_reader.get_manifest().await;

        let files = historical_reader.verify_and_get_manifest_files(manifest)?;

        let start_index = match files.binary_search_by_key(&self.current_checkpoint_number, |s| {
            s.checkpoint_seq_range.start
        }) {
            Ok(index) => index,
            Err(index) => index - 1,
        };

        for metadata in files
            .into_iter()
            .enumerate()
            .filter_map(|(index, metadata)| (index >= start_index).then_some(metadata))
        {
            let checkpoints = timeout(
                Duration::from_secs(self.reader_options.timeout_secs),
                historical_reader.iter_for_file(metadata.file_path()),
            )
            .await
            .map_err(|_| {
                IngestionError::HistoryRead(format!(
                    "reading checkpoint {} exceeded the timeout",
                    metadata.file_path()
                ))
            })??
            .filter(|c| c.checkpoint_summary.sequence_number >= self.current_checkpoint_number)
            .collect::<Vec<CheckpointData>>();

            for checkpoint in checkpoints {
                let size = bcs::serialized_size(&checkpoint)?;
                self.send_remote_checkpoint_with_capacity_check(Arc::new(checkpoint), size)
                    .await?;
            }
        }

        Ok(())
    }

    /// Fetches checkpoints from the live object store and streams them to a
    /// channel.
    async fn relay_from_live(
        &mut self,
        batch_size: usize,
        live: &dyn ObjectStore,
    ) -> IngestionResult<()> {
        let mut checkpoint_stream = (self.current_checkpoint_number..u64::MAX)
            .map(|checkpoint_number| fetch_from_object_store(live, checkpoint_number))
            .pipe(futures::stream::iter)
            .buffered(batch_size);
        while let Some((checkpoint, size)) = self
            .token
            .run_until_cancelled(checkpoint_stream.try_next())
            .await
            .transpose()?
            .flatten()
        {
            self.send_remote_checkpoint_with_capacity_check(checkpoint, size)
                .await?;
        }
        Ok(())
    }

    /// Fetches checkpoints from the fullnode through a gRPC streaming
    /// connection and streams them to a channel.
    async fn relay_from_fullnode(&mut self, client: &mut GrpcClient) -> IngestionResult<()> {
        let mut checkpoints_stream = client
            .stream_checkpoints(
                Some(self.current_checkpoint_number),
                None,
                Some(iota_grpc_client::CHECKPOINT_RESPONSE_CHECKPOINT_DATA),
                None,
                None,
            )
            .await
            .map_err(|e| {
                IngestionError::Grpc(format!("failed to initialize the checkpoint stream: {e}"))
            })?
            .into_inner();

        while let Some(grpc_checkpoint) = self
            .token
            .run_until_cancelled(checkpoints_stream.try_next())
            .await
            .transpose()?
            .flatten()
        {
            let checkpoint = grpc_checkpoint.checkpoint_data()?.try_into()?;
            let size = bcs::serialized_size(&checkpoint)?;
            self.send_remote_checkpoint_with_capacity_check(Arc::new(checkpoint), size)
                .await?;
        }

        Ok(())
    }

    /// Fetches remote checkpoints from the remote store and streams them to the
    /// channel.
    ///
    /// For every successfully fetched checkpoint, this function updates the
    /// current checkpoint number and the data limiter. If an error occurs while
    /// fetching a checkpoint, the function returns immediately with that error.
    async fn fetch_and_send_to_channel(&mut self) -> IngestionResult<()> {
        let Some(remote_store) = self.remote_store.as_ref().map(Arc::clone) else {
            return Ok(());
        };
        let batch_size = self.reader_options.batch_size;
        match remote_store.as_ref() {
            RemoteStore::Fullnode(client) => {
                self.relay_from_fullnode(&mut client.clone()).await?;
            }
            RemoteStore::HybridHistoricalStore { historical, live } => {
                if let Some(Err(err)) = self
                    .token
                    .clone()
                    .run_until_cancelled(self.relay_from_historical(historical))
                    .await
                {
                    if matches!(err, IngestionError::CheckpointNotAvailableYet) {
                        let live = live.as_ref().ok_or(err)?;
                        return self.relay_from_live(batch_size, live).await;
                    }
                    return Err(err);
                }
            }
        };
        Ok(())
    }

    /// Fetches and sends checkpoints to the channel with retry logic.
    ///
    /// Uses an exponential backoff strategy to retry failed requests.
    async fn fetch_and_send_to_channel_with_retry(&mut self) {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.max_elapsed_time = Some(Duration::from_secs(60));
        backoff.initial_interval = Duration::from_millis(100);
        backoff.current_interval = backoff.initial_interval;
        backoff.multiplier = 1.0;

        loop {
            match self.fetch_and_send_to_channel().await {
                Ok(_) => break,
                Err(IngestionError::MaxCheckpointsCapacityReached) => break,
                Err(IngestionError::CheckpointNotAvailableYet) => {
                    break info!("historical reader does not have the requested checkpoint yet");
                }
                Err(err) => match backoff.next_backoff() {
                    Some(duration) => {
                        if !err.to_string().to_lowercase().contains("not found") {
                            debug!(
                                "remote reader retry in {} ms. Error is {err:?}",
                                duration.as_millis(),
                            );
                        }
                        if self
                            .token
                            .run_until_cancelled(tokio::time::sleep(duration))
                            .await
                            .is_none()
                        {
                            break;
                        }
                    }
                    None => {
                        break error!("remote reader transient error {err:?}");
                    }
                },
            }
        }
    }

    /// Attempts to send a checkpoint from remote source to the channel if
    /// capacity allows.
    ///
    /// If the checkpoint's sequence number would exceed the allowed capacity,
    /// returns `IngestionError::MaxCheckpointsCapacityReached` and does not
    /// send. Otherwise, adds the checkpoint to the data limiter and sends
    /// it to the channel.
    async fn send_remote_checkpoint_with_capacity_check(
        &mut self,
        checkpoint: Arc<CheckpointData>,
        size: usize,
    ) -> IngestionResult<()> {
        if self.exceeds_capacity(checkpoint.checkpoint_summary.sequence_number) {
            return Err(IngestionError::MaxCheckpointsCapacityReached);
        }
        self.data_limiter.add(&checkpoint, size);
        self.send_checkpoint_to_channel(checkpoint).await
    }

    /// Sends a batch of local checkpoints to the channel in order.
    ///
    /// Each checkpoint is sent sequentially until a gap is detected (i.e., a
    /// checkpoint with a sequence number greater than the current
    /// checkpoint number). If a gap is found, the function breaks early. If
    /// sending fails, returns the error immediately.
    async fn send_local_checkpoints_to_channel(
        &mut self,
        checkpoints: Vec<Arc<CheckpointData>>,
    ) -> IngestionResult<()> {
        for checkpoint in checkpoints {
            if self.is_checkpoint_ahead(&checkpoint, self.current_checkpoint_number) {
                break;
            }
            self.send_checkpoint_to_channel(checkpoint).await?;
        }
        Ok(())
    }

    /// Sends a single checkpoint to the channel and advances the current
    /// checkpoint number.
    ///
    /// Asserts that the checkpoint's sequence number matches the expected
    /// current number. Increments the current checkpoint number after
    /// sending.
    async fn send_checkpoint_to_channel(
        &mut self,
        checkpoint: Arc<CheckpointData>,
    ) -> IngestionResult<()> {
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
        Ok(())
    }

    /// Sync from either local or remote source new checkpoints to be processed
    /// by the executor.
    async fn sync(&mut self) -> IngestionResult<()> {
        let mut remote_source = ReadSource::Local;
        let checkpoints = self.read_local_files_with_retry().await?;
        let should_fetch_from_remote = self.should_fetch_from_remote(&checkpoints);

        if should_fetch_from_remote {
            remote_source = ReadSource::Remote;
            self.fetch_and_send_to_channel_with_retry().await;
        } else {
            self.send_local_checkpoints_to_channel(checkpoints).await?;
        }

        info!(
            "Read from {remote_source}. Current checkpoint number: {}, pruning watermark: {}",
            self.current_checkpoint_number, self.last_pruned_watermark,
        );

        Ok(())
    }

    /// Run the main loop of the checkpoint reader actor.
    async fn run(mut self) {
        let (_inotify_tx, mut inotify_rx) = mpsc::channel::<()>(1);
        std::fs::create_dir_all(self.path()).expect("failed to create a directory");

        #[cfg(not(target_os = "macos"))]
        let _watcher = init_watcher(_inotify_tx, self.path());

        self.data_limiter.gc(self.last_pruned_watermark);
        self.gc_processed_files(self.last_pruned_watermark)
            .expect("failed to clean the directory");

        loop {
            tokio::select! {
                _ = self.token.cancelled() => break,
                Some(watermark) = self.gc_signal_rx.recv() => {
                    self.data_limiter.gc(watermark);
                    self.gc_processed_files(watermark).expect("failed to clean the directory");
                }
                Ok(Some(_)) | Err(_) = timeout(Duration::from_millis(self.reader_options.tick_interval_ms), inotify_rx.recv())  => {
                    self.sync().await.expect("failed to read checkpoint files");
                }
            }
        }
    }
}

/// Public API for interacting with the checkpoint reader actor.
///
/// It provides methods to receive streamed checkpoints, send garbage collection
/// signals, and gracefully shut down the background checkpoint reading task.
/// Internally, it communicates with a [`CheckpointReaderActor`], which manages
/// the actual checkpoint fetching and streaming logic.
pub(crate) struct CheckpointReader {
    handle: JoinHandle<()>,
    gc_signal_tx: mpsc::Sender<CheckpointSequenceNumber>,
    checkpoint_rx: mpsc::Receiver<Arc<CheckpointData>>,
    token: CancellationToken,
}

impl CheckpointReader {
    pub(crate) async fn new(
        starting_checkpoint_number: CheckpointSequenceNumber,
        config: CheckpointReaderConfig,
    ) -> IngestionResult<Self> {
        let (checkpoint_tx, checkpoint_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (gc_signal_tx, gc_signal_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);

        let remote_store = if let Some(url) = config.remote_store_url {
            Some(Arc::new(
                RemoteStore::new(
                    url,
                    config.reader_options.batch_size,
                    config.reader_options.timeout_secs,
                )
                .await?,
            ))
        } else {
            None
        };

        let path = match config.ingestion_path {
            Some(p) => p,
            None => tempfile::tempdir()?.keep(),
        };
        let token = CancellationToken::new();
        let reader = CheckpointReaderActor {
            path,
            current_checkpoint_number: starting_checkpoint_number,
            last_pruned_watermark: starting_checkpoint_number,
            checkpoint_tx,
            gc_signal_rx,
            remote_store,
            token: token.clone(),
            data_limiter: DataLimiter::new(config.reader_options.data_limit),
            reader_options: config.reader_options,
        };

        let handle = spawn_monitored_task!(reader.run());

        Ok(Self {
            handle,
            gc_signal_tx,
            checkpoint_rx,
            token,
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

    /// Gracefully shuts down the checkpoint reader task.
    ///
    /// It signals the background checkpoint reader actor to terminate, then
    /// awaits its completion. Any in-progress checkpoint reading or streaming
    /// operations will be stopped as part of the shutdown process.
    pub(crate) async fn shutdown(self) -> IngestionResult<()> {
        self.token.cancel();
        self.handle.await.map_err(|err| IngestionError::Shutdown {
            component: "checkpoint reader".into(),
            msg: err.to_string(),
        })
    }
}
