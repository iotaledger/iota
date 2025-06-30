// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use async_trait::async_trait;
use iota_data_ingestion_core::ProgressStore;
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{BigTableClient, KeyValueStoreReader, KeyValueStoreWriter};

/// Manages persistent progress information stored in BigTableDB.
///
/// This struct encapsulates operations for reading, writing, and
/// synchronizing watermark progress data to DB.
pub struct BigTableProgressStore {
    client: BigTableClient,
}

impl BigTableProgressStore {
    pub fn new(client: BigTableClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ProgressStore for BigTableProgressStore {
    type Error = anyhow::Error;

    async fn load(&mut self, _: String) -> Result<CheckpointSequenceNumber> {
        self.client.get_latest_checkpoint().await
    }

    async fn save(&mut self, _: String, checkpoint_number: CheckpointSequenceNumber) -> Result<()> {
        self.client.save_watermark(checkpoint_number).await
    }
}
