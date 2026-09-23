// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch object bookkeeping for P-COOL deterministic post-consensus
//! validation: object state as of the commit height being processed by the
//! consensus handler, independent of local execution and state-sync progress.
//!
//! Three views are kept, each an epoch table plus an in-memory overlay
//! holding the entries not yet durable:
//! - **handler-latest** - every version a commit the handler has processed
//!   produced, one row per (object, version), read by the exact version a
//!   transaction names, so writes need no ordering among themselves. Blind to
//!   execution driven by state sync running ahead of the handler.
//! - **sync-ahead records** - per-object markers written by state-sync-driven
//!   execution the handler has not reached yet, restoring the pre-sync view
//!   (`base_version` is the version the chain grew from - latest before sync
//!   ran ahead; `None` for an object the sync-ahead chain itself created).
//!   Empty in normal operation.
//! - **sheltered objects** - full bytes of input versions consumed by
//!   sync-driven execution, kept until the handler passes the consuming commit,
//!   so content reads survive aggressive pruning. Empty in normal operation.
//!
//! [`HandlerObjectState`] owns the overlays, the transaction-key ->
//! commit-index map,
//! and every invariant on them: overlay-first reads, and eviction only after
//! the corresponding table row is durable.
//!
//! The in-memory state changes at five points: the handler registers a
//! commit's kept transaction keys before they can be scheduled
//! ([`HandlerObjectState::assign_commit_to_transactions`], via the epoch
//! store); each execution records its writes before they become readable -
//! handler-latest rows when the key is in the index map, sync-ahead records
//! and sheltered bytes when it is not
//! ([`HandlerObjectState::record_executed_transaction`]); a fully executed
//! commit applies its remaining upserts, queues durable deletions for the sync
//! records it caught up past, and drops its map
//! entries ([`HandlerObjectState::record_commit_fully_executed`]); and the
//! two eviction methods below clear overlay entries once their rows are
//! durable.
//!
//! Completing a commit also raises the highest fully executed commit
//! ([`HandlerObjectState::wait_for_fully_executed_commit`]), the one value
//! this module publishes outward: validation of a commit waits on it, so that
//! it reads rows no execution can still add to.
//!
//! Durable writes happen at exactly two trigger points; everything else the
//! module does is in-memory plus point reads:
//! - the quarantine flush of a commit's output (once its checkpoint is
//!   certified and executed): inserts the commit's handler-latest rows
//!   atomically with `last_consensus_stats`, and drains the queued sync-record
//!   deletions ([`HandlerObjectState::write_commit_rows_to_batch`]);
//! - the checkpoint executor's auxiliary batch for a sync-executed checkpoint,
//!   durable before the watermark bump: inserts sync-ahead records and
//!   sheltered bytes ([`HandlerObjectState::write_sync_ahead_rows_to_batch`]).

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use dashmap::DashMap;
use iota_common::{debug_fatal, random_util::randomize_cache_capacity_in_tests};
use iota_metrics::monitored_mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use iota_sdk_types::{
    ObjectDigest, ObjectId, OwnedObjectReference, Owner, TransactionDigest, TransactionEffects,
    Version, WriteKind,
};
use iota_types::{
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaResult,
    object::Object,
    storage::{ObjectKey, ObjectStore},
    transaction::TransactionKey,
};
use itertools::chain;
use moka::{policy::EvictionPolicy, sync::SegmentedCache as MokaCache};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use typed_store::{Map, rocks::DBBatch};

use super::AuthorityEpochTables;
use crate::epoch::epoch_metrics::EpochMetrics;

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
    /// Index of the commit whose execution produced this row; reads at commit
    /// C treat rows above the horizon (C − K) as missing
    pub produced_at: CommitIndex,
    /// `Some` iff the object was created as shared this epoch; doubles as the
    /// created-shared flag for the shared-input checks.
    pub initial_shared_version: Option<Version>,
}

/// Marker for an object written by sync-ahead execution, restoring the
/// pre-sync view for validation reads.
///
/// One record covers a whole chain of sync-ahead writes to the object
/// (v -> v1 -> v2, ...): validation must treat every chain version uniformly
/// as missing and only `base_version` as still latest, so no per-version
/// data is needed. The record is consulted only while no handler-latest row
/// exists for the object; once the handler reaches any commit touching it,
/// the handler-latest row takes precedence on every read, so a record that
/// outlives parts of the chain never leaks a stale answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncAheadRecord {
    /// The version that was latest before sync ran ahead - the first version
    /// this epoch's sync-ahead chain consumed - and therefore the only
    /// version validation may still treat as live. `None` when the chain
    /// created the object itself, in which case every named version answers
    /// missing.
    pub base_version: Option<Version>,
    /// The highest version the chain has created so far; extended as the
    /// chain grows.
    pub latest_created: Version,
}

