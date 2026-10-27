// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The handler object state: the object versions that the commits assigned
//! by the consensus handler produce, kept per epoch for P-COOL deterministic
//! post-consensus validation. Validation reads object state as of a commit
//! from it, however far local execution has got.
//!
//! Terms used here:
//! - `C` is the commit being validated, and `K` is how many commits behind `C` validation reads the
//!   object state: the protocol config's `pcool_deterministic_validation_horizon_distance`.
//!   Validation at `C` reads the state as of commit `C - K`, called the horizon.
//! - A row is one entry of the handler object state. It records that a transaction in some commit
//!   produced version `V` of object `id`: the commit, the digest, and whether the object is live,
//!   deleted or wrapped at that version. "The row at `(id, V)`" is that entry. Rows are read at the
//!   exact version a transaction names or, for a shared object that is gone, at the object's
//!   highest version.
//! - Rows are written to the epoch table `handler_processed_objects`. Until then they are held in
//!   memory. A row held in memory is removed only after it is written to the table, and reads look
//!   in memory first.
//! - The roots of a commit are the keys of the transactions the handler put in the commit's pending
//!   checkpoints, cancelled transactions included.
//! - A commit is fully executed when every root of it has effects.
//! - The execution watcher is the task that waits for the effects of the roots of each assigned
//!   commit and then marks the commit fully executed (`ExecutionWatcher` in the consensus handler).
//! - The resume point is the last commit whose output the quarantine flush wrote. After a restart
//!   the handler replays the commits above it.
//!
//! On a committee validator every execution belongs to a commit the handler
//! has assigned: the checkpoint executor runs a transaction only after the
//! handler has assigned its commit, until the node switches to executing
//! synced checkpoints for the rest of the epoch, which stops the handler.
//! The change-epoch transaction and genesis run outside any commit and write
//! no rows. What they write is the state the next epoch starts with.
//!
//! [`HandlerObjectState`] owns the rows held in memory and the map from
//! transaction key to assigned commit.
//!
//! The rows and the map change at these points:
//! - the handler assigns a commit to its roots before any of them can be scheduled
//!   ([`HandlerObjectState::assign_commit_to_transactions`], through the epoch store);
//! - each execution of a transaction whose commit is assigned puts its rows in memory before its
//!   outputs become readable ([`HandlerObjectState::record_executed_transaction`]);
//! - a commit is completed, either by the execution watcher once it is fully executed
//!   ([`HandlerObjectState::record_commit_fully_executed`]) or by the quarantine flush of its
//!   output when that comes first ([`HandlerObjectState::complete_commit_at_flush`]). Either way
//!   the commit's rows are put in memory, skipping the rows already present, and the commit's keys
//!   are removed from the map;
//! - the execution watcher stops on an error
//!   ([`HandlerObjectState::record_execution_watcher_error`]). After that only a flush completes a
//!   commit, and a wait for a commit that is not fully executed panics;
//! - rows are removed from memory once they are written to the table.
//!
//! Completing a commit also raises the highest fully executed commit
//! ([`HandlerObjectState::wait_for_fully_executed_commit`]). Validation of a
//! commit waits on it, so that it reads rows no execution can still add to.
//!
//! Rows are written to the table at two points:
//! - the quarantine flush of a commit's output, once its checkpoint is certified and executed,
//!   writes the commit's rows not yet in the table, in the same batch as `last_consensus_stats`
//!   ([`HandlerObjectState::write_commit_rows_to_batch`]). For a commit the execution watcher
//!   completed, these are its rows still held in memory
//!   ([`HandlerObjectState::take_completed_commit_rows`]). Otherwise they are all the commit's
//!   rows, derived from its effects;
//! - for every executed checkpoint, the checkpoint executor writes the rows its executions put in
//!   memory, in a separate batch written just before the checkpoint's outputs and its watermark
//!   ([`HandlerObjectState::write_checkpoint_rows_to_batch`]). So an object is never written
//!   without its row. Rows never change once written for a key, so this batch takes no lock.
//!
//! Neither batch is synced to disk when written, so the order of these writes
//! holds across a process crash but not across a power loss or kernel crash.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    ops::Bound,
    sync::{Arc, OnceLock},
};

