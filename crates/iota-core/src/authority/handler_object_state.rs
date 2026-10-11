// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch object bookkeeping for P-COOL deterministic post-consensus
//! validation: object state as of the commit height being processed by the
//! consensus handler, independent of local execution progress.
//!
//! The bookkeeping is one epoch table of handler-processed rows plus an
//! in-memory overlay holding the rows not yet durable: every version a commit
//! the handler has processed produced, one row per (object, version), read by
//! the exact version a transaction names, so writes need no ordering among
//! themselves.
//!
//! On a committee validator every execution belongs to a commit the handler
//! has processed: the checkpoint executor runs a transaction only after the
//! handler has assigned its commit, until the node switches to executing
//! synced checkpoints for the rest of the epoch, which stops the handler.
//! The change-epoch transaction and genesis run outside any commit and write
//! no rows; what they write is the next epoch's start state.
//!
//! [`HandlerObjectState`] owns the overlay, the transaction-key ->
//! commit-index map, and every invariant on them: overlay-first reads, and
//! eviction only after the corresponding table row is durable.
//!
//! The in-memory state changes at these points:
//! - the handler registers a commit's kept transaction keys before they can be scheduled
//!   ([`HandlerObjectState::assign_commit_to_transactions`], via the epoch store);
//! - each execution of a registered key puts its rows in the overlay before its outputs become
//!   readable ([`HandlerObjectState::record_executed_transaction`]);
//! - a commit is completed, by the watcher once it is fully executed
//!   ([`HandlerObjectState::record_commit_fully_executed`]) or by its quarantine flush when that
//!   comes first ([`HandlerObjectState::complete_commit_at_flush`]). Either way the commit's rows
//!   go into the overlay and its map entries are dropped;
//! - the two eviction methods below clear overlay entries once their rows are durable.
//!
//! Completing a commit also raises the highest fully executed commit
//! ([`HandlerObjectState::wait_for_fully_executed_commit`]), the one value
//! this module publishes outward: validation of a commit waits on it, so that
//! it reads rows no execution can still add to.
//!
//! Durable writes happen at exactly two trigger points; everything else the
//! module does is in-memory plus point reads:
//! - the quarantine flush of a commit's output (once its checkpoint is certified and executed):
//!   inserts the commit's rows not yet durable atomically with `last_consensus_stats`
//!   ([`HandlerObjectState::write_commit_rows_to_batch`]). For a commit the watcher completed these
//!   are the rows still in the overlay ([`HandlerObjectState::take_completed_commit_rows`]);
//!   otherwise all of the commit's rows, derived from its effects;
//! - the checkpoint executor's batch for every executed checkpoint, durable before the checkpoint's
//!   outputs and its watermark bump: inserts the rows its executions left in the overlay
//!   ([`HandlerObjectState::write_checkpoint_rows_to_batch`]), so an object is never durable
//!   without its row. Rows never change once written for a key, so this batch takes no lock.
//!
//! Both batches go through `DBBatch::write`, which does not fsync. Every
//! ordering this module relies on between a durable write and what follows
//! it - write-then-evict, the effects in the perpetual store before the rows
//! derived from them - holds across a process crash, where the operating
//! system keeps the written data, and not across a power loss or kernel
//! crash, where either store may lose its tail independently of the other.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::Arc,
};

use dashmap::DashMap;
use iota_common::{debug_fatal, fatal, random_util::randomize_cache_capacity_in_tests};
use iota_metrics::monitored_mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use iota_sdk_types::{
    ObjectDigest, ObjectId, OwnedObjectReference, Owner, TransactionEffects, Version, WriteKind,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaResult,
    storage::ObjectKey,
    transaction::TransactionKey,
};
use itertools::chain;
use moka::{policy::EvictionPolicy, sync::SegmentedCache as MokaCache};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use typed_store::{Map, rocks::DBBatch};

use crate::{
    authority::authority_per_epoch_store::AuthorityEpochTables, epoch::epoch_metrics::EpochMetrics,
};

/// Position of a consensus commit in the epoch's commit sequence: dense,
/// starting at 1, identical on every validator. The node-side counterpart of
/// consensus's `CommitIndex`, widened like `ExecutionIndices::sub_dag_index`
/// so this module's persisted rows do not depend on the consensus crate's
/// representation.
pub type CommitIndex = u64;

/// Whether a handler-processed row describes a live object or a tombstone at a
/// given commit index.
///
/// `Deleted` and `Wrapped` yield the same drop verdict; the split mirrors the
/// store's two tombstone digests and their different futures - a wrapped
/// object can reappear via unwrap at a higher version, a deleted one never
/// does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandlerProcessedObjectKind {
    Live,
    Deleted,
    Wrapped,
}

