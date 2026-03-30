// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod adapter;
pub(crate) mod jobs;
pub(crate) mod task;

use std::sync::Arc;

use iota_types::full_checkpoint_content::CheckpointData;

use crate::{db::ConnectionPool, errors::IndexerError};

/// Processes checkpoints and commits processed data to the database.
#[async_trait::async_trait]
pub(crate) trait IngestionBackfill: Send + Sync {
    type ProcessedType: Send + Sync;

    /// Converts a `CheckpointData` into zero-or-more items (`ProcessedType`).
    async fn process_checkpoint(
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Vec<Self::ProcessedType>, IndexerError>;

    /// Stores a chunk of processed items.
    async fn persist_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError>;
}
