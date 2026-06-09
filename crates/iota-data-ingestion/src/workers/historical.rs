// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io::Cursor, ops::Range, sync::Arc, time::Duration};

use async_trait::async_trait;
use backoff::{SystemClock, exponential::ExponentialBackoffBuilder, future::retry};
use byteorder::{BigEndian, ByteOrder};
use bytes::Bytes;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_data_ingestion_core::{
    IngestionError, Reducer,
    history::{
        CHECKPOINT_FILE_MAGIC, MAGIC_BYTES,
        epoch_boundaries::{EpochBoundaries, read_epoch_boundaries, write_epoch_boundaries},
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
    committee::EpochId, full_checkpoint_content::CheckpointData,
    messages_checkpoint::CheckpointSequenceNumber,
};
use object_store::{
    DynObjectStore, Error as ObjectStoreError, ObjectStore, ObjectStoreExt, PutMode,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::RelayWorker;

const RECORD_EPOCH_BOUNDARY_TIMEOUT_SECS: u64 = 5;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct HistoricalWriterConfig {
    pub object_store_config: ObjectStoreConfig,
    pub commit_duration_seconds: u64,
    /// Optional seed for the epoch boundaries. When set, it initializes the
    /// epoch boundaries if not already initialized.
    #[serde(default)]
    pub epoch_boundaries: Option<EpochBoundaries>,
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

        if let Some(epoch_boundaries) = config.epoch_boundaries {
            reducer.seed_epoch_boundaries(epoch_boundaries).await?;
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

    /// Initializes the epoch boundaries from the provided seed if not already
    /// initialized.
    async fn seed_epoch_boundaries(&self, epoch_boundaries: EpochBoundaries) -> anyhow::Result<()> {
        match write_epoch_boundaries(
            &epoch_boundaries,
            self.remote_store.clone(),
            PutMode::Create,
        )
        .await
        {
            Ok(_) => {
                tracing::info!("Initialized epoch boundaries");
                Ok(())
            }
            Err(IngestionError::ObjectStore(ObjectStoreError::AlreadyExists { .. })) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Adds a new entry in the epoch boundaries maintained in the remote store.
    ///
    /// # Errors
    ///
    /// Fails if there is a gap between the epochs already recorded and the
    /// current one.
    async fn record_epoch_boundary(
        &self,
        epoch_id: EpochId,
        checkpoint_sequence_number: CheckpointSequenceNumber,
    ) -> anyhow::Result<()> {
        let mut epoch_boundaries = read_epoch_boundaries(self.remote_store.clone())
            .await
            .unwrap_or_default();
        epoch_boundaries.insert_next(epoch_id, checkpoint_sequence_number)?;

        let backoff = ExponentialBackoffBuilder::<SystemClock>::new()
            .with_max_interval(Duration::from_secs(RECORD_EPOCH_BOUNDARY_TIMEOUT_SECS))
            .build();
        retry(backoff, || async {
            write_epoch_boundaries(
                &epoch_boundaries,
                self.remote_store.clone(),
                PutMode::Overwrite,
            )
            .await
            .map_err(|e| {
                error!("failed to write epoch boundaries to the store: {:?}", &e);
                backoff::Error::transient(e)
            })
        })
        .await?;
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
        for checkpoint in batch {
            let data = Blob::encode(&checkpoint, BlobEncoding::Bcs)?;
            data.write(&mut buffer)?;
            if checkpoint.checkpoint_summary.is_last_checkpoint_of_epoch() {
                self.record_epoch_boundary(
                    checkpoint.checkpoint_summary.epoch,
                    checkpoint.checkpoint_summary.sequence_number,
                )
                .await?;
            }
        }
        self.upload(uploaded_range, self.prepare_data_to_upload(buffer)?)
            .await?;
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
