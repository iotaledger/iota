// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch verified metadata (`epoch_info`) held by every node, independent
//! of any API.
//!
//! Each row is an [`EpochInfoV2`]: the start-of-epoch identity plus the
//! close-of-epoch proof bundle ([`EpochInfoV1Entry`]). A row is seeded when its
//! epoch opens and finalized when the epoch closes. The table is append-only
//! and never pruned; the `epoch_info_indexed_watermark` tracks the highest
//! epoch whose contiguous prefix is finalized.

use iota_types::{
    committee::EpochId,
    full_checkpoint_content::CheckpointData,
    iota_system_state::IotaSystemStateTrait,
    messages_checkpoint::{
        CheckpointContents, CheckpointSequenceNumber, CheckpointSummary, VerifiedCheckpoint,
    },
    storage::{
        EpochInfoV1Entry, EpochInfoV2,
        error::{Error as StorageError, Kind as StorageErrorKind},
    },
};
use tracing::{info, warn};
use typed_store::{Map, TypedStoreError, rocks::DBBatch};

use crate::{authority::AuthorityStore, checkpoints::CheckpointStore};

impl CheckpointStore {
    /// Read the `epoch_info` row for `epoch`, if present.
    pub fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfoV2>, TypedStoreError> {
        self.tables.epoch_info.get(&epoch)
    }

    /// Read `epoch_info_indexed_watermark`: the highest epoch whose
    /// `epoch_info` row is finalized (the full close-of-epoch proof bundle
    /// present, see [`EpochInfoV2::is_finalized`]). `None` if no epoch has
    /// been fully indexed yet.
    pub fn highest_indexed_epoch(&self) -> Result<Option<EpochId>, TypedStoreError> {
        self.tables.epoch_info_indexed_watermark.get(&())
    }

    /// Persist fully-populated epoch rows (e.g. restored from a snapshot) and
    /// advance the `epoch_info_indexed_watermark` over the now-contiguous
    /// prefix.
    ///
    /// Must not run concurrently with live boundary indexing: it recomputes the
    /// watermark outside the per-boundary batch.
    pub fn insert_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<(), StorageError> {
        let mut batch = self.tables.epoch_info.batch();
        batch.insert_batch(
            &self.tables.epoch_info,
            rows.into_iter().map(|row| (row.epoch, row)),
        )?;
        batch.write()?;
        self.reconcile_epoch_info_watermark()?;
        Ok(())
    }

