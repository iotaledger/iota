// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO(https://github.com/iotaledger/iota/issues/12712): remove once swept.

//! One-time sweep of the superseded object versions left in the live
//! `objects` table of a database without historic buckets. They are moved
//! into the bucket of the epoch the sweep runs in, which keeps them for the
//! whole retention window.
//!
//! Putting older versions in the newest bucket is safe:
//! [`HistoricObjects::find_lt_or_eq_version`] searches buckets newest first,
//! which goes wrong only if a newer bucket holds a lower version of an object.
//! The sweep finishes before the node executes anything, so no other bucket
//! holds a version relocated by a commit yet.

use std::{ops::Bound, sync::Arc};

use iota_types::{
    committee::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber},
    object::Object,
    storage::ObjectKey,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use typed_store::traits::Map;

use crate::{
    authority::{
        AuthorityStore,
        authority_store_tables::AuthorityPerpetualTables,
        authority_store_types::{StoreObject, StoreObjectWrapper, try_construct_object},
        historic_objects::HistoricObjects,
    },
    checkpoints::CheckpointStore,
    progress_logger::ProgressLogger,
};

/// Keys one slice decides before it writes its batch, even in the middle of an
/// object id's versions, bounding its memory and what an interrupted run walks
/// again.
const KEYS_PER_SLICE: usize = 5_000;

/// Checkpoints one slice of the bounded walk resolves before it writes its
/// batch, bounding what an interrupted run has to replay.
const CHECKPOINTS_PER_SLICE: u64 = 200;

/// How far the sweep has got through the live `objects` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectBacklogSweepProgress {
    /// Every key up to and including this one has been swept; the next slice
    /// resumes above it.
    SweptThrough(ObjectKey),
    /// The sweep has finished, by either route, and later node starts do
    /// nothing.
    Done,
}

/// Moves the superseded object versions left in the live `objects` table into
/// `epoch`'s bucket, and records the tombstones alongside them. Returns at once
/// if an earlier run finished.
///
/// Where the objects pruner left its watermark, only the effects of the
/// checkpoints above it are read; otherwise the whole live table is walked.
///
/// Call this before starting anything that scans the live table for its latest
/// versions or can expire a historic bucket: an expiry could otherwise leave a
/// superseded version as the newest row of a deleted object.
///
/// # Errors
///
/// A failure is returned rather than retried. Progress is durable, so the next
/// start resumes where this one stopped.
pub async fn sweep(
    store: Arc<AuthorityStore>,
    checkpoint_store: Arc<CheckpointStore>,
    epoch: EpochId,
) -> IotaResult<()> {
    // Each slice is a range scan and a write batch, both blocking.
    tokio::task::spawn_blocking(move || {
        let sweep = ObjectBacklogSweep::new(&store);
        if sweep.is_done()? {
            return IotaResult::Ok(());
        }
        match sweep.bound(&checkpoint_store)? {
            Some(bound) => {
                sweep.sweep_above_bound(&checkpoint_store, epoch, bound)?;
            }
            None => {
                let mut progress = ProgressLogger::new(
                    "object backlog sweep",
                    "objects",
                    sweep.perpetual_tables.objects.estimated_len()?,
                );
                loop {
                    let (decided, more) = sweep.sweep_slice(epoch)?;
                    progress.advance(decided as u64);
                    if !more {
                        break;
                    }
                }
                progress.finish();
            }
        }
        IotaResult::Ok(())
    })
    .await
    .map_err(|e| IotaError::Storage(format!("the object backlog sweep task failed: {e}")))?
}

/// Moves every superseded version out of the live `objects` table into the
/// epoch's bucket.
struct ObjectBacklogSweep {
    perpetual_tables: Arc<AuthorityPerpetualTables>,
    historic_objects: Arc<HistoricObjects>,
    keys_per_slice: usize,
}

impl ObjectBacklogSweep {
    fn new(store: &AuthorityStore) -> Self {
        Self {
            perpetual_tables: store.perpetual_tables.clone(),
            historic_objects: store.get_historic_objects().clone(),
            keys_per_slice: KEYS_PER_SLICE,
        }
    }

    /// Whether an earlier run already finished, whichever route it took.
    fn is_done(&self) -> IotaResult<bool> {
        Ok(matches!(
            self.perpetual_tables
                .object_backlog_sweep_progress
                .get(&())?,
            Some(ObjectBacklogSweepProgress::Done)
        ))
    }

