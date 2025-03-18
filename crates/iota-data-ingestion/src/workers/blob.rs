// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::RangeInclusive, path::PathBuf, sync::Arc};

use anyhow::{Result, bail};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt, stream};
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::{FileProgressStore, ProgressStore, Worker};
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    committee::EpochId, full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, MultipartUpload, ObjectStore, path::Path};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::Mutex;

use crate::NetworkTipState;

/// Minimum allowed chunk size to be uploaded to remote store
const MIN_CHUNK_SIZE_MB: u64 = 5 * 1024 * 1024; // 5 MB
/// The maximum number of concurrent requests allowed when uploading checkpoint
/// chunk parts to remote store
const MAX_CONCURRENT_PARTS_UPLOAD: usize = 50;

const CHECKPOINT_FILE_SUFFIX: &str = "chk";
const LIVE_DIR_NAME: &str = "live";
const INGESTION_DIR_NAME: &str = "ingestion";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct BlobTaskConfig {
    pub object_store_config: ObjectStoreConfig,
    #[serde(deserialize_with = "deserialize_chunk")]
    pub checkpoint_chunk_size_mb: u64,
    pub node_rest_api: String,
    pub remote_store_progress_path: String,
}

fn deserialize_chunk<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let checkpoint_chunk_size = u64::deserialize(deserializer)? * 1024 * 1024;
    if checkpoint_chunk_size < MIN_CHUNK_SIZE_MB {
        return Err(serde::de::Error::custom("Chunk size must be at least 5 MB"));
    }
    Ok(checkpoint_chunk_size)
}

struct WorkerState {
    current_epoch: EpochId,
    remote_store_progress: OldestCheckpointStore,
}

pub struct BlobWorker {
    remote_store: Arc<DynObjectStore>,
    checkpoint_chunk_size_mb: u64,
    state: Arc<Mutex<WorkerState>>,
}

impl BlobWorker {
    pub async fn new(
        config: BlobTaskConfig,
        network_tip_state: NetworkTipState,
        latest_watermark: Option<CheckpointSequenceNumber>,
    ) -> anyhow::Result<Self> {
        let remote_store = config.object_store_config.make()?;
        let mut remote_store_progress =
            OldestCheckpointStore::new(config.remote_store_progress_path).await?;

        let state = match network_tip_state {
            NetworkTipState::CurrentEpoch { epoch } => WorkerState {
                current_epoch: epoch,
                remote_store_progress,
            },
            NetworkTipState::EpochChanged {
                epoch,
                first_chk_seq_num_of_epoch,
            } => {
                let last_deleted_checkpoint = first_chk_seq_num_of_epoch.saturating_sub(1);

                if let Some(watermark) = latest_watermark {
                    let old = remote_store_progress.get().await?;
                    Self::reset_remote_store(&remote_store, old..=watermark.saturating_sub(1))
                        .await?;
                    remote_store_progress.save(last_deleted_checkpoint).await?;
                }

                WorkerState {
                    current_epoch: epoch,
                    remote_store_progress,
                }
            }
        };

        Ok(Self {
            checkpoint_chunk_size_mb: config.checkpoint_chunk_size_mb,
            remote_store: config.object_store_config.make()?,
            state: Arc::new(Mutex::new(state)),
        })
    }

    /// Resets the remote object store by deleting checkpoints within the
    /// specified range.
    async fn reset_remote_store(
        remote_store: &dyn ObjectStore,
        range: RangeInclusive<CheckpointSequenceNumber>,
    ) -> anyhow::Result<()> {
        tracing::info!("delete checkpoints from remote store: {range:?}");

        let paths = range
            .into_iter()
            .map(|chk_seq_num| Ok(Self::file_path(chk_seq_num)))
            .collect::<Vec<_>>();

        let paths_stream = futures::stream::iter(paths).boxed();

        _ = remote_store
            .delete_stream(paths_stream)
            .try_collect::<Vec<Path>>()
            .await?;

        Ok(())
    }

    /// Uploads a Checkpoint blob to the Remote Store.
    ///
    /// If the blob size exceeds the configured `CHUNK_SIZE`,
    /// it uploads the blob in parts using multipart upload.
    /// Otherwise, it uploads the blob directly.
    async fn upload_blob(&self, bytes: Vec<u8>, chk_seq_num: u64, location: Path) -> Result<()> {
        if bytes.len() > self.checkpoint_chunk_size_mb as usize {
            return self
                .upload_blob_multipart(bytes, chk_seq_num, location)
                .await;
        }

        self.remote_store
            .put(&location, Bytes::from(bytes).into())
            .await?;

        Ok(())
    }

