// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod adapter;
pub(crate) mod task;

use std::sync::Arc;

use iota_types::full_checkpoint_content::CheckpointData;

use crate::{db::ConnectionPool, errors::IndexerError};

#[async_trait::async_trait]
pub(crate) trait IngestionBackfill: Send + Sync {
    type ProcessedType: Send + Sync;

    fn process_checkpoint(checkpoint: Arc<CheckpointData>) -> Vec<Self::ProcessedType>;
    async fn commit_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError>;
}
