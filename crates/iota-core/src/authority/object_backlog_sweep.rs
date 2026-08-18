// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The one-time sweep of the object versions superseded before this build.
//!
//! A superseded version now leaves the live `objects` table in the batch
//! that supersedes it, and arrives in the epoch's historic bucket. A
//! database written by an earlier build still holds roughly one retention
//! window of superseded versions in the live table, and the pruner that used
//! to drain them is gone, so they are walked once and deleted here.
//!
//! They are deleted rather than moved into a bucket:
//! [`HistoricObjects::find_lt_or_eq_version`] searches buckets newest first
//! and takes the first hit, which is only an answer because an object's
//! versions are relocated in increasing version order. A pre-upgrade version
//! put into the current epoch's bucket would sit above versions relocated
//! long before it, and a query bounded above one of those would be answered
//! with the older row.
//!
//! The walk runs while the node executes. A row it deletes is one another
//! row of the same object id sits above, so the transaction that wrote that
//! newer version has its effects committed and no retry needs the older one
//! as an input. A version superseded meanwhile is relocated into the current
//! epoch's bucket and deleted from the live table by the batch that
//! supersedes it, whether or not the walk has passed it.

use std::{
    ops::Bound,
    sync::{Arc, Weak},
    time::Duration,
};

use iota_metrics::spawn_monitored_task;
use iota_types::{committee::EpochId, error::IotaResult, storage::ObjectKey};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use typed_store::traits::Map;

use crate::authority::{
    AuthorityState, AuthorityStore,
    authority_store_tables::AuthorityPerpetualTables,
    authority_store_types::{StoreObject, StoreObjectWrapper},
    historic_objects::HistoricObjects,
};

/// Keys one slice takes on before it writes its batch and yields. A slice
/// runs to the end of the object id it stopped in the middle of, so it can
/// exceed this by the number of versions that one id has.
const KEYS_PER_SLICE: usize = 5_000;

/// Consecutive failed slices tolerated before the sweep leaves the rest to
/// the next node start.
const MAX_CONSECUTIVE_FAILURES: usize = 3;

/// Delay before a failed slice is attempted again.
const RETRY_DELAY: Duration = Duration::from_secs(5);

/// How far the sweep has got through the live `objects` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectBacklogSweepProgress {
    /// Every key up to and including this one has been swept; the next slice
    /// resumes above it.
    SweptThrough(ObjectKey),
    /// The whole table has been swept, and later node starts do nothing.
    Done,
}

/// Starts the sweep of the object versions superseded before this build,
/// unless an earlier run already walked the whole table.
///
/// Both handles are weak, so that a dropped node stops the sweep at its next
/// slice instead of holding its database open: `store` owns the tables being
/// swept, `state` answers which epoch's bucket the tombstones found are
/// recorded in.
pub(crate) fn spawn(state: Weak<AuthorityState>, store: Weak<AuthorityStore>) -> JoinHandle<()> {
    spawn_monitored_task!(sweep_backlog(state, store))
}

async fn sweep_backlog(state: Weak<AuthorityState>, store: Weak<AuthorityStore>) {
    match store
        .upgrade()
        .map(|store| ObjectBacklogSweep::new(&store).done())
    {
        None => return,
        Some(Ok(true)) => return,
        Some(Ok(false)) => {}
        Some(Err(e)) => {
            error!("cannot read how far the object backlog sweep got: {e}");
            return;
        }
    }
    info!("sweeping the object versions superseded before this build out of the live table");

    let mut consecutive_failures = 0;
    loop {
        let (Some(state), Some(store)) = (state.upgrade(), store.upgrade()) else {
            info!("stopping the object backlog sweep: the node is shutting down");
            return;
        };
        // Read afresh per slice: the epoch advances while the sweep runs, and
        // the tombstones a slice finds belong in the bucket of the epoch that
        // is current when the slice writes them. An older bucket may already
        // be past the retention the next reconfiguration applies.
        let epoch = state.load_epoch_store_one_call_per_task().epoch();
        let sweep = ObjectBacklogSweep::new(&store);
        // The slice is a range scan and a write batch, both blocking, and
        // the node is executing meanwhile.
        match tokio::task::spawn_blocking(move || sweep.sweep_slice(epoch)).await {
            Ok(Ok(true)) => {
                consecutive_failures = 0;
                tokio::task::yield_now().await;
            }
            Ok(Ok(false)) => {
                info!("the object backlog sweep reached the end of the live table");
                return;
            }
            Ok(Err(e)) => {
                consecutive_failures += 1;
                if consecutive_failures == MAX_CONSECUTIVE_FAILURES {
                    error!(
                        "leaving the rest of the object backlog to the next node start, which \
                         resumes from the same key: {e}"
                    );
                    return;
                }
                warn!("an object backlog sweep slice failed and is being retried: {e}");
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                error!("the object backlog sweep task failed: {e}");
                return;
            }
        }
    }
}

