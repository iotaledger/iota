// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//! Simple logic for relaying checkpoint data without any side effects.

use std::sync::Arc;

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_types::full_checkpoint_content::CheckpointData;

/// Simple worker that relays checkpoint data without any side effects.
pub struct RelayWorker;
#[async_trait]
impl Worker for RelayWorker {
    type Message = Arc<CheckpointData>;
    type Error = anyhow::Error;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        Ok(checkpoint)
    }
}
