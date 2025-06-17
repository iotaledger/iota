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

use crate::{IngestionError, IngestionResult, MAX_CHECKPOINTS_IN_PROGRESS};

pub type CheckpointResult = IngestionResult<(Arc<CheckpointData>, usize)>;

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

/// Reads and deserializes unprocessed checkpoint files from the given
/// directory, up to a capacity limit.
///
/// This function scans the specified path for checkpoint files whose sequence
/// number is greater than or equal to `current_checkpoint_number`. It attempts
/// to deserialize each file into a [`CheckpointData`], collecting them
/// into a vector. The process stops early if the provided `exceeds_capacity`
/// function returns `true` for a checkpoint's sequence number,
/// or when `MAX_CHECKPOINTS_IN_PROGRESS` files have been processed.
pub async fn read_local_files_with_capacity_check<F>(
    path: &Path,
    current_checkpoint_number: CheckpointSequenceNumber,
    exceeds_capacity: F,
) -> IngestionResult<Vec<Arc<CheckpointData>>>
where
    F: Fn(CheckpointSequenceNumber) -> bool,
{
    // files are already sorted by sequence number in ascending order
    let files = list_unprocessed_checkpoint_files(path, current_checkpoint_number)?;
    debug!("unprocessed local files {:?}", files);
    let mut checkpoints = vec![];
    for (_, filename) in files.iter().take(MAX_CHECKPOINTS_IN_PROGRESS) {
        let checkpoint = read_checkpoint_file(filename)?;
        if exceeds_capacity(checkpoint.checkpoint_summary.sequence_number) {
            break;
        }
        checkpoints.push(checkpoint);
    }
    Ok(checkpoints)
}

/// Reads and deserializes unprocessed checkpoint files from the given
/// directory with retry and capacity check.
///
/// This function  wraps [`read_local_files_with_capacity_check`] with an
/// exponential backoff retry mechanism to handle transient read errors.
pub async fn read_local_files_with_retry_and_capacity_check<F>(
    path: &Path,
    current_checkpoint_number: CheckpointSequenceNumber,
    exceeds_capacity: F,
) -> IngestionResult<Vec<Arc<CheckpointData>>>
where
    F: Fn(CheckpointSequenceNumber) -> bool + Copy,
{
    let backoff = backoff::ExponentialBackoff::default();
    backoff::future::retry(backoff, || async {
        read_local_files_with_capacity_check(path, current_checkpoint_number, exceeds_capacity)
            .await
            .map_err(|err| {
                info!("transient local read error {err:?}");
                backoff::Error::transient(err)
            })
    })
    .await
}

/// Lists unprocessed checkpoint files in the specified directory.
fn list_unprocessed_checkpoint_files(
    path: &Path,
    current_checkpoint_number: CheckpointSequenceNumber,
) -> IngestionResult<BTreeMap<CheckpointSequenceNumber, PathBuf>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let filename = entry.file_name();
        if let Some(sequence_number) = checkpoint_number_from_file_path(&filename) {
            if sequence_number >= current_checkpoint_number {
                files.insert(sequence_number, entry.path());
            }
        }
    }
    Ok(files)
}

/// Reads and deserializes a checkpoint file.
pub fn read_checkpoint_file(filename: &Path) -> IngestionResult<Arc<CheckpointData>> {
    let data = fs::read(filename)?;
    Blob::from_bytes::<Arc<CheckpointData>>(&data)
        .map_err(|err| IngestionError::DeserializeCheckpoint(err.to_string()))
}

pub fn checkpoint_number_from_file_path(file_name: &OsString) -> Option<CheckpointSequenceNumber> {
    file_name
        .to_str()
        .and_then(|s| s.rfind('.').map(|pos| &s[..pos]))
        .and_then(|s| s.parse().ok())
}

/// Sets up an inotify watcher on the given path and returns the watcher and a
/// receiver for notifications.
///
/// This function creates the directory if it does not exist, sets up a notify
/// watcher, and returns both the watcher and a receiver that yields a unit
/// value `()` whenever a filesystem event occurs.
pub fn setup_directory_watcher(path: &Path) -> (notify::RecommendedWatcher, mpsc::Receiver<()>) {
    let (inotify_sender, inotify_recv) = mpsc::channel(1);
    std::fs::create_dir_all(path).expect("failed to create a directory");
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Err(err) = res {
            eprintln!("watch error: {:?}", err);
        }
        inotify_sender
            .blocking_send(())
            .expect("Failed to send inotify update");
    })
    .expect("Failed to init inotify");

    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .expect("Inotify watcher failed");

    (watcher, inotify_recv)
}

/// Fetches and deserializes a checkpoint from an object store.
pub async fn fetch_from_object_store(
    store: &dyn ObjectStore,
    checkpoint_number: CheckpointSequenceNumber,
) -> CheckpointResult {
    let path = ObjectStorePath::from(format!("{}.chk", checkpoint_number));
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
