// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_rest_api::Client;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};

use crate::config::Config;

pub async fn download_checkpoint_summary_from_fullnode(
    config: &Config,
    checkpoint_number: u64,
) -> anyhow::Result<CertifiedCheckpointSummary> {
    // Download the checkpoint from the server
    let client = Client::new(config.full_node_url.as_str());
    Ok(client.get_checkpoint_summary(checkpoint_number).await?)
}

pub async fn download_full_checkpoint_from_fullnode(
    config: &Config,
    checkpoint_number: u64,
) -> anyhow::Result<CheckpointData> {
    // Downloading the checkpoint from the server
    let client: Client = Client::new(config.full_node_url.as_str());
    let full_checkpoint = client.get_full_checkpoint(checkpoint_number).await?;

    Ok(full_checkpoint)
}