/// One object write of a sync-ahead execution, as input to the sync-record
/// upsert: `consumed` is the input version this write superseded (`None` when
/// the execution created the object), `created` the version it produced
/// (tombstone version for deletions and wraps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncAheadWrite {
    pub id: ObjectId,
    pub consumed: Option<Version>,
    pub created: Version,
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
        .flat_map(|effects| handler_latest_upserts(effects, index))
        .collect()
}

/// Derives the handler-latest upserts for one executed transaction of the
/// commit at `produced_at`: created / mutated / unwrapped objects become
/// `Live` rows, deletions and wraps become tombstone rows.
pub fn handler_latest_upserts(
    effects: &TransactionEffects,
    produced_at: CommitIndex,
) -> Vec<(ObjectKey, HandlerProcessedObject)> {
    let changed = effects.all_changed_objects();
    let mut rows = Vec::with_capacity(changed.len());
    for (owned_ref, write_kind) in changed {
        let initial_shared_version = match (write_kind, owned_ref.owner) {
            // A shared object's creation row carries its initial shared
            // version - the created-shared flag the shared-input checks read.
            (WriteKind::Create, Owner::Shared(initial)) => Some(initial),
            // Shared-object mutations write nothing: no check consults shared
            // state beyond existence, creation, and deletion, and hot shared
            // objects (the Clock) would churn the row every commit.
            (_, Owner::Shared(_)) => continue,
            _ => None,
        };
        rows.push((
            ObjectKey(owned_ref.reference.object_id, owned_ref.reference.version),
            HandlerProcessedObject {
                digest: owned_ref.reference.digest,
                kind: HandlerProcessedObjectKind::Live,
                produced_at,
                initial_shared_version,
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
        rows.push((
            ObjectKey(reference.object_id, reference.version),
            HandlerProcessedObject {
                digest: reference.digest,
                kind,
                produced_at,
                initial_shared_version: None,
            },
        ));
    }
    rows
}

/// Derives the sync-ahead record writes for one sync-executed transaction:
/// one write per object the execution wrote. `old_metadata` is the
/// transaction's [`TransactionEffectsAPI::old_object_metadata`], passed in so
/// the caller computes it once for this and
/// [`consumed_input_keys_to_shelter`]. Shared-object mutations are skipped,
/// mirroring the handler-latest churn rule; shared creations and deletions
/// are included (a sync-ahead shared deletion must stay invisible to
/// validation, which the record's restored pre-sync existence provides).
pub fn sync_ahead_writes(
    effects: &TransactionEffects,
    old_metadata: &[OwnedObjectReference],
) -> Vec<SyncAheadWrite> {
    let old_versions: BTreeMap<ObjectId, Version> = old_metadata
        .iter()
        .map(|owned_ref| (owned_ref.reference.object_id, owned_ref.reference.version))
        .collect();
    let changed = effects.all_changed_objects();
    let mut writes = Vec::with_capacity(changed.len());
    for (owned_ref, write_kind) in changed {
        let id = owned_ref.reference.object_id;
        let consumed = match (write_kind, owned_ref.owner) {
            // Creations restore nothing. Unwraps restore nothing either: the
            // object had no live version before this chain ran, no matter
            // when the wrap happened - and if this same chain wrapped it, a
            // record carrying the pre-wrap version already exists and the
            // upsert keeps it.
            (WriteKind::Create | WriteKind::Unwrap, _) => None,
            (WriteKind::Mutate, Owner::Shared(_)) => continue,
            (WriteKind::Mutate, _) => old_versions.get(&id).copied(),
        };
        writes.push(SyncAheadWrite {
            id,
            consumed,
            created: owned_ref.reference.version,
        });
    }
    for reference in effects.unwrapped_then_deleted() {
        writes.push(SyncAheadWrite {
            id: reference.object_id,
            consumed: None,
            created: reference.version,
        });
    }
    for reference in chain(effects.deleted(), effects.wrapped()) {
        let id = reference.object_id;
        writes.push(SyncAheadWrite {
            id,
            consumed: old_versions.get(&id).copied(),
            created: reference.version,
        });
    }
    writes
}

/// The input versions a transaction consumed that shelter rows must cover:
/// address-owned versions, plus object-owned children conservatively (pending
/// the deferred receiving-objects design). Shared inputs are existence-only
/// and immutables are never superseded, so neither is sheltered.
pub fn consumed_input_keys_to_shelter(old_metadata: &[OwnedObjectReference]) -> Vec<ObjectKey> {
    old_metadata
        .iter()
        .filter(|owned_ref| matches!(owned_ref.owner, Owner::Address(_) | Owner::Object(_)))
        .map(|owned_ref| ObjectKey::from(&owned_ref.reference))
        .collect()
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
    /// completed. The execution hook consults it: hit -> handler-latest upsert;
    /// miss -> sync-ahead execution. Never persisted: replay after restart
    /// re-runs `assign_commit_to_transactions` before any (re-)execution
    /// can ask, and commits below the durable resume point never consult it
    /// again.
    commit_index_by_key: DashMap<TransactionKey, CommitIndex>,

    /// The keys assigned per commit index, so a fully executed commit can drop
    /// its map entries.
    keys_by_commit: Mutex<BTreeMap<CommitIndex, Vec<TransactionKey>>>,
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

    handler_latest_overlay: RwLock<BTreeMap<ObjectKey, HandlerProcessedObject>>,
    sync_ahead_overlay: RwLock<BTreeMap<ObjectId, SyncAheadRecord>>,
    sheltered_overlay: RwLock<BTreeMap<ObjectKey, Object>>,

    /// Sync-ahead records the handler has caught up past, pending deletion
    /// from the durable table; drained into the next flush batch.
    sync_ahead_record_deletions: Mutex<BTreeMap<CommitIndex, BTreeSet<ObjectId>>>,
    /// Sync-ahead records currently alive (created and not yet queued for
    /// deletion), so the per-commit cleanup can skip its table lookups
    /// entirely in normal operation, when no record exists. Increments and
    /// deletion-queue insertions pair exactly: a record recreated while its
    /// deletion is queued cancels the deletion and counts as a fresh record.
    live_sync_ahead_records_count: AtomicU64,

    /// Read-through cache over the durable handler-latest table, so the cold
    /// path of the hot per-input lookup is usually served without a table
    /// read. Holds present rows only (absence could go stale the moment a
    /// flush lands), filled by reads alone: a row never changes once written,
    /// so a flush has nothing to refresh or expire and a plain cache is
    /// enough. The other two tables get no cache: they are consulted only
    /// after a handler-latest miss and are empty in normal operation, where
    /// a table miss is a cheap bloom-filter negative.
    handler_latest_cache: MokaCache<ObjectKey, HandlerProcessedObject>,

    metrics: Arc<EpochMetrics>,
}

fn new_handler_latest_cache(capacity: u64) -> MokaCache<ObjectKey, HandlerProcessedObject> {
    MokaCache::builder(8)
        .max_capacity(randomize_cache_capacity_in_tests(capacity))
        .eviction_policy(EvictionPolicy::lru())
        .build()
}

impl HandlerObjectState {
    /// `resume_point` is the commit index the handler resumes from, which is
    /// also the highest commit known to be fully executed: everything at or
    /// below it has its rows on disk.
    pub fn new(
        tables: &AuthorityEpochTables,
        resume_point: CommitIndex,
        metrics: Arc<EpochMetrics>,
    ) -> Self {
        // Nonzero only when reopening mid-epoch with sync-ahead records on
        // disk; counting them keeps the cleanup short-circuit sound across a
        // restart.
        let live_sync_ahead_records_count = tables
            .sync_ahead_records
            .safe_iter()
            .try_fold(0u64, |count, entry| entry.map(|_| count + 1))
            .expect("AuthorityEpochTables should contain valid sync-ahead records");
        let (assigned_commits, assigned_commits_receiver) =
            unbounded_channel("handler_assigned_commits");
        // The gauges outlive the epoch; the overlays start empty.
        metrics
            .handler_object_state_highest_fully_executed_commit
            .set(resume_point as i64);
        metrics
            .handler_object_state_handler_processed_overlay_entries
            .set(0);
        metrics
            .handler_object_state_sync_ahead_overlay_entries
            .set(0);
        metrics
            .handler_object_state_sheltered_overlay_entries
            .set(0);
        metrics.handler_object_state_sheltered_overlay_bytes.set(0);
        Self {
            commit_index_by_key: DashMap::with_shard_amount(2048),
            keys_by_commit: Mutex::new(BTreeMap::new()),
            assigned_commits,
            assigned_commits_receiver: Mutex::new(Some(assigned_commits_receiver)),
            highest_fully_executed_commit: watch::Sender::new(resume_point),
            handler_latest_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_overlay: RwLock::new(BTreeMap::new()),
            sheltered_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_record_deletions: Mutex::new(BTreeMap::new()),
            live_sync_ahead_records_count: AtomicU64::new(live_sync_ahead_records_count),
            handler_latest_cache: new_handler_latest_cache(100_000),
            metrics,
        }
    }

    /// Records the roots of commit `index`, before any of them can be
    /// scheduled for execution, and hands the commit to the execution
    /// watcher. Called exactly once per commit: the watcher marks the commit
    /// fully executed after awaiting the roots it was handed, so a second
    /// call for the same index would let it drop entries it never awaited.
    pub fn assign_commit_to_transactions(&self, index: CommitIndex, roots: Vec<TransactionKey>) {
        for key in &roots {
            self.commit_index_by_key.insert(*key, index);
        }
        if self
            .keys_by_commit
            .lock()
            .insert(index, roots.clone())
            .is_some()
        {
            debug_fatal!("commit index {index} assigned twice");
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
    pub fn drop_commit_assignments(&self, index: CommitIndex) {
        if let Some(keys) = self.keys_by_commit.lock().remove(&index) {
            for key in keys {
                self.commit_index_by_key.remove(&key);
            }
        }
    }

    /// Records one executed transaction's object writes. Must be called by
    /// the execution hook before the outputs become readable, so no object is
    /// ever readable without its bookkeeping row. A transaction of a commit
    /// the handler has processed upserts handler-latest rows; a transaction
    /// only state sync knows yet writes sync-ahead records for every object
    /// it wrote and shelters the bytes of the owned input versions it
    /// consumed.
    ///
    /// `key` is the executed transaction's [`TransactionKey`], the identity
    /// the handler registered for it; a digest lookup would miss a
    /// randomness state update, registered under its randomness round.
    ///
    /// `loaded_input_objects` must serve every consumed owned input at its
    /// consumed version - including dynamic-field children and received
    /// objects, which are not part of the transaction's declared input objects;
    /// a missing one cannot be sheltered against pruning and is reported
    /// through `debug_fatal`.
    pub fn record_executed_transaction(
        &self,
        tables: &AuthorityEpochTables,
        key: &TransactionKey,
        effects: &TransactionEffects,
        loaded_input_objects: &dyn ObjectStore,
    ) -> IotaResult {
        if let Some(index) = self.commit_index_of(key) {
            self.upsert_handler_processed_rows(&handler_latest_upserts(effects, index));
            Ok(())
        } else {
            let old_metadata = effects.old_object_metadata();
            self.upsert_sync_ahead_writes(tables, sync_ahead_writes(effects, &old_metadata))?;
            self.shelter_consumed_inputs(
                consumed_input_keys_to_shelter(&old_metadata),
                loaded_input_objects,
                effects.transaction_digest(),
            )
        }
    }

    /// Marks commit `index` fully executed: applies the commit's
    /// handler-latest upserts (covering executions that raced the map
    /// registration), removes sync-ahead records whose chains the handler has
    /// now caught up past, and drops the transaction key -> commit index map
    /// entries.
    ///
    /// Does nothing once the quarantine flush has completed the commit: its
    /// rows are durable already, and an overlay upsert now would leave entries
    /// no flush is left to evict. The caller holds the quarantine lock, so the
    /// flush cannot complete the commit between the check and the upserts.
    ///
    /// Sheltered bytes are deliberately not evicted here: their eviction keys
    /// off the *flushed* commits, because a crash before this commit's output
    /// flushes replays and re-validates it, and those reads may still need the
    /// bytes for versions the store has already pruned.
    pub fn record_commit_fully_executed(
        &self,
        tables: &AuthorityEpochTables,
        index: CommitIndex,
        upserts: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        if !self.is_commit_assigned(index) {
            return Ok(());
        }
        // The upserts must be visible to readers before the sync records are
        // removed: a validation read that finds neither concludes the object
        // is untouched this epoch and consults epoch-start state.
        self.upsert_handler_processed_rows(upserts);
        self.remove_handled_sync_ahead_records(tables, index, upserts)?;
        self.drop_commit_assignments(index);
        // Strictly after the upserts: a validation released by this value
        // must find this commit's rows already readable.
        self.advance_highest_fully_executed_commit(index);
        Ok(())
    }

    /// Completes commit `index` from the quarantine flush, when the flush
    /// reaches it before the watcher does - a replay after a restart, or state
    /// sync running ahead of the checkpoint builder. Does nothing once the
    /// watcher has completed it.
    ///
    /// `rows` go to the flush batch rather than the overlay, so unlike the
    /// watcher path this only queues the sync-record deletions and drops the
    /// map entries; the completion signal moves once the batch is durable, in
    /// [`Self::evict_flushed_commit_rows`].
    pub fn complete_commit_at_flush(
        &self,
        tables: &AuthorityEpochTables,
        index: CommitIndex,
        rows: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        if !self.is_commit_assigned(index) {
            return Ok(());
        }
        self.remove_handled_sync_ahead_records(tables, index, rows)?;
        self.drop_commit_assignments(index);
        Ok(())
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
        if let Some(row) = self.handler_latest_overlay.read().get(key) {
            return Ok(Some(*row));
        }
        if let Some(row) = self.handler_latest_cache.get(key) {
            return Ok(Some(row));
        }
        let row = tables.handler_processed_objects.get(key)?;
        if let Some(row) = row {
            self.handler_latest_cache.insert(*key, row);
        }
        Ok(row)
    }

    /// The sync-ahead record for `id`, from the overlay or the durable table.
    pub fn sync_ahead_record(
        &self,
        tables: &AuthorityEpochTables,
        id: &ObjectId,
    ) -> IotaResult<Option<SyncAheadRecord>> {
        if let Some(record) = self.sync_ahead_overlay.read().get(id) {
            return Ok(Some(*record));
        }
        Ok(tables.sync_ahead_records.get(id)?)
    }

    /// The sheltered bytes of a consumed input version, from the overlay or
    /// the durable table.
    pub fn sheltered_object(
        &self,
        tables: &AuthorityEpochTables,
        key: &ObjectKey,
    ) -> IotaResult<Option<Object>> {
        if let Some(object) = self.sheltered_overlay.read().get(key) {
            return Ok(Some(object.clone()));
        }
        Ok(tables.sheltered_objects.get(key)?)
    }

    /// Stages a commit's handler-latest rows into `batch` - the quarantine
    /// flush of the commit's own output, atomic with `last_consensus_stats` -
    /// together with every queued sync-record deletion. The deletions must
    /// ride this batch and no other: a deleted record's reads are answered by
    /// handler-latest rows, so a deletion that became durable without them
    /// (say, through the checkpoint executor's batch) would, after a crash,
    /// leave reads falling through to the epoch-start rule against sync-ahead
    /// store state. After the batch is durably written - never before - pass
    /// the same commit index and rows to [`Self::evict_flushed_commit_rows`].
    ///
    /// Rows are keyed per version, so flushes of different commits may land
    /// in any order without one overwriting another. The staged deletions stay
    /// queued until eviction, so a sync-ahead write landing before the batch
    /// is durable still sees the record as dead.
    pub fn write_commit_rows_to_batch(
        &self,
        commit_index: CommitIndex,
        tables: &AuthorityEpochTables,
        batch: &mut DBBatch,
        handler_rows: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.handler_processed_objects,
            handler_rows.iter().map(|(key, row)| (key, row)),
        )?;
        // Only this commit's deletions are staged: a later commit's must not
        // become durable before that commit's rows.
        let deletions = self
            .sync_ahead_record_deletions
            .lock()
            .get(&commit_index)
            .cloned()
            .unwrap_or_default();
        batch.delete_batch(&tables.sync_ahead_records, deletions)?;
        Ok(())
    }

    /// Evicts a commit's handler-latest overlay entries once their rows are
    /// durable; the caller's write-then-evict order is what keeps a concurrent
    /// reader from finding a row in neither the overlay nor the table.
    ///
    /// Every deletion queued for this commit or an earlier one is dropped here
    /// too, now that this commit's batch is durable: a record whose replacing
    /// rows flushed earlier may be deleted by any later flush. A deletion lost
    /// to a batch that never became durable is re-queued when the commit
    /// replays.
    ///
    /// A flushed commit's checkpoint has executed, so this also raises the
    /// highest fully executed commit.
    pub fn evict_flushed_commit_rows(
        &self,
        commit_index: CommitIndex,
        handler_rows: &[(ObjectKey, HandlerProcessedObject)],
    ) {
        self.sync_ahead_record_deletions
            .lock()
            .retain(|&index, _| index > commit_index);
        {
            let mut overlay = self.handler_latest_overlay.write();
            for (key, _) in handler_rows {
                overlay.remove(key);
            }
            self.metrics
                .handler_object_state_handler_processed_overlay_entries
                .set(overlay.len() as i64);
        }
        // Last, so a validation released by the signal finds the rows.
        self.advance_highest_fully_executed_commit(commit_index);
    }

    /// Stages a sync-executed checkpoint's records and sheltered bytes into
    /// `batch` - the checkpoint executor's auxiliary batch, durable before
    /// the executed-checkpoint watermark bump that lets the pruner delete the
    /// consumed versions' perpetual rows. After the batch is durably written
    /// (never before), pass the same rows to
    /// [`Self::evict_flushed_sync_ahead_rows`].
    ///
    /// These writes carry no version guard: the caller must flush checkpoints
    /// in order, one at a time (the checkpoint executor does), or an older
    /// record could overwrite a newer durable one.
    pub fn write_sync_ahead_rows_to_batch(
        &self,
        tables: &AuthorityEpochTables,
        batch: &mut DBBatch,
        sync_rows: &[(ObjectId, SyncAheadRecord)],
        shelter_rows: &[(ObjectKey, Object)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.sync_ahead_records,
            sync_rows.iter().map(|(id, record)| (id, record)),
        )?;
        batch.insert_batch(
            &tables.sheltered_objects,
            shelter_rows.iter().map(|(key, object)| (key, object)),
        )?;
        Ok(())
    }

    /// Evicts sync-ahead overlay entries once their rows are durable. Must
    /// cover the checkpoint's full derived row set - including keys the write
    /// skipped as already present - because a replay after a crash re-inserts
    /// overlay entries for rows that are already durable, and this recurring
    /// eviction is what clears them. An entry extended since the flush is
    /// kept; shelter rows are immutable per key, so no such check is needed
    /// there.
    pub fn evict_flushed_sync_ahead_rows(
        &self,
        sync_rows: &[(ObjectId, SyncAheadRecord)],
        shelter_rows: &[(ObjectKey, Object)],
    ) {
        {
            let mut overlay = self.sync_ahead_overlay.write();
            for (id, record) in sync_rows {
                if overlay.get(id) == Some(record) {
                    overlay.remove(id);
                }
            }
            self.metrics
                .handler_object_state_sync_ahead_overlay_entries
                .set(overlay.len() as i64);
        }
        let mut overlay = self.sheltered_overlay.write();
        for (key, _) in shelter_rows {
            if let Some(object) = overlay.remove(key) {
                self.metrics
                    .handler_object_state_sheltered_overlay_bytes
                    .sub(object.object_size_for_gas_metering() as i64);
            }
        }
        self.metrics
            .handler_object_state_sheltered_overlay_entries
            .set(overlay.len() as i64);
    }

    /// The number of entries in the (handler-latest, sync-ahead, sheltered)
    /// overlays, for asserting eviction behavior.
    #[cfg(test)]
    pub fn overlay_sizes_for_testing(&self) -> (usize, usize, usize) {
        (
            self.handler_latest_overlay.read().len(),
            self.sync_ahead_overlay.read().len(),
            self.sheltered_overlay.read().len(),
        )
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
        let mut overlay = self.handler_latest_overlay.write();
        overlay.extend(rows.iter().copied());
        self.metrics
            .handler_object_state_handler_processed_overlay_entries
            .set(overlay.len() as i64);
    }

    fn upsert_sync_ahead_writes(
        &self,
        tables: &AuthorityEpochTables,
        writes: Vec<SyncAheadWrite>,
    ) -> IotaResult {
        if writes.is_empty() {
            return Ok(());
        }
        let mut overlay = self.sync_ahead_overlay.write();
        let mut deletions = self.sync_ahead_record_deletions.lock();
        for write in writes {
            let current = match overlay.get(&write.id) {
                Some(record) => Some(*record),
                // A record with a queued deletion is logically gone (the
                // handler caught up past its chain) but stays in the table
                // until the queue drains into a commit flush; it must be
                // invisible here, or a new chain would extend the dead record
                // and inherit its stale `base_version`.
                None if deletions
                    .iter()
                    .any(|(_, commit_deletion)| commit_deletion.contains(&write.id)) =>
                {
                    None
                }
                None => tables.sync_ahead_records.get(&write.id)?,
            };
            // Only an extension of the chain updates the record
            // (re-executions after a restart replay the same writes).
            if current.is_none_or(|record| write.created > record.latest_created) {
                if current.is_none() {
                    // Cancel any queued deletion of the dead record: it
                    // drains into a later flush batch, which must not destroy
                    // the new record's durable row.
                    deletions.iter_mut().for_each(|(_, commit_deletion)| {
                        commit_deletion.remove(&write.id);
                    });

                    self.live_sync_ahead_records_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                let base_version = match current {
                    // First write of the chain: what it consumed is what was
                    // latest before the chain started (`None` when it created
                    // the object).
                    None => write.consumed,
                    // The chain already has a record: keep its
                    // `base_version` and ignore `write.consumed` - later
                    // writes consume the chain's own outputs, which exist
                    // only on validators that synced ahead. In particular, a
                    // chain that created the object and then mutates it must
                    // keep `None`, or validation here would answer keep for a
                    // version every other validator answers missing.
                    Some(record) => record.base_version,
                };
                overlay.insert(
                    write.id,
                    SyncAheadRecord {
                        base_version,
                        latest_created: write.created,
                    },
                );
            }
        }
        self.metrics
            .handler_object_state_sync_ahead_overlay_entries
            .set(overlay.len() as i64);
        Ok(())
    }

    fn shelter_consumed_inputs(
        &self,
        keys: Vec<ObjectKey>,
        loaded_input_objects: &dyn ObjectStore,
        tx_digest: &TransactionDigest,
    ) -> IotaResult {
        if keys.is_empty() {
            return Ok(());
        }
        // Resolve outside the overlay lock: the store reads can reach the DB.
        let mut rows = Vec::with_capacity(keys.len());
        for (key, object) in keys
            .iter()
            .zip(loaded_input_objects.try_multi_get_objects_by_key(&keys)?)
        {
            match object {
                Some(object) => rows.push((*key, object)),
                None => debug_fatal!(
                    "input {key:?} consumed by sync-executed transaction {tx_digest} missing from \
                 the loaded inputs; its bytes cannot be sheltered against pruning"
                ),
            }
        }
        // A replay after a crash may re-insert rows that are already durable;
        // that is fine - the checkpoint executor's persist step re-runs on the
        // same replay and evicts them again, and the bytes are identical
        // either way.
        let mut overlay = self.sheltered_overlay.write();
        for (key, object) in rows {
            let size = object.object_size_for_gas_metering();
            if overlay.insert(key, object).is_none() {
                self.metrics
                    .handler_object_state_sheltered_overlay_bytes
                    .add(size as i64);
            }
        }
        self.metrics
            .handler_object_state_sheltered_overlay_entries
            .set(overlay.len() as i64);
        Ok(())
    }

    fn remove_handled_sync_ahead_records(
        &self,
        tables: &AuthorityEpochTables,
        index: CommitIndex,
        upserts: &[(ObjectKey, HandlerProcessedObject)],
    ) -> IotaResult {
        // Normal operation: no sync-ahead record exists anywhere, so the
        // per-commit cleanup costs one atomic load instead of a table lookup
        // per written object.
        if self.live_sync_ahead_records_count.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }
        let mut overlay = self.sync_ahead_overlay.write();
        let mut deletions = self.sync_ahead_record_deletions.lock();
        for (key, _) in upserts {
            let record = match overlay.get(&key.0) {
                Some(record) => Some(*record),
                None => tables.sync_ahead_records.get(&key.0)?,
            };
            // The handler has caught up past the whole sync-ahead chain; the
            // handler-latest row (already visible) now answers every read the
            // record used to.
            if record.is_some_and(|record| record.latest_created <= key.1) {
                overlay.remove(&key.0);
                // Queue the durable deletion even when the record was found
                // only in the overlay: an extended record leaves its older
                // durable row behind, and the checkpoint executor's persist
                // step (which derives rows from effects, not the overlay) can
                // still write this record after the removal. Deleting a key
                // that never became durable is a no-op.
                if deletions.entry(index).or_default().insert(key.0) {
                    self.live_sync_ahead_records_count
                        .fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
        self.metrics
            .handler_object_state_sync_ahead_overlay_entries
            .set(overlay.len() as i64);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, ObjectReference, RandomnessRound, SenderSignedTransaction};
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
        wrapped: ObjectId,
        unwrapped: ObjectId,
        gas: ObjectId,
    }

    const CREATED_SHARED_INITIAL_VERSION: u64 = 42;

    fn effects_fixture() -> EffectsFixture {
        let transaction = owned_inputs_tx(1);
        let gas = transaction.transaction().gas()[0].object_id;
        let created_owned = ObjectId::random();
        let created_shared = ObjectId::random();
        let mutated_owned = ObjectId::random();
        let deleted = ObjectId::random();
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
            .with_wrapped_objects([(wrapped, Version::from_u64(2))])
            .with_unwrapped_objects([(unwrapped, Owner::Address(Address::ZERO))])
            .build();
        EffectsFixture {
            effects,
            created_owned,
            created_shared,
            mutated_owned,
            deleted,
            wrapped,
            unwrapped,
            gas,
        }
    }

    #[test]
    fn handler_latest_upserts_map_every_write_kind() {
        let fixture = effects_fixture();
        let lamport = fixture.effects.lamport_version();
        let index: CommitIndex = 9;
        // Every id in the fixture is written once, so keying by id is exact.
        let rows: BTreeMap<ObjectId, (Version, HandlerProcessedObject)> =
            handler_latest_upserts(&fixture.effects, index)
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
        assert_eq!(version(&fixture.mutated_owned), lamport);
        assert_eq!(rows[&fixture.gas].kind, HandlerProcessedObjectKind::Live);
        assert_eq!(
            rows[&fixture.unwrapped].kind,
            HandlerProcessedObjectKind::Live
        );

        assert_eq!(
            rows[&fixture.deleted].kind,
            HandlerProcessedObjectKind::Deleted
        );
        assert_eq!(version(&fixture.deleted), lamport);
        assert_eq!(
            rows[&fixture.wrapped].kind,
            HandlerProcessedObjectKind::Wrapped
        );
        assert_eq!(version(&fixture.wrapped), lamport);

        assert_eq!(rows.len(), 7);
    }

    #[test]
    fn sync_ahead_writes_carry_consumed_versions() {
        let fixture = effects_fixture();
        let lamport = fixture.effects.lamport_version();
        let old_metadata = fixture.effects.old_object_metadata();
        let writes: BTreeMap<ObjectId, SyncAheadWrite> =
            sync_ahead_writes(&fixture.effects, &old_metadata)
                .into_iter()
                .map(|write| (write.id, write))
                .collect();

        assert_eq!(writes[&fixture.created_owned].consumed, None);
        assert_eq!(writes[&fixture.created_shared].consumed, None);
        assert_eq!(writes[&fixture.unwrapped].consumed, None);
        assert_eq!(
            writes[&fixture.mutated_owned].consumed,
            Some(Version::from_u64(5))
        );
        assert_eq!(
            writes[&fixture.deleted].consumed,
            Some(Version::from_u64(3))
        );
        assert_eq!(
            writes[&fixture.wrapped].consumed,
            Some(Version::from_u64(2))
        );
        assert_eq!(writes[&fixture.gas].consumed, Some(Version::from_u64(1)));
        for write in writes.values() {
            assert_eq!(write.created, lamport);
        }
        assert_eq!(writes.len(), 7);
    }

    #[test]
    fn shelter_keys_cover_consumed_owned_inputs() {
        let fixture = effects_fixture();
        let keys = consumed_input_keys_to_shelter(&fixture.effects.old_object_metadata());
        let expected = [
            ObjectKey(fixture.mutated_owned, Version::from_u64(5)),
            ObjectKey(fixture.deleted, Version::from_u64(3)),
            ObjectKey(fixture.wrapped, Version::from_u64(2)),
            ObjectKey(fixture.gas, Version::from_u64(1)),
        ];
        assert_eq!(keys.len(), expected.len());
        for key in expected {
            assert!(keys.contains(&key), "missing shelter key {key:?}");
        }
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
        let old_metadata = effects.old_object_metadata();

        let rows = handler_latest_upserts(&effects, 4);
        assert!(rows.iter().all(|(key, _)| key.0 != shared_id));
        let writes = sync_ahead_writes(&effects, &old_metadata);
        assert!(writes.iter().all(|write| write.id != shared_id));
        let keys = consumed_input_keys_to_shelter(&old_metadata);
        assert!(keys.iter().all(|key| key.0 != shared_id));
    }

    #[test]
    fn commit_index_map_assign_and_drop() {
        let (assigned_commits, assigned_commits_receiver) = unbounded_channel("test");
        let state = HandlerObjectState {
            commit_index_by_key: DashMap::with_shard_amount(2048),
            keys_by_commit: Mutex::new(BTreeMap::new()),
            assigned_commits,
            assigned_commits_receiver: Mutex::new(Some(assigned_commits_receiver)),
            highest_fully_executed_commit: watch::Sender::new(0),
            handler_latest_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_overlay: RwLock::new(BTreeMap::new()),
            sheltered_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_record_deletions: Mutex::new(BTreeMap::new()),
            live_sync_ahead_records_count: AtomicU64::new(0),
            handler_latest_cache: new_handler_latest_cache(100),
            metrics: EpochMetrics::new(&Registry::new()),
        };
        let key_a = TransactionKey::Digest(TransactionDigest::random());
        let key_b = TransactionKey::Digest(TransactionDigest::random());
        let key_c = TransactionKey::RandomnessRound(0, RandomnessRound::new(1));
        state.assign_commit_to_transactions(3, vec![key_a]);
        state.assign_commit_to_transactions(4, vec![key_b, key_c]);
        assert_eq!(state.commit_index_of(&key_a), Some(3));
        assert_eq!(state.commit_index_of(&key_b), Some(4));
        assert_eq!(state.commit_index_of(&key_c), Some(4));
        state.drop_commit_assignments(3);
        assert_eq!(state.commit_index_of(&key_a), None);
        assert_eq!(state.commit_index_of(&key_b), Some(4));
        state.drop_commit_assignments(4);
        assert_eq!(state.commit_index_of(&key_b), None);
        assert_eq!(state.commit_index_of(&key_c), None);
    }
}