/// One version an object took at a commit the consensus handler processed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerProcessedObject {
    pub digest: ObjectDigest,
    pub kind: HandlerProcessedObjectKind,
    /// Index of the commit whose execution produced this row; reads at a
    /// commit treat rows above that commit's validation horizon as missing.
    pub produced_at: CommitIndex,
    /// The initial shared version on a shared object's creation row and on
    /// its `Deleted` row. `None` on every other row. The shared-input checks
    /// read it as the created-shared flag and to verify a declared version.
    pub initial_shared_version: Option<Version>,
    /// The version of the object this row's write replaced: the consumed
    /// input of a mutation, deletion or wrap. `None` for a creation or an
    /// unwrap.
    pub replaced_version: Option<Version>,
}

/// The rows a commit's executed transactions produce. Both writers of durable
/// rows - the execution watcher and the quarantine flush - derive them here,
/// so the two cannot drift apart.
pub fn handler_rows_for_commit<'a>(
    effects: impl IntoIterator<Item = &'a TransactionEffects>,
    index: CommitIndex,
) -> Vec<(ObjectKey, HandlerProcessedObject)> {
    effects
        .into_iter()
        .flat_map(|effects| handler_processed_upserts(effects, index))
        .collect()
}

/// Derives the handler-processed upserts for one executed transaction of the
/// commit at `produced_at`: created / mutated / unwrapped objects become
/// `Live` rows, deletions and wraps become tombstone rows. A deleted shared
/// object's row keeps its initial shared version, taken from the consumed
/// input's owner.
pub fn handler_processed_upserts(
    effects: &TransactionEffects,
    produced_at: CommitIndex,
) -> Vec<(ObjectKey, HandlerProcessedObject)> {
    let changed = effects.all_changed_objects();
    let old_objects: BTreeMap<ObjectId, (Version, Owner)> = effects
        .old_object_metadata()
        .into_iter()
        .map(|owned_ref| {
            (
                owned_ref.reference().object_id,
                (owned_ref.reference().version, *owned_ref.owner()),
            )
        })
        .collect();

    let mut rows = Vec::with_capacity(changed.len());
    for (owned_ref, write_kind) in changed {
        if !needs_handler_row(write_kind, &owned_ref) {
            continue;
        }
        // A shared object's creation row carries its initial shared version -
        // the created-shared flag the shared-input checks read.
        let initial_shared_version = match *owned_ref.owner() {
            Owner::Shared(initial) => Some(initial),
            _ => None,
        };
        let replaced_version = match write_kind {
            WriteKind::Mutate => old_objects
                .get(&owned_ref.reference().object_id)
                .map(|(version, _)| *version),
            _ => None,
        };
        rows.push((
            ObjectKey(
                owned_ref.reference().object_id,
                owned_ref.reference().version,
            ),
            HandlerProcessedObject {
                digest: owned_ref.reference().digest,
                kind: HandlerProcessedObjectKind::Live,
                produced_at,
                initial_shared_version,
                replaced_version,
            },
        ));
    }
    let deleted = chain(effects.deleted(), effects.unwrapped_then_deleted())
        .map(|reference| (reference, HandlerProcessedObjectKind::Deleted));
    let wrapped = effects
        .wrapped()
        .into_iter()
        .map(|reference| (reference, HandlerProcessedObjectKind::Wrapped));
    for (reference, kind) in deleted.chain(wrapped) {
        let old_object = old_objects.get(&reference.object_id);
        let initial_shared_version = match old_object {
            Some((_, Owner::Shared(initial))) => Some(*initial),
            _ => None,
        };
        let replaced_version = old_object.map(|(version, _)| *version);
        rows.push((
            ObjectKey(reference.object_id, reference.version),
            HandlerProcessedObject {
                digest: reference.digest,
                kind,
                produced_at,
                initial_shared_version,
                replaced_version,
            },
        ));
    }
    rows
}

/// The keys of the rows [`handler_processed_upserts`] derives for `effects`,
/// in the same order, without building the rows.
pub fn handler_processed_keys(effects: &TransactionEffects) -> Vec<ObjectKey> {
    let live = effects
        .all_changed_objects()
        .into_iter()
        .filter(|(owned_ref, write_kind)| needs_handler_row(*write_kind, owned_ref))
        .map(|(owned_ref, _)| {
            ObjectKey(
                owned_ref.reference().object_id,
                owned_ref.reference().version,
            )
        });
    let tombstones = chain!(
        effects.deleted(),
        effects.unwrapped_then_deleted(),
        effects.wrapped()
    )
    .map(|reference| ObjectKey(reference.object_id, reference.version));
    live.chain(tombstones).collect()
}

/// Whether a created, mutated or unwrapped object needs a handler-processed
/// row; if so, the row is `Live`.
fn needs_handler_row(write_kind: WriteKind, owned_ref: &OwnedObjectReference) -> bool {
    match (write_kind, owned_ref.owner()) {
        // Shared-object mutations write nothing: no check consults shared
        // state beyond existence, creation, and deletion, and hot shared
        // objects (the Clock) would churn the row every commit.
        (WriteKind::Mutate | WriteKind::Unwrap, Owner::Shared(_)) => false,
        (WriteKind::Create | WriteKind::Mutate | WriteKind::Unwrap, _) => true,
        // `WriteKind` is non-exhaustive; a kind this code does not know
        // cannot be classified, so it is a bug here rather than a guess.
        _ => fatal!(
            "unknown write kind {write_kind:?} for {}",
            owned_ref.reference().object_id
        ),
    }
}

