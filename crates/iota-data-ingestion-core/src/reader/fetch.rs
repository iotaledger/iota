// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use iota_rest_api::{CheckpointData, Client};
use iota_storage::blob::Blob;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use notify::{RecursiveMode, Watcher};
use object_store::{ObjectStore, path::Path as ObjectStorePath};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::{
    IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS, history::CHECKPOINT_FILE_SUFFIX,
};

pub type CheckpointResult = IngestionResult<(Arc<CheckpointData>, usize)>;

/// Managing and processing checkpoint files in a directory.
pub(crate) trait LocalRead {
    /// Path is used as the source for reading checkpoint files.
    fn path(&self) -> &Path;

    /// Returns the current checkpoint sequence number.
    fn current_checkpoint_number(&self) -> CheckpointSequenceNumber;

    fn update_last_pruned_watermark(&mut self, watermark: CheckpointSequenceNumber);

    /// Returns `true` if the given checkpoint sequence number exceeds the
    /// allowed capacity.
    fn exceeds_capacity(&self, checkpoint_number: CheckpointSequenceNumber) -> bool;

    /// Returns `true` if the checkpoint's sequence number is ahead of the
    /// expected sequence number, indicating a gap in the processed
    /// checkpoints.
    fn is_checkpoint_ahead(
        &self,
        checkpoint: &CheckpointData,
        expected_sequence_number: CheckpointSequenceNumber,
    ) -> bool {
        checkpoint.checkpoint_summary.sequence_number > expected_sequence_number
    }

    /// Lists unprocessed checkpoint files in the specified directory.
    ///
    /// Scans the checkpoint directory for files whose sequence number is
    /// greater than or equal to the current checkpoint number. Returns a
    /// map of sequence numbers to file paths, sorted in ascending order.
    fn list_unprocessed_checkpoint_files(
        &self,
    ) -> IngestionResult<BTreeMap<CheckpointSequenceNumber, PathBuf>> {
        let mut files = BTreeMap::new();
        for entry in fs::read_dir(self.path())? {
            let entry = entry?;
            let filename = entry.file_name();
            if let Some(sequence_number) = self.checkpoint_number_from_file_path(&filename) {
                if sequence_number >= self.current_checkpoint_number() {
                    files.insert(sequence_number, entry.path());
                }
            }
        }
        Ok(files)
    }

    /// Reads and deserializes unprocessed checkpoint files from the directory,
    /// up to capacity.
    ///
    /// Iterates over unprocessed checkpoint files, deserializing each into a
    /// [`CheckpointData`]. Stops early if the capacity is exceeded, as
    /// determined by [`LocalRead::exceeds_capacity`], or when
    /// [`MAX_CHECKPOINTS_IN_PROGRESS`] files have been processed.
    fn read_local_files(&self) -> IngestionResult<Vec<Arc<CheckpointData>>> {
        // files are already sorted by sequence number in ascending order
        let files = self.list_unprocessed_checkpoint_files()?;
        debug!("unprocessed local files {:?}", files);
        let mut checkpoints = vec![];
        for (_, filename) in files.iter().take(MAX_CHECKPOINTS_IN_PROGRESS) {
            let checkpoint = self.read_checkpoint_file(filename)?;
            if self.exceeds_capacity(checkpoint.checkpoint_summary.sequence_number) {
                break;
            }
            checkpoints.push(checkpoint);
        }
        Ok(checkpoints)
    }

    /// Reads and deserializes unprocessed checkpoint files with retry and
    /// capacity check.
    ///
    /// This method wraps [`LocalRead::read_local_files`] with an
    /// exponential backoff retry mechanism to handle transient read errors.
    /// Retries are performed according to the default
    /// [`backoff::ExponentialBackoff`] policy.
    async fn read_local_files_with_retry(&self) -> IngestionResult<Vec<Arc<CheckpointData>>> {
        let backoff = backoff::ExponentialBackoff::default();
        backoff::future::retry(backoff, || async {
            self.read_local_files().map_err(|err| {
                info!("transient local read error {err:?}");
                backoff::Error::transient(err)
            })
        })
        .await
    }

    /// Reads and deserializes a checkpoint file.
    fn read_checkpoint_file(&self, filename: &Path) -> IngestionResult<Arc<CheckpointData>> {
        let data = fs::read(filename)?;
        Blob::from_bytes::<Arc<CheckpointData>>(&data)
            .map_err(|err| IngestionError::DeserializeCheckpoint(err.to_string()))
    }

    fn checkpoint_number_from_file_path(
        &self,
        file_name: &OsString,
    ) -> Option<CheckpointSequenceNumber> {
        file_name
            .to_str()
            .and_then(|s| s.rfind('.').map(|pos| &s[..pos]))
            .and_then(|s| s.parse().ok())
    }

    /// Cleans the local directory by removing all processed checkpoint files.
    fn gc_processed_files(&mut self, watermark: CheckpointSequenceNumber) -> IngestionResult<()> {
        info!("cleaning processed files, watermark is {watermark}");
        self.update_last_pruned_watermark(watermark);
        for entry in fs::read_dir(self.path())? {
            let entry = entry?;
            let filename = entry.file_name();
            if let Some(sequence_number) = self.checkpoint_number_from_file_path(&filename) {
                if sequence_number < watermark {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

    /// Sets up an inotify watcher on the given path and returns the watcher and
    /// a receiver for notifications.
    ///
    /// This function creates the directory if it does not exist, sets up a
    /// notify watcher, and returns both the watcher and a receiver that
    /// yields a unit value `()` whenever a filesystem event occurs.
    fn setup_directory_watcher(&self) -> (notify::RecommendedWatcher, mpsc::Receiver<()>) {
        let (inotify_sender, inotify_recv) = mpsc::channel(1);
        std::fs::create_dir_all(self.path()).expect("failed to create a directory");
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Err(err) = res {
                eprintln!("watch error: {err:?}");
            }
            inotify_sender
                .blocking_send(())
                .expect("Failed to send inotify update");
        })
        .expect("Failed to init inotify");

        watcher
            .watch(self.path(), RecursiveMode::NonRecursive)
            .expect("Inotify watcher failed");

        (watcher, inotify_recv)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ReadSource {
    Local,
    Remote,
}

impl Display for ReadSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadSource::Local => write!(f, "local"),
            ReadSource::Remote => write!(f, "remote"),
        }
    }
}

/// Fetches and deserializes a checkpoint from an object store.
pub async fn fetch_from_object_store(
    store: &dyn ObjectStore,
    checkpoint_number: CheckpointSequenceNumber,
) -> CheckpointResult {
    let path = ObjectStorePath::from(format!("{checkpoint_number}.{CHECKPOINT_FILE_SUFFIX}"));
    let response = store.get(&path).await?;
    let bytes = response.bytes().await?;
    Ok((
        Blob::from_bytes::<Arc<CheckpointData>>(&bytes)
            .map_err(|err| IngestionError::DeserializeCheckpoint(err.to_string()))?,
        bytes.len(),
    ))
}

/// Fetches and deserializes a checkpoint from a full node via REST API.
pub async fn fetch_from_full_node(
    client: &Client,
    checkpoint_number: CheckpointSequenceNumber,
) -> CheckpointResult {
    let checkpoint = client.get_full_checkpoint(checkpoint_number).await?;
    let size = bcs::serialized_size(&checkpoint)?;
    Ok((Arc::new(checkpoint), size))
}