use dashmap::DashMap;
use iota_common::{fatal, random_util::randomize_cache_capacity_in_tests};
use iota_metrics::monitored_mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use iota_sdk_types::{
    ObjectDigest, ObjectId, OwnedObjectReference, Owner, TransactionDigest, TransactionEffects,
    Version, WriteKind,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
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

/// Whether a row describes a live object, or a deleted or wrapped one, at its
/// version.
///
/// A transaction that uses a deleted or a wrapped version is dropped either
/// way. The two kinds match the store's two tombstone digests and are kept
/// apart because a wrapped object can come back at a higher version through
/// an unwrap, and a deleted one cannot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandlerProcessedObjectKind {
    Live,
    Deleted,
    Wrapped,
}

/// A row: one version an object took in a commit the consensus handler
/// assigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerProcessedObject {
    pub digest: ObjectDigest,
    pub kind: HandlerProcessedObjectKind,
    /// The commit whose transaction produced this version. Validation at
    /// commit `C` treats a row produced above the horizon `C - K` as missing.
    pub produced_at: CommitIndex,
    /// The initial shared version, on the row that creates a shared object
    /// and on its `Deleted` row. `None` on every other row. Validation reads
    /// it to tell an object created as shared from one that is not, and to
    /// check the initial shared version a transaction declares.
    pub initial_shared_version: Option<Version>,
    /// The version of the object this row's write replaced: the consumed
    /// input of a mutation, deletion or wrap. `None` for a creation or an
    /// unwrap.
    pub replaced_version: Option<Version>,
    /// The transaction that deleted the object, on a `Deleted` row. `None` on
    /// the other kinds. The shared reader returns it as the deleting
    /// transaction of a kept deleted shared input, so it needs no separate
    /// deletion marker.
    pub deleted_by: Option<TransactionDigest>,
}

/// The rows a commit's executed transactions produce. The execution watcher
/// and the quarantine flush both derive a commit's rows here, so they always
/// agree.
pub fn handler_rows_for_commit<'a>(
    effects: impl IntoIterator<Item = &'a TransactionEffects>,
    index: CommitIndex,
) -> Vec<(ObjectKey, HandlerProcessedObject)> {
    effects
        .into_iter()
        .flat_map(|effects| handler_processed_upserts(effects, index))
        .collect()
}