/// A commit the handler has processed, handed to the execution watcher so it
/// can mark the commit fully executed once every root has effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedCommit {
    pub index: CommitIndex,
    /// The commit's roots: the same keys written to its pending checkpoints,
    /// cancelled transactions included.
    pub roots: Vec<TransactionKey>,
}

/// The entries of `overlay` at `keys`, skipping absent ones.
fn present_entries<K: Ord, V: Clone>(
    overlay: &RwLock<BTreeMap<K, V>>,
    keys: impl IntoIterator<Item = K>,
) -> Vec<(K, V)> {
    let overlay = overlay.read();
    keys.into_iter()
        .filter_map(|key| overlay.get(&key).cloned().map(|value| (key, value)))
        .collect()
}

/// A commit whose rows a quarantine flush staged into its batch. The flush
/// cannot evict the matching overlay entries itself - its caller writes the
/// batch - so it hands them back to be evicted once the write is durable.
pub(super) struct FlushedCommitRows {
    pub index: CommitIndex,
    pub rows: Vec<(ObjectKey, HandlerProcessedObject)>,
}

/// In-memory side of the bookkeeping plus every operation composing it with
/// the durable tables. Overlays hold entries of commits whose rows are not
/// yet durable; an entry leaves only after the corresponding table row is
/// durable, and the read paths check the overlay first, so the transient
/// both-present state is harmless.
pub struct HandlerObjectState {
    /// Transaction key -> producing commit index for every kept transaction of
    /// commits the handler has processed but whose executions have not all
    /// completed. The execution hook consults it: a hit upserts the
    /// execution's rows. Never persisted: replay after
    /// restart re-runs `assign_commit_to_transactions` before any
    /// (re-)execution can ask, and commits below the durable resume point
    /// never consult it again.
    commit_index_by_key: DashMap<TransactionKey, CommitIndex>,

    /// The keys assigned per commit index, so a fully executed commit can drop
    /// its map entries.
    keys_by_commit: Mutex<BTreeMap<CommitIndex, Vec<TransactionKey>>>,
    /// The handler-processed row keys of every commit the watcher completed and
    /// the flush has not reached yet, so the flush finds the commit's rows
    /// without reading its effects. Holds no entry for a commit the flush
    /// completed itself.
    row_keys_by_completed_commit: Mutex<BTreeMap<CommitIndex, Vec<ObjectKey>>>,
    /// Hands each assigned commit to the execution watcher in processing
    /// order. Unbounded so commit processing never blocks on execution lag;
    /// the backlog is one entry per commit not yet fully executed.
    assigned_commits: UnboundedSender<AssignedCommit>,
    /// The watcher's end of `assigned_commits`, taken once when the watcher
    /// starts.
    assigned_commits_receiver: Mutex<Option<UnboundedReceiver<AssignedCommit>>>,
    /// Highest commit index whose commit, and every commit below it, is fully
    /// executed. Seeded from the durable resume point: a commit at or below it
    /// has its rows on disk, hence executed.
    highest_fully_executed_commit: watch::Sender<CommitIndex>,
    handler_processed_overlay: RwLock<BTreeMap<ObjectKey, HandlerProcessedObject>>,

    /// Read-through cache over the durable handler-processed table, so the cold
    /// path of the hot per-input lookup is usually served without a table
    /// read. Holds present rows only (absence could go stale the moment a
    /// flush lands), filled by reads alone: a row never changes once written,
    /// so a flush has nothing to refresh or expire and a plain cache is
    /// enough.
    handler_processed_cache: MokaCache<ObjectKey, HandlerProcessedObject>,

    metrics: Arc<EpochMetrics>,
}

fn new_handler_processed_cache(capacity: u64) -> MokaCache<ObjectKey, HandlerProcessedObject> {
    MokaCache::builder(8)
        .max_capacity(randomize_cache_capacity_in_tests(capacity))
        .eviction_policy(EvictionPolicy::lru())
        .build()
}

impl HandlerObjectState {
    /// `resume_point` is the commit index the handler resumes from, which is
    /// also the highest commit known to be fully executed: everything at or
    /// below it has its rows on disk.
    pub fn new(resume_point: CommitIndex, metrics: Arc<EpochMetrics>) -> Self {
        let (assigned_commits, assigned_commits_receiver) =
            unbounded_channel("handler_assigned_commits");
        // The gauges outlive the epoch; the overlays start empty.
        metrics
            .handler_object_state_highest_fully_executed_commit
            .set(resume_point as i64);
        metrics
            .handler_object_state_handler_processed_overlay_entries
            .set(0);
        Self {
            commit_index_by_key: DashMap::with_shard_amount(2048),
            keys_by_commit: Mutex::new(BTreeMap::new()),
            row_keys_by_completed_commit: Mutex::new(BTreeMap::new()),
            assigned_commits,
            assigned_commits_receiver: Mutex::new(Some(assigned_commits_receiver)),
            highest_fully_executed_commit: watch::Sender::new(resume_point),
            handler_processed_overlay: RwLock::new(BTreeMap::new()),
            handler_processed_cache: new_handler_processed_cache(100_000),
            metrics,
        }
    }

