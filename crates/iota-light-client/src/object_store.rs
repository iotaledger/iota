// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};
use object_store::{ObjectStore, path::Path};
use tracing::info;
use url::Url;

use crate::config::Config;

pub struct IotaObjectStore {
    store: Box<dyn ObjectStore>,
}

impl IotaObjectStore {
    pub fn new(config: &Config) -> Result<Self> {
        let url = Url::parse(
            config
                .object_store_url
                .as_ref()
                .ok_or_else(|| anyhow!("missing object store url"))?,
        )?;
        let (store, _) = object_store::parse_url(&url)?;
        Ok(Self { store })
    }

    pub async fn fetch_checkpoint_summary(&self, seq: u64) -> Result<CertifiedCheckpointSummary> {
        let full_checkpoint = self.fetch_full_checkpoint(seq).await?;

        Ok(full_checkpoint.checkpoint_summary)
    }

    pub async fn fetch_full_checkpoint(&self, seq: u64) -> Result<CheckpointData> {
        let path = Path::from(format!("{seq}.chk"));
        let response = self
            .store
            .get(&path)
            .await
            .context("Failed to fetch full checkpoint from object store")?;
        let bytes = response.bytes().await?;
        let (_, full_checkpoint) = bcs::from_bytes::<(u8, CheckpointData)>(&bytes)?;

        info!("Fetched full checkpoint '{path}' from object store:");

        Ok(full_checkpoint)
    }
}

#[async_trait]
pub trait ObjectStoreExt {
    async fn get_checkpoint_summary(&self, seq: u64) -> Result<CertifiedCheckpointSummary>;
}

#[async_trait]
impl ObjectStoreExt for IotaObjectStore {
    async fn get_checkpoint_summary(&self, seq: u64) -> Result<CertifiedCheckpointSummary> {
        self.fetch_checkpoint_summary(seq).await
    }
}

pub async fn download_checkpoint_summary(
    config: &Config,
    seq: u64,
) -> Result<CertifiedCheckpointSummary> {
    let store = IotaObjectStore::new(config)?;
    store.get_checkpoint_summary(seq).await
}
