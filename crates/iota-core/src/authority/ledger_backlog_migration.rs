// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO(https://github.com/iotaledger/iota/issues/12763): remove this module.

//! One-time move of the ledger and checkpoint rows in the flat tables of the
//! perpetual and checkpoint stores into the per-epoch buckets.
//!
//! A transaction's rows go to the bucket of the epoch that executed it, and a
//! checkpoint's contents and digest-keyed summary to the bucket of the epoch
//! that closed it, since that epoch decides when the row expires. Rows of
//! epochs the node's retention has already left behind are deleted instead.

use std::{collections::BTreeMap, ops::Bound, sync::Arc};

use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointDigest, TransactionDigest, TransactionEffectsDigest,
    TransactionKind,
};
use iota_types::{
    committee::EpochId,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    messages_checkpoint::CheckpointContentsExt,
    transaction::TransactionAPI,
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
    checkpoints::{CheckpointStore, EMPTY_CHECKPOINT_CONTENTS_DIGEST, empty_checkpoint_contents},
    progress_logger::ProgressLogger,
};

/// Rows read per write batch; bounds memory use and the work an interrupted
/// run repeats.
const KEYS_PER_SLICE: usize = 5_000;

const LEDGER_PASS: &str = "ledger backlog migration";
const CHECKPOINT_PASS: &str = "checkpoint backlog migration";

/// How far the migration has got through the flat perpetual ledger tables.
///
/// The variants are in drain order: `executed_effects`, `effects` and
/// `executed_transactions_to_checkpoint` give the earlier tables their epoch,
/// so they are drained last. The digest is the last key moved out of that
/// table, `None` if none has been yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerBacklogMigrationProgress {
    Transactions(Option<TransactionDigest>),
    Events(Option<TransactionDigest>),
    ExecutedEffects(Option<TransactionDigest>),
    Effects(Option<TransactionEffectsDigest>),
    TransactionCheckpoints(Option<TransactionDigest>),
    Done,
}

/// How far the migration has got through the checkpoint store's flat tables.
///
/// Summaries are drained first and take the contents row they name with them;
/// the contents rows left afterwards belong to no flat summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointBacklogMigrationProgress {
    Summaries(Option<CheckpointDigest>),
    ContentsWithoutSummary(Option<CheckpointContentsDigest>),
    Done,
}

