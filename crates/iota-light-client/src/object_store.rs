// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow, bail};
use iota_config::node::HistoricalReaderConfig;
use iota_data_ingestion_core::history::{
    epoch_boundaries::EpochBoundaries, reader::HistoricalReader,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};
use tracing::debug;

use crate::config::Config;

pub struct CheckpointStore {
    historical_reader: HistoricalReader,
}

impl CheckpointStore {
    pub fn new(config: &Config) -> Result<Self> {
        let Some(checkpoint_store_config) = config.checkpoint_store_config.clone() else {
            bail!("missing checkpoint store config");
        };

        let config = HistoricalReaderConfig {
            object_store_config: Some(checkpoint_store_config),
            concurrency: 5,
        };

        Ok(Self {
            historical_reader: HistoricalReader::new(config)?,
        })
    }

    /// Returns the archive's recorded end-of-epoch checkpoints (`epoch → last
    /// checkpoint of that epoch`).
    pub async fn end_of_epoch_checkpoints(&self) -> Result<EpochBoundaries> {
        Ok(self.historical_reader.epoch_boundaries().await?)
    }

    pub async fn fetch_checkpoint_summary(&self, seq: u64) -> Result<CertifiedCheckpointSummary> {
        let full_checkpoint = self.fetch_full_checkpoint(seq).await?;

        Ok(full_checkpoint.checkpoint_summary)
    }

    pub async fn fetch_full_checkpoint(&self, seq: u64) -> Result<CheckpointData> {
        self.historical_reader.sync_manifest_once().await?;
        let checkpoint = self
            .historical_reader
            .iter_for_range(seq..seq + 1)
            .await?
            .next()
            .ok_or_else(|| anyhow!("missing full checkpoint"))?;
        debug!("Fetched checkpoint '{seq}' from checkpoint store",);

        Ok(checkpoint)
    }
}