/// One walk over the live `objects` table, deleting every row that is
/// neither the newest version of its object id nor a tombstone.
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

    /// Whether the walk has already reached the end of the table on this
    /// database.
    fn done(&self) -> IotaResult<bool> {
        Ok(matches!(
            self.perpetual_tables
                .object_backlog_sweep_progress
                .get(&())?,
            Some(ObjectBacklogSweepProgress::Done)
        ))
    }

    /// Sweeps the rows above the recorded key, up to
    /// [`Self::keys_per_slice`], and records how far it got. Returns whether
    /// rows are left to sweep.
    ///
    /// The deletions, the tombstones recorded in `epoch`'s bucket and the
    /// progress row are one batch, so an interrupted run resumes at the key
    /// it last wrote and never skips a row.
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
        // The versions of the object id the scan is currently in, with
        // whether each one is a tombstone. An id's rows are adjacent and
        // ascending in version, since `ObjectKey` orders by both.
        let mut versions: Vec<(ObjectKey, bool)> = Vec::new();
        let mut swept_through = None;
        let mut keys_taken = 0;
        let mut sliced = false;

        for row in objects.safe_range_iter((lower_bound, Bound::Unbounded)) {
            let (key, object) = row?;
            if versions.first().is_some_and(|(first, _)| first.0 != key.0) {
                keys_taken += versions.len();
                swept_through = versions.last().map(|(key, _)| *key);
                Self::classify(&versions, &mut superseded, &mut tombstones);
                versions.clear();
                if keys_taken >= self.keys_per_slice {
                    sliced = true;
                    break;
                }
            }
            versions.push((key, is_tombstone(object)));
        }
        if !sliced && !versions.is_empty() {
            swept_through = versions.last().map(|(key, _)| *key);
            Self::classify(&versions, &mut superseded, &mut tombstones);
        }

        let mut batch = objects.batch();
        batch.delete_batch(objects, superseded.iter().copied())?;
        if !tombstones.is_empty() {
            // Recording a tombstone this far above where it was written is
            // safe: the rows beneath it have just been deleted, and no
            // bucket ever held them, so nothing is left for it to cover.
            // Its own epoch may have recorded it as well, in an older bucket
            // that expires first; deleting it twice is harmless.
            let bucket = self.historic_objects.ensure(epoch)?;
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
            deleted = superseded.len(),
            tombstones = tombstones.len(),
            "swept a slice of the superseded object versions"
        );
        Ok(sliced)
    }

    /// Sorts one object id's versions into the ones to delete and the
    /// tombstones to record, given that `versions` holds every row of that
    /// id in ascending version order.
    ///
    /// A tombstone is kept wherever it sits: an object wrapped and later
    /// unwrapped has one below its newest version, and a bounded read must
    /// still be able to tell that the object was gone at that version.
    fn classify(
        versions: &[(ObjectKey, bool)],
        superseded: &mut Vec<ObjectKey>,
        tombstones: &mut Vec<ObjectKey>,
    ) {
        let newest = versions.len() - 1;
        for (index, (key, tombstone)) in versions.iter().enumerate() {
            if *tombstone {
                tombstones.push(*key);
            } else if index != newest {
                superseded.push(*key);
            }
        }
    }
}

/// Whether a row of the live `objects` table records that the object was
/// deleted or wrapped at that version, rather than holding a version of it.
fn is_tombstone(object: StoreObjectWrapper) -> bool {
    matches!(
        object.migrate().into_inner(),
        StoreObject::Deleted | StoreObject::Wrapped
    )
}

#[cfg(test)]
#[path = "../unit_tests/object_backlog_sweep_tests.rs"]
mod tests;
