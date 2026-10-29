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

use std::{
    ops::Bound,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::VersionNumber,
    committee::EpochId,
    error::{IotaError, IotaResult},
    object::Object,
    storage::ObjectKey,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::debug;
use typed_store::{rocks::DBBatch, traits::Map};

use crate::{
    authority::{
        AuthorityStore,
        authority_store_tables::AuthorityPerpetualTables,
        authority_store_types::{StoreObject, StoreObjectWrapper, try_construct_object},
        historic_objects::HistoricObjects,
        ledger_backlog_migration::RangeProgress,
    },
    par_index_live_object_set::id_position,
    progress_logger::ProgressLogger,
};

/// Keys one slice decides before it writes its batch, even in the middle of an
/// object id's versions, bounding its memory and what an interrupted run walks
/// again.
const KEYS_PER_SLICE: usize = 5_000;

/// How many key ranges the walk of the whole live table splits it into, by
/// the first byte of the object id; each range is walked by its own thread.
/// All versions of an object share its id, so they fall in one range.
const WALK_RANGES: usize = 32;

/// How far the sweep has got through the live `objects` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectBacklogSweepProgress {
    /// How far each key range of the walk of the whole live table has got:
    /// every key up to and including a range's position has been swept.
    Walking(Vec<RangeProgress<ObjectKey>>),
    /// The sweep has finished, and later node starts do nothing.
    Done,
    /// Recorded without a sweep, by `iota-tool db-tool
    /// mark-object-backlog-swept`. Later node starts do nothing, but the
    /// versions a sweep had not reached stay in the live table.
    Skipped,
}

