// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io::{Cursor, Seek, SeekFrom}, ops::Range, sync::Arc};

use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use bytes::{Bytes, Buf, buf::Reader};
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::{
    Reducer,
    history::{
        CHECKPOINT_FILE_MAGIC, MAGIC_BYTES, EPOCH_BOUNDARIES_FILENAME,
        manifest::{
            Manifest, create_file_metadata_from_bytes, finalize_manifest, read_manifest_from_bytes,
            EpochBoundaries, read_epoch_boundaries_from_bytes, finalize_epoch_boundaries,
        },
    },
};
use iota_storage::{
    FileCompression, StorageFormat,
    blob::{Blob, BlobEncoding},
    compress, make_iterator,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{DynObjectStore, Error as ObjectStoreError, ObjectStore, ObjectStoreExt, path::Path};
use serde::{Deserialize, Serialize};

use crate::RelayWorker;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HistoricalWriterConfig {
    pub object_store_config: ObjectStoreConfig,
    pub commit_duration_seconds: u64,
    #[serde(default)]
    pub seed_epoch_boundaries: bool,
}

pub struct HistoricalReducer {
    remote_store: Arc<DynObjectStore>,
    commit_duration_ms: u64,
}

impl HistoricalReducer {
    pub async fn new(config: HistoricalWriterConfig) -> anyhow::Result<Self> {
        let remote_store = config.object_store_config.make()?;

        let reducer = Self {
            remote_store,
            commit_duration_ms: config.commit_duration_seconds * 1000,
        };

        if config.seed_epoch_boundaries {
            reducer.seed_epoch_boundaries().await?;
        }

        Ok(reducer)
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

    async fn read_epoch_boundaries(remote_store: &dyn ObjectStore) -> anyhow::Result<EpochBoundaries> {
        let path = Path::from(EPOCH_BOUNDARIES_FILENAME);
        Ok(match remote_store.get(&path).await {
            Ok(resp) => read_epoch_boundaries_from_bytes(resp.bytes().await?.to_vec())?,
            Err(ObjectStoreError::NotFound { .. }) => EpochBoundaries::default(),
            Err(err) => Err(err)?,
        })
    }

    async fn update_epoch_boundaries(
        &self,
        new_last_checkpoint_seq_nums: Vec<u64>,
    ) -> anyhow::Result<()> {
        if new_last_checkpoint_seq_nums.is_empty() {
            return Ok(());
        }
        let mut boundaries = Self::read_epoch_boundaries(self.remote_store.as_ref()).await?;
        boundaries.last_checkpoint_seq_nums.extend(new_last_checkpoint_seq_nums);
        let bytes = finalize_epoch_boundaries(boundaries)?;
        let path = Path::from(EPOCH_BOUNDARIES_FILENAME);
        self.remote_store.put(&path, bytes).await?;
        Ok(())
    }

    async fn seed_epoch_boundaries(&self) -> anyhow::Result<()> {
        let path = Path::from(EPOCH_BOUNDARIES_FILENAME);
        match self.remote_store.head(&path).await {
            Ok(_) => {
                return Ok(());
            }
            Err(ObjectStoreError::NotFound { .. }) => {}
            Err(err) => return Err(err.into()),
        }

        let manifest = Self::read_manifest(&self.remote_store).await?;
        let files = manifest.to_files();
        let mut last_checkpoint_seq_nums = vec![];
        for file in files {
            let file_path = file.file_path();
            let raw_data_batch = self.remote_store.get(&file_path).await?.bytes().await?;
            let mut reader = make_iterator::<CheckpointData, Reader<Bytes>>(
                CHECKPOINT_FILE_MAGIC,
                raw_data_batch.reader(),
            )?;
            for checkpoint in reader {
                if checkpoint.checkpoint_summary.end_of_epoch_data.is_some() {
                    last_checkpoint_seq_nums.push(checkpoint.checkpoint_summary.sequence_number);
                }
            }
        }

        let boundaries = EpochBoundaries { last_checkpoint_seq_nums };
        let bytes = finalize_epoch_boundaries(boundaries)?;
        self.remote_store.put(&path, bytes).await?;
        Ok(())
    }
}

#[async_trait]
impl Reducer<RelayWorker> for HistoricalReducer {
    async fn commit(&self, batch: &[Arc<CheckpointData>]) -> Result<(), anyhow::Error> {
        if batch.is_empty() {
            anyhow::bail!("commit batch can't be empty");
        }
        let mut buffer = vec![];
        let first_checkpoint = &batch[0];
        let start_checkpoint = first_checkpoint.checkpoint_summary.sequence_number;
        let uploaded_range = start_checkpoint..(start_checkpoint + batch.len() as u64);
        
        let mut end_of_epoch_checkpoints = vec![];
        for checkpoint in batch {
            let data = Blob::encode(&checkpoint, BlobEncoding::Bcs)?;
            data.write(&mut buffer)?;
            if checkpoint.checkpoint_summary.end_of_epoch_data.is_some() {
                end_of_epoch_checkpoints.push(checkpoint.checkpoint_summary.sequence_number);
            }
        }
        self.upload(uploaded_range, self.prepare_data_to_upload(buffer)?)
            .await?;

        if !end_of_epoch_checkpoints.is_empty() {
            self.update_epoch_boundaries(end_of_epoch_checkpoints).await?;
        }
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
        // close batch after genesis
        if next_checkpoint.checkpoint_summary.sequence_number == 1 {
            return true;
        }
        next_checkpoint.checkpoint_summary.epoch != first_checkpoint.epoch
            || next_checkpoint.checkpoint_summary.timestamp_ms
                > (self.commit_duration_ms + first_checkpoint.timestamp_ms)
    }
}
