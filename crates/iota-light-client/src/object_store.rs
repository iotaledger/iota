// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::num::NonZeroUsize;

use anyhow::{Result, anyhow};
use iota_config::node::ArchiveReaderConfig;
use iota_data_ingestion_core::history::reader::HistoricalReader;
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
        let checkpoint_store_config = config
            .checkpoint_store_config
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("missing checkpoint store url"))?;

        let config = ArchiveReaderConfig {
            remote_store_config: checkpoint_store_config,
            download_concurrency: NonZeroUsize::new(5).unwrap(),
            use_for_pruning_watermark: false,
        };

        Ok(Self {
            historical_reader: HistoricalReader::new(config)?,
        })
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
