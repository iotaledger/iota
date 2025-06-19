// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
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
use serde::{Deserialize, Serialize};
use tap::Pipe;
use tokio::{
    sync::{
        mpsc::{self},
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
        fetch::{CheckpointResult, LocalRead, ReadSource, fetch_from_full_node},
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
    /// A REST API endpoint for checkpoint data.
    Rest(String),
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
    Rest(iota_rest_api::Client),
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
            RemoteUrl::Rest(url) => RemoteStore::Rest(iota_rest_api::Client::new(url)),
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
                    .inspect_err(|e| error!("Unable to instantiate historical reader: {e}"))?;

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
    /// Signal when the reader should exit.
    shutdown_rx: oneshot::Receiver<()>,
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
                || checkpoints[0].checkpoint_summary.sequence_number
                    > self.current_checkpoint_number)
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

    /// Fetch checkpoints from the historical object store and stream them to a
    /// channel.
    async fn fetch_from_historical_object_store(
        &mut self,
        historical_reader: &HistoricalReader,
    ) -> IngestionResult<()> {
        if self.current_checkpoint_number > historical_reader.latest_available_checkpoint().await? {
            historical_reader.sync_manifest_once().await?;
        }

        let manifest = historical_reader.get_manifest().await;

        let latest_available_checkpoint = manifest
            .next_checkpoint_seq_num()
            .checked_sub(1)
            .ok_or_else(|| {
                IngestionError::HistoryRead("no checkpoint data in the remote store".into())
            })?;

        if self.current_checkpoint_number > latest_available_checkpoint {
            return Err(IngestionError::CheckpointNotAvailableYet);
        }

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
            let checkpoints = historical_reader
                .iter_for_file(metadata.file_path())
                .await?
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
            RemoteStore::Rest(client) => {
                let mut checkpoint_stream = (self.current_checkpoint_number..u64::MAX)
                    .map(|checkpoint_number| fetch_from_full_node(client, checkpoint_number))
                    .pipe(futures::stream::iter)
                    .buffered(batch_size);

                while let Some(checkpoint_reuslt) = checkpoint_stream.next().await {
                    let (checkpoint, size) = checkpoint_reuslt?;
                    self.send_remote_checkpoint_with_capacity_check(checkpoint, size)
                        .await?;
                }
            }
            RemoteStore::HybridHistoricalStore { historical, live } => {
                if let Err(err) = self.fetch_from_historical_object_store(historical).await {
                    if matches!(err, IngestionError::CheckpointNotAvailableYet) {
                        let live = match live {
                            Some(live) => live,
                            None => return Err(err),
                        };

                        let mut checkpoint_stream = (self.current_checkpoint_number..u64::MAX)
                            .map(|checkpoint_number| {
                                Self::fetch_from_live_object_store(live, checkpoint_number)
                            })
                            .pipe(futures::stream::iter)
                            .buffered(batch_size);

                        while let Some(checkpoint_reuslt) = checkpoint_stream.next().await {
                            let (checkpoint, size) = checkpoint_reuslt?;
                            self.send_remote_checkpoint_with_capacity_check(checkpoint, size)
                                .await?;
                        }
                        return Ok(());
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
                Err(err) => {
                    // once reached the tip of the network, the historical reader can take some
                    // time to issue a new checkpoint file, we reset the
                    // backoff only in this case.
                    if matches!(err, IngestionError::CheckpointNotAvailableYet)
                        && backoff.next_backoff().is_none()
                    {
                        info!(
                            "Resetting backoff, historical reader does not have the requested checkpoint yet"
                        );
                        backoff.reset();
                    };

                    match backoff.next_backoff() {
                        Some(duration) => {
                            if !err.to_string().contains("404") {
                                debug!(
                                    "remote reader retry in {} ms. Error is {:?}",
                                    duration.as_millis(),
                                    err
                                );
                            }
                            tokio::time::sleep(duration).await
                        }
                        None => {
                            error!("remote reader transient error {:?}", err);
                            break;
                        }
                    }
                }
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
    /// Each checkpoint is sent sequentially. If sending fails, returns the
    /// error immediately.
    async fn send_local_checkpoints_to_channel(
        &mut self,
        checkpoints: Vec<Arc<CheckpointData>>,
    ) -> IngestionResult<()> {
        for checkpoint in checkpoints {
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
        self.send_local_checkpoints_to_channel(checkpoints).await?;

        if should_fetch_from_remote {
            remote_source = ReadSource::Remote;
            self.fetch_and_send_to_channel_with_retry().await;
        }

        info!(
            "Read from {remote_source}. Current checkpoint number: {}, pruning watermark: {}",
            self.current_checkpoint_number, self.last_pruned_watermark,
        );

        Ok(())
    }

    /// Run the main loop of the checkpoint reader actor.
    async fn run(mut self) {
        let (_watcher, mut inotify_rx) = self.setup_directory_watcher();
        self.data_limiter.gc(self.last_pruned_watermark);
        self.gc_processed_files(self.last_pruned_watermark)
            .expect("Failed to clean the directory");

        loop {
            tokio::select! {
                _ = &mut self.shutdown_rx => break,
                Some(watermark) = self.gc_signal_rx.recv() => {
                    self.data_limiter.gc(watermark);
                    self.gc_processed_files(watermark).expect("Failed to clean the directory");
                }
                Ok(Some(_)) | Err(_) = timeout(Duration::from_millis(self.reader_options.tick_interval_ms), inotify_rx.recv())  => {
                    self.sync().await.expect("Failed to read checkpoint files");
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
    shutdown_tx: oneshot::Sender<()>,
    gc_signal_tx: mpsc::Sender<CheckpointSequenceNumber>,
    checkpoint_rx: mpsc::Receiver<Arc<CheckpointData>>,
}

impl CheckpointReader {
    pub(crate) async fn new(
        starting_checkpoint_number: CheckpointSequenceNumber,
        config: CheckpointReaderConfig,
    ) -> IngestionResult<Self> {
        let (checkpoint_tx, checkpoint_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (gc_signal_tx, gc_signal_rx) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let remote_store: OptionFuture<_> = config
            .remote_store_url
            .map(|url| {
                RemoteStore::new(
                    url,
                    config.reader_options.batch_size,
                    config.reader_options.timeout_secs,
                )
            })
            .into();

        let remote_store = remote_store.await.transpose()?.map(Arc::new);

        let path = match config.ingestion_path {
            Some(p) => p,
            None => tempfile::tempdir()?.into_path(),
        };

        let reader = CheckpointReaderActor {
            path,
            current_checkpoint_number: starting_checkpoint_number,
            last_pruned_watermark: starting_checkpoint_number,
            checkpoint_tx,
            gc_signal_rx,
            remote_store,
            shutdown_rx,
            data_limiter: DataLimiter::new(config.reader_options.data_limit),
            reader_options: config.reader_options,
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

    /// Gracefully shuts down the checkpoint reader task.
    ///
    /// It signals the background checkpoint reader actor to terminate, then
    /// awaits its completion. Any in-progress checkpoint reading or streaming
    /// operations will be stopped as part of the shutdown process.
    pub(crate) async fn shutdown(self) -> IngestionResult<()> {
        _ = self.shutdown_tx.send(());
        self.handle.await.map_err(|err| IngestionError::Shutdown {
            component: "CheckpointReader".into(),
            msg: err.to_string(),
        })
    }
}
