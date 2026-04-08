// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use backoff::backoff::Backoff;
use futures::StreamExt;
use iota_grpc_client::Client as GrpcClient;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::ObjectStore;
use tap::pipe::Pipe;
use tokio::{
    sync::{
        mpsc::{self, error::TryRecvError},
        oneshot,
    },
    time::timeout,
};
use tracing::{debug, error, info};

#[cfg(not(target_os = "macos"))]
use crate::reader::fetch::init_watcher;
use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, create_remote_store_client,
    reader::fetch::{
        CheckpointResult, GRPC_MAX_DECODING_MESSAGE_SIZE_BYTES, LocalRead, ReadSource,
        fetch_from_full_node, fetch_from_object_store,
    },
};

/// Implements a checkpoint reader that monitors a local directory.
/// Designed for setups where the indexer daemon is colocated with FN.
/// This implementation is push-based and utilizes the inotify API.
pub struct CheckpointReader {
    path: PathBuf,
    remote_store_url: Option<String>,
    remote_store_options: Vec<(String, String)>,
    current_checkpoint_number: CheckpointSequenceNumber,
    last_pruned_watermark: CheckpointSequenceNumber,
    checkpoint_sender: mpsc::Sender<Arc<CheckpointData>>,
    processed_receiver: mpsc::Receiver<CheckpointSequenceNumber>,
    remote_fetcher_receiver: Option<mpsc::Receiver<CheckpointResult>>,
    exit_receiver: oneshot::Receiver<()>,
    options: ReaderOptions,
    data_limiter: DataLimiter,
}

