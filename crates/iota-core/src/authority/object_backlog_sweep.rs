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

/// Keys one slice decides before it writes its batch and yields. A slice
/// stops at this many wherever it is, including in the middle of an object
/// id's versions, so it bounds what the slice holds in memory whatever the
/// table looks like.
const KEYS_PER_SLICE: usize = 5_000;

/// Delay before a failed slice is attempted again, doubled after each further
/// failure up to [`MAX_RETRY_DELAY`].
const RETRY_DELAY: Duration = Duration::from_secs(5);

/// Longest a failed slice waits before it is attempted again.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(300);

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
/// unless an earlier run already walked the whole table or this node keeps
/// every epoch's superseded versions.
///
/// Both handles are weak, so that a dropped node stops the sweep at its next
/// slice instead of holding its database open: `store` owns the tables being
/// swept, `state` answers which epoch's bucket the tombstones found are
/// recorded in.
pub(crate) fn spawn(
    state: Weak<AuthorityState>,
    store: Weak<AuthorityStore>,
    num_epochs_to_retain: u64,
) -> JoinHandle<()> {
    spawn_monitored_task!(sweep_backlog(state, store, num_epochs_to_retain))
}

async fn sweep_backlog(
    state: Weak<AuthorityState>,
    store: Weak<AuthorityStore>,
    num_epochs_to_retain: u64,
) {
    if num_epochs_to_retain == u64::MAX {
        // This node expires no bucket either, so the backlog is left where it
        // is: the bounded read consults the live table and the buckets
        // together and takes the newer answer, and deleting it would throw
        // away the object history this configuration exists to keep.
        info!(
            "not sweeping the object versions superseded before this build: this node retains \
             every epoch's superseded versions"
        );
        return;
    }
    info!("sweeping the object versions superseded before this build out of the live table");

    // A failing slice is retried for as long as the node runs, and so is the
    // very first one, whose read of how far an earlier run got is otherwise
    // the one failure in this walk with no retry to fall back on. Nothing
    // about a slice makes the next one fail too: it re-reads the epoch, the
    // progress row and the range it works on, and the failure it is likeliest
    // to meet — an epoch refused because reconfiguration has raised the
    // retention floor but not yet swapped the epoch store — lasts only as
    // long as that reopen. Giving up instead would leave the backlog in the
    // live table, and with it every bucket unexpirable until a restart.
    let mut retry_delay = RETRY_DELAY;
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
                retry_delay = RETRY_DELAY;
                tokio::task::yield_now().await;
            }
            Ok(Ok(false)) => {
                info!("the object backlog sweep reached the end of the live table");
                return;
            }
            Ok(Err(e)) => {
                warn!(
                    ?retry_delay,
                    "an object backlog sweep slice failed and is being retried, \
                     which holds up expiring the historic buckets: {e}"
                );
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
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

    /// Sweeps up to [`Self::keys_per_slice`] rows above the recorded key and
    /// records how far it got. Returns whether rows are left to sweep.
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
        // The row the scan has read but not yet decided on, with whether it
        // is a tombstone. One row of lookahead is all a decision needs: a row
        // is superseded exactly when the next row belongs to the same object
        // id, since `ObjectKey` orders by id and then by version.
        let mut undecided: Option<(ObjectKey, bool)> = None;
        let mut swept_through = None;
        let mut decided = 0;
        let mut sliced = false;

        for row in objects.safe_range_iter((lower_bound, Bound::Unbounded)) {
            let (key, object) = row?;
            if let Some((previous, tombstone)) = undecided {
                Self::decide(
                    previous,
                    tombstone,
                    previous.0 == key.0,
                    &mut superseded,
                    &mut tombstones,
                );
                swept_through = Some(previous);
                decided += 1;
                if decided >= self.keys_per_slice {
                    // `key` stays undecided, and the next slice reads it
                    // again: the watermark is `previous`.
                    sliced = true;
                    break;
                }
            }
            undecided = Some((key, is_tombstone(object)));
        }
        if !sliced {
            if let Some((last, tombstone)) = undecided {
                // Nothing follows it, so it is the newest version of its id.
                Self::decide(last, tombstone, false, &mut superseded, &mut tombstones);
                swept_through = Some(last);
            }
        }

        let mut batch = objects.batch();
        batch.delete_batch(objects, superseded.iter().copied())?;
        if !tombstones.is_empty() {
            // Recording a tombstone this far above where it was written is
            // safe: the rows beneath it have just been deleted, and no
            // bucket ever held them, so nothing is left for it to cover.
            // Its own epoch may have recorded it as well, in an older
            // bucket; no bucket expires until this sweep has reached the end
            // of the table, and deleting the same head twice is a no-op.
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

    /// Sorts one row into the ones to delete and the tombstones to record,
    /// given whether a higher version of the same object id follows it.
    ///
    /// A tombstone is kept wherever it sits: an object wrapped and later
    /// unwrapped has one below its newest version, and a bounded read must
    /// still be able to tell that the object was gone at that version.
    fn decide(
        key: ObjectKey,
        tombstone: bool,
        higher_version_follows: bool,
        superseded: &mut Vec<ObjectKey>,
        tombstones: &mut Vec<ObjectKey>,
    ) {
        if tombstone {
            tombstones.push(key);
        } else if higher_version_follows {
            superseded.push(key);
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
