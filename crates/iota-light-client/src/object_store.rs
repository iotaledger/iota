// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
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
            &config
                .object_store_url
                .as_ref()
                .ok_or_else(|| anyhow!("missing object store url"))?,
        )?;
        let (store, _) = object_store::parse_url(&url)?;
        Ok(Self { store })
    }

    pub async fn download_checkpoint_summary(
        &self,
        checkpoint_number: u64,
    ) -> Result<CertifiedCheckpointSummary> {
        let path = Path::from(format!("{}.chk", checkpoint_number));
        let response = self.store.get(&path).await?;
        let bytes = response.bytes().await?;

        let (_, blob) = bcs::from_bytes::<(u8, CheckpointData)>(&bytes)?;

        info!("Downloaded checkpoint summary: {}", checkpoint_number);
        Ok(blob.checkpoint_summary)
    }

    pub async fn get_full_checkpoint(&self, checkpoint_number: u64) -> Result<CheckpointData> {
        let path = Path::from(format!("{}.chk", checkpoint_number));
        info!("Request full checkpoint: {}", path);
        let response = self
            .store
            .get(&path)
            .await
            .map_err(|_| anyhow!("Cannot get full checkpoint from object store"))?;
        let bytes = response.bytes().await?;
        let (_, full_checkpoint) = bcs::from_bytes::<(u8, CheckpointData)>(&bytes)?;
        Ok(full_checkpoint)
    }
}

#[async_trait]
pub trait ObjectStoreExt {
    async fn get_checkpoint_summary(
        &self,
        checkpoint_number: u64,
    ) -> Result<CertifiedCheckpointSummary>;
}

#[async_trait]
impl ObjectStoreExt for IotaObjectStore {
    async fn get_checkpoint_summary(
        &self,
        checkpoint_number: u64,
    ) -> Result<CertifiedCheckpointSummary> {
        self.download_checkpoint_summary(checkpoint_number).await
    }
}

pub async fn download_checkpoint_summary(
    config: &Config,
    checkpoint_number: u64,
) -> Result<CertifiedCheckpointSummary> {
    let store = IotaObjectStore::new(config)?;
    store.get_checkpoint_summary(checkpoint_number).await
}