/// Moves the rows of the flat ledger and checkpoint tables into the bucket of
/// the epoch each belongs to, deleting those of epochs outside
/// `epochs_to_retain_for_checkpoints`. `epoch` is the epoch the node is
/// starting in.
///
/// Call this before starting any service: until it returns, rows still in the
/// flat tables cannot be read, and the checkpoint executor panics on a
/// checkpoint it cannot resolve. Progress is durable, so after a failure the
/// next start resumes where this one stopped.
pub async fn migrate(
    store: Arc<AuthorityStore>,
    checkpoint_store: Arc<CheckpointStore>,
    epoch: EpochId,
    epochs_to_retain_for_checkpoints: Option<u64>,
) -> IotaResult<()> {
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

struct LedgerBacklogMigration {
    perpetual_tables: Arc<AuthorityPerpetualTables>,
    historic_ledger: Arc<HistoricLedger>,
    checkpoint_store: Arc<CheckpointStore>,
    /// The epoch the node is starting in.
    epoch: EpochId,
    /// The oldest epoch this node's retention still keeps; rows below it are
    /// deleted rather than moved.
    floor: EpochId,
    keys_per_slice: usize,
}

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

/// Rows read from a flat table in one batch.
struct Slice<K, V> {
    /// The rows to move, grouped by the epoch whose bucket they belong in.
    by_epoch: BTreeMap<EpochId, Vec<(K, V)>>,
    /// The rows to delete: below the floor, or with no epoch.
    expired: Vec<(K, V)>,
    /// Every key read; all are deleted from the flat table.
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

    /// `at(watermark)` while rows are left in the table, `finished` once it is
    /// drained.
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

    fn run(&mut self) -> IotaResult<()> {
        // Before the drains, so a run interrupted mid-drain never leaves the
        // synced watermark above rows the drains deleted. Skipped once the
        // drains are done, so later starts keep what state sync fetched ahead
        // of execution.
        if self.migration_pending()? {
            self.pin_floor()?;
            self.rewind_synced_watermark()?;
        }
        let mut counts = self.drain_ledger()?;
        counts.add(&self.drain_checkpoints()?);
        if counts.moved > 0 || counts.expired > 0 {
            info!(
                moved = counts.moved,
                expired = counts.expired,
                floor = self.floor,
                "the ledger backlog migration reached the end of the flat tables"
            );
        }

        // Stops state sync from advertising checkpoints below the floor. Based
        // on the floor rather than on what this run deleted, since a resumed
        // run may have deleted nothing itself. Logged rather than returned, so
        // a watermark write cannot keep the node from starting.
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

    fn drain_ledger(&self) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let mut counts = MigrationCounts::default();
        let mut logger = None;
        loop {
            // Read from disk so a resumed run takes the same path as an
            // uninterrupted one.
            let progress = self
                .perpetual_tables
                .ledger_backlog_migration_progress
                .get(&())?
                .unwrap_or(Progress::Transactions(None));
            let tables = &self.perpetual_tables;
            let (step, total) = match &progress {
                Progress::Transactions(_) => ("transactions", tables.transactions.estimated_len()?),
                Progress::Events(_) => ("events", tables.events_2.estimated_len()?),
                Progress::ExecutedEffects(_) => {
                    ("executed effects", tables.executed_effects.estimated_len()?)
                }
                Progress::Effects(_) => ("effects", tables.effects.estimated_len()?),
                Progress::TransactionCheckpoints(_) => (
                    "transaction checkpoints",
                    tables.executed_transactions_to_checkpoint.estimated_len()?,
                ),
                Progress::Done => {
                    Self::close_step(&mut logger);
                    return Ok(counts);
                }
            };
            let logger = Self::open_step(&mut logger, LEDGER_PASS, step, total);
            let slice = match progress {
                Progress::Transactions(from) => self.move_transactions(from)?,
                Progress::Events(from) => self.move_events(from)?,
                Progress::ExecutedEffects(from) => self.move_executed_effects(from)?,
                Progress::Effects(from) => self.move_effects(from)?,
                Progress::TransactionCheckpoints(from) => {
                    self.move_transaction_checkpoints(from)?
                }
                Progress::Done => unreachable!("the step match above returns on Done"),
            };
            logger.advance((slice.moved + slice.expired) as u64);
            counts.add(&slice);
        }
    }

    fn drain_checkpoints(&self) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let mut counts = MigrationCounts::default();
        let mut logger = None;
        loop {
            let progress = self
                .checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .get(&())?
                .unwrap_or(Progress::Summaries(None));
            let tables = &self.checkpoint_store.tables;
            let (step, total) = match &progress {
                Progress::Summaries(_) => {
                    ("summaries", tables.checkpoint_by_digest.estimated_len()?)
                }
                Progress::ContentsWithoutSummary(_) => {
                    ("contents", tables.checkpoint_content.estimated_len()?)
                }
                Progress::Done => {
                    Self::close_step(&mut logger);
                    return Ok(counts);
                }
            };
            let logger = Self::open_step(&mut logger, CHECKPOINT_PASS, step, total);
            let slice = match progress {
                Progress::Summaries(from) => self.move_checkpoint_summaries(from)?,
                Progress::ContentsWithoutSummary(from) => {
                    self.move_contents_without_summary(from)?
                }
                Progress::Done => unreachable!("the step match above returns on Done"),
            };
            logger.advance((slice.moved + slice.expired) as u64);
            counts.add(&slice);
        }
    }

    /// Returns the logger for `step`, finishing the previous step's logger
    /// when the step changes.
    fn open_step<'a>(
        logger: &'a mut Option<ProgressLogger>,
        pass: &'static str,
        step: &'static str,
        total: u64,
    ) -> &'a mut ProgressLogger {
        if logger.as_ref().is_none_or(|open| open.step() != step) {
            Self::close_step(logger);
            *logger = Some(ProgressLogger::new(pass, step, total));
        }
        logger.as_mut().expect("open for this step")
    }

    fn close_step(logger: &mut Option<ProgressLogger>) {
        if let Some(open) = logger.take() {
            open.finish();
        }
    }

    /// Whether either drain's recorded progress is short of `Done`.
    fn migration_pending(&self) -> IotaResult<bool> {
        let ledger = self
            .perpetual_tables
            .ledger_backlog_migration_progress
            .get(&())?;
        let checkpoints = self
            .checkpoint_store
            .tables
            .checkpoint_backlog_migration_progress
            .get(&())?;
        Ok(
            !matches!(ledger, Some(LedgerBacklogMigrationProgress::Done))
                || !matches!(checkpoints, Some(CheckpointBacklogMigrationProgress::Done)),
        )
    }

    /// Brings the synced watermark back to the executed one, so state sync
    /// fetches again whatever the migration dropped.
    ///
    /// The migration drops the rows of transactions synced but not yet
    /// executed, since it cannot tell their epoch. The checkpoint executor
    /// would panic reading them by digest; with the watermark rewound it waits
    /// for state sync to fetch them again instead.
    fn rewind_synced_watermark(&self) -> IotaResult<()> {
        let before = self
            .checkpoint_store
            .get_highest_synced_checkpoint_seq_number()?;
        let after = self.checkpoint_store.rewind_highest_synced_to_executed()?;
        if before != after {
            info!(
                from = before,
                to = after,
                "rewinding the synced watermark to the executed checkpoint so state sync \
                 fetches the checkpoints the migration could not attribute"
            );
        }
        Ok(())
    }

    /// Takes the floor of the run that started the migration, recording this
    /// run's floor if it is that run. A resumed run under a changed retention
    /// would otherwise keep epochs an earlier run already deleted rows of, and
    /// report them as still held.
    fn pin_floor(&mut self) -> IotaResult<()> {
        let pinned = &self.perpetual_tables.ledger_backlog_migration_floor;
        match pinned.get(&())? {
            Some(floor) => self.floor = floor,
            None => pinned.insert(&(), &self.floor)?,
        }
        Ok(())
    }

    /// The epoch that executed `digest`, or `None` when this node has no
    /// record of executing it, as for a transaction state sync fetched but
    /// execution has not reached (see [`Self::rewind_synced_watermark`]).
    fn transaction_epoch(&self, digest: &TransactionDigest) -> IotaResult<Option<EpochId>> {
        let tables = &self.perpetual_tables;
        if let Some((epoch, _)) = tables.executed_transactions_to_checkpoint.get(digest)? {
            return Ok(Some(epoch));
        }
        if let Some(effects_digest) = tables.executed_effects.get(digest)? {
            if let Some(effects) = tables.effects.get(&effects_digest)? {
                return Ok(Some(effects.epoch()));
            }
        }
        Ok(None)
    }

    /// Reads up to [`Self::keys_per_slice`] rows of `flat` above
    /// `resume_above`, grouped by the epoch `epoch_of` gives them.
    fn read_slice<K, V>(
        &self,
        flat: &DBMap<K, V>,
        resume_above: Option<K>,
        epoch_of: impl Fn(&K, &V) -> IotaResult<Option<EpochId>>,
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
                // This row is left for the next slice to read again.
                slice.sliced = true;
                break;
            }
            let (key, value) = row?;
            let epoch = epoch_of(&key, &value)?;
            slice.keys.push(key);
            slice.watermark = Some(key);
            match epoch {
                Some(epoch) if epoch >= self.floor => {
                    slice.by_epoch.entry(epoch).or_default().push((key, value));
                }
                _ => slice.expired.push((key, value)),
            }
        }
        Ok(slice)
    }

    /// Moves one slice of a flat perpetual ledger table into the buckets and
    /// records `progress`, all in one write batch.
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
        let slice = self.read_slice(flat, from, |digest, transaction| {
            if let Some(epoch) = self.transaction_epoch(digest)? {
                return Ok(Some(epoch));
            }
            // A validator stores a randomness state update before executing it,
            // since its signature cannot always be built again after a restart.
            Ok(match transaction.inner().transaction().kind() {
                TransactionKind::RandomnessStateUpdate(update) => Some(update.epoch),
                _ => None,
            })
        })?;
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
        let slice = self.read_slice(flat, from, |_, effects| Ok(Some(effects.epoch())))?;
        let progress = slice.progress(Progress::Effects, Progress::TransactionCheckpoints(None));
        self.move_ledger_slice(flat, slice, |bucket| &bucket.effects, |row| row, progress)
    }

    fn move_transaction_checkpoints(
        &self,
        from: Option<TransactionDigest>,
    ) -> IotaResult<MigrationCounts> {
        use LedgerBacklogMigrationProgress as Progress;

        let flat = &self.perpetual_tables.executed_transactions_to_checkpoint;
        let slice = self.read_slice(flat, from, |_, (epoch, _)| Ok(Some(*epoch)))?;
        let progress = slice.progress(Progress::TransactionCheckpoints, Progress::Done);
        // The bucket's epoch is the checkpoint's epoch, so only the sequence
        // number is kept.
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
    /// Checkpoints in different epochs can share one contents row (every empty
    /// checkpoint does), so each epoch's bucket gets its own copy; expiring one
    /// epoch then cannot leave another's checkpoint without contents.
    fn move_checkpoint_summaries(
        &self,
        from: Option<CheckpointDigest>,
    ) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let tables = &self.checkpoint_store.tables;
        let flat = &tables.checkpoint_by_digest;
        let slice = self.read_slice(flat, from, |_, summary| Ok(Some(summary.inner().epoch)))?;
        let progress = slice.progress(Progress::Summaries, Progress::ContentsWithoutSummary(None));

        let mut batch = flat.batch();
        let mut moved = 0;
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
                    // Already moved for another epoch's checkpoint with the
                    // same contents.
                    None => {
                        if let Some(contents) = self
                            .checkpoint_store
                            .historic_checkpoints
                            .find_contents(&digest)?
                        {
                            found.push((digest, contents));
                        } else if digest == *EMPTY_CHECKPOINT_CONTENTS_DIGEST {
                            // The checkpoint pruner may have deleted the row
                            // all empty checkpoints share; its contents are
                            // fixed, so it is rebuilt.
                            found.push((digest, empty_checkpoint_contents()));
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
        // An expired summary's contents stay in the flat table, since a
        // retained summary in a later slice may share them; the rest go to
        // `move_contents_without_summary`.
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
    /// the bucket of the epoch their transactions were executed in, and
    /// records how far it got. Must run after the ledger has been drained,
    /// which is where that epoch is read from.
    ///
    /// Such rows are the contents of expired summaries, which are deleted,
    /// and of checkpoints this validator built but has not yet certified; the
    /// digest-keyed summary only arrives at
    /// [`CheckpointStore::insert_certified_checkpoint`]. The row every empty
    /// checkpoint shares names no transaction, and goes into the bucket of
    /// the epoch the migration runs in.
    fn move_contents_without_summary(
        &self,
        from: Option<CheckpointContentsDigest>,
    ) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let tables = &self.checkpoint_store.tables;
        let flat = &tables.checkpoint_content;
        let slice = self.read_slice(flat, from, |_, contents| {
            let Some(first) = contents.iter().next() else {
                return Ok(Some(self.epoch));
            };
            // A checkpoint holds the transactions of its own epoch only.
            Ok(self
                .historic_ledger
                .find_epoch(&first.transaction)?
                .map(|(epoch, _)| epoch))
        })?;
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