    /// Records the roots of commit `index`, before any of them can be
    /// scheduled for execution, and hands the commit to the execution
    /// watcher. Called exactly once per commit: the watcher marks the commit
    /// fully executed after awaiting the roots it was handed, so a second
    /// call for the same index would let it drop entries it never awaited. A
    /// second call is therefore reported and leaves the first assignment in
    /// place.
    pub fn assign_commit_to_transactions(&self, index: CommitIndex, roots: Vec<TransactionKey>) {
        match self.keys_by_commit.lock().entry(index) {
            Entry::Occupied(_) => {
                debug_fatal!("commit index {index} assigned twice");
                return;
            }
            Entry::Vacant(entry) => {
                entry.insert(roots.clone());
            }
        }
        for key in &roots {
            self.commit_index_by_key.insert(*key, index);
        }
        // A closed receiver means the watcher has exited with the epoch;
        // nothing is left to complete.
        self.assigned_commits
            .send(AssignedCommit { index, roots })
            .ok();
    }

    /// The watcher's end of the assigned-commit channel; `None` once a
    /// watcher has taken it.
    pub fn take_assigned_commits_receiver(&self) -> Option<UnboundedReceiver<AssignedCommit>> {
        self.assigned_commits_receiver.lock().take()
    }

    /// Receiver of the highest fully executed commit; see the field's docs.
    /// Callers that wait for a specific commit want
    /// [`Self::wait_for_fully_executed_commit`] instead.
    pub fn subscribe_highest_fully_executed_commit(&self) -> watch::Receiver<CommitIndex> {
        self.highest_fully_executed_commit.subscribe()
    }

    /// Waits until commit `index` and everything below it is fully executed,
    /// returning immediately when that already holds. The caller bounds the
    /// wait with `within_alive_epoch`: at epoch end the last commits may never
    /// complete.
    pub async fn wait_for_fully_executed_commit(&self, index: CommitIndex) {
        let mut highest = self.highest_fully_executed_commit.subscribe();
        while *highest.borrow_and_update() < index {
            highest
                .changed()
                .await
                .expect("the sender is owned by this epoch's state and outlives every waiter");
        }
    }

    /// The commit index that kept this transaction, if the handler has
    /// processed that commit and it is not fully executed yet.
    pub fn commit_index_of(&self, key: &TransactionKey) -> Option<CommitIndex> {
        self.commit_index_by_key.get(key).map(|index| *index)
    }

    /// Drops the key → index entries of a fully executed commit, keeping the
    /// map bounded.
    fn drop_commit_assignments(&self, index: CommitIndex) {
        if let Some(keys) = self.keys_by_commit.lock().remove(&index) {
            for key in keys {
                self.commit_index_by_key.remove(&key);
            }
        }
    }

    /// Puts one executed transaction's rows in the overlay when the handler
    /// has assigned its commit, and returns whether it had. Must be called by
    /// the execution hook before the outputs become readable, so no object is
    /// ever readable without its row.
    ///
    /// `key` is the executed transaction's [`TransactionKey`], the identity
    /// the handler registered for it; a digest lookup would miss a
    /// randomness state update, registered under its randomness round.
    pub fn record_executed_transaction(
        &self,
        key: &TransactionKey,
        effects: &TransactionEffects,
    ) -> bool {
        let Some(index) = self.commit_index_of(key) else {
            return false;
        };
        self.upsert_handler_processed_rows(&handler_processed_upserts(effects, index));
        true
    }

    /// Marks commit `index` fully executed: applies the commit's
    /// handler-processed upserts, keeps their keys for the commit's flush
    /// ([`Self::take_completed_commit_rows`]), and drops the transaction key
    /// -> commit index map entries.
    ///
    /// Does nothing once the quarantine flush has completed the commit: its
    /// rows are durable already, and an overlay upsert now would leave entries
    /// no flush is left to evict. The caller holds the quarantine lock, so the
    /// flush cannot complete the commit between the check and the upserts.
    pub fn record_commit_fully_executed(
        &self,
        index: CommitIndex,
        upserts: &[(ObjectKey, HandlerProcessedObject)],
    ) {
        if !self.is_commit_assigned(index) {
            return;
        }
        self.upsert_handler_processed_rows(upserts);
        self.row_keys_by_completed_commit
            .lock()
            .insert(index, upserts.iter().map(|(key, _)| *key).collect());
        self.drop_commit_assignments(index);
        // Strictly after the upserts: a validation released by this value
        // must find this commit's rows already readable.
        self.advance_highest_fully_executed_commit(index);
    }

