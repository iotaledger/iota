// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap, num::NonZeroUsize, ops::Range, path::PathBuf, sync::Arc, time::Duration,
};

use anyhow::{Context, Result, anyhow};
use backoff::future::retry;
use bytes::Bytes;
use futures::StreamExt;
use itertools::Itertools;
use object_store::{DynObjectStore, Error, ObjectStore, ObjectStoreExt, path::Path};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use url::Url;

use crate::object_store::{
    ObjectStoreDeleteExt, ObjectStoreGetExt, ObjectStoreListExt, ObjectStorePutExt,
};

pub const MANIFEST_FILENAME: &str = "MANIFEST";
pub const EPOCH_METADATA_FILENAME: &str = "_epoch_metadata.json";
/// Marker file written to an epoch directory in the store once all files for
/// that epoch have been written.
pub const SUCCESS_MARKER: &str = "_SUCCESS";

#[derive(Serialize, Deserialize)]
pub struct RootManifest {
    /// Epoch number paired with its end timestamp in ms (or 0 when unknown).
    pub available_epochs: Vec<(u64, u64)>,
}

impl RootManifest {
    pub fn new(available_epochs: Vec<(u64, u64)>) -> Self {
        RootManifest { available_epochs }
    }

    pub fn epoch_exists(&self, epoch: u64) -> bool {
        self.available_epochs.iter().any(|(e, _)| *e == epoch)
    }

