// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO(https://github.com/iotaledger/iota/issues/12763): remove this
// module once every database has migrated its pre-bucket ledger and checkpoint
// history.

//! The one-time move of the ledger and checkpoint history written before this
//! build into the per-epoch buckets.
//!
//! A transaction's body, effects, events, loaded runtime objects and
//! finalizing checkpoint now go into the bucket of the epoch that executed it,
//! and a checkpoint's contents and digest-keyed summary into the bucket of the
//! epoch that closed it. A database written by an earlier build still holds
//! all of that in the flat tables of the perpetual and checkpoint stores,
//! where nothing reads it and no retention reclaims it, so it is walked once
//! and moved here.
//!
//! Every row goes to the epoch it belongs to rather than to the epoch the walk
//! runs in, because the epoch is what decides when the row expires: a node
//! switching from unlimited to a finite retention must drop the epochs that
//! are already outside its window, and a row filed under the wrong epoch would
//! either outlive the rest of its transaction's record or disappear ahead of
//! it. Where the node's retention has already left an epoch behind, its rows
//! are deleted instead of moved, so the walk does not create buckets the next
//! reconfiguration would only drop again.
//!
//! The walk runs at node startup, before any service starts, and it is not
//! optional: nothing else reads the flat rows it moves, so until it finishes a
//! node cannot resolve a checkpoint written before the upgrade by digest, and
//! the checkpoint executor turns that into a panic on any node past epoch 0. A
//! failure is returned rather than retried, and the watermarks it records are
//! durable, so the next start resumes where this one stopped instead of
//! beginning again.

use std::{collections::BTreeMap, ops::Bound, sync::Arc};

use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointDigest, TransactionDigest, TransactionEffectsDigest,
};
use iota_types::{
    committee::EpochId,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, info};
use typed_store::{
    rocks::{DBMap, TaggedDBMap},
    traits::Map,
};

use crate::{
    authority::{
        AuthorityStore,
        authority_store_tables::AuthorityPerpetualTables,
        historic_ledger::{HistoricLedger, HistoricLedgerBucket},
    },
    checkpoints::CheckpointStore,
};

/// Rows one slice moves before it writes its batch. A slice stops at this many
/// wherever it is, so it bounds how much of a flat table the slice holds in
/// memory and how much of it an interrupted run has to read again.
const KEYS_PER_SLICE: usize = 5_000;

/// How far the migration has got through the flat perpetual ledger tables.
///
/// The variants are the order the tables are drained in, and that order is not
/// free: `executed_effects`, `effects` and
/// `executed_transactions_to_checkpoint` are where the tables above them take
/// their epoch from, so they are drained after their readers. The digest a
/// variant carries is the last key moved out of that table, and `None` means
/// the table has not been touched yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerBacklogMigrationProgress {
    Transactions(Option<TransactionDigest>),
    Events(Option<TransactionDigest>),
    ExecutedEffects(Option<TransactionDigest>),
    Effects(Option<TransactionEffectsDigest>),
    TransactionCheckpoints(Option<TransactionDigest>),
    /// Every flat perpetual ledger table is drained, and later node starts do
    /// nothing.
    Done,
}

/// How far the migration has got through the checkpoint store's flat tables.
///
/// The digest-keyed summaries are drained first, since each one names the
/// contents row that belongs with it; whatever contents rows are left
/// afterwards belong to no summary this table holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointBacklogMigrationProgress {
    Summaries(Option<CheckpointDigest>),
    ContentsWithoutSummary(Option<CheckpointContentsDigest>),
    /// Both flat checkpoint tables are drained, and later node starts do
    /// nothing.
    Done,
}

/// Moves the ledger and checkpoint history written before this build into the
/// bucket of the epoch each row belongs to, deleting the rows of the epochs
/// `epochs_to_retain_for_checkpoints` has already left behind. Returns once
/// both stores' flat tables are empty, at once on a database an earlier run
/// already drained.
///
/// Call this before starting any service: until it returns, a checkpoint
/// written before the upgrade cannot be resolved by digest and a transaction
/// executed before it cannot be read at all.
///
/// `epoch` is the epoch the node is starting in. A failure is returned rather
/// than retried, since nothing that comes after may run until the move is
/// finished; the watermarks it records are durable, so the next start resumes
/// where this one stopped.
pub async fn migrate(
    store: Arc<AuthorityStore>,
    checkpoint_store: Arc<CheckpointStore>,
    epoch: EpochId,
    epochs_to_retain_for_checkpoints: Option<u64>,
) -> IotaResult<()> {
    info!(
        "migrating the ledger and checkpoint history written before this build into the epoch \
         buckets"
    );

    // Each slice is a range scan and a write batch, both blocking.
    tokio::task::spawn_blocking(move || {
        LedgerBacklogMigration::new(
            &store,
            checkpoint_store,
            epoch,
            epochs_to_retain_for_checkpoints,
        )
        .run()
    })
    .await
    .map_err(|e| IotaError::Storage(format!("the ledger backlog migration task failed: {e}")))?
}