    /// The checkpoint above which the backlog can still hold a superseded
    /// version, `None` when the whole live table has to be walked instead.
    fn bound(
        &self,
        checkpoint_store: &CheckpointStore,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        let Some(bound) = self.perpetual_tables.object_backlog_sweep_bound.get(&())? else {
            return Ok(None);
        };
        // The checkpoints above the bound name the backlog, so all of them must
        // still be here. The checkpoint pruner may have outpaced the objects
        // pruner, and a version no remaining checkpoint names would be missed.
        let pruned = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .unwrap_or(0);
        if bound < pruned {
            warn!(
                bound,
                pruned,
                "the checkpoints above the objects pruner's watermark have themselves been \
                 pruned, so they no longer name the backlog; walking the whole live table"
            );
            return Ok(None);
        }
        Ok(Some(bound))
    }

    /// Relocates the versions the checkpoints above `bound` superseded, and
    /// records the tombstones they wrote. Every version superseded at or below
    /// `bound` is already gone.
    ///
    /// A pre-image the live table no longer holds is skipped: a checkpoint may
    /// be replayed across the watermark, or an earlier slice relocated it.
    fn sweep_above_bound(
        &self,
        checkpoint_store: &CheckpointStore,
        epoch: EpochId,
        bound: CheckpointSequenceNumber,
    ) -> IotaResult<()> {
        // Walk up to the synced watermark, not the executed one: a crash
        // between committing a checkpoint's effects and bumping
        // `HighestExecuted` leaves superseded versions above the executed
        // watermark. A checkpoint not executed yet has no effects to read.
        let executed = checkpoint_store.get_highest_executed_checkpoint_seq_number()?;
        let synced = checkpoint_store.get_highest_synced_checkpoint_seq_number()?;
        let Some(highest) = synced.max(executed) else {
            // Nothing has been executed or synced, so nothing can have been
            // superseded.
            return self.mark_done();
        };
        let resumed = self
            .perpetual_tables
            .object_backlog_sweep_checkpoint
            .get(&())?;
        let mut next = resumed.unwrap_or(bound).saturating_add(1);
        let mut progress = ProgressLogger::new(
            "object backlog sweep above the objects pruner's watermark",
            "checkpoints",
            highest.saturating_sub(next).saturating_add(1),
        );
        while next <= highest {
            let last = highest.min(next.saturating_add(CHECKPOINTS_PER_SLICE - 1));
            let ended_at = self.sweep_checkpoint_slice(checkpoint_store, epoch, next, last)?;
            progress.advance(ended_at - next + 1);
            next = ended_at.saturating_add(1);
        }
        progress.finish();
        self.mark_done()
    }

    /// Relocates the versions superseded by the checkpoints in `first..=last`,
    /// recording progress in the same batch. Returns the last checkpoint
    /// swept, which is below `last` if the slice reached the row cap.
    fn sweep_checkpoint_slice(
        &self,
        checkpoint_store: &CheckpointStore,
        epoch: EpochId,
        first: CheckpointSequenceNumber,
        last: CheckpointSequenceNumber,
    ) -> IotaResult<CheckpointSequenceNumber> {
        let objects = &self.perpetual_tables.objects;
        let mut superseded = Vec::new();
        let mut tombstones = Vec::new();
        // One checkpoint can supersede any number of versions, so also stop
        // at the first checkpoint boundary past the row cap.
        let mut ended_at = last;
        for sequence_number in first..=last {
            if superseded.len() + tombstones.len() >= self.keys_per_slice {
                ended_at = sequence_number - 1;
                break;
            }
            let Some(summary) =
                checkpoint_store.get_checkpoint_by_sequence_number(sequence_number)?
            else {
                // `bound` guarantees the range was not pruned, so a missing
                // checkpoint is one the node never had.
                continue;
            };
            let Some(contents) =
                checkpoint_store.get_checkpoint_contents(&summary.contents_digest)?
            else {
                continue;
            };
            for digests in contents.iter() {
                let Some(effects) = self.perpetual_tables.effects.get(&digests.effects)? else {
                    continue;
                };
                for modified in effects.modified_at_versions() {
                    let key = ObjectKey(*modified.object_id(), modified.version());
                    let Some(row) = objects.get(&key)? else {
                        continue;
                    };
                    if let StoreObject::Value(value) = row.migrate().into_inner() {
                        superseded.push((key, try_construct_object(&key, *value)?));
                    }
                }
                for (object_id, version) in effects.all_tombstones() {
                    tombstones.push(ObjectKey(object_id, version));
                }
            }
        }

        let relocated = superseded.len();
        let mut batch = objects.batch();
        if !superseded.is_empty() || !tombstones.is_empty() {
            let bucket = self.historic_objects.ensure(epoch)?;
            let keys: Vec<ObjectKey> = superseded.iter().map(|(key, _)| *key).collect();
            batch.insert_batch_tagged(&bucket.objects, superseded)?;
            batch.delete_batch(objects, keys)?;
            batch
                .insert_batch_tagged(&bucket.tombstones, tombstones.iter().map(|key| (*key, ())))?;
        }
        batch.insert_batch(
            &self.perpetual_tables.object_backlog_sweep_checkpoint,
            [((), ended_at)],
        )?;
        batch.write()?;

        debug!(
            first,
            last = ended_at,
            relocated,
            tombstones = tombstones.len(),
            "swept the superseded versions of a slice of checkpoints"
        );
        Ok(ended_at)
    }