    /// Uploads a large Checkpoint blob to the Remote Store using multipart
    /// upload.
    ///
    /// This function divides the input `bytes` into chunks of size `CHUNK_SIZE`
    /// and uploads each chunk individually.
    /// Finally, it completes the multipart upload by assembling all the
    /// uploaded parts.
    async fn upload_blob_multipart(
        &self,
        bytes: Vec<u8>,
        chk_seq_num: u64,
        location: Path,
    ) -> Result<()> {
        let mut multipart = self.remote_store.put_multipart(&location).await?;
        let chunks = bytes.chunks(self.checkpoint_chunk_size_mb as usize);
        let total_chunks = chunks.len();

        let parts_futures = chunks
            .into_iter()
            .map(|chunk| multipart.put_part(Bytes::copy_from_slice(chunk).into()))
            .collect::<Vec<_>>();

        let mut buffered_uploaded_parts = stream::iter(parts_futures)
            .buffer_unordered(MAX_CONCURRENT_PARTS_UPLOAD)
            .enumerate();

        while let Some((uploaded_chunk_id, part_result)) = buffered_uploaded_parts.next().await {
            match part_result {
                Ok(()) => {
                    tracing::info!(
                        "uploaded checkpoint {chk_seq_num} chunk {}/{total_chunks}",
                        uploaded_chunk_id + 1
                    );
                }
                Err(err) => {
                    tracing::error!("error uploading part: {err}");
                    multipart.abort().await?;
                    bail!("checkpoint {chk_seq_num} multipart upload aborted");
                }
            }
        }

        let start_time = std::time::Instant::now();
        multipart.complete().await?;
        tracing::info!(
            "checkpoint {chk_seq_num} multipart completion request finished in {:?}",
            start_time.elapsed()
        );

        Ok(())
    }

    /// Checks and handles an epoch transition.
    ///
    /// It checks if a new epoch has started. If a new epoch is detected, it
    /// updates the internal state, resets the remote store, and updates the
    /// remote store progress.
    async fn check_and_handle_epoch_transition(
        &self,
        new_epoch: u64,
        chk_seq_num: u64,
    ) -> Result<()> {
        let mut guard = self.state.lock().await;
        if new_epoch <= guard.current_epoch {
            return Ok(());
        }
        let old_epoch = guard.current_epoch;
        guard.current_epoch = new_epoch;

        let last_epoch_checkpoint = chk_seq_num.saturating_sub(1);

        tracing::info!(
            "transitioning from epoch {old_epoch} to epoch {new_epoch}, last checkpoint of old epoch: {last_epoch_checkpoint}",
        );

        let start_checkpoint = guard.remote_store_progress.get().await?;
        Self::reset_remote_store(&self.remote_store, start_checkpoint..=last_epoch_checkpoint)
            .await?;

        guard.remote_store_progress.save(chk_seq_num).await?;

        Ok(())
    }

    /// Constructs a file path for a checkpoint file based on the checkpoint
    /// sequence number.
    fn file_path(chk_seq_num: CheckpointSequenceNumber) -> Path {
        Path::from(INGESTION_DIR_NAME)
            .child(LIVE_DIR_NAME)
            .child(format!("{chk_seq_num}.{CHECKPOINT_FILE_SUFFIX}"))
    }
}

#[async_trait]
impl Worker for BlobWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        let chk_seq_num = checkpoint.checkpoint_summary.sequence_number;
        let epoch = checkpoint.checkpoint_summary.epoch;

        self.check_and_handle_epoch_transition(epoch, chk_seq_num)
            .await?;

        let bytes = Blob::encode(&checkpoint, BlobEncoding::Bcs)?.to_bytes();
        let location = Self::file_path(chk_seq_num);

        self.upload_blob(
            bytes,
            checkpoint.checkpoint_summary.sequence_number,
            location,
        )
        .await?;

        Ok(())
    }
}

/// Manages persistent storage of the oldest checkpoint sequence number in a
/// JSON file.
pub struct OldestCheckpointStore(FileProgressStore);

impl OldestCheckpointStore {
    /// Creates a new `OldestCheckpointStore` by opening or creating the file at
    /// the specified path.
    pub async fn new(path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        FileProgressStore::new(path.into())
            .await
            .map(Self)
            .map_err(Into::into)
    }

    /// Retrieves the oldest checkpoint sequence number from the file.
    pub async fn get(&mut self) -> Result<CheckpointSequenceNumber> {
        let oldest = self.0.load("oldest_checkpoint_seq_num".into()).await?;
        Ok(CheckpointSequenceNumber::from(oldest))
    }

    /// Saves the provided checkpoint sequence number as the oldest.
    pub async fn save(&mut self, oldest: CheckpointSequenceNumber) -> anyhow::Result<()> {
        self.0
            .save("oldest_checkpoint_seq_num".into(), oldest)
            .await
            .map_err(Into::into)
    }
}