impl LocalRead for CheckpointReader {
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

/// Options for configuring how the checkpoint reader fetches new checkpoints.
#[derive(Clone)]
pub struct ReaderOptions {
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
}

impl Default for ReaderOptions {
    fn default() -> Self {
        Self {
            tick_interval_ms: 100,
            timeout_secs: 5,
            batch_size: 10,
            data_limit: 0,
        }
    }
}

/// Remote checkpoint store backends.
enum RemoteStore {
    Hybrid(Box<dyn ObjectStore>, Box<iota_grpc_client::Client>),
}

impl CheckpointReader {
    async fn remote_fetch_checkpoint_internal(
        store: &RemoteStore,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> CheckpointResult {
        match store {
            RemoteStore::Hybrid(store, client) => {
                match fetch_from_full_node(client, checkpoint_number).await {
                    Ok(result) => Ok(result),
                    Err(_) => fetch_from_object_store(store, checkpoint_number).await,
                }
            }
        }
    }

    async fn remote_fetch_checkpoint(
        store: &RemoteStore,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> CheckpointResult {
        let mut backoff = backoff::ExponentialBackoff::default();
        backoff.max_elapsed_time = Some(Duration::from_secs(60));
        backoff.initial_interval = Duration::from_millis(100);
        backoff.current_interval = backoff.initial_interval;
        backoff.multiplier = 1.0;
        loop {
            match Self::remote_fetch_checkpoint_internal(store, checkpoint_number).await {
                Ok(data) => return Ok(data),
                Err(err) => match backoff.next_backoff() {
                    Some(duration) => {
                        if !err.to_string().contains("404") {
                            debug!(
                                "remote reader retry in {} ms. Error is {err:?}",
                                duration.as_millis(),
                            );
                        }
                        tokio::time::sleep(duration).await
                    }
                    None => return Err(err),
                },
            }
        }
    }

    async fn start_remote_fetcher(&mut self) -> mpsc::Receiver<CheckpointResult> {
        let batch_size = self.options.batch_size;
        let start_checkpoint = self.current_checkpoint_number;
        let (sender, receiver) = mpsc::channel(batch_size);
        let url = self
            .remote_store_url
            .clone()
            .expect("remote store url must be set");
        let remote_store_options = self.remote_store_options.clone();
        let timeout_secs = self.options.timeout_secs;

        let (fullnode_url, object_store_url) = url.split_once('|').unwrap_or((&url, &url));

        let object_store = create_remote_store_client(
            object_store_url.to_string(),
            remote_store_options,
            timeout_secs,
        )
        .expect("failed to create remote store client");

        let grpc_client = GrpcClient::connect(fullnode_url)
            .await
            .map(|client| {
                client.with_max_decoding_message_size(GRPC_MAX_DECODING_MESSAGE_SIZE_BYTES)
            })
            .expect("failed to connect to gRPC fullnode");

        let store = RemoteStore::Hybrid(object_store, Box::new(grpc_client));

        spawn_monitored_task!(async move {
            let mut checkpoint_stream = (start_checkpoint..u64::MAX)
                .map(|checkpoint_number| Self::remote_fetch_checkpoint(&store, checkpoint_number))
                .pipe(futures::stream::iter)
                .buffered(batch_size);

            while let Some(checkpoint) = checkpoint_stream.next().await {
                if sender.send(checkpoint).await.is_err() {
                    info!("remote reader dropped");
                    break;
                }
            }
        });
        receiver
    }

    async fn remote_fetch(&mut self) -> Vec<Arc<CheckpointData>> {
        let mut checkpoints = vec![];
        if self.remote_fetcher_receiver.is_none() {
            self.remote_fetcher_receiver = Some(self.start_remote_fetcher().await);
        }
        while !self.exceeds_capacity(self.current_checkpoint_number + checkpoints.len() as u64) {
            match self.remote_fetcher_receiver.as_mut().unwrap().try_recv() {
                Ok(Ok((checkpoint, size))) => {
                    self.data_limiter.add(&checkpoint, size);
                    checkpoints.push(checkpoint);
                }
                Ok(Err(err)) => {
                    error!("remote reader transient error {:?}", err);
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
        let mut checkpoints = self.read_local_files_with_retry().await?;

        let mut read_source = ReadSource::Local;
        if self.remote_store_url.is_some()
            && (checkpoints.is_empty()
                || checkpoints[0].checkpoint_summary.sequence_number
                    > self.current_checkpoint_number)
        {
            checkpoints = self.remote_fetch().await;
            read_source = ReadSource::Remote;
        } else {
            // cancel remote fetcher execution because local reader has made progress
            self.remote_fetcher_receiver = None;
        }

        info!(
            "Read from {read_source}. Current checkpoint number: {}, pruning watermark: {}, new updates: {:?}",
            self.current_checkpoint_number,
            self.last_pruned_watermark,
            checkpoints.len(),
        );
        for checkpoint in checkpoints {
            if matches!(read_source, ReadSource::Local)
                && self.is_checkpoint_ahead(&checkpoint, self.current_checkpoint_number)
            {
                break;
            }
            assert_eq!(
                checkpoint.checkpoint_summary.sequence_number,
                self.current_checkpoint_number
            );
            self.checkpoint_sender.send(checkpoint).await.map_err(|_| {
                IngestionError::Channel(
                    "unable to send checkpoint to executor, receiver half closed".to_owned(),
                )
            })?;
            self.current_checkpoint_number += 1;
        }
        Ok(())
    }

    pub fn initialize(
        path: PathBuf,
        starting_checkpoint_number: CheckpointSequenceNumber,
        remote_store_url: Option<String>,
        remote_store_options: Vec<(String, String)>,
        options: ReaderOptions,
    ) -> (
        Self,
        mpsc::Receiver<Arc<CheckpointData>>,
        mpsc::Sender<CheckpointSequenceNumber>,
        oneshot::Sender<()>,
    ) {
        let (checkpoint_sender, checkpoint_recv) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (processed_sender, processed_receiver) = mpsc::channel(MAX_CHECKPOINTS_IN_PROGRESS);
        let (exit_sender, exit_receiver) = oneshot::channel();
        let reader = Self {
            path,
            remote_store_url,
            remote_store_options,
            current_checkpoint_number: starting_checkpoint_number,
            last_pruned_watermark: starting_checkpoint_number,
            checkpoint_sender,
            processed_receiver,
            remote_fetcher_receiver: None,
            exit_receiver,
            data_limiter: DataLimiter::new(options.data_limit),
            options,
        };
        (reader, checkpoint_recv, processed_sender, exit_sender)
    }

    pub async fn run(mut self) -> IngestionResult<()> {
        let (_inotify_sender, mut inotify_recv) = mpsc::channel::<()>(1);
        fs::create_dir_all(self.path()).expect("failed to create a directory");

        #[cfg(not(target_os = "macos"))]
        let _watcher = init_watcher(_inotify_sender, self.path());

        self.data_limiter.gc(self.last_pruned_watermark);
        self.gc_processed_files(self.last_pruned_watermark)
            .expect("failed to clean the directory");
        loop {
            tokio::select! {
                _ = &mut self.exit_receiver => break,
                Some(gc_checkpoint_number) = self.processed_receiver.recv() => {
                    self.data_limiter.gc(gc_checkpoint_number);
                    self.gc_processed_files(gc_checkpoint_number).expect("failed to clean the directory");
                }
                Ok(Some(_)) | Err(_) = timeout(Duration::from_millis(self.options.tick_interval_ms), inotify_recv.recv())  => {
                    self.sync().await.expect("failed to read checkpoint files");
                }
            }
        }
        Ok(())
    }
}

/// Tracks and limits the total in-progress data size for checkpoint processing.
///
/// `DataLimiter` is used to prevent excessive memory usage by keeping track of
/// the cumulative size of checkpoints currently being processed. It maintains a
/// queue of checkpoint sequence numbers and their associated sizes, and
/// provides methods to check if the limit is exceeded, add new checkpoints, and
/// perform garbage collection of processed checkpoints.
pub struct DataLimiter {
    /// The maximum allowed in-progress data size (in bytes). Zero means no
    /// limit.
    limit: usize,
    /// A mapping from checkpoint sequence number to its data size (in bytes)
    queue: BTreeMap<CheckpointSequenceNumber, usize>,
    /// The current total in-progress data size (in bytes).
    in_progress: usize,
}

impl DataLimiter {
    /// Creates a new `DataLimiter` with the specified memory limit (in bytes).
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            queue: BTreeMap::new(),
            in_progress: 0,
        }
    }

    /// Returns `true` if the current in-progress data size exceeds the
    /// configured limit.
    pub fn exceeds(&self) -> bool {
        self.limit > 0 && self.in_progress >= self.limit
    }

    /// Adds a checkpoint's data size to the in-progress queue.
    pub fn add(&mut self, checkpoint: &CheckpointData, size: usize) {
        if self.limit == 0 {
            return;
        }
        self.in_progress += size;
        self.queue
            .insert(checkpoint.checkpoint_summary.sequence_number, size);
    }

    /// Performs garbage collection by removing all checkpoints with a sequence
    /// number less than the given `watermark`, and recalculates the total
    /// in-progress size.
    pub fn gc(&mut self, watermark: CheckpointSequenceNumber) {
        if self.limit == 0 {
            return;
        }
        self.queue = self.queue.split_off(&watermark);
        self.in_progress = self.queue.values().sum();
    }
}
