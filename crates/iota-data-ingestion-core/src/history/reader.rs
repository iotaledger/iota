// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::Range, sync::Arc, time::Duration};

use bytes::{Buf, Bytes, buf::Reader};
use futures::{Stream, StreamExt, TryStreamExt};
use iota_config::node::ArchiveReaderConfig as HistoricalReaderConfig;
use iota_storage::{
    compute_sha3_checksum_for_bytes, make_iterator,
    object_store::{ObjectStoreGetExt, http::HttpDownloaderBuilder, util::get},
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::path::Path;
use tokio::sync::{
    Mutex,
    oneshot::{self, Sender},
};
use tracing::info;

use crate::{
    IngestionError,
    errors::IngestionResult as Result,
    history::{
        CHECKPOINT_FILE_MAGIC,
        manifest::{FileMetadata, Manifest, read_manifest},
    },
};

#[derive(Clone)]
pub struct HistoricalReader {
    concurrency: usize,
    #[expect(dead_code)]
    /// We store this to get dropped along with the
    /// reader and hence terminate the manifest sync
    /// process.
    sender: Arc<Sender<()>>,
    manifest: Arc<Mutex<Manifest>>,
    remote_object_store: Arc<dyn ObjectStoreGetExt>,
}

impl HistoricalReader {
    pub fn new(config: HistoricalReaderConfig) -> Result<Self> {
        let remote_object_store = if config.remote_store_config.no_sign_request {
            config.remote_store_config.make_http()?
        } else {
            config.remote_store_config.make().map(Arc::new)?
        };
        let (sender, recv) = oneshot::channel();
        let manifest = Arc::new(Mutex::new(Manifest::new(0)));
        // Start a background tokio task to keep local manifest in sync with remote
        Self::spawn_manifest_sync_task(remote_object_store.clone(), manifest.clone(), recv);
        Ok(Self {
            manifest,
            sender: Arc::new(sender),
            remote_object_store,
            concurrency: config.download_concurrency.get(),
        })
    }

    /// This function verifies the manifest and returns the file metadata
    /// sorted by the starting sequence number.
    ///
    /// More specifically it verifies that the files in the remote store
    /// cover the entire range of checkpoints from sequence number 0
    /// until the latest available checkpoint with no missing checkpoint.
    pub fn verify_and_get_manifest_files(&self, manifest: Manifest) -> Result<Vec<FileMetadata>> {
        let mut files = manifest.to_files();
        if files.is_empty() {
            return Err(IngestionError::HistoryRead(
                "unexpected empty remote store of historical data".to_string(),
            ));
        }

        files.sort_by_key(|f| f.checkpoint_seq_range.start);

        assert!(
            files
                .windows(2)
                .all(|w| w[1].checkpoint_seq_range.start == w[0].checkpoint_seq_range.end)
        );

        assert_eq!(files.first().map(|f| f.checkpoint_seq_range.start), Some(0));

        Ok(files)
    }

    /// This function downloads checkpoint data files and ensures their
    /// computed checksum matches the one in manifest.
    pub async fn verify_file_consistency(&self, files: Vec<FileMetadata>) -> Result<()> {
        let remote_object_store = self.remote_object_store.clone();
        futures::stream::iter(files.iter())
            .map(|metadata| {
                let remote_object_store = remote_object_store.clone();
                async move {
                    let checkpoint_data = get(&remote_object_store, &metadata.file_path()).await?;
                    Ok::<(Bytes, &FileMetadata), IngestionError>((checkpoint_data, metadata))
                }
            })
            .boxed()
            .buffer_unordered(self.concurrency)
            .try_for_each(|(checkpoint_data, metadata)| {
                let checksum = compute_sha3_checksum_for_bytes(checkpoint_data).map_err(Into::into);
                let result = checksum.and_then(|checksum| {
                    if checksum == metadata.sha3_digest {
                        return Ok(());
                    };
                    Err(IngestionError::HistoryRead(format!(
                        "checksum doesn't match for file: {:?}",
                        metadata.file_path()
                    )))
                });
                futures::future::ready(result)
            })
            .await
    }

    /// Stream blobs of [`Bytes`] that include checkpoint data for the specified
    /// range.
    ///
    /// This method retrieves files with batches of serialized checkpoint
    /// data from the remote store, and streams the respective contents
    /// as blobs.
    ///
    /// # Errors
    ///
    /// Returns an error if resolving the files that need to be fetched from the
    /// remote store fails.
    ///
    /// Additionally the stream may fail if fetching the file from the remote
    /// store fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use futures::StreamExt;
    ///
    /// let range = 100..200;
    /// let mut stream = historical_reader.stream_blobs_for_range(range.clone()).await?;
    /// while let Some(Ok(blob)) = stream.next().await {
    ///     // we can now iterate over the checkpoint data
    ///     for data in make_blob_iterator_for_range(blob, range.clone())? {
    ///         println!("Received checkpoint data: {data:?}");
    ///     }
    /// }
    /// ```
    pub async fn stream_blobs_for_range(
        &self,
        checkpoint_range: Range<CheckpointSequenceNumber>,
    ) -> Result<impl Stream<Item = Result<Bytes>> + Send + use<'_>> {
        let files = self.get_files_for_range(checkpoint_range).await?;
        Ok(futures::stream::iter(files)
            .map(move |metadata| async move {
                let remote_object_store = Arc::clone(&self.remote_object_store);
                let file_path = metadata.file_path();
                Ok(get(&remote_object_store, &file_path).await?)
            })
            .buffered(self.concurrency))
    }

    /// Construct an [`Iterator`] over [`CheckpointData`] for the specified
    /// range.
    ///
    /// This method eagerly consumes the stream of blobs returned from
    /// [`Self::stream_blobs_for_range`] and holds the data in memory until
    /// the iterator is consumed.
    ///
    /// For lazy processing of the blobs use directly
    /// [`Self::stream_blobs_for_range`] along with
    /// [`make_blob_iterator_for_range`].
    pub async fn iter_for_range(
        &self,
        checkpoint_range: Range<CheckpointSequenceNumber>,
    ) -> Result<impl Iterator<Item = CheckpointData>> {
        let blobs = self
            .stream_blobs_for_range(checkpoint_range.clone())
            .await?
            .try_collect::<Vec<_>>()
            .await?;
        let data_iterators = blobs
            .into_iter()
            .map(|blob| {
                let range = checkpoint_range.clone();
                make_blob_iterator_for_range(blob, range)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(data_iterators.into_iter().flatten())
    }

    /// Iterate [`CheckpointData`] from the given remote file.
    ///
    /// This method retrieves the file with batches of serialized checkpoint
    /// data from the remote store, decodes the raw data, and streams the
    /// deserialized values.
    ///
    /// # Errors
    ///
    /// Returns an error in the following cases:
    ///
    /// * If fetching the file from the remote store fails.
    /// * If the file is corrupted and fails to decode.
    pub async fn iter_for_file(
        &self,
        file_path: Path,
    ) -> Result<impl Iterator<Item = CheckpointData>> {
        let raw_data_batch = get(&self.remote_object_store, &file_path).await?;
        make_blob_iterator(raw_data_batch)
    }

    /// Return latest available checkpoint in archive.
    pub async fn latest_available_checkpoint(&self) -> Result<CheckpointSequenceNumber> {
        self.manifest
            .lock()
            .await
            .next_checkpoint_seq_num()
            .checked_sub(1)
            .ok_or_else(|| {
                IngestionError::HistoryRead("no checkpoint data in the remote store".into())
            })
    }

    pub fn remote_store_identifier(&self) -> String {
        self.remote_object_store.to_string()
    }

    /// Syncs the Manifest from remote store.
    pub async fn sync_manifest_once(&self) -> Result<()> {
        Self::sync_manifest(self.remote_object_store.clone(), self.manifest.clone()).await?;
        Ok(())
    }

    pub async fn get_manifest(&self) -> Manifest {
        self.manifest.lock().await.clone()
    }

    /// Copies Manifest from remote store to the given Manifest.
    async fn sync_manifest(
        remote_store: Arc<dyn ObjectStoreGetExt>,
        manifest: Arc<Mutex<Manifest>>,
    ) -> Result<()> {
        let new_manifest = read_manifest(remote_store.clone()).await?;
        let mut locked = manifest.lock().await;
        *locked = new_manifest;
        Ok(())
    }

    /// Resolve the files to fetch for the specified range.
    ///
    /// The method retrieves the manifest from the remote store and
    /// searches for the files that cover the given range of checkpoint
    /// data.
    ///
    /// # Errors
    ///
    /// The method fails if the remote store has no data, or if the
    /// manifest fails to verify.
    async fn get_files_for_range(
        &self,
        checkpoint_range: Range<CheckpointSequenceNumber>,
    ) -> Result<impl Iterator<Item = FileMetadata>> {
        let manifest = self.get_manifest().await;

        let latest_available_checkpoint = manifest
            .next_checkpoint_seq_num()
            .checked_sub(1)
            .ok_or_else(|| {
                IngestionError::HistoryRead("no checkpoint data in the remote store".into())
            })?;

        if checkpoint_range.start > latest_available_checkpoint {
            return Err(IngestionError::HistoryRead(format!(
                "latest available checkpoint is: {latest_available_checkpoint}",
            )));
        }

        let files = self.verify_and_get_manifest_files(manifest)?;

        let start_index = match files
            .binary_search_by_key(&checkpoint_range.start, |s| s.checkpoint_seq_range.start)
        {
            Ok(index) => index,
            Err(index) => index - 1,
        };

        let end_index = match files
            .binary_search_by_key(&checkpoint_range.end, |s| s.checkpoint_seq_range.start)
        {
            Ok(index) => index,
            Err(index) => index,
        };

        Ok(files
            .into_iter()
            .enumerate()
            .filter_map(move |(index, metadata)| {
                (index >= start_index && index < end_index).then_some(metadata)
            }))
    }

    fn spawn_manifest_sync_task(
        remote_store: Arc<dyn ObjectStoreGetExt>,
        manifest: Arc<Mutex<Manifest>>,
        mut recv: oneshot::Receiver<()>,
    ) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::sync_manifest(remote_store.clone(), manifest.clone()).await?;
                    }
                    _ = &mut recv => break,
                }
            }
            info!("terminating the manifest sync loop");
            Ok::<(), IngestionError>(())
        });
    }
}

fn make_blob_iterator(blob: Bytes) -> Result<impl Iterator<Item = CheckpointData>> {
    Ok(make_iterator::<CheckpointData, Reader<Bytes>>(
        CHECKPOINT_FILE_MAGIC,
        blob.reader(),
    )?)
}

/// Construct an iterator over a blob of checkpoint data.
///
/// The iterator filters checkpoints that belong to the specified range.
///
/// # Errors
///
/// The function fails if the blob is corrupted and fails to decode.
pub fn make_blob_iterator_for_range(
    blob: Bytes,
    range: Range<CheckpointSequenceNumber>,
) -> Result<impl Iterator<Item = CheckpointData>> {
    Ok(make_blob_iterator(blob)?
        .filter(move |data| range.contains(&data.checkpoint_summary.sequence_number)))
}