/// One walk over the flat ledger and checkpoint tables, moving every row into
/// the bucket of the epoch it belongs to.
struct LedgerBacklogMigration {
    perpetual_tables: Arc<AuthorityPerpetualTables>,
    historic_ledger: Arc<HistoricLedger>,
    checkpoint_store: Arc<CheckpointStore>,
    /// The epoch the node is starting in, and the epoch a row whose own epoch
    /// nothing on disk records is filed under.
    epoch: EpochId,
    /// The oldest epoch this node's retention still keeps. Rows below it are
    /// deleted rather than moved, since the next reconfiguration would drop
    /// their bucket anyway.
    floor: EpochId,
    keys_per_slice: usize,
}

/// How many rows a slice, a table or the whole walk moved into a bucket, and
/// how many it deleted because their epoch was already below the floor.
#[derive(Default)]
struct MigrationCounts {
    moved: usize,
    expired: usize,
}

impl MigrationCounts {
    fn add(&mut self, other: &Self) {
        self.moved += other.moved;
        self.expired += other.expired;
    }
}

/// One slice's worth of rows read from a flat table, sorted by what is to
/// become of them.
struct Slice<K, V> {
    /// The rows to move, grouped by the epoch whose bucket they belong in.
    by_epoch: BTreeMap<EpochId, Vec<(K, V)>>,
    /// The rows whose epoch is below the retention floor. Kept rather than
    /// counted, because a caller may still have to follow what such a row
    /// names.
    expired: Vec<(K, V)>,
    /// Every key read, whether moved or expired; all are deleted from the flat
    /// table.
    keys: Vec<K>,
    /// The last key read, `None` when nothing was left above the resume point.
    watermark: Option<K>,
    /// Whether rows are left above `watermark`.
    sliced: bool,
}

impl<K, V> Slice<K, V> {
    fn new() -> Self {
        Self {
            by_epoch: BTreeMap::new(),
            expired: Vec::new(),
            keys: Vec::new(),
            watermark: None,
            sliced: false,
        }
    }

    /// The progress this slice leaves behind: its watermark while rows are
    /// left in the table it read, and `finished` once that table is drained.
    fn progress<P>(&self, at: impl Fn(Option<K>) -> P, finished: P) -> P
    where
        K: Copy,
    {
        match self.watermark {
            Some(key) if self.sliced => at(Some(key)),
            _ => finished,
        }
    }
}

impl LedgerBacklogMigration {
    fn new(
        store: &AuthorityStore,
        checkpoint_store: Arc<CheckpointStore>,
        epoch: EpochId,
        epochs_to_retain_for_checkpoints: Option<u64>,
    ) -> Self {
        // The floor the last reconfiguration applied: it counted its retention
        // back from the epoch it left, one below the epoch the node is now
        // running in. Counting back from the running epoch instead would
        // delete an epoch this node still serves — including the one the
        // executed and synced watermarks name while the running epoch's first
        // checkpoint has yet to be executed.
        let floor =
            epochs_to_retain_for_checkpoints.map_or(0, |retained| epoch.saturating_sub(retained));
        Self {
            perpetual_tables: store.perpetual_tables.clone(),
            historic_ledger: store.get_historic_ledger().clone(),
            checkpoint_store,
            epoch,
            floor,
            keys_per_slice: KEYS_PER_SLICE,
        }
    }

    /// Drains both stores' flat tables, then records how much of the
    /// checkpoint range the node no longer holds.
    fn run(&self) -> IotaResult<()> {
        let mut counts = self.drain_ledger()?;
        counts.add(&self.drain_checkpoints()?);
        info!(
            moved = counts.moved,
            expired = counts.expired,
            floor = self.floor,
            "the ledger backlog migration reached the end of the flat tables"
        );

        // What the node still holds is what it may advertise: below the floor
        // the whole history is gone, and a state-sync peer told those
        // checkpoints are available would ask for contents that are gone.
        // Asked of the floor rather than of what this run deleted, since a run
        // that resumed past the last expired slice deleted nothing itself and
        // would otherwise leave the claim standing until the next
        // reconfiguration. The call is monotonic, so repeating it on every
        // start costs nothing. Reported rather than returned — the history
        // itself has moved, and failing the start over a watermark would leave
        // the node unable to come up at all.
        if self.floor > 0 {
            if let Err(e) = self
                .checkpoint_store
                .advance_highest_pruned_checkpoint(self.floor)
            {
                error!("failed to record the checkpoint range the migration deleted: {e}");
            }
        }
        Ok(())
    }

