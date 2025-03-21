// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::Range, sync::Arc};

use anyhow::{Result, bail};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{StreamExt, stream};
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::Worker;
use iota_rest_api::Client;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    committee::EpochId, full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, MultipartUpload, ObjectStore, path::Path};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::Mutex;

use crate::common;

/// Minimum allowed chunk size to be uploaded to remote store
const MIN_CHUNK_SIZE_MB: u64 = 5 * 1024 * 1024; // 5 MB
/// The maximum number of concurrent requests allowed when uploading checkpoint
/// chunk parts to remote store
const MAX_CONCURRENT_PARTS_UPLOAD: usize = 50;
const MAX_CONCURRENT_DELETE_REQUESTS: usize = 10;

const CHECKPOINT_FILE_SUFFIX: &str = "chk";
const LIVE_DIR_NAME: &str = "live";
const INGESTION_DIR_NAME: &str = "ingestion";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct BlobTaskConfig {
    pub object_store_config: ObjectStoreConfig,
    #[serde(deserialize_with = "deserialize_chunk")]
    pub checkpoint_chunk_size_mb: u64,
    pub node_rest_api_url: String,
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

pub struct BlobWorker {
    remote_store: Arc<DynObjectStore>,
    rest_client: Client,
    checkpoint_chunk_size_mb: u64,
    current_epoch: Arc<Mutex<EpochId>>,
}

impl BlobWorker {
    pub fn new(
        config: BlobTaskConfig,
        rest_client: Client,
        current_epoch: EpochId,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            checkpoint_chunk_size_mb: config.checkpoint_chunk_size_mb,
            remote_store: config.object_store_config.make()?,
            current_epoch: Arc::new(Mutex::new(current_epoch)),
            rest_client,
        })
    }

    /// Resets the remote object store by deleting checkpoints within the
    /// specified range.
    pub async fn reset_remote_store(
        &self,
        range: Range<CheckpointSequenceNumber>,
    ) -> anyhow::Result<()> {
        tracing::info!("delete checkpoints from remote store: {range:?}");

        let paths = range
            .into_iter()
            .map(|chk_seq_num| Ok(Self::file_path(chk_seq_num)))
            .collect::<Vec<_>>();

        let paths_stream = futures::stream::iter(paths).boxed();

        _ = self
            .remote_store
            .delete_stream(paths_stream)
            .for_each_concurrent(MAX_CONCURRENT_DELETE_REQUESTS, |delete_result| async {
                _ = delete_result.inspect_err(|err| tracing::warn!("deletion failed with: {err}"));
            })
            .await;

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

        {
            let mut current_epoch = self.current_epoch.lock().await;
            if epoch > *current_epoch {
                let delete_start = common::epoch_first_checkpoint_sequence_number(
                    &self.rest_client,
                    *current_epoch,
                )
                .await?;
                self.reset_remote_store(delete_start..chk_seq_num).await?;
                // we update the epoch once we made sure that reset was successful.
                *current_epoch = epoch;
            }
        }

        let bytes = Blob::encode(&checkpoint, BlobEncoding::Bcs)?.to_bytes();
        self.upload_blob(bytes, chk_seq_num, Self::file_path(chk_seq_num))
            .await?;

        Ok(())
    }
}
