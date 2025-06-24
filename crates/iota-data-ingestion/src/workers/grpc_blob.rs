// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use iota_data_ingestion_core::Worker;
use iota_grpc_api::client::GrpcNodeClient;
use iota_storage::blob::{Blob, BlobEncoding};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, path::Path};
use tokio::sync::Mutex;
use tracing;

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

    pub fn file_path(chk_seq_num: CheckpointSequenceNumber) -> Path {
        Path::from(INGESTION_DIR_NAME)
            .child(LIVE_DIR_NAME)
            .child(format!("{chk_seq_num}.{CHECKPOINT_FILE_SUFFIX}"))
    }

    async fn upload_blob(&self, bytes: Vec<u8>, _chk_seq_num: u64, location: Path) -> Result<()> {
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
                // Epoch transition detected. Fetch first checkpoint of previous epoch and reset
                // remote store.
                tracing::info!(
                    "Epoch transition: {} -> {}. Performing remote store reset.",
                    *current_epoch,
                    epoch
                );
                let mut grpc_client = GrpcNodeClient::connect(&self.grpc_url).await?;
                let delete_start = grpc_client
                    .get_epoch_first_checkpoint_sequence_number(*current_epoch)
                    .await?;
                if delete_start < chk_seq_num {
                    // Only reset if there is something to delete
                    let paths = (delete_start..chk_seq_num)
                        .map(|chk_seq_num| Ok(Self::file_path(chk_seq_num)))
                        .collect::<Vec<_>>();
                    let paths_stream = futures::stream::iter(paths).boxed();
                    self.remote_store
                        .delete_stream(paths_stream)
                        .for_each_concurrent(10, |delete_result| async {
                            if let Err(err) = delete_result {
                                tracing::warn!("deletion failed with: {err}");
                            }
                        })
                        .await;
                }
                // Update epoch after reset
                *current_epoch = epoch;
            }
        }
        let bytes = Blob::encode(&checkpoint, BlobEncoding::Bcs)?.to_bytes();
        self.upload_blob(bytes, chk_seq_num, Self::file_path(chk_seq_num))
            .await?;
        Ok(())
    }
}
