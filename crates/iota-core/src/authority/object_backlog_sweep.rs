// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO(https://github.com/iotaledger/iota/issues/12712): remove this module
// once every database has swept the pre-bucket backlog.

//! The one-time sweep of the object versions superseded before this build.
//!
//! A superseded version now leaves the live `objects` table in the batch
//! that supersedes it, and arrives in the epoch's historic bucket. A
//! database written by an earlier build still holds roughly one retention
//! window of superseded versions in the live table, and the pruner that used
//! to drain them is gone, so they are walked once and relocated here, into
//! the bucket of the epoch the walk runs in.
//!
//! They go into that bucket even though they are older than the versions an
//! earlier epoch's bucket holds. [`HistoricObjects::find_lt_or_eq_version`]
//! searches buckets newest first and takes the first hit, so what would give
//! a wrong answer is a newer bucket holding a lower version of the same
//! object. This walk cannot produce one: it finishes before the node executes
//! anything, so no bucket holds a version a commit relocated, and every
//! version the walk itself relocates lands in that one bucket, where order
//! does not matter, since a bucket is searched by a reverse range scan that
//! takes the newest version under the bound.
//!
//! The epoch the walk runs in rather than an older one, because with a
//! retention of `N` epochs at epoch `E` the oldest bucket kept after the next
//! boundary is `E - N + 1`: an older bucket would be dropped one boundary
//! later and take history with it that the node could otherwise still serve.
//! The current epoch's bucket gives these versions the whole retention
//! window, and retaining them for up to one window too long is the harmless
//! direction.
//!
//! The walk runs at node startup, before any service that could expire a
//! historic bucket has started. A bucket's tombstone heads may only be
//! deleted once every version beneath them is out of reach; the heads this
//! walk records land in the same bucket as the versions it relocates, so the
//! two expire together, but a version superseded before this build sits in
//! the live table until the walk reaches it, and finishing the walk first is
//! what keeps an expiry from leaving such a version as the newest row of a
//! deleted object.

use std::{ops::Bound, sync::Arc};

use iota_types::{
    committee::EpochId,
    error::{IotaError, IotaResult},
    object::Object,
    storage::ObjectKey,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use typed_store::traits::Map;

use crate::authority::{
    AuthorityStore,
    authority_store_tables::AuthorityPerpetualTables,
    authority_store_types::{StoreObject, StoreObjectWrapper, try_construct_object},
    historic_objects::HistoricObjects,
};

/// Keys one slice decides before it writes its batch. A slice stops at this
/// many wherever it is, including in the middle of an object id's versions, so
/// it bounds how many versions the slice holds in memory whatever the table
/// looks like, and bounds what an interrupted run has to walk again.
const KEYS_PER_SLICE: usize = 5_000;

/// How far the sweep has got through the live `objects` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectBacklogSweepProgress {
    /// Every key up to and including this one has been swept; the next slice
    /// resumes above it.
    SweptThrough(ObjectKey),
    /// The whole table has been swept, and later node starts do nothing.
    Done,
}

/// Walks the whole live `objects` table, relocating the versions superseded
/// before this build into `epoch`'s bucket and recording the tombstones it
/// finds there too. Returns once nothing of that backlog is left, at once on a
/// database an earlier run already walked through.
///
/// Call this before starting anything that can expire a historic bucket, and
/// before anything that scans the live table for its latest versions: until it
/// returns, that table holds a retention window of rows no reader wants.
///
/// A failure is returned rather than retried, since nothing that comes after
/// may run until the walk is finished. The watermark it records is durable, so
/// the next start resumes where this one stopped.
pub async fn sweep(store: Arc<AuthorityStore>, epoch: EpochId) -> IotaResult<()> {
    info!("sweeping the object versions superseded before this build out of the live table");

    // Each slice is a range scan and a write batch, both blocking.
    tokio::task::spawn_blocking(move || {
        let sweep = ObjectBacklogSweep::new(&store);
        while sweep.sweep_slice(epoch)? {}
        info!("the object backlog sweep reached the end of the live table");
        IotaResult::Ok(())
    })
    .await
    .map_err(|e| IotaError::Storage(format!("the object backlog sweep task failed: {e}")))?
}

/// One walk over the live `objects` table, relocating every row that is
/// neither the newest version of its object id nor a tombstone into the
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

    /// Sweeps up to [`Self::keys_per_slice`] rows above the recorded key and
    /// records how far it got. Returns whether rows are left to sweep.
    ///
    /// Each relocated version's insert into `epoch`'s bucket and its delete
    /// from the live table are one batch, together with the tombstones
    /// recorded in that bucket and the progress row: a crash leaves every
    /// version in one of the two tables, and an interrupted run resumes at
    /// the key it last wrote and never skips a row.
    fn sweep_slice(&self, epoch: EpochId) -> IotaResult<bool> {
        let objects = &self.perpetual_tables.objects;
        let progress = &self.perpetual_tables.object_backlog_sweep_progress;
        let lower_bound = match progress.get(&())? {
            Some(ObjectBacklogSweepProgress::Done) => return Ok(false),
            Some(ObjectBacklogSweepProgress::SweptThrough(key)) => Bound::Excluded(key),
            None => Bound::Unbounded,
        };

        let mut superseded = Vec::new();
        let mut tombstones = Vec::new();
        // The row the scan has read but not yet decided on. One row of
        // lookahead is all a decision needs: a row is superseded exactly when
        // the next row belongs to the same object id, since `ObjectKey` orders
        // by id and then by version.
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
            // Recording a tombstone this far above where it was written is
            // safe: the versions beneath it go into the same bucket, in this
            // batch or an earlier slice's, so they are out of reach as soon
            // as the head is. Its own epoch may have recorded it as well, in
            // an older bucket; neither bucket can expire before this walk is
            // over, and deleting the same head twice is a no-op.
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
        Ok(sliced)
    }

    /// Sorts one row into the versions to relocate and the tombstones to
    /// record, given whether a higher version of the same object id follows
    /// it.
    ///
    /// A tombstone is kept wherever it sits: an object wrapped and later
    /// unwrapped has one below its newest version, and a bounded read must
    /// still be able to tell that the object was gone at that version.
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
