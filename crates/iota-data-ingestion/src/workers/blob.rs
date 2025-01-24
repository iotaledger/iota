// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use iota_data_ingestion_core::{Worker, create_remote_store_client_with_ops};
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::{MultipartUpload, ObjectStore, RetryConfig, path::Path};
use serde::{Deserialize, Serialize};

const CHUNK_SIZE: usize = 50 * 1024 * 1024; // 50 MB
const PARALLEL_CHUNKS_UPLOAD: usize = 10;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BlobTaskConfig {
    pub url: String,
    pub remote_store_options: Vec<(String, String)>,
    pub timeout_secs: u64,
}

pub struct BlobWorker {
    remote_store: Box<dyn ObjectStore>,
}

impl BlobWorker {
    pub fn new(config: BlobTaskConfig) -> Self {
        Self {
            remote_store: create_remote_store_client_with_ops(
                config.url,
                config.remote_store_options,
                config.timeout_secs,
                RetryConfig {
                    max_retries: 10,
                    retry_timeout: Duration::from_secs(config.timeout_secs + 1),
                    ..Default::default()
                },
            )
            .expect("failed to create remote store client"),
        }
    }

    /// Uploads a Checkpoint blob to the Remote Store.
    ///
    /// If the blob size exceeds the configured `CHUNK_SIZE`,
    /// it uploads the blob in parts using multipart upload.
    /// Otherwise, it uploads the blob directly.
    async fn upload_blob(&self, bytes: Vec<u8>, chk_seq_num: u64, location: Path) -> Result<()> {
        if bytes.len() > CHUNK_SIZE {
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

        let chunks = bytes.chunks(CHUNK_SIZE);
        let total_chunks = chunks.len();

        let mut parts_futures = vec![];
        for (chunk_id, chunk) in chunks.enumerate() {
            tracing::info!(
                "Preparing checkpoint {chk_seq_num} chunk {}/{total_chunks}",
                chunk_id + 1
            );

            parts_futures.push(multipart.put_part(Bytes::copy_from_slice(chunk).into()));
        }

        // send chunks in parallel to the remote store
        for (chunk_id, chunk) in parts_futures.chunks_mut(PARALLEL_CHUNKS_UPLOAD).enumerate() {
            tracing::info!(
                "Sending checkpoint {chk_seq_num} chunks {}-{} of {total_chunks}",
                chunk_id * PARALLEL_CHUNKS_UPLOAD + 1,
                (chunk_id + 1) * PARALLEL_CHUNKS_UPLOAD.min(total_chunks)
            );
            let start_time = std::time::Instant::now();
            futures::future::try_join_all(chunk).await?;
            tracing::info!(
                "multipart checkpoint {chk_seq_num} sent in {:?}",
                start_time.elapsed()
            );
        }

        let start_time = std::time::Instant::now();
        multipart.complete().await?;
        tracing::info!(
            "multipart checkpoint {chk_seq_num} uploaded in {:?}",
            start_time.elapsed()
        );

        Ok(())
    }
}

#[async_trait]
impl Worker for BlobWorker {
    async fn process_checkpoint(&self, checkpoint: CheckpointData) -> Result<()> {
        let bytes = Blob::encode(&checkpoint, BlobEncoding::Bcs)?.to_bytes();
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
