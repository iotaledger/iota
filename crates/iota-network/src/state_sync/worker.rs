// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use anemo::async_trait;
use anyhow::{Context, anyhow, ensure};
use iota_data_ingestion_core::{Reducer, Worker};
use iota_storage::verify_checkpoint_linkage;
use iota_types::{
    committee::{Committee, EpochId},
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointSequenceNumber, FullCheckpointContents,
        VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    storage::WriteStore,
};
use tracing::debug;

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
        let committee = self.0.get_committee(summary.epoch());
        // As many workers run as there are cores, so keep their CPU-bound
        // verification off the runtime's worker threads, where it would hold up
        // checkpoint execution — which in turn gates how far sync may run
        // ahead.
        tokio::task::spawn_blocking(move || {
            let signatures_verified = match committee {
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
        })
        .await?
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
pub(crate) struct StateSyncReducer<S> {
    pub(crate) store: S,
    pub(crate) metrics: Metrics,
    /// How far the synced watermark may run ahead of the executed one before
    /// insertion waits for execution to catch up.
    pub(crate) max_checkpoints_ahead_of_execution: u64,
}

/// How many checkpoints the reducer commits per store insertion at most,
/// bounding the size of the underlying write batches.
const MAX_CHECKPOINTS_PER_COMMIT: usize = 500;

#[async_trait]
impl<S: WriteStore + Clone + Send + Sync + 'static> Reducer<StateSyncWorker<S>>
    for StateSyncReducer<S>
{
    async fn commit(&self, batch: &[VerifiedArchiveCheckpoint]) -> anyhow::Result<()> {
        self.verify_deferred_signatures(batch).await?;
        self.wait_for_execution_to_catch_up(batch).await;

        let mut to_insert = Vec::with_capacity(batch.len());
        let mut prev_checkpoint = None;
        for message in batch {
            let verified_checkpoint =
                self.verify_against_previous(message, prev_checkpoint.as_ref())?;
            prev_checkpoint = Some(verified_checkpoint.clone());
            to_insert.push((verified_checkpoint, message.contents.clone()));
        }

        self.store
            .try_insert_synced_checkpoints(to_insert)
            .map_err(|e| anyhow!("failed to insert synced checkpoints: {e}"))?;

        for _ in batch {
            self.metrics
                .update_checkpoints_synced_from_checkpoint_archive();
        }
        Ok(())
    }

    /// Closes a batch at the size cap, and at epoch boundaries so that an
    /// epoch's last checkpoint — which carries the next committee — is
    /// committed before any checkpoint of the next epoch needs that committee
    /// for its deferred signature verification.
    fn should_close_batch(
        &self,
        batch: &[VerifiedArchiveCheckpoint],
        next_item: Option<&VerifiedArchiveCheckpoint>,
    ) -> bool {
        let Some(next) = next_item else {
            return true;
        };
        batch.len() >= MAX_CHECKPOINTS_PER_COMMIT
            || batch
                .last()
                .is_some_and(|last| last.summary.epoch() != next.summary.epoch())
    }
}

impl<S: WriteStore + Clone> StateSyncReducer<S> {
    /// Waits until the whole batch fits within
    /// `max_checkpoints_ahead_of_execution` of the executed watermark.
    ///
    /// Synced contents can only be pruned once executed, so inserting far
    /// ahead of execution grows disk usage without bound. Waiting here rather
    /// than bounding what the workers download keeps the disk bound while
    /// leaving the run's range untouched; execution advances from the
    /// checkpoints already in the store, so it makes progress in the meantime.
    async fn wait_for_execution_to_catch_up(&self, batch: &[VerifiedArchiveCheckpoint]) {
        let (Some(first), Some(last)) = (batch.first(), batch.last()) else {
            return;
        };
        let required = required_executed_checkpoint(
            last.summary.sequence_number,
            self.max_checkpoints_ahead_of_execution,
        )
        // Execution can only reach checkpoints that are inserted, so never
        // wait past the batch's predecessor: a cap below the batch size would
        // otherwise wait for a checkpoint this very batch has to insert.
        .min(first.summary.sequence_number.saturating_sub(1));

        let highest_executed = self.store.get_highest_executed_checkpoint_seq_number();
        if highest_executed.unwrap_or(0) >= required {
            return;
        }
        debug!(
            "checkpoint archive sync paused: inserting checkpoint {} needs executed checkpoint \
             {required}, which is at {highest_executed:?}",
            last.summary.sequence_number
        );
        self.store.wait_for_executed_checkpoint(required).await;
        debug!("checkpoint archive sync resumed at executed checkpoint {required}");
    }

    /// Verifies the authority signatures the workers had to defer, batched per
    /// epoch. The deferred checkpoints sit at the head of an epoch, so by
    /// commit time the previous epoch's last checkpoint — which carries their
    /// committee — is committed and the committee is in the store.
    async fn verify_deferred_signatures(
        &self,
        batch: &[VerifiedArchiveCheckpoint],
    ) -> anyhow::Result<()> {
        let mut deferred: BTreeMap<EpochId, Vec<CertifiedCheckpointSummary>> = BTreeMap::new();
        for message in batch.iter().filter(|message| !message.signatures_verified) {
            deferred
                .entry(message.summary.epoch())
                .or_default()
                .push(message.summary.clone());
        }

        for (epoch, summaries) in deferred {
            let committee = self
                .store
                .get_committee(epoch)
                .context(format!("missing committee for epoch {epoch} in store"))?;
            tokio::task::spawn_blocking(move || batch_verify_signatures(&summaries, &committee))
                .await??;
        }
        Ok(())
    }

    /// Chain-checks one checkpoint against its predecessor — the previous
    /// checkpoint of the batch being committed, or the store's copy at the
    /// start of a batch.
    fn verify_against_previous(
        &self,
        message: &VerifiedArchiveCheckpoint,
        prev_in_batch: Option<&VerifiedCheckpoint>,
    ) -> anyhow::Result<VerifiedCheckpoint> {
        let sequence_number = message.summary.sequence_number;
        if let Some(existing) = self
            .store
            .get_checkpoint_by_sequence_number(sequence_number)
        {
            // The contents will be inserted under the stored summary, and
            // mismatched contents would only panic after the transactions are
            // written, so reject archive data that diverges from the store
            // before any write.
            ensure!(
                existing.digest() == message.summary.digest(),
                "archive checkpoint {sequence_number} does not match the checkpoint already in the store"
            );
            return Ok(existing);
        }

        let prev_checkpoint_seq_num = sequence_number
            .checked_sub(1)
            .context("checkpoint seq num underflow")?;
        let prev_checkpoint = match prev_in_batch {
            Some(prev) if prev.sequence_number == prev_checkpoint_seq_num => prev.clone(),
            _ => self
                .store
                .get_checkpoint_by_sequence_number(prev_checkpoint_seq_num)
                .context(format!(
                    "missing previous checkpoint {prev_checkpoint_seq_num} in store"
                ))?,
        };

        verify_checkpoint_linkage(&prev_checkpoint, message.summary.clone())
            .map(VerifiedCheckpoint::new_unchecked)
            .map_err(|_| anyhow!("checkpoint linkage verification failed"))
    }
}

/// The executed checkpoint that inserting `sequence_number` waits for, so that
/// insertion stays within `max_ahead_of_execution` checkpoints of execution.
pub(crate) fn required_executed_checkpoint(
    sequence_number: CheckpointSequenceNumber,
    max_ahead_of_execution: u64,
) -> CheckpointSequenceNumber {
    sequence_number.saturating_sub(max_ahead_of_execution)
}

/// Verifies the authority signatures of `summaries`, all certified by
/// `committee`, in one batched signature verification. On failure the
/// summaries are verified one by one, so that the error names the offending
/// checkpoint.
fn batch_verify_signatures(
    summaries: &[CertifiedCheckpointSummary],
    committee: &Committee,
) -> anyhow::Result<()> {
    let Err(batch_error) =
        CertifiedCheckpointSummary::batch_verify_authority_signatures(summaries, committee)
    else {
        return Ok(());
    };
    for summary in summaries {
        summary
            .verify_authority_signatures(committee)
            .map_err(|e| {
                anyhow!(
                    "checkpoint {} signature verification failed: {e}",
                    summary.sequence_number
                )
            })?;
    }
    Err(anyhow!(
        "checkpoint signature batch verification failed: {batch_error}"
    ))
}
