// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod digest_task;
pub(crate) mod ingestion_backfill_task;
pub(crate) mod raw_checkpoints;
pub(crate) mod tx_affected_objects;

use crate::database::ConnectionPool;
use iota_types::full_checkpoint_content::CheckpointData;

#[async_trait::async_trait]
pub trait IngestionBackfillTrait: Send + Sync {
    type ProcessedType: Send + Sync;

    fn process_checkpoint(checkpoint: &CheckpointData) -> Vec<Self::ProcessedType>;
    async fn commit_chunk(pool: ConnectionPool, processed_data: Vec<Self::ProcessedType>);
}
