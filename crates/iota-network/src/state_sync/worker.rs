// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anemo::async_trait;
use anyhow::{Context, anyhow};
use iota_data_ingestion_core::{Reducer, Worker};
use iota_storage::{verify_checkpoint, verify_checkpoint_linkage};
use iota_types::{
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, FullCheckpointContents, VerifiedCheckpoint,
        VerifiedCheckpointContents,
    },
    storage::WriteStore,
};

use crate::state_sync::metrics::Metrics;

/// A checkpoint downloaded from the archive whose content digests — and,
/// unless deferred, authority signatures — have been verified by a
/// [`StateSyncWorker`], pending the chain-linkage check and insertion in the
/// [`StateSyncReducer`].
pub(crate) struct VerifiedArchiveCheckpoint {
    summary: CertifiedCheckpointSummary,
    contents: VerifiedCheckpointContents,
    /// False when the committee for the checkpoint's epoch was not yet in the
    /// store, because the previous epoch's last checkpoint had not been
    /// committed; the reducer verifies the signatures for those instead.
    signatures_verified: bool,
}

/// Verifies checkpoints downloaded from the archive.
///
/// Multiple workers run concurrently, so this performs only the CPU-heavy
/// per-checkpoint verification that doesn't depend on the previous checkpoint:
/// authority signatures and content digests. The [`StateSyncReducer`] receives
/// the results ordered by sequence number and performs the chain-linkage check
/// and the store insertion.
pub(crate) struct StateSyncWorker<S>(pub(crate) S);

#[async_trait]
impl<S: WriteStore + Clone + Send + Sync + 'static> Worker for StateSyncWorker<S> {
    type Error = anyhow::Error;
    type Message = VerifiedArchiveCheckpoint;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> anyhow::Result<Self::Message> {
        let summary = checkpoint.checkpoint_summary.clone();
        let signatures_verified = match self.0.get_committee(summary.epoch()) {
            Some(committee) => {
                summary
                    .verify_authority_signatures(&committee)
                    .map_err(|e| anyhow!("checkpoint signature verification failed: {e}"))?;
                true
            }
            None => false,
        };
        let full_contents = FullCheckpointContents::from_contents_and_execution_data(
            checkpoint.checkpoint_contents.clone(),
            checkpoint.transactions.iter().map(|t| t.execution_data()),
        );
        full_contents.verify_digests(summary.contents_digest)?;
        let contents = VerifiedCheckpointContents::new_unchecked(full_contents);
        Ok(VerifiedArchiveCheckpoint {
            summary,
            contents,
            signatures_verified,
        })
    }
}

/// Chain-checks and commits checkpoints verified by [`StateSyncWorker`]s.
///
/// This is the single sequential stage of archive sync: batches arrive
/// ordered by sequence number, and each checkpoint is linked to the previous
/// one before its summary and contents are inserted into the store. It also
/// verifies the signatures the workers had to defer — the checkpoints at the
/// head of an epoch whose committee only becomes available here, once the
/// previous epoch's last checkpoint is committed.
pub(crate) struct StateSyncReducer<S>(pub(crate) S, pub(crate) Metrics);

#[async_trait]
impl<S: WriteStore + Clone + Send + Sync + 'static> Reducer<StateSyncWorker<S>>
    for StateSyncReducer<S>
{
    async fn commit(&self, batch: &[VerifiedArchiveCheckpoint]) -> anyhow::Result<()> {
        for message in batch {
            let verified_checkpoint = self.get_or_insert_verified_checkpoint(message)?;
            self.0
                .insert_checkpoint_contents(&verified_checkpoint, message.contents.clone());
            self.0
                .update_highest_synced_checkpoint(&verified_checkpoint);
            self.1.update_checkpoints_synced_from_checkpoint_archive();
        }
        Ok(())
    }
}

impl<S: WriteStore + Clone> StateSyncReducer<S> {
    fn get_or_insert_verified_checkpoint(
        &self,
        message: &VerifiedArchiveCheckpoint,
    ) -> anyhow::Result<VerifiedCheckpoint> {
        let sequence_number = message.summary.sequence_number;
        if let Some(existing) = self.0.get_checkpoint_by_sequence_number(sequence_number) {
            return Ok(existing);
        }

        let prev_checkpoint_seq_num = sequence_number
            .checked_sub(1)
            .context("checkpoint seq num underflow")?;
        let prev_checkpoint = self
            .0
            .get_checkpoint_by_sequence_number(prev_checkpoint_seq_num)
            .context(format!(
                "missing previous checkpoint {prev_checkpoint_seq_num} in store"
            ))?;

        let verified_checkpoint = if message.signatures_verified {
            verify_checkpoint_linkage(&prev_checkpoint, message.summary.clone())
                .map(VerifiedCheckpoint::new_unchecked)
                .map_err(|_| anyhow!("checkpoint linkage verification failed"))?
        } else {
            verify_checkpoint(&prev_checkpoint, &self.0, message.summary.clone())
                .map_err(|_| anyhow!("checkpoint verification failed"))?
        };

        self.0.insert_checkpoint(&verified_checkpoint);
        self.0
            .update_highest_verified_checkpoint(&verified_checkpoint);
        Ok(verified_checkpoint)
    }
}