    /// Finalize the closing epoch with its proof bundle and seed the next
    /// epoch's row. A no-op on non-boundary checkpoints.
    ///
    /// Must be called in epoch order; relies on boundaries arriving
    /// contiguously so the watermark advances by exactly one each time.
    pub(crate) fn index_epoch_boundary(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<(), StorageError> {
        let mut batch = self.tables.epoch_info.batch();
        self.index_epoch(checkpoint, &mut batch)?;
        batch.write()?;
        Ok(())
    }

    /// `None` when `epoch_info` is contiguously populated from genesis through
    /// at least the last closed epoch this node executed; otherwise
    /// `Some((highest_indexed, last_executed_epoch))` describing the gap a
    /// startup guard must reject. Measured against the last executed closed
    /// epoch, not a target epoch, so a node still catching up isn't flagged.
    pub fn epoch_info_gap(&self) -> Result<Option<(Option<EpochId>, EpochId)>, StorageError> {
        let Some(open_epoch) = self.first_open_epoch()? else {
            return Ok(None); // nothing executed yet
        };
        let Some(last_executed) = open_epoch.checked_sub(1) else {
            return Ok(None); // still in the genesis epoch; no closed epoch
        };
        let highest_indexed = self.highest_indexed_epoch()?;
        // `<`, not `!=`: a backfill seeded past local execution is a superset.
        Ok((highest_indexed < Some(last_executed)).then_some((highest_indexed, last_executed)))
    }

    /// Seed the current (open) epoch's `epoch_info` row if still missing. No-op
    /// if already seeded or its start checkpoint can't yet be derived locally;
    /// call again once the closed-epoch rows are seeded.
    pub fn ensure_current_epoch_info(
        &self,
        authority_store: &AuthorityStore,
    ) -> Result<(), StorageError> {
        // Seed the *open* epoch, not the highest executed checkpoint's epoch.
        // The two differ when that checkpoint closed its epoch — the state a
        // snapshot restore always lands in. The closed epoch's row comes from
        // EPOCH_INFO; the next epoch's row has no other writer until its own
        // close, where `index_epoch` would find it missing, skip the finalize,
        // and wedge the watermark below it for good. With nothing executed yet
        // the open epoch is genesis epoch 0.
        let current_epoch = self.first_open_epoch()?.unwrap_or(0);

        if self.tables.epoch_info.get(&current_epoch)?.is_some() {
            return Ok(());
        }

        let Some(start_checkpoint) = self.current_epoch_start_checkpoint(current_epoch)? else {
            // Skip rather than fail startup; re-seeded once the backfill lands
            // the previous epoch's row.
            warn!(
                epoch = current_epoch,
                "skipping current-epoch seed: previous epoch's last checkpoint is \
                 unknown locally; deferring to the snapshot backfill"
            );
            return Ok(());
        };

        let system_state = iota_types::iota_system_state::get_iota_system_state(authority_store)
            .map_err(|e| StorageError::custom(format!("Failed to find system state: {e}")))?;

        let epoch_info = EpochInfoV2 {
            epoch: current_epoch,
            start_checkpoint,
            start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
            system_state,
            epoch_close_proof: None,
        };

        self.tables
            .epoch_info
            .insert(&epoch_info.epoch, &epoch_info)?;

        Ok(())
    }

    /// Index locally any closed epochs still missing above the
    /// `epoch_info_indexed_watermark`, by replaying their closing checkpoints
    /// from local data.
    ///
    /// Only needed when the latest published formal snapshot lags this node's
    /// executed history by more than one epoch (a delayed snapshot pipeline):
    /// the backfill then seeds a prefix that ends below the locally executed
    /// epochs, and the rows in between can only come from their own closing
    /// checkpoints. Best-effort: stops at the first epoch whose checkpoint data
    /// is already pruned, leaving the rest to a newer snapshot. Must not run
    /// concurrently with live indexing, like [`Self::insert_epoch_info`].
    pub fn index_missing_epochs_locally(
        &self,
        authority_store: &AuthorityStore,
    ) -> Result<(), StorageError> {
        let Some(last_executed) = self
            .first_open_epoch()?
            .and_then(|open| open.checked_sub(1))
        else {
            return Ok(()); // no closed epoch yet
        };
        let Some(highest_indexed) = self.highest_indexed_epoch()? else {
            // Without a seeded prefix the replay would start at genesis, which
            // backfill-dependent nodes don't have; live indexing covers the rest.
            return Ok(());
        };
        if highest_indexed >= last_executed {
            return Ok(());
        }

        // Replay the closing checkpoints of epochs `[highest_indexed,
        // last_executed]` in order: the first one re-finalizes the already
        // complete prefix end and creates the next epoch's row (an epoch's
        // start state only exists in the previous epoch's closing checkpoint),
        // each subsequent one finalizes a missing row and creates the next.
        for epoch in highest_indexed..=last_executed {
            // The map is never pruned, but the checkpoint data it points to may
            // be: missing data ends what we can rebuild locally.
            let checkpoint_data = (|| -> Result<Option<CheckpointData>, StorageError> {
                let Some(seq) = self.get_epoch_last_checkpoint_seq_number(epoch)? else {
                    return Ok(None);
                };
                let Some(summary) = self.get_checkpoint_by_sequence_number(seq)? else {
                    return Ok(None);
                };
                let Some(contents) = self.get_checkpoint_contents(&summary.content_digest)? else {
                    return Ok(None);
                };
                match assemble_sparse_checkpoint_data(authority_store, summary, contents) {
                    Ok(data) => Ok(Some(data)),
                    // Pruned-away transactions/effects/objects are the expected
                    // end of what can be rebuilt locally; anything else is a
                    // real storage failure and must propagate.
                    Err(e) if e.kind() == StorageErrorKind::Missing => Ok(None),
                    Err(e) => Err(e),
                }
            })()?;
            let Some(checkpoint_data) = checkpoint_data else {
                warn!(
                    epoch,
                    "cannot index epoch locally (its closing checkpoint's data is pruned); \
                     leaving the remaining epochs to a newer snapshot"
                );
                return Ok(());
            };

            self.index_epoch_boundary(&checkpoint_data)?;
        }
        info!(
            "locally indexed epochs ({highest_indexed}, {last_executed}] not covered by the \
             snapshot backfill"
        );
        Ok(())
    }

    /// The current epoch's start checkpoint, or `None` when it can't be derived
    /// from local data (no previous-epoch `epoch_info` row and no
    /// `epoch_last_checkpoint_map` entry). `None` means "skip for now", not an
    /// error.
    fn current_epoch_start_checkpoint(
        &self,
        current_epoch: EpochId,
    ) -> Result<Option<CheckpointSequenceNumber>, StorageError> {
        // The first epoch starts at checkpoint 0.
        if current_epoch == 0 {
            return Ok(Some(0));
        }
        let previous_epoch = current_epoch - 1;

        // Prefer the previous epoch's recorded end checkpoint.
        if let Some(end_checkpoint) = self
            .tables
            .epoch_info
            .get(&previous_epoch)?
            .and_then(|info| info.end_checkpoint())
        {
            return Ok(Some(end_checkpoint + 1));
        }

        // Unlike the checkpoint summaries, the map is never pruned.
        Ok(self
            .get_epoch_last_checkpoint_seq_number(previous_epoch)?
            .map(|seq| seq + 1))
    }

    /// Finalize the closing epoch's row with its close-of-epoch proof bundle
    /// and seed the next epoch's row. A no-op on non-boundary checkpoints.
    /// Stages all writes into `batch` so the caller commits them atomically.
    fn index_epoch(
        &self,
        checkpoint: &CheckpointData,
        batch: &mut DBBatch,
    ) -> Result<(), StorageError> {
        let Some((epoch_info, end_of_epoch_events)) = checkpoint.epoch_info()? else {
            return Ok(());
        };
        let new_epoch_id = epoch_info.epoch;

        // Finalize `prev_epoch`'s row with the close-of-epoch proof bundle and
        // advance the watermark. Genesis has no previous epoch to finalize; the
        // close-of-epoch write for epoch 0 fires when epoch 1 is seeded, so the
        // watermark stays absent until then.
        if new_epoch_id > 0 {
            let prev_epoch = new_epoch_id - 1;
            // If no row exists for `prev_epoch`, this node didn't see its
            // start (e.g. bootstrapped mid-epoch). Skip the upsert; the row
            // stays absent and the watermark stays behind, so the snapshot
            // writer correctly refuses to publish until an external backfill
            // fills the gap.
            if let Some(mut previous_epoch) = self.tables.epoch_info.get(&prev_epoch)? {
                // The closing checkpoint's last tx is `prev_epoch`'s
                // epoch-change tx; its effects and the system-state objects it
                // wrote (object `0x5` and its inner state object) are this
                // boundary's proof material.
                let change_epoch_tx = checkpoint.end_of_epoch_transaction().ok_or_else(|| {
                    StorageError::custom(format!(
                        "checkpoint {} closes epoch {prev_epoch} but carries no \
                         epoch-change transaction",
                        checkpoint.checkpoint_summary.sequence_number,
                    ))
                })?;
                // Serialized to raw bytes, not the decoded view, so they verify
                // against the effects' written-object digests at restore time
                // (see `get_iota_system_state_objects` for why only these two).
                let next_epoch_start_system_state_objects =
                    iota_types::iota_system_state::get_iota_system_state_objects(
                        &change_epoch_tx.output_objects.as_slice(),
                    )
                    .map_err(|e| {
                        StorageError::custom(format!(
                            "extracting next-epoch start-state objects: {e}"
                        ))
                    })?
                    .iter()
                    .map(bcs::to_bytes)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        StorageError::custom(format!("serializing start-state objects: {e}"))
                    })?;

                previous_epoch.epoch_close_proof = Some(EpochInfoV1Entry {
                    last_checkpoint_summary: checkpoint.checkpoint_summary.clone(),
                    last_checkpoint_contents: checkpoint.checkpoint_contents.clone(),
                    end_of_epoch_tx_effects: change_epoch_tx.effects.clone(),
                    // Safe-mode boundaries run `advance_epoch_safe_mode`, which
                    // mutates `0x5` but emits no events, so the list is empty.
                    end_of_epoch_tx_events: end_of_epoch_events.unwrap_or_default(),
                    next_epoch_start_system_state_objects,
                });
                batch.insert_batch(&self.tables.epoch_info, [(prev_epoch, previous_epoch)])?;
                self.try_advance_epoch_info_watermark(prev_epoch, batch)?;
            }
        }

