// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::full_checkpoint_content::CheckpointData;
use tracing::info;

use crate::{
    backfill::backfill_instances::ingestion_backfills::IngestionBackfillTrait,
    database::ConnectionPool,
};

/// Dummy backfill that only prints the sequence number and checkpoint of the
/// digest. Intended to benchmark backfill performance.
pub struct DigestBackfill;

#[async_trait::async_trait]
impl IngestionBackfillTrait for DigestBackfill {
    type ProcessedType = ();

    fn process_checkpoint(checkpoint: &CheckpointData) -> Vec<Self::ProcessedType> {
        let cp = checkpoint.checkpoint_summary.sequence_number;
        let digest = checkpoint.checkpoint_summary.content_digest;
        info!("{cp}: {digest}");

        vec![]
    }

    async fn commit_chunk(_pool: ConnectionPool, _processed_data: Vec<Self::ProcessedType>) {}
}
