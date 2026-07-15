// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anemo::async_trait;
use anyhow::{Context, anyhow};
use iota_data_ingestion_core::Worker;
use iota_storage::verify_checkpoint;
use iota_types::{
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, FullCheckpointContents, VerifiedCheckpoint,
        VerifiedCheckpointContents,
    },
    storage::WriteStore,
};

use crate::state_sync::metrics::Metrics;

pub(crate) struct StateSyncWorker<S>(pub(crate) S, pub(crate) Metrics);

#[async_trait]
impl<S: WriteStore + Clone + Send + Sync + 'static> Worker for StateSyncWorker<S> {
    type Error = anyhow::Error;
    type Message = ();

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> anyhow::Result<()> {
        let verified_checkpoint = get_or_insert_verified_checkpoint(
            &self.0,
            checkpoint.checkpoint_summary.clone(),
            true,
        )?;
        let full_contents = FullCheckpointContents::from_contents_and_execution_data(
            checkpoint.checkpoint_contents.clone(),
            checkpoint.transactions.iter().map(|t| t.execution_data()),
        );
        full_contents.verify_digests(verified_checkpoint.content_digest)?;
        let verified_contents = VerifiedCheckpointContents::new_unchecked(full_contents);
        self.0
            .insert_checkpoint_contents(&verified_checkpoint, verified_contents);
        self.0
            .update_highest_synced_checkpoint(&verified_checkpoint);
        self.1.update_checkpoints_synced_from_checkpoint_archive();
        Ok(())
    }
}

pub fn get_or_insert_verified_checkpoint<S>(
    store: &S,
    certified_checkpoint: CertifiedCheckpointSummary,
    verify: bool,
) -> anyhow::Result<VerifiedCheckpoint>
where
    S: WriteStore + Clone,
{
    store
        .get_checkpoint_by_sequence_number(certified_checkpoint.sequence_number)
        .map(Ok::<VerifiedCheckpoint, anyhow::Error>)
        .unwrap_or_else(|| {
            let verified_checkpoint = if verify {
                // Verify checkpoint summary
                let prev_checkpoint_seq_num = certified_checkpoint
                    .sequence_number
                    .checked_sub(1)
                    .context("Checkpoint seq num underflow")?;
                let prev_checkpoint = store
                    .get_checkpoint_by_sequence_number(prev_checkpoint_seq_num)
                    .context(format!(
                        "Missing previous checkpoint {prev_checkpoint_seq_num} in store",
                    ))?;

                verify_checkpoint(&prev_checkpoint, store, certified_checkpoint)
                    .map_err(|_| anyhow!("Checkpoint verification failed"))?
            } else {
                VerifiedCheckpoint::new_unchecked(certified_checkpoint)
            };
            // Insert checkpoint summary
            store.insert_checkpoint(&verified_checkpoint);
            // Update highest verified checkpoint watermark
            store.update_highest_verified_checkpoint(&verified_checkpoint);
            Ok::<VerifiedCheckpoint, anyhow::Error>(verified_checkpoint)
        })
        .map_err(|e| anyhow!("Failed to get verified checkpoint: {e:?}"))
}