        // seed the new epoch's row; its close-of-epoch proof is filled in when
        // the next epoch's boundary is indexed.
        let new_info = EpochInfoV2 {
            epoch: epoch_info.epoch,
            start_checkpoint: epoch_info.start_checkpoint,
            start_timestamp_ms: epoch_info.start_timestamp_ms,
            system_state: epoch_info.system_state,
            epoch_close_proof: None,
        };
        batch.insert_batch(&self.tables.epoch_info, [(new_epoch_id, new_info)])?;

        Ok(())
    }

    /// Advance the `epoch_info_indexed_watermark` only if there is no gap.
    fn try_advance_epoch_info_watermark(
        &self,
        prev_epoch: EpochId,
        batch: &mut DBBatch,
    ) -> Result<(), StorageError> {
        let next_expected = self
            .tables
            .epoch_info_indexed_watermark
            .get(&())?
            .map_or(0, |e| e.saturating_add(1));
        if prev_epoch == next_expected {
            batch.insert_batch(
                &self.tables.epoch_info_indexed_watermark,
                [((), prev_epoch)],
            )?;
        }
        Ok(())
    }

    /// Recompute `epoch_info_indexed_watermark` = the highest epoch whose
    /// contiguous prefix `[0, epoch]` is finalized (see
    /// [`EpochInfoV2::is_finalized`]). Only ever raises it.
    ///
    /// Unlike [`Self::try_advance_epoch_info_watermark`] (the live +1 step),
    /// this can jump the watermark across a whole seeded prefix, which is what
    /// a snapshot backfill needs.
    fn reconcile_epoch_info_watermark(&self) -> Result<(), TypedStoreError> {
        // `[0, watermark]` is already known complete, so resume the scan from
        // `watermark + 1` rather than re-scanning the whole table.
        let current = self.tables.epoch_info_indexed_watermark.get(&())?;
        let mut next = current.map_or(0, |w| w + 1);
        for entry in self
            .tables
            .epoch_info
            .safe_iter_with_bounds(Some(next), None)
        {
            let (epoch_id, info) = entry?;
            if epoch_id != next || !info.is_finalized() {
                break;
            }
            next += 1;
        }
        if let Some(highest) = next.checked_sub(1) {
            if Some(highest) > current {
                self.tables
                    .epoch_info_indexed_watermark
                    .insert(&(), &highest)?;
            }
        }
        Ok(())
    }

    /// The first not-yet-closed epoch: the highest executed checkpoint's epoch,
    /// plus one if that checkpoint already closed its epoch.
    fn first_open_epoch(&self) -> Result<Option<EpochId>, StorageError> {
        Ok(self
            .get_highest_executed_checkpoint()?
            .map(|highest| open_epoch_of(highest.data())))
    }
}