    /// Completes commit `index` from the quarantine flush, when the flush
    /// reaches it before the watcher does, as in a replay after a restart.
    /// Does nothing once the watcher has completed it.
    ///
    /// Like [`Self::record_commit_fully_executed`], this puts `rows` in the
    /// overlay and drops the commit's map entries. The caller writes `rows`
    /// with [`Self::write_commit_rows_to_batch`] right after, and passes the
    /// same rows to [`Self::evict_flushed_commit_rows`] once that batch is
    /// durable; the completion signal moves there.
    pub fn complete_commit_at_flush(
        &self,
        index: CommitIndex,
        rows: &[(ObjectKey, HandlerProcessedObject)],
    ) {
        if !self.is_commit_assigned(index) {
            return;
        }
        self.upsert_handler_processed_rows(rows);
        self.drop_commit_assignments(index);
    }

    /// The rows of commit `index` its flush must write, when the watcher
    /// completed the commit: those still in the overlay. `None` when the
    /// watcher has not completed it, in which case the flush derives the rows
    /// from effects and completes the commit itself through
    /// [`Self::complete_commit_at_flush`]. Forgets the commit's row keys, and
    /// those of any earlier commit, so call only from the commit's flush.
    ///
    /// A row missing from the overlay is already durable: when the watcher
    /// completed the commit every one of these rows was in the overlay or
    /// durable, and only a durable write evicts a row - here, the batch of
    /// the row's checkpoint, written before the commit's flush.
    pub fn take_completed_commit_rows(
        &self,
        index: CommitIndex,
    ) -> Option<Vec<(ObjectKey, HandlerProcessedObject)>> {
        let keys = {
            let mut row_keys = self.row_keys_by_completed_commit.lock();
            let keys = row_keys.remove(&index);
            row_keys.retain(|&completed, _| completed > index);
            keys?
        };
        Some(present_entries(&self.handler_processed_overlay, keys))
    }

    /// Whether commit `index` is still waiting to be completed. False once
    /// either completion path has run for it, which is what makes the second
    /// one a no-op.
    fn is_commit_assigned(&self, index: CommitIndex) -> bool {
        self.keys_by_commit.lock().contains_key(&index)
    }

    /// The handler-processed row at `key`, from the overlay or the durable
    /// table.
    ///
    /// Returns the row unconditionally; a caller validating at a commit index
    /// must apply the reading condition itself (`produced_at` above its
    /// horizon answers missing).
    pub fn handler_processed_object(
        &self,
        tables: &AuthorityEpochTables,
        key: &ObjectKey,
    ) -> IotaResult<Option<HandlerProcessedObject>> {
        if let Some(row) = self.handler_processed_overlay.read().get(key) {
            return Ok(Some(*row));
        }
        if let Some(row) = self.handler_processed_cache.get(key) {
            return Ok(Some(row));
        }
        let row = tables.handler_processed_objects.get(key)?;
        if let Some(row) = row {
            self.handler_processed_cache.insert(*key, row);
        }
        Ok(row)
    }