    /// Moves every row of the six flat perpetual ledger tables into its
    /// epoch's bucket.
    fn drain_ledger(&self) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let mut counts = MigrationCounts::default();
        loop {
            // Read back from disk rather than carried in memory, so a resumed
            // run and an uninterrupted one take the same path.
            let progress = self
                .perpetual_tables
                .ledger_backlog_migration_progress
                .get(&())?
                .unwrap_or(Progress::Transactions(None));
            let slice = match progress {
                Progress::Transactions(from) => self.move_transactions(from)?,
                Progress::Events(from) => self.move_events(from)?,
                Progress::ExecutedEffects(from) => self.move_executed_effects(from)?,
                Progress::Effects(from) => self.move_effects(from)?,
                Progress::TransactionCheckpoints(from) => {
                    self.move_transaction_checkpoints(from)?
                }
                Progress::Done => return Ok(counts),
            };
            counts.add(&slice);
        }
    }

    /// Moves every row of the checkpoint store's two flat tables into its
    /// epoch's bucket.
    fn drain_checkpoints(&self) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let mut counts = MigrationCounts::default();
        loop {
            let progress = self
                .checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .get(&())?
                .unwrap_or(Progress::Summaries(None));
            let slice = match progress {
                Progress::Summaries(from) => self.move_checkpoint_summaries(from)?,
                Progress::ContentsWithoutSummary(from) => {
                    self.move_contents_without_summary(from)?
                }
                Progress::Done => return Ok(counts),
            };
            counts.add(&slice);
        }
    }

    /// The epoch a transaction-keyed row belongs to, which is the epoch that
    /// executed the transaction.
    ///
    /// Taken from `executed_transactions_to_checkpoint`, which records it for
    /// every transaction a fullnode has finalized, and otherwise from the
    /// transaction's own effects, which a validator has as well — that table
    /// is written on fullnodes only. The two agree: a checkpoint of an epoch
    /// finalizes only transactions that epoch executed.
    ///
    /// A transaction whose epoch neither records — a body persisted or synced
    /// but never executed — is filed under the epoch the migration runs in.
    /// That keeps it for a whole retention window instead of risking an expiry
    /// that is due already, which is the same choice the object backlog sweep
    /// makes for the versions it cannot place.
    fn transaction_epoch(&self, digest: &TransactionDigest) -> IotaResult<EpochId> {
        let tables = &self.perpetual_tables;
        if let Some((epoch, _)) = tables.executed_transactions_to_checkpoint.get(digest)? {
            return Ok(epoch);
        }
        if let Some(effects_digest) = tables.executed_effects.get(digest)? {
            if let Some(effects) = tables.effects.get(&effects_digest)? {
                return Ok(effects.epoch());
            }
        }
        Ok(self.epoch)
    }

    /// Reads up to [`Self::keys_per_slice`] rows of `flat` above
    /// `resume_above`, sorting each into the bucket it belongs in or into the
    /// expired rows by the epoch `epoch_of` gives it.
    fn read_slice<K, V>(
        &self,
        flat: &DBMap<K, V>,
        resume_above: Option<K>,
        epoch_of: impl Fn(&K, &V) -> IotaResult<EpochId>,
    ) -> IotaResult<Slice<K, V>>
    where
        K: Serialize + DeserializeOwned + Copy,
        V: Serialize + DeserializeOwned,
    {
        let lower_bound = resume_above.map_or(Bound::Unbounded, Bound::Excluded);
        let mut slice = Slice::new();
        for (read, row) in flat
            .safe_range_iter((lower_bound, Bound::Unbounded))
            .enumerate()
        {
            if read == self.keys_per_slice {
                // One row past the slice, read but left undecided: it is what
                // says rows are left, and the next slice reads it again.
                slice.sliced = true;
                break;
            }
            let (key, value) = row?;
            let epoch = epoch_of(&key, &value)?;
            slice.keys.push(key);
            slice.watermark = Some(key);
            if epoch < self.floor {
                slice.expired.push((key, value));
            } else {
                slice.by_epoch.entry(epoch).or_default().push((key, value));
            }
        }
        Ok(slice)
    }

    /// Moves one slice of a flat perpetual ledger table into the buckets and
    /// records how far it got.
    ///
    /// The bucket inserts, the flat deletes and the progress row are one
    /// batch, so a row is always in exactly one of the two places and the
    /// watermark cannot advance without the move landing.
    fn move_ledger_slice<K, V, W>(
        &self,
        flat: &DBMap<K, V>,
        slice: Slice<K, V>,
        bucket_table: impl Fn(&HistoricLedgerBucket) -> &TaggedDBMap<K, W>,
        into_row: impl Fn(V) -> W,
        progress: LedgerBacklogMigrationProgress,
    ) -> IotaResult<MigrationCounts>
    where
        K: Serialize + DeserializeOwned + Copy,
        V: Serialize + DeserializeOwned,
        W: Serialize + DeserializeOwned,
    {
        let mut batch = flat.batch();
        let mut moved = 0;
        for (epoch, rows) in slice.by_epoch {
            let bucket = self.historic_ledger.ensure(epoch)?;
            moved += rows.len();
            batch.insert_batch_tagged(
                bucket_table(bucket.as_ref()),
                rows.into_iter().map(|(key, value)| (key, into_row(value))),
            )?;
        }
        batch.delete_batch(flat, &slice.keys)?;
        batch.insert_batch(
            &self.perpetual_tables.ledger_backlog_migration_progress,
            [((), progress)],
        )?;
        batch.write()?;

        let expired = slice.expired.len();
        debug!(
            moved,
            expired, "migrated a slice of the flat ledger history"
        );
        Ok(MigrationCounts { moved, expired })
    }

    fn move_transactions(&self, from: Option<TransactionDigest>) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.transactions;
        let slice = self.read_slice(flat, from, |digest, _| self.transaction_epoch(digest))?;
        let progress = slice.progress(Progress::Transactions, Progress::Events(None));
        self.move_ledger_slice(
            flat,
            slice,
            |bucket| &bucket.transactions,
            |row| row,
            progress,
        )
    }

    fn move_events(&self, from: Option<TransactionDigest>) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.events_2;
        let slice = self.read_slice(flat, from, |digest, _| self.transaction_epoch(digest))?;
        let progress = slice.progress(Progress::Events, Progress::ExecutedEffects(None));
        self.move_ledger_slice(flat, slice, |bucket| &bucket.events, |row| row, progress)
    }

    fn move_executed_effects(
        &self,
        from: Option<TransactionDigest>,
    ) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.executed_effects;
        let slice = self.read_slice(flat, from, |digest, _| self.transaction_epoch(digest))?;
        let progress = slice.progress(Progress::ExecutedEffects, Progress::Effects(None));
        self.move_ledger_slice(
            flat,
            slice,
            |bucket| &bucket.executed_effects,
            |row| row,
            progress,
        )
    }

    fn move_effects(&self, from: Option<TransactionEffectsDigest>) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.effects;
        // Keyed by effects digest, so the epoch comes from the effects
        // themselves. It is the epoch the execution record keyed by
        // transaction digest resolves to as well, which is what keeps a
        // transaction's effects in the bucket that record names.
        let slice = self.read_slice(flat, from, |_, effects| Ok(effects.epoch()))?;
        let progress = slice.progress(Progress::Effects, Progress::TransactionCheckpoints(None));
        self.move_ledger_slice(flat, slice, |bucket| &bucket.effects, |row| row, progress)
    }

    fn move_transaction_checkpoints(
        &self,
        from: Option<TransactionDigest>,
    ) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.executed_transactions_to_checkpoint;
        let slice = self.read_slice(flat, from, |_, (epoch, _)| Ok(*epoch))?;
        let progress = slice.progress(Progress::TransactionCheckpoints, Progress::Done);
        // The bucket's epoch is the epoch of the finalizing checkpoint, so the
        // row there holds only the sequence number.
        self.move_ledger_slice(
            flat,
            slice,
            |bucket| &bucket.tx_to_checkpoint,
            |(_, sequence)| sequence,
            progress,
        )
    }

    /// Moves one slice of the flat digest-keyed checkpoint summaries into the
    /// buckets, each with the contents row it names, and records how far it
    /// got.
    ///
    /// A summary carries the epoch that closed it, so nothing else has to be
    /// consulted to place it, and taking its contents with it keeps a
    /// checkpoint's two rows in one bucket — they expire together, as the
    /// per-row pruner they replace deleted them together.
    ///
    /// Two checkpoints in different epochs can name one contents row: every
    /// checkpoint that carries no transaction has the same contents digest,
    /// and the flat table holds one row for the pair. Each epoch's bucket gets
    /// a copy of its own, read back out of the bucket the first of them was
    /// filed in once the flat row is gone, so that expiring the older epoch
    /// does not leave a retained checkpoint with a summary and no contents.
    fn move_checkpoint_summaries(
        &self,
        from: Option<CheckpointDigest>,
    ) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let tables = &self.checkpoint_store.tables;
        let flat = &tables.checkpoint_by_digest;
        let slice = self.read_slice(flat, from, |_, summary| Ok(summary.inner().epoch))?;
        let progress = slice.progress(Progress::Summaries, Progress::ContentsWithoutSummary(None));

        let mut batch = flat.batch();
        let mut moved = 0;
        // Every contents row this slice touched, deleted whether it went into
        // a bucket or went with an expired summary.
        let mut contents_keys = Vec::new();
        for (epoch, summaries) in &slice.by_epoch {
            let bucket = self.checkpoint_store.historic_checkpoints.ensure(*epoch)?;
            let named: Vec<CheckpointContentsDigest> = summaries
                .iter()
                .map(|(_, summary)| summary.inner().contents_digest)
                .collect();
            let mut found = Vec::with_capacity(named.len());
            for (digest, flat_row) in named
                .iter()
                .copied()
                .zip(tables.checkpoint_content.multi_get(&named)?)
            {
                match flat_row {
                    Some(contents) => {
                        contents_keys.push(digest);
                        found.push((digest, contents));
                    }
                    // An earlier slice took the row for a checkpoint of
                    // another epoch that names the same contents, so this
                    // epoch's copy comes back out of that epoch's bucket.
                    None => {
                        if let Some(contents) = self
                            .checkpoint_store
                            .historic_checkpoints
                            .find_contents(&digest)?
                        {
                            found.push((digest, contents));
                        }
                    }
                }
            }
            moved += summaries.len() + found.len();
            batch.insert_batch_tagged(
                &bucket.checkpoint_by_digest,
                summaries.iter().map(|(digest, summary)| (digest, summary)),
            )?;
            batch.insert_batch_tagged(&bucket.checkpoint_content, found)?;
        }
        contents_keys.extend(
            slice
                .expired
                .iter()
                .map(|(_, summary)| summary.inner().contents_digest),
        );
        batch.delete_batch(&tables.checkpoint_content, &contents_keys)?;
        batch.delete_batch(flat, &slice.keys)?;
        batch.insert_batch(
            &tables.checkpoint_backlog_migration_progress,
            [((), progress)],
        )?;
        batch.write()?;

        let expired = slice.expired.len();
        debug!(
            moved,
            expired, "migrated a slice of the flat checkpoint summaries"
        );
        Ok(MigrationCounts { moved, expired })
    }

    /// Moves one slice of the contents rows the summary pass left behind into
    /// the bucket of the epoch the migration runs in, and records how far it
    /// got.
    ///
    /// A contents row carries neither an epoch nor a sequence number, and the
    /// summary that names it is what places it — so a row still here once
    /// every flat summary has moved cannot be placed at all. State sync, which
    /// writes almost all of this history, writes the summary first and the
    /// contents after, so its rows always have a summary to be placed by; only
    /// [`CheckpointStore::insert_genesis_checkpoint`] writes the contents
    /// first, leaving one unplaceable row if the node stopped between the two.
    /// Filing such a row under the running epoch keeps it rather than dropping
    /// contents a summary already in a bucket may still name.
    fn move_contents_without_summary(
        &self,
        from: Option<CheckpointContentsDigest>,
    ) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let tables = &self.checkpoint_store.tables;
        let flat = &tables.checkpoint_content;
        let slice = self.read_slice(flat, from, |_, _| Ok(self.epoch))?;
        let progress = slice.progress(Progress::ContentsWithoutSummary, Progress::Done);

        let mut batch = flat.batch();
        let mut moved = 0;
        for (epoch, rows) in slice.by_epoch {
            let bucket = self.checkpoint_store.historic_checkpoints.ensure(epoch)?;
            moved += rows.len();
            batch.insert_batch_tagged(&bucket.checkpoint_content, rows)?;
        }
        batch.delete_batch(flat, &slice.keys)?;
        batch.insert_batch(
            &tables.checkpoint_backlog_migration_progress,
            [((), progress)],
        )?;
        batch.write()?;

        let expired = slice.expired.len();
        debug!(
            moved,
            expired, "migrated a slice of the checkpoint contents no summary names"
        );
        Ok(MigrationCounts { moved, expired })
    }
}

#[cfg(test)]
#[path = "../unit_tests/ledger_backlog_migration_tests.rs"]
mod tests;