/// The open epoch as of `checkpoint`: its own epoch, plus one if it closed it.
fn open_epoch_of(checkpoint: &CheckpointSummary) -> EpochId {
    if checkpoint.is_last_checkpoint_of_epoch() {
        checkpoint.epoch + 1
    } else {
        checkpoint.epoch
    }
}

/// Load a `CheckpointData`, including events for any transaction that emitted
/// them (the epoch-change tx's events feed the EPOCH_INFO proof bundle); a
/// pruned transactions/effects/objects/events table surfaces as a `Missing`
/// error.
fn assemble_sparse_checkpoint_data(
    authority_store: &AuthorityStore,
    summary: VerifiedCheckpoint,
    contents: CheckpointContents,
) -> Result<CheckpointData, StorageError> {
    use iota_types::{
        effects::TransactionEffectsAPI, full_checkpoint_content::CheckpointTransaction,
    };

    let transaction_digests = contents
        .iter()
        .map(|execution_digests| execution_digests.transaction)
        .collect::<Vec<_>>();
    let transactions = authority_store
        .multi_get_transaction_blocks(&transaction_digests)?
        .into_iter()
        .map(|maybe_transaction| {
            maybe_transaction.ok_or_else(|| StorageError::missing("missing transaction"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let effects = authority_store
        .multi_get_executed_effects(&transaction_digests)?
        .into_iter()
        .map(|maybe_effects| maybe_effects.ok_or_else(|| StorageError::missing("missing effects")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut full_transactions = Vec::with_capacity(transactions.len());
    for (tx, fx) in transactions.into_iter().zip(effects) {
        let input_objects =
            iota_types::storage::get_transaction_input_objects(authority_store, &fx)?;
        let output_objects =
            iota_types::storage::get_transaction_output_objects(authority_store, &fx)?;

        // Load events for any emitting tx. Only the last (epoch-change) tx's
        // events are consumed downstream, but loading all keeps the assembled
        // `CheckpointData` self-consistent and costs little. A pruned events
        // table surfaces as a `Missing` error.
        let events = if fx.events_digest().is_some() {
            Some(
                authority_store
                    .get_events(fx.transaction_digest())
                    .map_err(|e| StorageError::custom(format!("loading events: {e}")))?
                    .ok_or_else(|| {
                        StorageError::missing("missing events for an emitting transaction")
                    })?,
            )
        } else {
            None
        };

        let full_transaction = CheckpointTransaction {
            transaction: tx.into(),
            effects: fx,
            events,
            input_objects,
            output_objects,
        };

        full_transactions.push(full_transaction);
    }

    let checkpoint_data = CheckpointData {
        checkpoint_summary: summary.into(),
        checkpoint_contents: contents,
        transactions: full_transactions,
    };

    Ok(checkpoint_data)
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::GasCostSummary;
    use iota_types::{
        crypto::AuthorityStrongQuorumSignInfo,
        digests::TransactionDigest,
        effects::{TransactionEffects, TransactionEffectsExtForTesting, TransactionEvents},
        iota_system_state::IotaSystemState,
        message_envelope::Envelope,
        messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSummary, EndOfEpochData},
    };
    use typed_store::Map;

    use super::*;
    use crate::checkpoints::CheckpointStore;

    fn certified_summary(epoch: EpochId, sequence_number: u64) -> CertifiedCheckpointSummary {
        certified_summary_with(epoch, sequence_number, None)
    }

    fn certified_summary_with(
        epoch: EpochId,
        sequence_number: u64,
        end_of_epoch_data: Option<EndOfEpochData>,
    ) -> CertifiedCheckpointSummary {
        let summary = CheckpointSummary {
            epoch,
            sequence_number,
            network_total_transactions: 0,
            content_digest: Default::default(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: GasCostSummary::default(),
            end_of_epoch_data,
            timestamp_ms: 0,
            version_specific_data: Vec::new(),
            checkpoint_commitments: Vec::new(),
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        Envelope::new_from_data_and_sig(summary, sig)
    }

    /// An executed (non-boundary) checkpoint.
    fn executed_checkpoint(epoch: EpochId, sequence_number: u64) -> VerifiedCheckpoint {
        VerifiedCheckpoint::new_unchecked(certified_summary(epoch, sequence_number))
    }

    /// An executed close-of-epoch checkpoint.
    fn closing_checkpoint(epoch: EpochId, sequence_number: u64) -> VerifiedCheckpoint {
        VerifiedCheckpoint::new_unchecked(certified_summary_with(
            epoch,
            sequence_number,
            Some(EndOfEpochData {
                next_epoch_committee: Vec::new(),
                next_epoch_protocol_version: 1.into(),
                epoch_commitments: Vec::new(),
                epoch_supply_change: 0,
            }),
        ))
    }

    /// A finalized `EpochInfoV2` row (its `epoch_close_proof` is `Some`).
    fn complete_epoch_info(epoch: EpochId) -> EpochInfoV2 {
        EpochInfoV2 {
            epoch,
            start_checkpoint: 0,
            start_timestamp_ms: 0,
            system_state: IotaSystemState::for_testing(epoch, 1),
            epoch_close_proof: Some(EpochInfoV1Entry {
                last_checkpoint_summary: certified_summary(epoch, 0),
                last_checkpoint_contents: CheckpointContents::new_with_digests_only_for_tests(
                    std::iter::empty(),
                ),
                end_of_epoch_tx_effects: TransactionEffects::new_empty_v1_for_testing(
                    TransactionDigest::ZERO,
                ),
                end_of_epoch_tx_events: TransactionEvents::default(),
                next_epoch_start_system_state_objects: Vec::new(),
            }),
        }
    }

    /// `insert_epoch_info` reconciles the watermark to the contiguous-prefix
    /// maximum: rows above a gap don't advance it; filling the gap advances it
    /// across the whole now-contiguous prefix.
    #[tokio::test]
    async fn insert_epoch_info_round_trips_and_advances_watermark() {
        let store = CheckpointStore::new_for_tests();

        // A first insert that doesn't start at genesis leaves the watermark
        // absent: the contiguous-from-0 prefix is still empty.
        store
            .insert_epoch_info(vec![complete_epoch_info(5)])
            .unwrap();
        assert!(store.get_epoch_info(5).unwrap().is_some());
        assert_eq!(store.highest_indexed_epoch().unwrap(), None);

        store
            .insert_epoch_info(vec![
                complete_epoch_info(0),
                complete_epoch_info(1),
                complete_epoch_info(2),
            ])
            .unwrap();
        for epoch in 0..=2 {
            assert!(
                store.get_epoch_info(epoch).unwrap().is_some(),
                "epoch {epoch} row must be present after insert"
            );
        }
        assert_eq!(store.highest_indexed_epoch().unwrap(), Some(2));

        // A row at epoch 4 leaves a gap at epoch 3, so the watermark stays at 2.
        store
            .insert_epoch_info(vec![complete_epoch_info(4)])
            .unwrap();
        assert_eq!(store.highest_indexed_epoch().unwrap(), Some(2));

        // Filling the gap at epoch 3 makes [0, 5] contiguous, so the watermark
        // jumps across every stranded row to 5.
        store
            .insert_epoch_info(vec![complete_epoch_info(3)])
            .unwrap();
        assert_eq!(store.highest_indexed_epoch().unwrap(), Some(5));
    }

    /// `reconcile` is monotonic: a seed covering only a short prefix must not
    /// lower a watermark the live indexer already advanced further.
    #[tokio::test]
    async fn reconcile_epoch_info_watermark_never_regresses() {
        let store = CheckpointStore::new_for_tests();

        store
            .insert_epoch_info(vec![
                complete_epoch_info(0),
                complete_epoch_info(1),
                complete_epoch_info(2),
            ])
            .unwrap();
        assert_eq!(store.highest_indexed_epoch().unwrap(), Some(2));

        // Simulate a concurrent live advance to a higher epoch.
        store
            .tables
            .epoch_info_indexed_watermark
            .insert(&(), &5)
            .unwrap();

        // A reconcile that only sees the [0, 2] prefix must NOT lower it.
        store.reconcile_epoch_info_watermark().unwrap();
        assert_eq!(
            store.highest_indexed_epoch().unwrap(),
            Some(5),
            "reconcile must not regress a higher watermark"
        );
    }

    /// `current_epoch_start_checkpoint` returns `None` when neither the
    /// previous epoch's row nor a map entry exists, and `Some` for genesis,
    /// from the map entry alone, and from the previous epoch's row
    /// (preferred).
    #[tokio::test]
    async fn current_epoch_start_checkpoint_skips_when_history_unavailable() {
        let store = CheckpointStore::new_for_tests();

        // Genesis always starts at checkpoint 0; no history needed.
        assert_eq!(store.current_epoch_start_checkpoint(0).unwrap(), Some(0));

        // Epoch 5 with no previous-epoch row and no map entry -> `None`.
        assert_eq!(store.current_epoch_start_checkpoint(5).unwrap(), None);

        // The never-pruned map entry alone resolves the start, even with every
        // checkpoint summary absent: epoch 4 ended at 41 -> start is 42.
        store
            .insert_epoch_last_checkpoint(4, &executed_checkpoint(4, 41))
            .unwrap();
        assert_eq!(store.current_epoch_start_checkpoint(5).unwrap(), Some(42));

        // The previous epoch's row takes precedence over the map.
        // `complete_epoch_info(4)` has `end_checkpoint == Some(0)` -> start 1.
        store
            .tables
            .epoch_info
            .insert(&4, &complete_epoch_info(4))
            .unwrap();
        assert_eq!(store.current_epoch_start_checkpoint(5).unwrap(), Some(1));
    }

    /// A snapshot restore leaves the previous epoch's closing checkpoint as the
    /// highest executed one; the open epoch is the next one.
    #[tokio::test]
    async fn first_open_epoch_steps_past_a_closing_checkpoint() {
        let store = CheckpointStore::new_for_tests();

        // Nothing executed yet.
        assert_eq!(store.first_open_epoch().unwrap(), None);

        // Mid-epoch checkpoint: its own epoch is still open.
        let mid = executed_checkpoint(3, 10);
        store.insert_verified_checkpoint(&mid).unwrap();
        store.update_highest_executed_checkpoint(&mid).unwrap();
        assert_eq!(store.first_open_epoch().unwrap(), Some(3));

        // Close-of-epoch checkpoint: epoch 3 is closed, epoch 4 is open.
        let closing = closing_checkpoint(3, 11);
        store.insert_verified_checkpoint(&closing).unwrap();
        store.update_highest_executed_checkpoint(&closing).unwrap();
        assert_eq!(store.first_open_epoch().unwrap(), Some(4));
    }

    /// `epoch_info_gap` flags a contiguous prefix that falls short of the last
    /// executed closed epoch — and nothing else: no closed epoch yet and a
    /// backfill seeded past local execution both count as complete.
    #[tokio::test]
    async fn epoch_info_gap_flags_short_prefix_not_overshoot() {
        let store = CheckpointStore::new_for_tests();

        // Nothing executed yet -> nothing to guard.
        assert_eq!(store.epoch_info_gap().unwrap(), None);

        // Genesis epoch still open -> no closed epoch -> no gap.
        let genesis = executed_checkpoint(0, 0);
        store.insert_verified_checkpoint(&genesis).unwrap();
        store.update_highest_executed_checkpoint(&genesis).unwrap();
        assert_eq!(store.epoch_info_gap().unwrap(), None);

        // Executed into epoch 2 (synthetic jump) -> closed epochs [0, 1]; an
        // empty index and a prefix short of epoch 1 are both gaps.
        let in_epoch_2 = executed_checkpoint(2, 1);
        store.insert_verified_checkpoint(&in_epoch_2).unwrap();
        store
            .update_highest_executed_checkpoint(&in_epoch_2)
            .unwrap();
        assert_eq!(store.epoch_info_gap().unwrap(), Some((None, 1)));
        store
            .insert_epoch_info(vec![complete_epoch_info(0)])
            .unwrap();
        assert_eq!(store.epoch_info_gap().unwrap(), Some((Some(0), 1)));

        // Complete through the last closed epoch -> no gap.
        store
            .insert_epoch_info(vec![complete_epoch_info(1)])
            .unwrap();
        assert_eq!(store.epoch_info_gap().unwrap(), None);

        // A backfill seeded beyond local execution is a superset, not a gap.
        store
            .insert_epoch_info(vec![complete_epoch_info(2), complete_epoch_info(3)])
            .unwrap();
        assert_eq!(store.epoch_info_gap().unwrap(), None);
    }

    /// `try_advance_epoch_info_watermark` advances the watermark only when
    /// `prev_epoch` extends the contiguous prefix by exactly one.
    #[tokio::test]
    async fn try_advance_epoch_info_watermark_is_gap_aware() {
        let store = CheckpointStore::new_for_tests();

        let advance = |epoch| {
            let mut batch = store.tables.epoch_info_indexed_watermark.batch();
            store
                .try_advance_epoch_info_watermark(epoch, &mut batch)
                .unwrap();
            batch.write().unwrap();
            store.highest_indexed_epoch().unwrap()
        };

        // From absent: prev_epoch=5 must NOT advance (bootstrap mid-history).
        assert_eq!(advance(5), None);
        // From absent: prev_epoch=0 advances to 0 (genesis close).
        assert_eq!(advance(0), Some(0));
        // From 0: prev_epoch=2 must NOT advance (gap at 1).
        assert_eq!(advance(2), Some(0));
        // From 0: prev_epoch=1 advances to 1.
        assert_eq!(advance(1), Some(1));
        // From 1: prev_epoch=1 again is a no-op (already covered).
        assert_eq!(advance(1), Some(1));
        // From 1: prev_epoch=2 advances to 2.
        assert_eq!(advance(2), Some(2));
    }
}
