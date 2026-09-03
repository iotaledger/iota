// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch object bookkeeping for P-COOL deterministic post-consensus
//! validation: object state as of the commit height being processed by the
//! consensus handler, independent of local execution and state-sync progress.
//!
//! Three views are kept, each an epoch table plus an in-memory overlay
//! holding the entries not yet durable:
//! - **handler-latest** - the latest state of every object touched by a commit
//!   the handler has processed, one row per object, overwritten in place. Blind
//!   to execution driven by state sync running ahead of the handler.
//! - **sync-ahead records** - per-object markers written by state-sync-driven
//!   execution the handler has not reached yet, restoring the pre-sync view
//!   (`base_version` is the version the chain grew from - latest before sync
//!   ran ahead; `None` for an object the sync-ahead chain itself created).
//!   Empty in normal operation.
//! - **sheltered objects** - full bytes of input versions consumed by
//!   sync-driven execution, kept until the handler passes the consuming commit,
//!   so content reads survive aggressive pruning. Empty in normal operation.
//!
//! [`HandlerObjectState`] owns the overlays, the digest -> commit-round map,
//! and every invariant on them: overlay-first reads, version-monotone
//! upserts, and eviction only after the corresponding table row is durable.
//!
//! The in-memory state changes at five points: the handler registers a
//! commit's kept digests before they can be scheduled
//! ([`HandlerObjectState::assign_commit`], via the epoch store); each
//! execution records its writes before they become readable - handler-latest
//! rows when the digest is in the round map, sync-ahead records and sheltered
//! bytes when it is not ([`HandlerObjectState::record_executed_transaction`]);
//! a fully executed commit applies its remaining upserts, queues durable
//! deletions for the sync records it caught up past, and drops its map
//! entries ([`HandlerObjectState::record_commit_fully_executed`]); and the
//! two eviction methods below clear overlay entries once their rows are
//! durable.
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
use iota_sdk_types::{
    ObjectDigest, ObjectId, OwnedObjectReference, Owner, TransactionDigest, TransactionEffects,
    Version, WriteKind,
};
use iota_types::{
    base_types::CommitRound,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaResult,
    object::Object,
    storage::ObjectKey,
};
use itertools::chain;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use typed_store::{Map, rocks::DBBatch};

use super::AuthorityEpochTables;
use crate::execution_cache::cache_types::{IsNewer, MonotonicCache, Ticket};

/// Whether a handler-latest row describes a live object or a tombstone.
///
/// `Deleted` and `Wrapped` yield the same drop verdict; the split mirrors the
/// store's two tombstone digests and their different futures - a wrapped
/// object can reappear via unwrap at a higher version, a deleted one never
/// does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandlerLatestObjectKind {
    Live,
    Deleted,
    Wrapped,
}

/// The latest state of an object as of the consensus handler's frontier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerLatestObject {
    pub version: Version,
    pub digest: ObjectDigest,
    pub kind: HandlerLatestObjectKind,
    /// Round of the commit whose execution produced this row; reads at commit
    /// C treat rows above the horizon (C − K) as missing
    pub produced_at: CommitRound,
    /// `Some` iff the object was created as shared this epoch; doubles as the
    /// created-shared flag for the shared-input checks.
    pub initial_shared_version: Option<Version>,
}

