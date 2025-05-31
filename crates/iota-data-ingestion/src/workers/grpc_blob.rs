// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use iota_data_ingestion_core::Worker;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, path::Path};
use tokio::sync::Mutex;

const CHECKPOINT_FILE_SUFFIX: &str = "chk";
const LIVE_DIR_NAME: &str = "live";
const INGESTION_DIR_NAME: &str = "ingestion";

pub struct GrpcBlobWorker {
    remote_store: Arc<DynObjectStore>,
    grpc_url: String,
    checkpoint_chunk_size_mb: u64,
    current_epoch: Arc<Mutex<u64>>,
}

impl GrpcBlobWorker {
    pub fn new(
        remote_store: Arc<DynObjectStore>,
        grpc_url: String,
        checkpoint_chunk_size_mb: u64,
        current_epoch: u64,
    ) -> Self {
        Self {
            remote_store,
            grpc_url,
            checkpoint_chunk_size_mb,
            current_epoch: Arc::new(Mutex::new(current_epoch)),
        }
    }

    fn file_path(chk_seq_num: CheckpointSequenceNumber) -> Path {
        Path::from(INGESTION_DIR_NAME)
            .child(LIVE_DIR_NAME)
            .child(format!("{chk_seq_num}.{CHECKPOINT_FILE_SUFFIX}"))
    }

    async fn upload_blob(&self, bytes: Vec<u8>, chk_seq_num: u64, location: Path) -> Result<()> {
        self.remote_store
            .put(&location, Bytes::from(bytes).into())
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Worker for GrpcBlobWorker {
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
                // For simplicity, skip remote store reset logic here. Add if needed.
                *current_epoch = epoch;
            }
        }
        let bytes = Blob::encode(&checkpoint, BlobEncoding::Bcs)?.to_bytes();
        self.upload_blob(bytes, chk_seq_num, Self::file_path(chk_seq_num))
            .await?;
        Ok(())
    }
}
