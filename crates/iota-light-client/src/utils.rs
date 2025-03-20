// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use iota_rest_api::Client;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};

use crate::{config::Config, object_store::IotaObjectStore};

pub async fn download_checkpoint_summary_from_object_store_with_fallback(
    config: &Config,
    checkpoint_number: u64,
) -> anyhow::Result<CertifiedCheckpointSummary> {
    let checkpoint_summary = if let Ok(object_store) = IotaObjectStore::new(config) {
        // Download the checkpoint from the object store
        object_store
            .download_checkpoint_summary(checkpoint_number)
            .await
            .map_err(|e| anyhow!(format!("Cannot download summary from object store: {e}")))?
    } else {
        // Fallback: download the checkpoint from the server
        let client = Client::new(config.full_node_url.as_str());
        client
            .get_checkpoint_summary(checkpoint_number)
            .await
            .map_err(|e| anyhow!(format!("Cannot download summary from full node: {e}")))?
    };

    Ok(checkpoint_summary)
}

pub async fn download_full_checkpoint_from_object_store_with_fallback(
    config: &Config,
    checkpoint_number: u64,
) -> anyhow::Result<CheckpointData> {
    let full_checkpoint = if let Ok(object_store) = IotaObjectStore::new(config) {
        // Download the checkpoint from the object store
        object_store
            .get_full_checkpoint(checkpoint_number)
            .await
            .map_err(|e| {
                anyhow!(format!(
                    "Cannot download full checkpoint from object store: {e}"
                ))
            })?
    } else {
        // Downloading the checkpoint from the server
        let client: Client = Client::new(config.full_node_url.as_str());
        client
            .get_full_checkpoint(checkpoint_number)
            .await
            .map_err(|e| {
                anyhow!(format!(
                    "Cannot download full checkpoint from full node: {e}"
                ))
            })?
    };

    Ok(full_checkpoint)
}