/// The rows one executed transaction of commit `produced_at` produces: a
/// `Live` row for each created, mutated or unwrapped object, and a `Deleted`
/// or `Wrapped` row for each deleted or wrapped one. A deleted shared object's
/// row keeps its initial shared version, taken from the consumed input's
/// owner, and every `Deleted` row names the transaction.
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
        // Only the creation of a shared object gets here with a shared owner,
        // so only its creation row holds the initial shared version.
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
                deleted_by: None,
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
        let deleted_by =
            (kind == HandlerProcessedObjectKind::Deleted).then(|| *effects.transaction_digest());
        rows.push((
            ObjectKey(reference.object_id, reference.version),
            HandlerProcessedObject {
                digest: reference.digest,
                kind,
                produced_at,
                initial_shared_version,
                replaced_version,
                deleted_by,
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

/// Whether a created, mutated or unwrapped object needs a row. If so, the
/// row is `Live`.
fn needs_handler_row(write_kind: WriteKind, owned_ref: &OwnedObjectReference) -> bool {
    match (write_kind, owned_ref.owner()) {
        // Mutations of shared objects write no row: validation only asks
        // whether a shared object exists, was created as shared, or was
        // deleted, and a row on every commit for a busy object such as the
        // Clock would be wasted.
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

/// A commit the handler has assigned, handed to the execution watcher so it
/// can mark the commit fully executed once every root has effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedCommit {
    pub index: CommitIndex,
    /// The keys of the transactions the handler put in the commit's pending
    /// checkpoints, cancelled transactions included.
    pub roots: Vec<TransactionKey>,
}

/// The rows in `overlay` at `keys`, skipping absent ones.
fn present_entries<K: Ord, V: Clone>(
    overlay: &RwLock<BTreeMap<K, V>>,
    keys: impl IntoIterator<Item = K>,
) -> Vec<(K, V)> {
    let overlay = overlay.read();
    keys.into_iter()
        .filter_map(|key| overlay.get(&key).cloned().map(|value| (key, value)))
        .collect()
}

/// A commit whose rows a quarantine flush staged into its batch. The caller
/// of the flush writes the batch, so the flush hands the rows back for the
/// caller to remove from memory after that write.
pub(super) struct FlushedCommitRows {
    pub index: CommitIndex,
    pub rows: Vec<(ObjectKey, HandlerProcessedObject)>,
}

/// The rows held in memory and the map from transaction key to assigned
/// commit, with every operation that combines them with the table. A row
/// held in memory is removed only after it is written to the table, and
/// reads look in memory first, so a row found in both places reads the same.
pub struct HandlerObjectState {
    /// The assigned commit of every root of the commits that are not
    /// completed yet. [`Self::record_executed_transaction`] looks the executed
    /// transaction up here, and records its rows if it is found. Not written
    /// to disk: a replay after a restart assigns each commit again before any
    /// of its transactions can execute again, and the commits at or below the
    /// resume point are not looked up again.
    commit_index_by_key: DashMap<TransactionKey, CommitIndex>,

    /// The keys assigned to each commit, so a completed commit can remove its
    /// map entries.
    keys_by_commit: Mutex<BTreeMap<CommitIndex, Vec<TransactionKey>>>,
    /// The row keys of every commit the execution watcher completed and the
    /// flush has not reached yet, so the flush finds the commit's rows
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
    /// executed. Seeded from the resume point: a commit at or below it has its
    /// rows in the table, so it is executed.
    highest_fully_executed_commit: watch::Sender<CommitIndex>,
    /// The error the execution watcher stopped on. Once set, only a flush
    /// moves `highest_fully_executed_commit`.
    execution_watcher_error: OnceLock<String>,
    /// The rows not yet written to the table.
    handler_processed_overlay: RwLock<BTreeMap<ObjectKey, HandlerProcessedObject>>,

    /// Read-through cache over the table, so most reads of a row not held in
    /// memory skip the table read. Holds only rows that were found, since a
    /// flush can add a row at any moment. Filled by reads only: a row never
    /// changes once written, so a flush has nothing to update.
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
    /// also the highest commit known to be fully executed: every commit at or
    /// below it has its rows in the table.
    pub fn new(resume_point: CommitIndex, metrics: Arc<EpochMetrics>) -> Self {
        let (assigned_commits, assigned_commits_receiver) =
            unbounded_channel("handler_assigned_commits");
        // The gauges outlive the epoch, and a new state holds no rows in
        // memory.
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
            execution_watcher_error: OnceLock::new(),
            handler_processed_overlay: RwLock::new(BTreeMap::new()),
            handler_processed_cache: new_handler_processed_cache(100_000),
            metrics,
        }
    }

    /// Assigns commit `index` to its roots and hands the commit to the
    /// execution watcher. Call it before any of the roots can be scheduled for
    /// execution.
    ///
    /// # Panics
    ///
    /// If commit `index` is already assigned. A second call would let the
    /// watcher drop map entries of roots it never awaited, and only a bug in
    /// the consensus handler processes a commit twice on one epoch store.
    pub fn assign_commit_to_transactions(&self, index: CommitIndex, roots: Vec<TransactionKey>) {
        match self.keys_by_commit.lock().entry(index) {
            Entry::Occupied(_) => fatal!("commit index {index} assigned twice"),
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

    /// The commit index above every commit assigned so far. For tests that
    /// process commits or execute transactions without the consensus handler.
    pub fn next_commit_index_for_testing(&self) -> CommitIndex {
        let highest_assigned = self
            .keys_by_commit
            .lock()
            .last_key_value()
            .map_or(0, |(index, _)| *index);
        highest_assigned.max(*self.highest_fully_executed_commit.borrow()) + 1
    }

    /// Assigns the keys among `keys` that have no commit yet to one new
    /// commit, above every commit assigned so far, as the consensus handler
    /// does before it schedules a commit's transactions. For tests that
    /// execute transactions without the handler.
    pub fn assign_to_a_new_commit_for_testing(&self, keys: &[TransactionKey]) {
        let unassigned: Vec<_> = keys
            .iter()
            .filter(|key| !self.commit_index_by_key.contains_key(key))
            .copied()
            .collect();
        if unassigned.is_empty() {
            return;
        }
        self.assign_commit_to_transactions(self.next_commit_index_for_testing(), unassigned);
    }

    /// Records every commit up to `index` as fully executed. For tests that
    /// run no execution watcher, so a commit they process does not wait for
    /// the earlier ones.
    pub fn mark_fully_executed_for_testing(&self, index: CommitIndex) {
        self.highest_fully_executed_commit
            .send_if_modified(|highest| {
                let modified = *highest < index;
                *highest = (*highest).max(index);
                modified
            });
    }

    /// Records that the execution watcher stopped on `error` and wakes every
    /// [`Self::wait_for_fully_executed_commit`] caller, so none of them waits
    /// on a commit only a flush could complete.
    pub fn record_execution_watcher_error(&self, error: &IotaError) {
        // Only the first error is kept: the watcher stops on it.
        let _ = self.execution_watcher_error.set(error.to_string());
        self.highest_fully_executed_commit.send_modify(|_| {});
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
    ///
    /// # Panics
    ///
    /// If the commit is not fully executed and the execution watcher has
    /// stopped on an error. The commit would then complete only through a
    /// flush, which needs its checkpoint certified with signatures carried by
    /// commits the caller has not processed. Release builds abort on any
    /// panic, so this stops the node. Builds that unwind, such as tests and
    /// the simulator, stop only the caller's task, the consensus handler,
    /// unless `CRASH_ON_PANIC` is set. A restart of the process re-seeds the
    /// highest fully executed commit from the resume point.
    pub async fn wait_for_fully_executed_commit(&self, index: CommitIndex) {
        let mut highest = self.highest_fully_executed_commit.subscribe();
        while *highest.borrow_and_update() < index {
            if let Some(error) = self.execution_watcher_error.get() {
                fatal!(
                    "commit {index} is not fully executed and the execution watcher stopped \
                     on error: {error}"
                );
            }
            highest
                .changed()
                .await
                .expect("the sender is owned by this epoch's state and outlives every waiter");
        }
    }

    /// The commit the handler assigned to this transaction, if that commit is
    /// not completed yet.
    pub fn commit_index_of(&self, key: &TransactionKey) -> Option<CommitIndex> {
        self.commit_index_by_key.get(key).map(|index| *index)
    }

    /// Removes the map entries of a completed commit.
    fn drop_commit_assignments(&self, index: CommitIndex) {
        if let Some(keys) = self.keys_by_commit.lock().remove(&index) {
            for key in keys {
                self.commit_index_by_key.remove(&key);
            }
        }
    }

    /// Puts one executed transaction's rows in memory if the handler has
    /// assigned its commit, and returns whether it had. Call it before the
    /// transaction's outputs become readable, so no object is ever readable
    /// without its row.
    ///
    /// `key` is the executed transaction's [`TransactionKey`], the key the
    /// handler assigned. A randomness state update is assigned under its
    /// randomness round, so its digest would not be found.
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

    /// Marks commit `index` fully executed: puts the commit's rows in memory,
    /// keeps their keys for the commit's flush
    /// ([`Self::take_completed_commit_rows`]), and removes the commit's map
    /// entries.
    ///
    /// `upserts` are all the commit's rows, and `inserts` the ones
    /// [`Self::rows_not_yet_present`] returned for them. Only `inserts` are
    /// put in memory. A row already present is in memory or in the table with
    /// the same value, and putting it in memory again would make the commit's
    /// flush write a row a second time after the checkpoint executor already
    /// wrote it in its own batch. The keys kept for the flush cover all of
    /// `upserts`.
    ///
    /// Does nothing once the quarantine flush has completed the commit: its
    /// rows are in the table already, and rows put in memory now would never
    /// be removed. The caller holds the quarantine lock, so the flush cannot
    /// complete the commit between that check and the insert.
    pub fn record_commit_fully_executed(
        &self,
        index: CommitIndex,
        upserts: &[(ObjectKey, HandlerProcessedObject)],
        inserts: &[(ObjectKey, HandlerProcessedObject)],
    ) {
        if !self.is_commit_assigned(index) {
            return;
        }
        self.upsert_handler_processed_rows(inserts);
        self.row_keys_by_completed_commit
            .lock()
            .insert(index, upserts.iter().map(|(key, _)| *key).collect());
        self.drop_commit_assignments(index);
        // Strictly after the upserts: a validation released by this value
        // must find this commit's rows already readable.
        self.advance_highest_fully_executed_commit(index);
    }

    /// The rows of `upserts` with no row yet at their key, in memory, in the
    /// read-through cache or in the table. Unlike
    /// [`Self::handler_processed_object`], a row found in the table is not
    /// added to the cache, since no validation is about to read it.
    pub fn rows_not_yet_present(
        &self,
        tables: &AuthorityEpochTables,
        upserts: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult<Vec<(ObjectKey, HandlerProcessedObject)>> {
        let not_in_memory: Vec<_> = {
            let overlay = self.handler_processed_overlay.read();
            upserts
                .iter()
                .filter(|(key, _)| {
                    !overlay.contains_key(key) && !self.handler_processed_cache.contains_key(key)
                })
                .copied()
                .collect()
        };
        let durable = tables
            .handler_processed_objects
            .multi_contains_keys(not_in_memory.iter().map(|(key, _)| key))?;
        Ok(not_in_memory
            .into_iter()
            .zip(durable)
            .filter_map(|(row, durable)| (!durable).then_some(row))
            .collect())
    }

    /// Completes commit `index` from the quarantine flush, when the flush
    /// reaches it before the execution watcher does, as in a replay after a
    /// restart. Does nothing once the watcher has completed it.
    ///
    /// Like [`Self::record_commit_fully_executed`], this puts `rows` in memory
    /// and removes the commit's map entries. The caller writes `rows` with
    /// [`Self::write_commit_rows_to_batch`] right after, and passes the same
    /// rows to [`Self::evict_flushed_commit_rows`] once that batch is written.
    /// That call, not this one, raises the highest fully executed commit.
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

    /// Returns the rows the flush of commit `index` must write: the commit's
    /// rows still held in memory. Returns `None` if the execution watcher has
    /// not completed the commit. The flush then derives the rows from effects
    /// and calls [`Self::complete_commit_at_flush`].
    ///
    /// Also forgets the row keys of this commit and of every earlier one, so
    /// call it only from the commit's flush.
    ///
    /// A row of the commit that is not held in memory is not an error: the
    /// checkpoint executor already wrote it to the table and removed it from
    /// memory.
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
    /// either the execution watcher or the flush has completed it, so the
    /// second completion does nothing.
    fn is_commit_assigned(&self, index: CommitIndex) -> bool {
        self.keys_by_commit.lock().contains_key(&index)
    }

    /// The row at `key`, from memory, the read-through cache or the table.
    ///
    /// Returns the row whatever commit produced it. A caller validating at
    /// commit `C` must treat a row produced above `C - K` as missing.
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

    /// The row of `id` at its highest version, from memory or the table,
    /// whichever holds the higher version. A key present in both holds the
    /// same row. Like [`Self::handler_processed_object`], it returns the row
    /// whatever commit produced it.
    ///
    /// The read-through cache is not used, here or in
    /// [`Self::lowest_handler_processed_row_above`]: it is keyed by exact
    /// version, so it cannot tell which versions of `id` have a row.
    pub fn highest_handler_processed_row(
        &self,
        tables: &AuthorityEpochTables,
        id: &ObjectId,
    ) -> IotaResult<Option<(ObjectKey, HandlerProcessedObject)>> {
        let overlay_row = self
            .handler_processed_overlay
            .read()
            .range(ObjectKey::min_for_id(id)..=ObjectKey::max_for_id(id))
            .next_back()
            .map(|(key, row)| (*key, *row));
        let durable_row = tables
            .handler_processed_objects
            .safe_iter_with_prefix_reversed(id)
            .next()
            .transpose()?;
        Ok(match (overlay_row, durable_row) {
            (Some(overlay), Some(durable)) => Some(if overlay.0 >= durable.0 {
                overlay
            } else {
                durable
            }),
            (overlay, durable) => overlay.or(durable),
        })
    }

    /// The row of `id` at its lowest version above `version`, from memory or
    /// the table, whichever holds the lower version. A key present in both
    /// holds the same row. Returns the row whatever commit produced it and,
    /// like [`Self::highest_handler_processed_row`], does not use the
    /// read-through cache.
    pub fn lowest_handler_processed_row_above(
        &self,
        tables: &AuthorityEpochTables,
        id: &ObjectId,
        version: Version,
    ) -> IotaResult<Option<(ObjectKey, HandlerProcessedObject)>> {
        let overlay_row = self
            .handler_processed_overlay
            .read()
            .range((
                Bound::Excluded(ObjectKey(*id, version)),
                Bound::Included(ObjectKey::max_for_id(id)),
            ))
            .next()
            .map(|(key, row)| (*key, *row));
        let durable_row = tables
            .handler_processed_objects
            .safe_iter_with_prefix_from(id, Bound::Excluded(&version))
            .next()
            .transpose()?;
        Ok(match (overlay_row, durable_row) {
            (Some(overlay), Some(durable)) => Some(if overlay.0 <= durable.0 {
                overlay
            } else {
                durable
            }),
            (overlay, durable) => overlay.or(durable),
        })
    }

    /// The row of `id` at its highest version below `version`, from memory or
    /// the table, whichever holds the higher version. A key present in both
    /// holds the same row. Returns the row whatever commit produced it and,
    /// like [`Self::highest_handler_processed_row`], does not use the
    /// read-through cache.
    ///
    /// A caller walking down from the highest row to find the version live
    /// as of commit `C - K` relies on the versions of `id` rising with
    /// `produced_at`. That holds for an object that changes only through a
    /// shared parent, such as a dynamic field of the coin deny list: its
    /// versions are assigned in the order of the commits that change it.
    pub fn highest_handler_processed_row_below(
        &self,
        tables: &AuthorityEpochTables,
        id: &ObjectId,
        version: Version,
    ) -> IotaResult<Option<(ObjectKey, HandlerProcessedObject)>> {
        let below = ObjectKey::min_for_id(id)..ObjectKey(*id, version);
        let overlay_row = self
            .handler_processed_overlay
            .read()
            .range(below.clone())
            .next_back()
            .map(|(key, row)| (*key, *row));
        let durable_row = tables
            .handler_processed_objects
            .safe_range_iter_reversed(below)
            .next()
            .transpose()?;
        Ok(match (overlay_row, durable_row) {
            (Some(overlay), Some(durable)) => Some(if overlay.0 >= durable.0 {
                overlay
            } else {
                durable
            }),
            (overlay, durable) => overlay.or(durable),
        })
    }

    /// Stages a commit's rows into `batch`, the batch of the quarantine flush
    /// of the commit's output, which also writes `last_consensus_stats`.
    /// After the batch is written, and not before, pass the same commit index
    /// and rows to [`Self::evict_flushed_commit_rows`].
    ///
    /// Rows are keyed per version, so flushes of different commits may be
    /// written in any order without one overwriting another.
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

    /// Removes a commit's rows from memory. Call it only after the batch from
    /// [`Self::write_commit_rows_to_batch`] is written, so a concurrent read
    /// always finds each row in memory or in the table.
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

    /// The rows the executions behind `effects` put in memory, at every key
    /// [`Self::record_executed_transaction`] could have written. `effects` are
    /// the transactions of one checkpoint whose outputs are not written yet.
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

    /// Stages a checkpoint's rows into `batch`, a separate batch the
    /// checkpoint executor writes just before the checkpoint's outputs. After
    /// the batch is written, and not before, pass the same rows to
    /// [`Self::evict_flushed_checkpoint_rows`]. Rows never change once written
    /// for a key, so checkpoints may write in any order and concurrently with
    /// a commit's flush.
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

    /// Removes a checkpoint's rows from memory. Call it only after the batch
    /// from [`Self::write_checkpoint_rows_to_batch`] is written.
    ///
    /// After a crash, executing the checkpoint again puts the same rows in
    /// memory again, and calling this for that execution removes them.
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

    /// The number of rows held in memory, for asserting when they are
    /// removed.
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

    /// Puts rows in memory. Rows are keyed per version, so a row never hides a
    /// newer row of the same object, whatever order executions and the
    /// execution watcher arrive in.
    ///
    /// Do not call it for a commit the flush has already written: its rows
    /// would stay in memory with no flush left to remove them.
    /// [`Self::record_commit_fully_executed`] and
    /// [`Self::complete_commit_at_flush`] check [`Self::is_commit_assigned`]
    /// under the quarantine lock first.
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

    /// A state with no rows in memory, built without the epoch tables.
    fn empty_state() -> HandlerObjectState {
        let (assigned_commits, assigned_commits_receiver) = unbounded_channel("test");
        HandlerObjectState {
            commit_index_by_key: DashMap::with_shard_amount(2048),
            keys_by_commit: Mutex::new(BTreeMap::new()),
            row_keys_by_completed_commit: Mutex::new(BTreeMap::new()),
            assigned_commits,
            assigned_commits_receiver: Mutex::new(Some(assigned_commits_receiver)),
            highest_fully_executed_commit: watch::Sender::new(0),
            execution_watcher_error: OnceLock::new(),
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
        state.record_commit_fully_executed(3, &[], &[]);
        assert_eq!(state.commit_index_of(&key_a), None);
        assert_eq!(state.commit_index_of(&key_b), Some(4));
        state.record_commit_fully_executed(4, &[], &[]);
        assert_eq!(state.commit_index_of(&key_b), None);
        assert_eq!(state.commit_index_of(&key_c), None);
    }

    /// A second assignment of the same commit stops the node.
    #[test]
    #[should_panic(expected = "commit index 3 assigned twice")]
    fn commit_assigned_twice_panics() {
        let state = empty_state();
        state.assign_commit_to_transactions(
            3,
            vec![TransactionKey::Digest(TransactionDigest::random())],
        );
        state.assign_commit_to_transactions(
            3,
            vec![TransactionKey::Digest(TransactionDigest::random())],
        );
    }

    /// A commit is completed once: whichever of full execution and the
    /// quarantine flush comes second leaves the rows held in memory
    /// unchanged.
    #[test]
    fn second_completion_of_a_commit_does_nothing() {
        let state = empty_state();
        let row = |produced_at| HandlerProcessedObject {
            digest: ObjectDigest::random(),
            kind: HandlerProcessedObjectKind::Live,
            produced_at,
            initial_shared_version: None,
            replaced_version: None,
            deleted_by: None,
        };
        let key = |version| ObjectKey(ObjectId::random(), Version::from_u64(version));

        // The flush completes commit 3 first, as in a replay after a restart.
        state.assign_commit_to_transactions(3, vec![]);
        state.complete_commit_at_flush(3, &[(key(1), row(3))]);
        assert_eq!(state.overlay_size_for_testing(), 1);
        let late = [(key(2), row(3))];
        state.record_commit_fully_executed(3, &late, &late);
        assert_eq!(state.overlay_size_for_testing(), 1);

        // Full execution completes commit 4 first.
        state.assign_commit_to_transactions(4, vec![]);
        let executed = [(key(3), row(4))];
        state.record_commit_fully_executed(4, &executed, &executed);
        assert_eq!(state.overlay_size_for_testing(), 2);
        state.complete_commit_at_flush(4, &[(key(4), row(4))]);
        assert_eq!(state.overlay_size_for_testing(), 2);
    }
}