    /// Parse a root MANIFEST from its JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Serialize this root MANIFEST into its JSON bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Serialize, Deserialize)]
pub struct EpochMetadata {
    pub epoch_end_timestamp_ms: u64,
}

impl EpochMetadata {
    pub fn to_bytes(&self) -> Result<Bytes> {
        Ok(Bytes::from(serde_json::to_vec(self)?))
    }
}

pub async fn get<S: ObjectStoreGetExt>(store: &S, src: &Path) -> Result<Bytes> {
    let bytes = retry(backoff::ExponentialBackoff::default(), || async {
        store.get_bytes(src).await.map_err(|e| {
            error!("Failed to read file from object store with error: {:?}", &e);
            backoff::Error::transient(e)
        })
    })
    .await?;
    Ok(bytes)
}

/// Writes bytes in the store with specified path.
pub async fn put<S: ObjectStorePutExt>(store: &S, src: &Path, bytes: Bytes) -> Result<()> {
    retry(backoff::ExponentialBackoff::default(), || async {
        if !bytes.is_empty() {
            store.put_bytes(src, bytes.clone()).await.map_err(|e| {
                error!("Failed to write file to object store with error: {:?}", &e);
                backoff::Error::transient(e)
            })
        } else {
            warn!("Not copying empty file: {:?}", src);
            Ok(())
        }
    })
    .await?;
    Ok(())
}

pub async fn copy_file<S: ObjectStoreGetExt, D: ObjectStorePutExt>(
    src: &Path,
    dest: &Path,
    src_store: &S,
    dest_store: &D,
) -> Result<()> {
    let bytes = get(src_store, src).await?;
    if !bytes.is_empty() {
        put(dest_store, dest, bytes).await
    } else {
        warn!("Not copying empty file: {:?}", src);
        Ok(())
    }
}

pub async fn delete_files<S: ObjectStoreDeleteExt>(
    files: &[Path],
    store: &S,
    concurrency: NonZeroUsize,
) -> Result<Vec<()>> {
    let results: Vec<Result<()>> = futures::stream::iter(files)
        .map(|f| {
            retry(backoff::ExponentialBackoff::default(), || async {
                store.delete_object(f).await.map_err(|e| {
                    error!("Failed to delete file on object store with error: {:?}", &e);
                    backoff::Error::transient(e)
                })
            })
        })
        .boxed()
        .buffer_unordered(concurrency.into())
        .collect()
        .await;
    results.into_iter().collect()
}

pub async fn delete_recursively<S: ObjectStoreDeleteExt + ObjectStoreListExt>(
    path: &Path,
    store: &S,
    concurrency: NonZeroUsize,
) -> Result<Vec<()>> {
    let mut paths_to_delete = vec![];
    let mut paths = store.list_objects(Some(path)).await;
    while let Some(res) = paths.next().await {
        if let Ok(object_metadata) = res {
            paths_to_delete.push(object_metadata.location);
        } else {
            return Err(res.err().unwrap().into());
        }
    }
    delete_files(&paths_to_delete, store, concurrency).await
}

pub fn path_to_filesystem(local_dir_path: PathBuf, location: &Path) -> anyhow::Result<PathBuf> {
    // Convert an `object_store::path::Path` to `std::path::PathBuf`
    let path = std::fs::canonicalize(local_dir_path)?;
    let mut url = Url::from_file_path(&path)
        .map_err(|_| anyhow!("Failed to parse input path: {}", path.display()))?;
    url.path_segments_mut()
        .map_err(|_| anyhow!("Failed to get path segments: {}", path.display()))?
        .pop_if_empty()
        .extend(location.parts());
    let new_path = url
        .to_file_path()
        .map_err(|_| anyhow!("Failed to convert url to path: {}", url.as_str()))?;
    Ok(new_path)
}

/// This function will find all child directories in the input store which are
/// of the form "epoch_num" and return a map of epoch number to the directory
/// path
pub async fn find_all_dirs_with_epoch_prefix(
    store: &Arc<DynObjectStore>,
    prefix: Option<&Path>,
) -> anyhow::Result<BTreeMap<u64, Path>> {
    let mut dirs = BTreeMap::new();
    let entries = store.list_with_delimiter(prefix).await?;
    for entry in entries.common_prefixes {
        if let Some(filename) = entry.filename() {
            if !filename.starts_with("epoch_") || filename.ends_with(".tmp") {
                continue;
            }
            let epoch = filename
                .split_once('_')
                .context("Failed to split dir name")
                .map(|(_, epoch)| epoch.parse::<u64>())??;
            dirs.insert(epoch, entry);
        }
    }
    Ok(dirs)
}

/// Finds all epochs in the store and returns them as a sorted list, paired
/// with each epoch's end timestamp in ms when its metadata file is present, or
/// 0 otherwise.
pub async fn list_all_epochs(object_store: Arc<DynObjectStore>) -> Result<Vec<(u64, u64)>> {
    let remote_epoch_dirs = find_all_dirs_with_epoch_prefix(&object_store, None).await?;
    let mut out = vec![];
    let mut success_marker_found = false;
    for (epoch, path) in remote_epoch_dirs.iter().sorted() {
        let success_marker = path.child(SUCCESS_MARKER);
        let get_result = object_store.get(&success_marker).await;
        match get_result {
            Err(_) => {
                if !success_marker_found {
                    error!("No success marker found for epoch: {epoch}");
                }
            }
            Ok(_) => {
                let metadata_path = path.child(EPOCH_METADATA_FILENAME);
                let epoch_end_timestamp_ms = match object_store.get_bytes(&metadata_path).await {
                    Ok(bytes) => match serde_json::from_slice::<EpochMetadata>(&bytes) {
                        Ok(metadata) => metadata.epoch_end_timestamp_ms,
                        Err(err) => {
                            warn!("Failed to parse epoch metadata for epoch {epoch}: {err}");
                            0
                        }
                    },
                    Err(_) => 0,
                };
                out.push((*epoch, epoch_end_timestamp_ms));
                success_marker_found = true;
            }
        }
    }
    Ok(out)
}

/// Writes the epochs existed in the store to the root MANIFEST (contains only a
/// list of epochs in the store) every 300 seconds.
// TODO: Is 300 seconds too frequent? Or should this be triggered by other
// events?
pub async fn run_manifest_update_loop(
    store: Arc<DynObjectStore>,
    mut recv: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    let mut update_interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        tokio::select! {
            _now = update_interval.tick() => {
                if let Ok(available_epochs) = list_all_epochs(store.clone()).await {
                    let manifest_path = Path::from(MANIFEST_FILENAME);
                    let manifest = RootManifest { available_epochs };
                    put(&store, &manifest_path, Bytes::from(manifest.to_bytes()?)).await?;
                }
            },
             _ = recv.recv() => break,
        }
    }
    Ok(())
}