    /// Stages a commit's handler-processed rows into `batch`, the quarantine
    /// flush of the commit's own output, atomic with `last_consensus_stats`.
    /// After the batch is durably written - never before - pass the same
    /// commit index and rows to [`Self::evict_flushed_commit_rows`].
    ///
    /// Rows are keyed per version, so flushes of different commits may land
    /// in any order without one overwriting another.
    pub fn write_commit_rows_to_batch(
        &self,
        tables: &AuthorityEpochTables,
        batch: &mut DBBatch,
        handler_rows: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.handler_processed_objects,
            handler_rows.iter().map(|(key, row)| (key, row)),
        )?;
        Ok(())
    }

    /// Evicts a commit's handler-processed overlay entries once their rows are
    /// durable; the caller's write-then-evict order is what keeps a concurrent
    /// reader from finding a row in neither the overlay nor the table.
    ///
    /// A flushed commit's checkpoint has executed, so this also raises the
    /// highest fully executed commit.
    pub fn evict_flushed_commit_rows(
        &self,
        commit_index: CommitIndex,
        handler_rows: &[(ObjectKey, HandlerProcessedObject)],
    ) {
        self.remove_handler_overlay_rows(handler_rows);
        // Last, so a validation released by the signal finds the rows.
        self.advance_highest_fully_executed_commit(commit_index);
    }

    /// The handler rows the executions behind `effects` left in the overlay,
    /// at every key the execution hook could have written. `effects` are the
    /// transactions of one checkpoint whose outputs are not yet durable.
    pub fn checkpoint_rows<'a>(
        &self,
        effects: impl IntoIterator<Item = &'a TransactionEffects>,
    ) -> Vec<(ObjectKey, HandlerProcessedObject)> {
        if self.handler_processed_overlay.read().is_empty() {
            return Vec::new();
        }
        let keys: Vec<ObjectKey> = effects
            .into_iter()
            .flat_map(handler_processed_keys)
            .collect();
        present_entries(&self.handler_processed_overlay, keys)
    }

    /// Stages a checkpoint's handler rows into `batch`, the checkpoint
    /// executor's batch. After the batch is durably written (never before),
    /// pass the same rows to [`Self::evict_flushed_checkpoint_rows`]. Rows
    /// never change once written for a key, so checkpoints may write in any
    /// order and concurrently with a commit's flush.
    pub fn write_checkpoint_rows_to_batch(
        &self,
        tables: &AuthorityEpochTables,
        batch: &mut DBBatch,
        rows: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.handler_processed_objects,
            rows.iter().map(|(key, row)| (key, row)),
        )?;
        Ok(())
    }

    /// Evicts a checkpoint's handler rows from the overlay once they are
    /// durable.
    ///
    /// A handler row evicted here may be inserted again by its commit's
    /// completion, with the same value; the commit's flush evicts it then. A
    /// replay after a crash re-inserts entries whose rows are already durable,
    /// and re-running this for the re-executed checkpoint clears them.
    pub fn evict_flushed_checkpoint_rows(&self, rows: &[(ObjectKey, HandlerProcessedObject)]) {
        self.remove_handler_overlay_rows(rows);
    }

    fn remove_handler_overlay_rows(&self, rows: &[(ObjectKey, HandlerProcessedObject)]) {
        if rows.is_empty() {
            return;
        }
        let mut overlay = self.handler_processed_overlay.write();
        for (key, _) in rows {
            overlay.remove(key);
        }
        self.metrics
            .handler_object_state_handler_processed_overlay_entries
            .set(overlay.len() as i64);
    }

    /// The number of entries in the handler-processed overlay, for asserting
    /// eviction behavior.
    #[cfg(test)]
    pub fn overlay_size_for_testing(&self) -> usize {
        self.handler_processed_overlay.read().len()
    }

    /// Raises the highest fully executed commit to `index`; a repeated or late
    /// completion of an earlier commit never lowers it.
    fn advance_highest_fully_executed_commit(&self, index: CommitIndex) {
        let advanced = self
            .highest_fully_executed_commit
            .send_if_modified(|highest| {
                let advanced = index > *highest;
                if advanced {
                    *highest = index;
                }
                advanced
            });
        if advanced {
            self.metrics
                .handler_object_state_highest_fully_executed_commit
                .set(index as i64);
        }
    }

    /// Inserts rows into the overlay. Rows are keyed per version, so an
    /// insert never shadows a newer row of the same object, whatever order
    /// the hook and the watcher arrive in. A watcher completing a commit that
    /// already flushed must not reach here - its rows would sit in the overlay
    /// with no flush left to evict them - which is what the caller's check of
    /// the commit against the quarantine rules out.
    fn upsert_handler_processed_rows(&self, rows: &[(ObjectKey, HandlerProcessedObject)]) {
        if rows.is_empty() {
            return;
        }
        let mut overlay = self.handler_processed_overlay.write();
        overlay.extend(rows.iter().copied());
        self.metrics
            .handler_object_state_handler_processed_overlay_entries
            .set(overlay.len() as i64);
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{
        Address, ObjectReference, RandomnessRound, SenderSignedTransaction, TransactionDigest,
    };
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{effects::TestEffectsBuilder, transaction::TransactionAPI};
    use prometheus_filtered::Registry;

    use super::*;

    fn owned_inputs_tx(gas_version: u64) -> SenderSignedTransaction {
        let tx_data = TestTransactionBuilder::new(
            Address::ZERO,
            ObjectReference::new(
                ObjectId::random(),
                Version::from_u64(gas_version),
                ObjectDigest::random(),
            ),
            0,
        )
        .transfer_iota(None, Address::ZERO)
        .build();
        SenderSignedTransaction::new(tx_data, vec![])
    }

    struct EffectsFixture {
        effects: TransactionEffects,
        created_owned: ObjectId,
        created_shared: ObjectId,
        mutated_owned: ObjectId,
        deleted: ObjectId,
        deleted_shared: ObjectId,
        wrapped: ObjectId,
        unwrapped: ObjectId,
        gas: ObjectId,
    }

    const CREATED_SHARED_INITIAL_VERSION: u64 = 42;
    const DELETED_SHARED_INITIAL_VERSION: u64 = 4;
    const DELETED_SHARED_CONSUMED_VERSION: u64 = 6;

    fn effects_fixture() -> EffectsFixture {
        let transaction = owned_inputs_tx(1);
        let gas = transaction.transaction().gas()[0].object_id;
        let created_owned = ObjectId::random();
        let created_shared = ObjectId::random();
        let mutated_owned = ObjectId::random();
        let deleted = ObjectId::random();
        let deleted_shared = ObjectId::random();
        let wrapped = ObjectId::random();
        let unwrapped = ObjectId::random();
        let effects = TestEffectsBuilder::new(&transaction)
            .with_created_objects([
                (created_owned, Owner::Address(Address::ZERO)),
                (
                    created_shared,
                    Owner::Shared(Version::from_u64(CREATED_SHARED_INITIAL_VERSION)),
                ),
            ])
            .with_mutated_objects([(
                mutated_owned,
                Version::from_u64(5),
                Owner::Address(Address::ZERO),
            )])
            .with_deleted_objects([(deleted, Version::from_u64(3))])
            .with_deleted_objects_owned_by([(
                deleted_shared,
                Version::from_u64(DELETED_SHARED_CONSUMED_VERSION),
                Owner::Shared(Version::from_u64(DELETED_SHARED_INITIAL_VERSION)),
            )])
            .with_wrapped_objects([(wrapped, Version::from_u64(2))])
            .with_unwrapped_objects([(unwrapped, Owner::Address(Address::ZERO))])
            .build();
        EffectsFixture {
            effects,
            created_owned,
            created_shared,
            mutated_owned,
            deleted,
            deleted_shared,
            wrapped,
            unwrapped,
            gas,
        }
    }

    #[test]
    fn handler_processed_upserts_map_every_write_kind() {
        let fixture = effects_fixture();
        let lamport = fixture.effects.lamport_version();
        let index: CommitIndex = 9;
        assert_eq!(
            handler_processed_keys(&fixture.effects),
            handler_processed_upserts(&fixture.effects, index)
                .into_iter()
                .map(|(key, _)| key)
                .collect::<Vec<_>>()
        );
        // Every id in the fixture is written once, so keying by id is exact.
        let rows: BTreeMap<ObjectId, (Version, HandlerProcessedObject)> =
            handler_processed_upserts(&fixture.effects, index)
                .into_iter()
                .map(|(key, row)| (key.0, (key.1, row)))
                .collect();
        let version = |id: &ObjectId| rows[id].0;
        let rows: BTreeMap<ObjectId, HandlerProcessedObject> =
            rows.iter().map(|(id, (_, row))| (*id, *row)).collect();

        let created_owned = &rows[&fixture.created_owned];
        assert_eq!(created_owned.kind, HandlerProcessedObjectKind::Live);
        assert_eq!(version(&fixture.created_owned), lamport);
        assert_eq!(created_owned.produced_at, index);
        assert_eq!(created_owned.initial_shared_version, None);

        let created_shared = &rows[&fixture.created_shared];
        assert_eq!(created_shared.kind, HandlerProcessedObjectKind::Live);
        assert_eq!(
            created_shared.initial_shared_version,
            Some(Version::from_u64(CREATED_SHARED_INITIAL_VERSION))
        );

        assert_eq!(
            rows[&fixture.mutated_owned].kind,
            HandlerProcessedObjectKind::Live
        );
        assert_eq!(
            rows[&fixture.mutated_owned].replaced_version,
            Some(Version::from_u64(5))
        );
        assert_eq!(created_owned.replaced_version, None);
        assert_eq!(rows[&fixture.unwrapped].replaced_version, None);
        assert_eq!(
            rows[&fixture.deleted].replaced_version,
            Some(Version::from_u64(3))
        );
        assert_eq!(
            rows[&fixture.wrapped].replaced_version,
            Some(Version::from_u64(2))
        );
        assert_eq!(
            rows[&fixture.deleted_shared].replaced_version,
            Some(Version::from_u64(DELETED_SHARED_CONSUMED_VERSION))
        );
        assert_eq!(version(&fixture.mutated_owned), lamport);
        assert_eq!(rows[&fixture.gas].kind, HandlerProcessedObjectKind::Live);
        assert_eq!(
            rows[&fixture.unwrapped].kind,
            HandlerProcessedObjectKind::Live
        );

        let deleted = &rows[&fixture.deleted];
        assert_eq!(deleted.kind, HandlerProcessedObjectKind::Deleted);
        assert_eq!(version(&fixture.deleted), lamport);
        assert_eq!(deleted.initial_shared_version, None);

        // A deleted shared object's tombstone row keeps the initial shared
        // version, the only place a validator that executed the deletion can
        // still read it.
        let deleted_shared = &rows[&fixture.deleted_shared];
        assert_eq!(deleted_shared.kind, HandlerProcessedObjectKind::Deleted);
        assert_eq!(version(&fixture.deleted_shared), lamport);
        assert_eq!(
            deleted_shared.initial_shared_version,
            Some(Version::from_u64(DELETED_SHARED_INITIAL_VERSION))
        );

        let wrapped = &rows[&fixture.wrapped];
        assert_eq!(wrapped.kind, HandlerProcessedObjectKind::Wrapped);
        assert_eq!(version(&fixture.wrapped), lamport);
        assert_eq!(wrapped.initial_shared_version, None);

        assert_eq!(rows.len(), 8);
    }

    #[test]
    fn shared_input_mutation_writes_nothing() {
        use iota_sdk_types::SharedObjectReference;
        use iota_types::{
            programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::CallArg,
        };

        let shared_id = ObjectId::random();
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .obj(CallArg::Shared(SharedObjectReference::new(
                shared_id,
                Version::from_u64(7),
                true,
            )))
            .unwrap();
        let tx_data = TestTransactionBuilder::new(
            Address::ZERO,
            ObjectReference::new(
                ObjectId::random(),
                Version::from_u64(1),
                ObjectDigest::random(),
            ),
            0,
        )
        .programmable(builder.finish())
        .build();
        let transaction = SenderSignedTransaction::new(tx_data, vec![]);
        let effects = TestEffectsBuilder::new(&transaction)
            .with_shared_input_versions(BTreeMap::from([(shared_id, Version::from_u64(20))]))
            .build();

        let rows = handler_processed_upserts(&effects, 4);
        assert!(rows.iter().all(|(key, _)| key.0 != shared_id));
    }

    /// A state with empty overlays, built without the epoch tables.
    fn empty_state() -> HandlerObjectState {
        let (assigned_commits, assigned_commits_receiver) = unbounded_channel("test");
        HandlerObjectState {
            commit_index_by_key: DashMap::with_shard_amount(2048),
            keys_by_commit: Mutex::new(BTreeMap::new()),
            row_keys_by_completed_commit: Mutex::new(BTreeMap::new()),
            assigned_commits,
            assigned_commits_receiver: Mutex::new(Some(assigned_commits_receiver)),
            highest_fully_executed_commit: watch::Sender::new(0),
            handler_processed_overlay: RwLock::new(BTreeMap::new()),
            handler_processed_cache: new_handler_processed_cache(100),
            metrics: EpochMetrics::new(&Registry::new()),
        }
    }

    #[test]
    fn commit_index_map_assign_and_drop() {
        let state = empty_state();
        let key_a = TransactionKey::Digest(TransactionDigest::random());
        let key_b = TransactionKey::Digest(TransactionDigest::random());
        let key_c = TransactionKey::RandomnessRound(0, RandomnessRound::new(1));
        state.assign_commit_to_transactions(3, vec![key_a]);
        state.assign_commit_to_transactions(4, vec![key_b, key_c]);
        assert_eq!(state.commit_index_of(&key_a), Some(3));
        assert_eq!(state.commit_index_of(&key_b), Some(4));
        assert_eq!(state.commit_index_of(&key_c), Some(4));
        state.record_commit_fully_executed(3, &[]);
        assert_eq!(state.commit_index_of(&key_a), None);
        assert_eq!(state.commit_index_of(&key_b), Some(4));
        state.record_commit_fully_executed(4, &[]);
        assert_eq!(state.commit_index_of(&key_b), None);
        assert_eq!(state.commit_index_of(&key_c), None);
    }

    /// A second assignment of the same commit keeps the first root list and
    /// sends the watcher nothing more. In test builds `debug_fatal!` panics,
    /// so the second call runs under `catch_unwind`.
    #[test]
    fn commit_assigned_twice_keeps_the_first_assignment() {
        let state = empty_state();
        let mut receiver = state
            .take_assigned_commits_receiver()
            .expect("a new state holds the watcher's receiver");
        let first = TransactionKey::Digest(TransactionDigest::random());
        let second = TransactionKey::Digest(TransactionDigest::random());

        state.assign_commit_to_transactions(3, vec![first]);
        let second_call = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            state.assign_commit_to_transactions(3, vec![second]);
        }));
        assert!(second_call.is_err(), "the second assignment is reported");

        assert_eq!(state.commit_index_of(&first), Some(3));
        assert_eq!(state.commit_index_of(&second), None);
        state.drop_commit_assignments(3);
        assert_eq!(state.commit_index_of(&first), None);
        let assigned = receiver
            .try_recv()
            .expect("the first assignment reaches the watcher");
        assert_eq!((assigned.index, assigned.roots), (3, vec![first]));
        assert!(receiver.try_recv().is_err());
    }

    /// A commit is completed once: whichever of full execution and the
    /// quarantine flush comes second leaves the overlay untouched.
    #[test]
    fn second_completion_of_a_commit_does_nothing() {
        let state = empty_state();
        let row = |produced_at| HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind: HandlerProcessedObjectKind::Live,
            produced_at,
            initial_shared_version: None,
            replaced_version: None,
        };
        let key = |version| ObjectKey(ObjectId::random(), Version::from_u64(version));

        // The flush completes commit 3 first, as in a replay after a restart.
        state.assign_commit_to_transactions(3, vec![]);
        state.complete_commit_at_flush(3, &[(key(1), row(3))]);
        assert_eq!(state.overlay_size_for_testing(), 1);
        state.record_commit_fully_executed(3, &[(key(2), row(3))]);
        assert_eq!(state.overlay_size_for_testing(), 1);

        // Full execution completes commit 4 first.
        state.assign_commit_to_transactions(4, vec![]);
        state.record_commit_fully_executed(4, &[(key(3), row(4))]);
        assert_eq!(state.overlay_size_for_testing(), 2);
        state.complete_commit_at_flush(4, &[(key(4), row(4))]);
        assert_eq!(state.overlay_size_for_testing(), 2);
    }
}