    fn mark_done(&self) -> IotaResult<()> {
        self.perpetual_tables.mark_object_backlog_swept()
    }

    /// Sweeps up to [`Self::keys_per_slice`] rows above the recorded key, in
    /// one batch with the progress row. Returns how many rows it decided and
    /// whether any are left to sweep.
    fn sweep_slice(&self, epoch: EpochId) -> IotaResult<(usize, bool)> {
        let objects = &self.perpetual_tables.objects;
        let progress = &self.perpetual_tables.object_backlog_sweep_progress;
        let lower_bound = match progress.get(&())? {
            Some(ObjectBacklogSweepProgress::Done) => return Ok((0, false)),
            Some(ObjectBacklogSweepProgress::SweptThrough(key)) => Bound::Excluded(key),
            None => Bound::Unbounded,
        };

        let mut superseded = Vec::new();
        let mut tombstones = Vec::new();
        // `ObjectKey` orders by id and then version, so a row is superseded
        // exactly when the next row has the same object id.
        let mut undecided: Option<(ObjectKey, StoreObjectWrapper)> = None;
        let mut swept_through = None;
        let mut decided = 0;
        let mut sliced = false;

        for row in objects.safe_range_iter((lower_bound, Bound::Unbounded)) {
            let (key, object) = row?;
            if let Some((previous, row)) = undecided.take() {
                Self::decide(
                    previous,
                    row,
                    previous.0 == key.0,
                    &mut superseded,
                    &mut tombstones,
                )?;
                swept_through = Some(previous);
                decided += 1;
                if decided >= self.keys_per_slice {
                    // `key` stays undecided, and the next slice reads it
                    // again: the watermark is `previous`.
                    sliced = true;
                    break;
                }
            }
            undecided = Some((key, object));
        }
        if !sliced {
            if let Some((last, row)) = undecided {
                // Nothing follows it, so it is the newest version of its id.
                Self::decide(last, row, false, &mut superseded, &mut tombstones)?;
                swept_through = Some(last);
            }
        }

        let relocated = superseded.len();
        let mut batch = objects.batch();
        if !superseded.is_empty() || !tombstones.is_empty() {
            let bucket = self.historic_objects.ensure(epoch)?;
            let keys: Vec<ObjectKey> = superseded.iter().map(|(key, _)| *key).collect();
            batch.insert_batch_tagged(&bucket.objects, superseded)?;
            batch.delete_batch(objects, keys)?;
            // Recording a tombstone in a later bucket than its own epoch's is
            // safe: the versions beneath it go into this same bucket, so they
            // expire together.
            batch
                .insert_batch_tagged(&bucket.tombstones, tombstones.iter().map(|key| (*key, ())))?;
        }
        let recorded = match swept_through {
            Some(key) if sliced => ObjectBacklogSweepProgress::SweptThrough(key),
            _ => ObjectBacklogSweepProgress::Done,
        };
        batch.insert_batch(progress, [((), recorded)])?;
        batch.write()?;

        debug!(
            relocated,
            tombstones = tombstones.len(),
            "swept a slice of the superseded object versions"
        );
        Ok((decided, sliced))
    }

    /// Sorts one row into the versions to relocate and the tombstones to
    /// record.
    ///
    /// A tombstone stays in the live table even below an object's newest
    /// version: an object wrapped and later unwrapped has one there, and a
    /// bounded read must still see that the object was gone at that version.
    fn decide(
        key: ObjectKey,
        row: StoreObjectWrapper,
        higher_version_follows: bool,
        superseded: &mut Vec<(ObjectKey, Object)>,
        tombstones: &mut Vec<ObjectKey>,
    ) -> IotaResult<()> {
        match row.migrate().into_inner() {
            StoreObject::Deleted | StoreObject::Wrapped => tombstones.push(key),
            StoreObject::Value(value) if higher_version_follows => {
                superseded.push((key, try_construct_object(&key, *value)?));
            }
            StoreObject::Value(_) => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../unit_tests/object_backlog_sweep_tests.rs"]
mod tests;