impl IsNewer for HandlerLatestObject {
    fn is_newer_than(&self, other: &Self) -> bool {
        self.version > other.version
    }
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

/// Derives the handler-latest upserts for one executed transaction of the
/// commit at `produced_at`: created / mutated / unwrapped objects become
/// `Live` rows, deletions and wraps become tombstone rows.
pub fn handler_latest_upserts(
    effects: &TransactionEffects,
    produced_at: CommitRound,
) -> Vec<(ObjectId, HandlerLatestObject)> {
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
            owned_ref.reference.object_id,
            HandlerLatestObject {
                version: owned_ref.reference.version,
                digest: owned_ref.reference.digest,
                kind: HandlerLatestObjectKind::Live,
                produced_at,
                initial_shared_version,
            },
        ));
    }
    let deleted = chain(effects.deleted(), effects.unwrapped_then_deleted())
        .map(|reference| (reference, HandlerLatestObjectKind::Deleted));
    let wrapped = effects
        .wrapped()
        .into_iter()
        .map(|reference| (reference, HandlerLatestObjectKind::Wrapped));
    for (reference, kind) in deleted.chain(wrapped) {
        rows.push((
            reference.object_id,
            HandlerLatestObject {
                version: reference.version,
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

/// In-memory side of the bookkeeping plus every operation composing it with
/// the durable tables. Overlays hold entries of commits whose rows are not
/// yet durable; an entry leaves only after the corresponding table row is
/// durable, and the read paths check the overlay first, so the transient
/// both-present state is harmless.
pub struct HandlerObjectState {
    /// Digest → producing commit round for every kept transaction of commits
    /// the handler has processed but whose executions have not all completed.
    /// The execution hook consults it: hit → handler-latest upsert; miss →
    /// sync-ahead execution. Never persisted: replay after restart re-runs
    /// `assign_commit_to_transactions` before any (re-)execution can ask, and
    /// commits below the durable resume point never consult it again.
    commit_round_by_digest: DashMap<TransactionDigest, CommitRound>,
    /// The digests assigned per round, so a fully executed commit can drop
    /// its map entries.
    digests_by_commit: Mutex<BTreeMap<CommitRound, Vec<TransactionDigest>>>,

    handler_latest_overlay: RwLock<BTreeMap<ObjectId, HandlerLatestObject>>,
    sync_ahead_overlay: RwLock<BTreeMap<ObjectId, SyncAheadRecord>>,
    sheltered_overlay: RwLock<BTreeMap<ObjectKey, Arc<Object>>>,

    /// Sync-ahead records the handler has caught up past, pending deletion
    /// from the durable table; drained into the next flush batch.
    sync_record_deletions: Mutex<BTreeSet<ObjectId>>,
    /// Sync-ahead records currently alive (created and not yet queued for
    /// deletion), so the per-commit cleanup can skip its table lookups
    /// entirely in normal operation, when no record exists. Increments and
    /// deletion-queue insertions pair exactly: a record recreated while its
    /// deletion is queued cancels the deletion and counts as a fresh record.
    live_sync_records: AtomicU64,

    /// Read-through cache over the durable handler-latest table, so the cold
    /// path of the hot per-input lookup is usually served without a table
    /// read. Holds present rows only (absence could go stale the moment a
    /// flush lands); refreshed write-through by `evict_flushed_commit_rows`,
    /// with the ticket protocol rejecting stale inserts from readers racing
    /// a flush. The other two tables get no cache: they are consulted only
    /// after a handler-latest miss and are empty in normal operation, where
    /// a table miss is a cheap bloom-filter negative.
    handler_latest_cache: MonotonicCache<ObjectId, HandlerLatestObject>,
}

impl HandlerObjectState {
    pub fn new(tables: &AuthorityEpochTables) -> Self {
        // Nonzero only when reopening mid-epoch with sync-ahead records on
        // disk; counting them keeps the cleanup short-circuit sound across a
        // restart.
        let live_sync_records = tables
            .sync_ahead_records
            .safe_iter()
            .try_fold(0u64, |count, entry| entry.map(|_| count + 1))
            .expect("AuthorityEpochTables should contain valid sync-ahead records");
        Self {
            commit_round_by_digest: DashMap::with_shard_amount(2048),
            digests_by_commit: Mutex::new(BTreeMap::new()),
            handler_latest_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_overlay: RwLock::new(BTreeMap::new()),
            sheltered_overlay: RwLock::new(BTreeMap::new()),
            sync_record_deletions: Mutex::new(BTreeSet::new()),
            live_sync_records: AtomicU64::new(live_sync_records),
            handler_latest_cache: MonotonicCache::new(randomize_cache_capacity_in_tests(100_000)),
        }
    }

    /// Records the kept transactions of commit `round`, before any of them
    /// can be scheduled for execution.
    pub fn assign_commit(&self, round: CommitRound, digests: Vec<TransactionDigest>) {
        for digest in &digests {
            self.commit_round_by_digest.insert(*digest, round);
        }
        self.digests_by_commit.lock().insert(round, digests);
    }

    /// The commit round that kept this transaction, if the handler has
    /// processed that commit and it is not fully executed yet.
    pub fn commit_round_of(&self, digest: &TransactionDigest) -> Option<CommitRound> {
        self.commit_round_by_digest.get(digest).map(|round| *round)
    }

    /// Drops the digest → round entries of a fully executed commit, keeping
    /// the map bounded.
    pub fn drop_commit_assignments(&self, round: CommitRound) {
        if let Some(digests) = self.digests_by_commit.lock().remove(&round) {
            for digest in digests {
                self.commit_round_by_digest.remove(&digest);
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
    /// `loaded_input_objects` must contain every consumed owned input at its
    /// consumed version - including dynamic-field children and received
    /// objects, which are not part of the declared input objects; a missing
    /// one cannot be sheltered against pruning and is reported through
    /// `debug_fatal`.
    pub fn record_executed_transaction(
        &self,
        tables: &AuthorityEpochTables,
        effects: &TransactionEffects,
        loaded_input_objects: &[Object],
    ) -> IotaResult {
        if let Some(round) = self.commit_round_of(effects.transaction_digest()) {
            self.upsert_handler_latest_rows(tables, &handler_latest_upserts(effects, round))
        } else {
            let old_metadata = effects.old_object_metadata();
            self.upsert_sync_ahead_writes(tables, sync_ahead_writes(effects, &old_metadata))?;
            self.shelter_consumed_inputs(
                consumed_input_keys_to_shelter(&old_metadata),
                loaded_input_objects,
                effects.transaction_digest(),
            );
            Ok(())
        }
    }

    /// Marks commit `round` fully executed: applies the commit's
    /// handler-latest upserts (covering executions that raced the map
    /// registration), removes sync-ahead records whose chains the handler has
    /// now caught up past, and drops the commit's digest → round map entries.
    ///
    /// Sheltered bytes are deliberately not evicted here: their eviction keys
    /// off the *flushed* frontier, because a crash before this commit's
    /// output flushes replays and re-validates it, and those reads may still
    /// need the bytes for versions the store has already pruned.
    pub fn record_commit_fully_executed(
        &self,
        tables: &AuthorityEpochTables,
        round: CommitRound,
        upserts: &[(ObjectId, HandlerLatestObject)],
    ) -> IotaResult {
        // The upserts must be visible to readers before the sync records are
        // removed: a validation read that finds neither concludes the object
        // is untouched this epoch and consults epoch-start state.
        self.upsert_handler_latest_rows(tables, upserts)?;
        self.remove_passed_sync_records(tables, upserts)?;
        self.drop_commit_assignments(round);
        Ok(())
    }

    /// The latest state of `id` as of the handler frontier, from the overlay
    /// or the durable table.
    ///
    /// Returns the row unconditionally; a caller validating at a commit round
    /// must apply the reading condition itself (`produced_at` above its
    /// horizon answers missing). Writers and cleanup need the unfiltered
    /// frontier row, so no horizon is applied here.
    pub fn handler_latest(
        &self,
        tables: &AuthorityEpochTables,
        id: &ObjectId,
    ) -> IotaResult<Option<HandlerLatestObject>> {
        // The ticket snapshots this object's cache generation before any
        // read. The cache fill below succeeds only if the generation is
        // still the same; a flush that caches a newer row in the meantime
        // (`Ticket::Write` in `evict_flushed_commit_rows`) bumps it, and the
        // fill is discarded - the row read from the table here would be
        // older than what the flush cached, and must not replace it.
        let ticket = self.handler_latest_cache.get_ticket_for_read(id);
        if let Some(row) = self.handler_latest_overlay.read().get(id) {
            return Ok(Some(*row));
        }
        if let Some(entry) = self.handler_latest_cache.get(id) {
            return Ok(Some(*entry.lock()));
        }
        let row = tables.handler_latest_objects.get(id)?;
        if let Some(row) = row {
            self.handler_latest_cache.insert(id, row, ticket).ok();
        }
        Ok(row)
    }

    /// The sync-ahead record for `id`, from the overlay or the durable table.
    pub fn sync_record(
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
    ) -> IotaResult<Option<Arc<Object>>> {
        if let Some(object) = self.sheltered_overlay.read().get(key) {
            return Ok(Some(object.clone()));
        }
        Ok(tables.sheltered_objects.get(key)?.map(Arc::new))
    }

    /// Stages a commit's handler-latest rows into `batch` - the quarantine
    /// flush of the commit's own output, atomic with `last_consensus_stats` -
    /// together with every queued sync-record deletion. The deletions must
    /// ride this batch and no other: a deleted record's reads are answered by
    /// handler-latest rows, so a deletion that became durable without them
    /// (say, through the checkpoint executor's batch) would, after a crash,
    /// leave reads falling through to the epoch-start rule against sync-ahead
    /// store state. After the batch is durably written - never before - pass
    /// the same rows to [`Self::evict_flushed_commit_rows`].
    ///
    /// These writes carry no version guard: the caller must flush commits in
    /// commit order, one flush at a time (the quarantine flush does), or an
    /// older row could overwrite a newer durable one.
    pub fn write_commit_rows_to_batch(
        &self,
        tables: &AuthorityEpochTables,
        batch: &mut DBBatch,
        handler_rows: &[(ObjectId, HandlerLatestObject)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.handler_latest_objects,
            handler_rows.iter().map(|(id, row)| (id, row)),
        )?;
        // A deletion lost to a batch that never commits is re-queued when the
        // commit replays.
        let deletions = std::mem::take(&mut *self.sync_record_deletions.lock());
        batch.delete_batch(&tables.sync_ahead_records, deletions)?;
        Ok(())
    }

    /// Evicts a commit's handler-latest overlay entries once their rows are
    /// durable. The cache is refreshed write-through before each entry is
    /// removed, so a concurrent reader never observes a row absent from both;
    /// an entry upserted again since the flush (a newer, still-unflushed row)
    /// is kept.
    pub fn evict_flushed_commit_rows(&self, handler_rows: &[(ObjectId, HandlerLatestObject)]) {
        let mut overlay = self.handler_latest_overlay.write();
        for (id, row) in handler_rows {
            self.handler_latest_cache
                .insert(id, *row, Ticket::Write)
                .ok();
            if overlay.get(id) == Some(row) {
                overlay.remove(id);
            }
        }
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
        shelter_rows: &[(ObjectKey, Arc<Object>)],
    ) -> IotaResult {
        batch.insert_batch(
            &tables.sync_ahead_records,
            sync_rows.iter().map(|(id, record)| (id, record)),
        )?;
        batch.insert_batch(
            &tables.sheltered_objects,
            shelter_rows
                .iter()
                .map(|(key, object)| (key, object.as_ref())),
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
        shelter_rows: &[(ObjectKey, Arc<Object>)],
    ) {
        {
            let mut overlay = self.sync_ahead_overlay.write();
            for (id, record) in sync_rows {
                if overlay.get(id) == Some(record) {
                    overlay.remove(id);
                }
            }
        }
        let mut overlay = self.sheltered_overlay.write();
        for (key, _) in shelter_rows {
            overlay.remove(key);
        }
    }

    /// The number of entries in the (handler-latest, sync-ahead, sheltered)
    /// overlays, for asserting eviction behavior.
    #[cfg(test)]
    pub fn overlay_lens_for_testing(&self) -> (usize, usize, usize) {
        (
            self.handler_latest_overlay.read().len(),
            self.sync_ahead_overlay.read().len(),
            self.sheltered_overlay.read().len(),
        )
    }

    fn upsert_handler_latest_rows(
        &self,
        tables: &AuthorityEpochTables,
        rows: &[(ObjectId, HandlerLatestObject)],
    ) -> IotaResult {
        if rows.is_empty() {
            return Ok(());
        }
        let mut overlay = self.handler_latest_overlay.write();
        for (id, row) in rows {
            match overlay.get(id) {
                // The writers racing on one id within a commit all write the
                // same value; across commits a later commit's row always has
                // a higher version, so the guard makes the row a monotone
                // function of handler progress.
                Some(current) => {
                    if row.version >= current.version {
                        overlay.insert(*id, *row);
                    }
                }
                None => {
                    // Compare against the durable row: watchers can fire out
                    // of commit order, so once the overlay entry is evicted
                    // this is the only guard keeping an older row from
                    // shadowing the newer durable one through the
                    // overlay-first read; strict `>` also keeps a watcher
                    // firing after its commit flushed from re-adding a row
                    // nothing would ever evict.
                    let durable = tables.handler_latest_objects.get(id)?;
                    if durable.is_none_or(|durable| row.version > durable.version) {
                        overlay.insert(*id, *row);
                    }
                }
            }
        }
        Ok(())
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
        let mut deletions = self.sync_record_deletions.lock();
        for write in writes {
            let current = match overlay.get(&write.id) {
                Some(record) => Some(*record),
                // A record with a queued deletion is logically gone (the
                // handler caught up past its chain) but stays in the table
                // until the queue drains into a commit flush; it must be
                // invisible here, or a new chain would extend the dead record
                // and inherit its stale `base_version`.
                None if deletions.contains(&write.id) => None,
                None => tables.sync_ahead_records.get(&write.id)?,
            };
            // Only an extension of the chain updates the record
            // (re-executions after a restart replay the same writes).
            if current.is_none_or(|record| write.created > record.latest_created) {
                if current.is_none() {
                    // Cancel any queued deletion of the dead record: it
                    // drains into a later flush batch, which must not destroy
                    // the new record's durable row.
                    deletions.remove(&write.id);

                    self.live_sync_records.fetch_add(1, Ordering::Relaxed);
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
        Ok(())
    }

    fn shelter_consumed_inputs(
        &self,
        keys: Vec<ObjectKey>,
        loaded_input_objects: &[Object],
        tx_digest: &TransactionDigest,
    ) {
        if keys.is_empty() {
            return;
        }
        let mut needed: BTreeSet<ObjectKey> = keys.into_iter().collect();
        let mut overlay = self.sheltered_overlay.write();
        for object in loaded_input_objects {
            let key = ObjectKey(object.id(), object.version());
            // A replay after a crash may re-insert rows that are already
            // durable; that is fine - the checkpoint executor's persist step
            // re-runs on the same replay and evicts them again, and the
            // bytes are identical either way.
            if needed.remove(&key) {
                overlay.insert(key, Arc::new(object.clone()));
            }
        }
        for key in needed {
            debug_fatal!(
                "input {key:?} consumed by sync-executed transaction {tx_digest} missing from \
                 the loaded inputs; its bytes cannot be sheltered against pruning"
            );
        }
    }

    fn remove_passed_sync_records(
        &self,
        tables: &AuthorityEpochTables,
        upserts: &[(ObjectId, HandlerLatestObject)],
    ) -> IotaResult {
        // Normal operation: no sync-ahead record exists anywhere, so the
        // per-commit cleanup costs one atomic load instead of a table lookup
        // per written object.
        if self.live_sync_records.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }
        let mut overlay = self.sync_ahead_overlay.write();
        let mut deletions = self.sync_record_deletions.lock();
        for (id, row) in upserts {
            let record = match overlay.get(id) {
                Some(record) => Some(*record),
                None => tables.sync_ahead_records.get(id)?,
            };
            // The handler has caught up past the whole sync-ahead chain; the
            // handler-latest row (already visible) now answers every read the
            // record used to.
            if record.is_some_and(|record| record.latest_created <= row.version) {
                overlay.remove(id);
                // Queue the durable deletion even when the record was found
                // only in the overlay: an extended record leaves its older
                // durable row behind, and the checkpoint executor's persist
                // step (which derives rows from effects, not the overlay) can
                // still write this record after the removal. Deleting a key
                // that never became durable is a no-op.
                if deletions.insert(*id) {
                    self.live_sync_records.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, ObjectReference, SenderSignedTransaction};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{effects::TestEffectsBuilder, transaction::TransactionAPI};

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
        let round: CommitRound = 9;
        let rows: BTreeMap<ObjectId, HandlerLatestObject> =
            handler_latest_upserts(&fixture.effects, round)
                .into_iter()
                .collect();

        let created_owned = &rows[&fixture.created_owned];
        assert_eq!(created_owned.kind, HandlerLatestObjectKind::Live);
        assert_eq!(created_owned.version, lamport);
        assert_eq!(created_owned.produced_at, round);
        assert_eq!(created_owned.initial_shared_version, None);

        let created_shared = &rows[&fixture.created_shared];
        assert_eq!(created_shared.kind, HandlerLatestObjectKind::Live);
        assert_eq!(
            created_shared.initial_shared_version,
            Some(Version::from_u64(CREATED_SHARED_INITIAL_VERSION))
        );

        assert_eq!(
            rows[&fixture.mutated_owned].kind,
            HandlerLatestObjectKind::Live
        );
        assert_eq!(rows[&fixture.mutated_owned].version, lamport);
        assert_eq!(rows[&fixture.gas].kind, HandlerLatestObjectKind::Live);
        assert_eq!(rows[&fixture.unwrapped].kind, HandlerLatestObjectKind::Live);

        assert_eq!(
            rows[&fixture.deleted].kind,
            HandlerLatestObjectKind::Deleted
        );
        assert_eq!(rows[&fixture.deleted].version, lamport);
        assert_eq!(
            rows[&fixture.wrapped].kind,
            HandlerLatestObjectKind::Wrapped
        );
        assert_eq!(rows[&fixture.wrapped].version, lamport);

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
        assert!(rows.iter().all(|(id, _)| *id != shared_id));
        let writes = sync_ahead_writes(&effects, &old_metadata);
        assert!(writes.iter().all(|write| write.id != shared_id));
        let keys = consumed_input_keys_to_shelter(&old_metadata);
        assert!(keys.iter().all(|key| key.0 != shared_id));
    }

    #[test]
    fn commit_round_map_assign_and_drop() {
        let state = HandlerObjectState {
            commit_round_by_digest: DashMap::with_shard_amount(2048),
            digests_by_commit: Mutex::new(BTreeMap::new()),
            handler_latest_overlay: RwLock::new(BTreeMap::new()),
            sync_ahead_overlay: RwLock::new(BTreeMap::new()),
            sheltered_overlay: RwLock::new(BTreeMap::new()),
            sync_record_deletions: Mutex::new(BTreeSet::new()),
            live_sync_records: AtomicU64::new(0),
            handler_latest_cache: MonotonicCache::new(100),
        };
        let digest_a = TransactionDigest::random();
        let digest_b = TransactionDigest::random();
        state.assign_commit(3, vec![digest_a]);
        state.assign_commit(4, vec![digest_b]);
        assert_eq!(state.commit_round_of(&digest_a), Some(3));
        assert_eq!(state.commit_round_of(&digest_b), Some(4));
        state.drop_commit_assignments(3);
        assert_eq!(state.commit_round_of(&digest_a), None);
        assert_eq!(state.commit_round_of(&digest_b), Some(4));
    }
}
