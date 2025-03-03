// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::{Result, bail};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{StreamExt, stream};
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::Worker;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::{DynObjectStore, MultipartUpload, ObjectStore, path::Path};
use serde::{Deserialize, Deserializer, Serialize};

/// Minimum allowed chunk size to be uploaded to remote store
const MIN_CHUNK_SIZE_MB: u64 = 5 * 1024 * 1024; // 5 MB
/// The maximum number of concurrent requests allowed when uploading checkpoint
/// chunk parts to remote store
const MAX_CONCURRENT_PARTS_UPLOAD: usize = 50;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct BlobTaskConfig {
    pub object_store_config: ObjectStoreConfig,
    #[serde(deserialize_with = "deserialize_chunk")]
    pub checkpoint_chunk_size_mb: u64,
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
    checkpoint_chunk_size_mb: u64,
}

impl BlobWorker {
    pub fn new(config: BlobTaskConfig) -> anyhow::Result<Self> {
        let remote_store = config.object_store_config.make()?;
        Ok(Self {
            checkpoint_chunk_size_mb: config.checkpoint_chunk_size_mb,
            remote_store,
        })
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
}

#[async_trait]
impl Worker for BlobWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<Self::Message, Self::Error> {
        let bytes = Blob::encode(checkpoint, BlobEncoding::Bcs)?.to_bytes();
        let location = Path::from(format!(
            "{}.chk",
            checkpoint.checkpoint_summary.sequence_number
        ));

        self.upload_blob(
            bytes,
            checkpoint.checkpoint_summary.sequence_number,
            location,
        )
        .await?;

        Ok(())
    }
}