/// This function will find all child directories in the input store which are
/// of the form "epoch_num" and return a map of epoch number to the directory
/// path
pub async fn find_all_files_with_epoch_prefix(
    store: &Arc<DynObjectStore>,
    prefix: Option<&Path>,
) -> anyhow::Result<Vec<Range<u64>>> {
    let mut ranges = Vec::new();
    let entries = store.list_with_delimiter(prefix).await?;
    for entry in entries.objects {
        let checkpoint_seq_range = entry
            .location
            .filename()
            .ok_or(anyhow!("Illegal file name"))?
            .split_once('.')
            .context("Failed to split dir name")?
            .0
            .split_once('_')
            .context("Failed to split dir name")
            .map(|(start, end)| Range {
                start: start.parse::<u64>().unwrap(),
                end: end.parse::<u64>().unwrap(),
            })?;

        ranges.push(checkpoint_seq_range);
    }
    Ok(ranges)
}

/// This function will find missing epoch directories in the input store and
/// return a list of such epoch numbers. If the highest epoch directory in the
/// store is `epoch_N` then it is expected that the store will have all epoch
/// directories from `epoch_0` to `epoch_N`. Additionally, any epoch directory
/// should have the passed in marker file present or else that epoch number is
/// already considered as missing.
/// The returned list will contain epoch_N+1.
pub async fn find_missing_epochs_dirs(
    store: &Arc<DynObjectStore>,
    success_marker: &str,
) -> anyhow::Result<Vec<u64>> {
    let remote_checkpoints_by_epoch = find_all_dirs_with_epoch_prefix(store, None).await?;
    let mut dirs: Vec<_> = remote_checkpoints_by_epoch.iter().collect();
    dirs.sort_by_key(|(epoch_num, _path)| *epoch_num);
    let mut candidate_epoch: u64 = 0;
    let mut missing_epochs = Vec::new();
    for (epoch_num, path) in dirs {
        while candidate_epoch < *epoch_num {
            // The whole epoch directory is missing
            missing_epochs.push(candidate_epoch);
            candidate_epoch += 1;
            continue;
        }
        let success_marker = path.child(success_marker);
        let get_result = store.get(&success_marker).await;
        match get_result {
            Err(Error::NotFound { .. }) => {
                error!("No success marker found in remote store for epoch: {epoch_num}");
                missing_epochs.push(*epoch_num);
            }
            Err(_) => {
                // Probably a transient error
                warn!(
                    "Failed while trying to read success marker in remote store for epoch: {epoch_num}"
                );
            }
            Ok(_) => {
                // Nothing to do
            }
        }
        candidate_epoch += 1
    }
    missing_epochs.push(candidate_epoch);
    Ok(missing_epochs)
}

pub fn get_path(prefix: &str) -> Path {
    Path::from(prefix)
}

#[cfg(test)]
mod tests {
    use std::{fs, num::NonZeroUsize};

    use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
    use object_store::path::Path;
    use tempfile::TempDir;

    use crate::object_store::util::delete_recursively;

    #[tokio::test]
    pub async fn test_delete_recursively() -> anyhow::Result<()> {
        let input = TempDir::new()?;
        let input_path = input.path();
        let child = input_path.join("child");
        fs::create_dir(&child)?;
        let file1 = child.join("file1");
        fs::write(file1, b"Lorem ipsum")?;
        let grandchild = child.join("grand_child");
        fs::create_dir(&grandchild)?;
        let file2 = grandchild.join("file2");
        fs::write(file2, b"Lorem ipsum")?;

        let input_store = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::File),
            directory: Some(input_path.to_path_buf()),
            ..Default::default()
        }
        .make()?;

        delete_recursively(
            &Path::from("child"),
            &input_store,
            NonZeroUsize::new(1).unwrap(),
        )
        .await?;

        assert!(!input_path.join("child").join("file1").exists());
        assert!(
            !input_path
                .join("child")
                .join("grand_child")
                .join("file2")
                .exists()
        );
        Ok(())
    }
}