/// Moves the superseded object versions left in the live `objects` table into
/// `epoch`'s bucket, and records the tombstones alongside them. Returns at once
/// if an earlier run finished.
///
/// Call this before starting anything that scans the live table for its
/// latest versions or can expire a historic bucket: an expiry could otherwise
/// leave a superseded version as the newest row of a deleted object.
///
/// # Errors
///
/// A failure is returned rather than retried. Progress is durable, so the next
/// start resumes where this one stopped.
pub async fn sweep(store: Arc<AuthorityStore>, epoch: EpochId) -> IotaResult<()> {
    // Each slice is a range scan and a write batch, both blocking.
    tokio::task::spawn_blocking(move || {
        let sweep = ObjectBacklogSweep::new(&store);
        if sweep.is_done()? {
            return IotaResult::Ok(());
        }
        sweep.walk_in_parallel(epoch)
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

    /// Whether an earlier run already finished.
    fn is_done(&self) -> IotaResult<bool> {
        Ok(matches!(
            self.perpetual_tables
                .object_backlog_sweep_progress
                .get(&())?,
            Some(ObjectBacklogSweepProgress::Done | ObjectBacklogSweepProgress::Skipped)
        ))
    }

    fn mark_done(&self, epoch: EpochId) -> IotaResult<()> {
        self.perpetual_tables
            .mark_object_backlog_swept_into(epoch)?;
        self.historic_objects.record_backlog_sweep_epoch(epoch);
        self.historic_objects.mark_backlog_swept();
        Ok(())
    }

    /// Walks the whole live table, in [`WALK_RANGES`] key ranges with a thread
    /// per range, resuming each range from its recorded position. Each slice
    /// records every range's position in the batch of its rows; the writes
    /// are serialized, so the progress row always holds every range's latest
    /// written position.
    fn walk_in_parallel(&self, epoch: EpochId) -> IotaResult<()> {
        let ranges = match self
            .perpetual_tables
            .object_backlog_sweep_progress
            .get(&())?
        {
            Some(ObjectBacklogSweepProgress::Done | ObjectBacklogSweepProgress::Skipped) => {
                return Ok(());
            }
            Some(ObjectBacklogSweepProgress::Walking(ranges)) if ranges.len() == WALK_RANGES => {
                ranges
            }
            // A different number of ranges restarts the walk, which is safe:
            // a relocated version is gone from the live table, and the rows
            // walked again are decided the same way.
            _ => vec![RangeProgress::At(None); WALK_RANGES],
        };
        let logger = Mutex::new(
            ProgressLogger::new(
                "object backlog sweep",
                "objects",
                self.perpetual_tables.objects.estimated_len()?,
            )
            .measured_by_position(walked_fraction(&ranges)),
        );
        let ranges = Mutex::new(ranges);
        let failed = AtomicBool::new(false);
        // The database starts tasks on the caller's runtime, e.g. to sample
        // iterator metrics.
        let runtime = tokio::runtime::Handle::try_current().ok();

        std::thread::scope(|scope| {
            let walkers: Vec<_> = (0..WALK_RANGES)
                .map(|range| {
                    let (ranges, logger, failed, runtime) = (&ranges, &logger, &failed, &runtime);
                    scope.spawn(move || {
                        let _runtime = runtime.as_ref().map(|runtime| runtime.enter());
                        let result = self.walk_range(range, epoch, ranges, logger, failed);
                        if result.is_err() {
                            failed.store(true, Ordering::Relaxed);
                        }
                        result
                    })
                })
                .collect();
            walkers.into_iter().try_for_each(|walker| {
                walker.join().unwrap_or_else(|_| {
                    Err(IotaError::Storage(
                        "an object backlog sweep thread panicked".to_string(),
                    ))
                })
            })
        })?;
        logger.into_inner().finish();
        self.mark_done(epoch)
    }

    /// Walks key range `range` one slice at a time, stopping early if another
    /// range fails.
    fn walk_range(
        &self,
        range: usize,
        epoch: EpochId,
        ranges: &Mutex<Vec<RangeProgress<ObjectKey>>>,
        logger: &Mutex<ProgressLogger>,
        failed: &AtomicBool,
    ) -> IotaResult<()> {
        loop {
            let RangeProgress::At(from) = ranges.lock()[range] else {
                return Ok(());
            };
            if failed.load(Ordering::Relaxed) {
                return Ok(());
            }
            let (mut batch, decided, next) = self.sweep_slice(range, from, epoch)?;
            let mut positions = ranges.lock();
            let mut written = positions.clone();
            written[range] = next;
            batch.insert_batch(
                &self.perpetual_tables.object_backlog_sweep_progress,
                [((), ObjectBacklogSweepProgress::Walking(written.clone()))],
            )?;
            batch.write()?;
            // Only once written, or another range could record a position
            // whose rows a failed write left behind.
            *positions = written;
            let walked = walked_fraction(&positions);
            drop(positions);
            let mut logger = logger.lock();
            logger.set_position(walked);
            logger.advance(decided as u64);
        }
    }

    /// Builds the batch sweeping up to [`Self::keys_per_slice`] rows of key
    /// range `range` above `from`. Returns it with how many rows it decided
    /// and where the range stands once it is written.
    fn sweep_slice(
        &self,
        range: usize,
        from: Option<ObjectKey>,
        epoch: EpochId,
    ) -> IotaResult<(DBBatch, usize, RangeProgress<ObjectKey>)> {
        let objects = &self.perpetual_tables.objects;
        let (range_start, range_end) = walk_range_bounds(range);
        let lower_bound = from.map_or(Bound::Included(range_start), Bound::Excluded);

        let mut superseded = Vec::new();
        let mut tombstones = Vec::new();
        // `ObjectKey` orders by id and then version, so a row is superseded
        // exactly when the next row has the same object id.
        let mut undecided: Option<(ObjectKey, StoreObjectWrapper)> = None;
        let mut swept_through = None;
        let mut decided = 0;
        let mut sliced = false;

        for row in objects.safe_range_iter((lower_bound, range_end)) {
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
                // Nothing follows it in the range, which ends between two
                // object ids, so it is the newest version of its id.
                Self::decide(last, row, false, &mut superseded, &mut tombstones)?;
                decided += 1;
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
        let next = match swept_through {
            Some(key) if sliced => RangeProgress::At(Some(key)),
            _ => RangeProgress::Done,
        };

        debug!(
            relocated,
            tombstones = tombstones.len(),
            "swept a slice of the superseded object versions"
        );
        Ok((batch, decided, next))
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

/// The keys of walk range `range`: those whose object id's first byte is in
/// its share of `0..=255`.
fn walk_range_bounds(range: usize) -> (ObjectKey, Bound<ObjectKey>) {
    let first_key = |range: usize| {
        let mut bytes = [0; ObjectId::LENGTH];
        bytes[0] = u8::try_from(range * 256 / WALK_RANGES).expect("below 256");
        ObjectKey(ObjectId::new(bytes), VersionNumber::from_u64(0))
    };
    let end = if range + 1 < WALK_RANGES {
        Bound::Excluded(first_key(range + 1))
    } else {
        Bound::Unbounded
    };
    (first_key(range), end)
}

/// How much of the live table the walk has covered, from 0 to 1, by each
/// range's place in its share of the object id space. Unlike a key count,
/// this holds on a table full of deletions and after a restart.
fn walked_fraction(ranges: &[RangeProgress<ObjectKey>]) -> f64 {
    let span = (1u64 << 64u32.saturating_sub(WALK_RANGES.ilog2())) as f64;
    let sum: f64 = ranges
        .iter()
        .enumerate()
        .map(|(range, progress)| match progress {
            RangeProgress::Done => 1.0,
            RangeProgress::At(None) => 0.0,
            RangeProgress::At(Some(key)) => {
                let start = id_position(&walk_range_bounds(range).0.0);
                (id_position(&key.0).saturating_sub(start) as f64 / span).clamp(0.0, 1.0)
            }
        })
        .sum();
    sum / ranges.len().max(1) as f64
}

#[cfg(test)]
#[path = "../unit_tests/object_backlog_sweep_tests.rs"]
mod tests;
