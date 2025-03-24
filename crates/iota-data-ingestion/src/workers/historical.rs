// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io::Cursor, ops::Range, sync::Arc};

use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use bytes::Bytes;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::{
    Reducer,
    history::{
        CHECKPOINT_FILE_MAGIC, MAGIC_BYTES,
        manifest::{
            Manifest, create_file_metadata_from_bytes, finalize_manifest, read_manifest_from_bytes,
        },
    },
};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
    compress,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, Error as ObjectStoreError, ObjectStore};
use serde::{Deserialize, Serialize};

use crate::RelayWorker;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HistoricalWriterConfig {
    pub object_store_config: ObjectStoreConfig,
    pub commit_duration_seconds: u64,
}

pub struct HistoricalReducer {
    remote_store: Arc<DynObjectStore>,
    commit_duration_ms: u64,
}

impl HistoricalReducer {
    pub async fn new(config: HistoricalWriterConfig) -> anyhow::Result<Self> {
        let remote_store = config.object_store_config.make()?;

        Ok(Self {
            remote_store,
            commit_duration_ms: config.commit_duration_seconds * 1000,
        })
    }

    async fn upload(
        &self,
        checkpoint_range: Range<CheckpointSequenceNumber>,
        data: Bytes,
    ) -> anyhow::Result<()> {
        let file_metadata =
            create_file_metadata_from_bytes(data.clone(), checkpoint_range.clone())?;
        self.remote_store
            .put(&file_metadata.file_path(), data.into())
            .await?;
        let mut manifest = Self::read_manifest(&self.remote_store).await?;
        manifest.update(checkpoint_range.end, file_metadata);

        let bytes = finalize_manifest(manifest)?;
        self.remote_store
            .put(&Manifest::file_path(), bytes.into())
            .await?;
        Ok(())
    }

    fn prepare_data_to_upload(&self, mut checkpoint_data: Vec<u8>) -> anyhow::Result<Bytes> {
        let mut buffer = vec![0; MAGIC_BYTES];
        BigEndian::write_u32(&mut buffer, CHECKPOINT_FILE_MAGIC);
        buffer.push(StorageFormat::Blob.into());
        buffer.push(FileCompression::Zstd.into());
        buffer.append(&mut checkpoint_data);
        let mut compressed_buffer = vec![];
        let mut cursor = Cursor::new(buffer);
        compress(&mut cursor, &mut compressed_buffer)?;
        Ok(Bytes::from(compressed_buffer))
    }

    pub async fn get_watermark(&self) -> anyhow::Result<CheckpointSequenceNumber> {
        let manifest = Self::read_manifest(&self.remote_store).await?;
        Ok(manifest.next_checkpoint_seq_num())
    }

    async fn read_manifest(remote_store: &dyn ObjectStore) -> anyhow::Result<Manifest> {
        Ok(match remote_store.get(&Manifest::file_path()).await {
            Ok(resp) => read_manifest_from_bytes(resp.bytes().await?.to_vec())?,
            Err(ObjectStoreError::NotFound { .. }) => Manifest::new(0),
            Err(err) => Err(err)?,
        })
    }
}

#[async_trait]
impl Reducer<RelayWorker> for HistoricalReducer {
    async fn commit(&self, batch: Vec<Arc<CheckpointData>>) -> Result<(), anyhow::Error> {
        if batch.is_empty() {
            anyhow::bail!("commit batch can't be empty");
        }
        let mut buffer = vec![];
        let first_checkpoint = &batch[0];
        let start_checkpoint = first_checkpoint.checkpoint_summary.sequence_number;
        let uploaded_range = start_checkpoint..(start_checkpoint + batch.len() as u64);
        for checkpoint in batch {
            let data = Blob::encode(&checkpoint, BlobEncoding::Bcs)?;
            data.write(&mut buffer)?;
        }
        self.upload(uploaded_range, self.prepare_data_to_upload(buffer)?)
            .await
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
        // close batch after genesis
        if next_checkpoint.checkpoint_summary.sequence_number == 1 {
            return true;
        }
        next_checkpoint.checkpoint_summary.epoch != first_checkpoint.epoch
            || next_checkpoint.checkpoint_summary.timestamp_ms
                > (self.commit_duration_ms + first_checkpoint.timestamp_ms)
    }
}
