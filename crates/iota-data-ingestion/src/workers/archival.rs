// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io::Cursor, sync::Arc};

use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use bytes::Bytes;
use iota_archival::{
    CHECKPOINT_FILE_MAGIC, FileType, Manifest, SUMMARY_FILE_MAGIC, create_file_metadata_from_bytes,
    finalize_manifest, read_manifest_from_bytes,
};
use iota_data_ingestion_core::{Reducer, create_remote_store_client};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
    compress,
};
use iota_types::{
    base_types::{EpochId, ExecutionData},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CheckpointSequenceNumber, FullCheckpointContents},
};
use object_store::{ObjectStore, path::Path};
use serde::{Deserialize, Serialize};

use crate::workers::RelayWorker;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct ArchivalConfig {
    pub remote_url: String,
    pub remote_store_options: Vec<(String, String)>,
    pub commit_file_size: usize,
    pub commit_duration_seconds: u64,
}

pub struct ArchivalReducer {
    remote_store: Box<dyn ObjectStore>,
    commit_duration_ms: u64,
}

impl ArchivalReducer {
    pub async fn new(config: ArchivalConfig) -> anyhow::Result<Self> {
        let remote_store =
            create_remote_store_client(config.remote_url, config.remote_store_options, 10)?;

        Ok(Self {
            remote_store,
            commit_duration_ms: config.commit_duration_seconds * 1000,
        })
    }

    async fn upload(
        &self,
        epoch: EpochId,
        start: CheckpointSequenceNumber,
        end: CheckpointSequenceNumber,
        summary_buffer: Vec<u8>,
        buffer: Vec<u8>,
    ) -> anyhow::Result<()> {
        let checkpoint_file_path = format!("epoch_{}/{}.chk", epoch, start);
        let chk_bytes = self
            .upload_file(
                Path::from(checkpoint_file_path.clone()),
                CHECKPOINT_FILE_MAGIC,
                &buffer,
            )
            .await?;
        let summary_file_path = format!("epoch_{}/{}.sum", epoch, start);
        let sum_bytes = self
            .upload_file(
                Path::from(summary_file_path.clone()),
                SUMMARY_FILE_MAGIC,
                &summary_buffer,
            )
            .await?;
        let mut manifest = Self::read_manifest(&self.remote_store).await?;
        let checkpoint_file_metadata = create_file_metadata_from_bytes(
            chk_bytes,
            FileType::CheckpointContent,
            epoch,
            start..end,
        )?;
        let summary_file_metadata = create_file_metadata_from_bytes(
            sum_bytes,
            FileType::CheckpointSummary,
            epoch,
            start..end,
        )?;
        manifest.update(epoch, end, checkpoint_file_metadata, summary_file_metadata);

        let bytes = finalize_manifest(manifest)?;
        self.remote_store
            .put(&Path::from("MANIFEST"), bytes.into())
            .await?;
        Ok(())
    }

    async fn upload_file(
        &self,
        location: Path,
        magic: u32,
        content: &[u8],
    ) -> anyhow::Result<Bytes> {
        let mut buffer = vec![0; 4];
        BigEndian::write_u32(&mut buffer, magic);
        buffer.push(StorageFormat::Blob.into());
        buffer.push(FileCompression::Zstd.into());
        buffer.extend_from_slice(content);
        let mut compressed_buffer = vec![];
        let mut cursor = Cursor::new(buffer);
        compress(&mut cursor, &mut compressed_buffer)?;
        self.remote_store
            .put(&location, Bytes::from(compressed_buffer.clone()).into())
            .await?;
        Ok(Bytes::from(compressed_buffer))
    }

    pub async fn get_watermark(&self) -> anyhow::Result<CheckpointSequenceNumber> {
        let manifest = Self::read_manifest(&self.remote_store).await?;
        Ok(manifest.next_checkpoint_seq_num())
    }

    async fn read_manifest(remote_store: &dyn ObjectStore) -> anyhow::Result<Manifest> {
        Ok(match remote_store.get(&Path::from("MANIFEST")).await {
            Ok(resp) => read_manifest_from_bytes(resp.bytes().await?.to_vec())?,
            Err(err) if err.to_string().contains("404") => Manifest::new(0, 0),
            Err(err) => Err(err)?,
        })
    }
}

#[async_trait]
impl Reducer<RelayWorker> for ArchivalReducer {
    async fn commit(&self, batch: &[Arc<CheckpointData>]) -> Result<(), anyhow::Error> {
        if batch.is_empty() {
            return Err(anyhow::anyhow!("commit batch can't be empty"));
        }
        let mut summary_buffer = vec![];
        let mut buffer = vec![];
        let first_checkpoint = &batch[0];
        let epoch = first_checkpoint.checkpoint_summary.epoch;
        let start_checkpoint = first_checkpoint.checkpoint_summary.sequence_number;
        let mut last_checkpoint = start_checkpoint;
        for checkpoint in batch {
            let full_checkpoint_contents = FullCheckpointContents::from_contents_and_execution_data(
                checkpoint.checkpoint_contents.clone(),
                checkpoint
                    .transactions
                    .iter()
                    .map(|t| ExecutionData::new(t.transaction.clone(), t.effects.clone())),
            );
            let contents_blob = Blob::encode(&full_checkpoint_contents, BlobEncoding::Bcs).unwrap();
            let summary_blob =
                Blob::encode(&checkpoint.checkpoint_summary, BlobEncoding::Bcs).unwrap();
            contents_blob.write(&mut buffer).unwrap();
            summary_blob.write(&mut summary_buffer).unwrap();
            last_checkpoint += 1;
        }
        self.upload(
            epoch,
            start_checkpoint,
            last_checkpoint,
            summary_buffer,
            buffer,
        )
        .await
        .unwrap();
        Ok(())
    }

    fn should_close_batch(
        &self,
        batch: &[Arc<CheckpointData>],
        next_item: Option<&Arc<CheckpointData>>,
    ) -> bool {
        // never close a batch without a trigger condition
        if batch.is_empty() || next_item.is_none() {
            return false;
        }
        let first_checkpoint = &batch[0].checkpoint_summary;
        let next_checkpoint = next_item.expect("invariant's checked");
        next_checkpoint.checkpoint_summary.epoch != first_checkpoint.epoch
            || next_checkpoint.checkpoint_summary.timestamp_ms
                > (self.commit_duration_ms + first_checkpoint.timestamp_ms)
    }
}
