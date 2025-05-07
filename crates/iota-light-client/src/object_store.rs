// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::num::NonZeroUsize;

use anyhow::{Result, anyhow};
use iota_config::{
    node::ArchiveReaderConfig,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_data_ingestion_core::history::reader::HistoricalReader;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};
use tracing::info;

use crate::config::Config;

pub struct CheckpointStore {
    historical_reader: HistoricalReader,
}

impl CheckpointStore {
    pub fn new(config: &Config) -> Result<Self> {
        let url = config
            .object_store_url
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("missing checkpoint store url"))?;

        let object_store_config = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::S3),
            aws_endpoint: Some(url),
            aws_virtual_hosted_style_request: true,
            no_sign_request: true,
            ..Default::default()
        };
        let config = ArchiveReaderConfig {
            remote_store_config: object_store_config,
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
        use futures::StreamExt;
        self.historical_reader.sync_manifest_once().await?;
        let checkpoint = self
            .historical_reader
            .iter_for_range(seq..seq + 1)
            .await?
            .next()
            .ok_or_else(|| anyhow!("missing full checkpoint"))?;

        info!("Fetched full checkpoint '{seq}' from checkpoint store:",);

        Ok(checkpoint)
    }
}
