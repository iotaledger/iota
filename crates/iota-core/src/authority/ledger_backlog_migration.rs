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

use std::{
    collections::{BTreeMap, btree_map::Entry},
    ops::Bound,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointDigest, TransactionDigest, TransactionEffects,
    TransactionEffectsDigest, TransactionEvents, TransactionKind,
};
use iota_types::{
    committee::EpochId,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, VerifiedCheckpoint},
    transaction::{TransactionAPI, TrustedTransaction},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, info, warn};
use typed_store::{
    DbIterator,
    rocks::{DBBatch, DBMap, SstFileBuffer, available_disk_space, bulk_ingestion_options},
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

/// How many key ranges each of the large flat tables is split into by the
/// first byte of its key; each range is drained by its own thread.
const KEY_RANGES: usize = 8;

const BACKLOG_PASS: &str = "ledger and checkpoint backlog migration";

/// Where the ledger drain keeps its SST files inside the perpetual
/// database's directory until it ingests them.
const LEDGER_SST_DIR: &str = "ledger_backlog_migration";
const CHECKPOINT_PASS: &str = "checkpoint backlog migration";

/// How far one key range of a flat table has been drained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeProgress<K> {
    /// The last key moved, `None` before the first slice.
    At(Option<K>),
    Done,
}

/// How far the migration has got through the flat perpetual ledger tables,
/// per key range. `transactions` covers the four tables keyed by transaction
/// digest, which are walked together; `effects` the table keyed by effects
/// digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerBacklogMigrationProgress {
    Draining {
        transactions: Vec<RangeProgress<TransactionDigest>>,
        effects: Vec<RangeProgress<TransactionEffectsDigest>>,
    },
    Done,
}

/// How far the migration has got through the checkpoint store's flat tables.
///
/// Summaries are drained first, per key range, and take the contents row they
/// name with them; the contents rows left afterwards belong to no flat
/// summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointBacklogMigrationProgress {
    Summaries(Vec<RangeProgress<CheckpointDigest>>),
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
    /// How much memory one ledger range's rows may take before they are
    /// written to SST files and ingested.
    sst_buffer_limit: usize,
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

/// The latest position of every key range of the ledger drain.
struct LedgerRanges {
    transactions: Vec<RangeProgress<TransactionDigest>>,
    effects: Vec<RangeProgress<TransactionEffectsDigest>>,
}

/// What the drain threads share: one progress log over all of them, the
/// totals, and whether one has failed, which stops the others.
struct ParallelProgress {
    logger: Mutex<ProgressLogger>,
    moved: AtomicUsize,
    expired: AtomicUsize,
    failed: AtomicBool,
    /// The runtime of the calling thread, since the database starts tasks on
    /// it, e.g. to sample iterator metrics.
    runtime: Option<tokio::runtime::Handle>,
}

impl ParallelProgress {
    /// Runs one thread's drain, stopping the others if it fails.
    fn watch(&self, drain: impl FnOnce() -> IotaResult<()>) -> IotaResult<()> {
        let _runtime = self.runtime.as_ref().map(|runtime| runtime.enter());
        let result = drain();
        if result.is_err() {
            self.failed.store(true, Ordering::Relaxed);
        }
        result
    }

    fn stopped(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    fn record(&self, counts: &MigrationCounts, read: usize) {
        self.moved.fetch_add(counts.moved, Ordering::Relaxed);
        self.expired.fetch_add(counts.expired, Ordering::Relaxed);
        self.logger.lock().advance(read as u64);
    }
}

/// The SST files the ledger drain writes, kept in a directory of their own
/// until they are ingested into the buckets.
struct LedgerSstFiles {
    dir: PathBuf,
    /// The buckets' column family options.
    options: typed_store::rocksdb::Options,
    next_file: AtomicUsize,
}

impl LedgerSstFiles {
    /// Writes `buffer` to a new file and adds its path to `paths`, if it holds
    /// any rows.
    fn write<K: Serialize, V: Serialize>(
        &self,
        buffer: &mut SstFileBuffer<K, V>,
        paths: &mut Vec<PathBuf>,
    ) -> IotaResult<()> {
        if buffer.is_empty() {
            return Ok(());
        }
        let path = self.dir.join(format!(
            "{}.sst",
            self.next_file.fetch_add(1, Ordering::Relaxed)
        ));
        buffer.write_file(&path, &self.options)?;
        paths.push(path);
        Ok(())
    }
}

/// The rows of one bucket a ledger range has read but not yet ingested, one
/// buffer per table.
struct BucketRows {
    bucket: Arc<HistoricLedgerBucket>,
    transactions: SstFileBuffer<(u8, TransactionDigest), TrustedTransaction>,
    events: SstFileBuffer<(u8, TransactionDigest), TransactionEvents>,
    executed_effects: SstFileBuffer<(u8, TransactionDigest), TransactionEffectsDigest>,
    tx_to_checkpoint: SstFileBuffer<(u8, TransactionDigest), CheckpointSequenceNumber>,
    effects: SstFileBuffer<(u8, TransactionEffectsDigest), TransactionEffects>,
}

impl BucketRows {
    fn new(bucket: Arc<HistoricLedgerBucket>) -> Self {
        Self {
            bucket,
            transactions: SstFileBuffer::new(),
            events: SstFileBuffer::new(),
            executed_effects: SstFileBuffer::new(),
            tx_to_checkpoint: SstFileBuffer::new(),
            effects: SstFileBuffer::new(),
        }
    }

    fn size_in_bytes(&self) -> usize {
        self.transactions.size_in_bytes()
            + self.events.size_in_bytes()
            + self.executed_effects.size_in_bytes()
            + self.tx_to_checkpoint.size_in_bytes()
            + self.effects.size_in_bytes()
    }

    /// Writes the rows to one SST file per table and moves them into the
    /// bucket.
    ///
    /// The files of one call hold different tables, so they do not overlap;
    /// and since a range reads its keys in order, neither do they overlap the
    /// files of the range's earlier calls nor those of other ranges. RocksDB
    /// can therefore place them in the bucket's bottom level, where they need
    /// no compaction.
    fn ingest(mut self, files: &LedgerSstFiles) -> IotaResult<()> {
        let mut paths = Vec::new();
        files.write(&mut self.transactions, &mut paths)?;
        files.write(&mut self.events, &mut paths)?;
        files.write(&mut self.executed_effects, &mut paths)?;
        files.write(&mut self.tx_to_checkpoint, &mut paths)?;
        files.write(&mut self.effects, &mut paths)?;
        self.bucket.transactions.ingest_sst_files(&paths)?;
        Ok(())
    }
}

/// The rows a ledger range has read but not yet ingested, per bucket.
#[derive(Default)]
struct PendingRows {
    buckets: BTreeMap<EpochId, BucketRows>,
}

impl PendingRows {
    fn bucket(&mut self, ledger: &HistoricLedger, epoch: EpochId) -> IotaResult<&mut BucketRows> {
        Ok(match self.buckets.entry(epoch) {
            Entry::Occupied(rows) => rows.into_mut(),
            Entry::Vacant(rows) => rows.insert(BucketRows::new(ledger.ensure(epoch)?)),
        })
    }

    fn size_in_bytes(&self) -> usize {
        self.buckets.values().map(BucketRows::size_in_bytes).sum()
    }

    fn ingest(&mut self, files: &LedgerSstFiles) -> IotaResult<()> {
        for rows in std::mem::take(&mut self.buckets).into_values() {
            rows.ingest(files)?;
        }
        Ok(())
    }
}

/// What reading one slice of a ledger key range found.
struct LedgerSlice<K> {
    counts: MigrationCounts,
    /// Keys read from the flat tables.
    read: usize,
    /// Where the key range stands once the slice's rows are ingested.
    next: RangeProgress<K>,
}

/// One slice of a key range of the checkpoint summaries, ready to be written.
struct SliceBatch<K> {
    batch: DBBatch,
    counts: MigrationCounts,
    /// Keys read from the flat table.
    read: usize,
    /// Where the key range stands once the batch is written.
    next: RangeProgress<K>,
}

/// Refuses with the amount when the file system `dir` is on, or will be
/// created on, has less than `needed` bytes free; starts without the check,
/// with a warning, when the free space cannot be determined.
pub(crate) fn ensure_free_space(dir: &Path, needed: u64) -> IotaResult<()> {
    let Some(free) = available_disk_space(dir) else {
        warn!(
            needed = %disk_size(needed),
            dir = %dir.display(),
            "unable to determine the free disk space for the ledger migration; starting it \
             without the check"
        );
        return Ok(());
    };
    if free < needed {
        return Err(IotaError::Storage(format!(
            "the ledger migration needs about {} of free disk space under {} to copy the \
             retained transaction and checkpoint history into the per-epoch buckets, but \
             only {} are free. Free up space, or lower \
             num-epochs-to-retain-for-checkpoints, and start the node again",
            disk_size(needed),
            dir.display(),
            disk_size(free)
        )));
    }
    info!(
        needed = %disk_size(needed),
        free = %disk_size(free),
        "the ledger migration has the disk space it needs"
    );
    Ok(())
}

/// `bytes` for a message: GiB, rounded up, from 1 GiB on, MiB below.
fn disk_size(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{} GiB", bytes.div_ceil(1 << 30))
    } else {
        format!("{} MiB", bytes.div_ceil(1 << 20))
    }
}

/// The bytes the migration copies into the buckets, from `flat_bytes` in the
/// flat tables holding the epochs from `oldest_held_epoch` to `epoch`: their
/// share from `floor` on, assuming epochs of similar size, plus a tenth for
/// the buckets' own overhead.
fn estimated_copy_bytes(
    flat_bytes: u64,
    epoch: EpochId,
    floor: EpochId,
    oldest_held_epoch: EpochId,
) -> u64 {
    let held = epoch.saturating_sub(oldest_held_epoch) + 1;
    let kept = epoch.saturating_sub(floor.max(oldest_held_epoch)) + 1;
    let copied = (flat_bytes as u128 * kept.min(held) as u128 / held as u128) as u64;
    copied + copied / 10
}

/// The keys of key range `range` above `from`; range `i` holds the keys whose
/// first byte is in its share of `0..=255`.
fn key_range<K>(
    range: usize,
    from: Option<K>,
    key: impl Fn([u8; 32]) -> K,
) -> (Bound<K>, Bound<K>) {
    let first_key = |range: usize| {
        let mut bytes = [0; 32];
        bytes[0] = u8::try_from(range * 256 / KEY_RANGES).expect("below 256");
        key(bytes)
    };
    let lower = match from {
        Some(from) => Bound::Excluded(from),
        None => Bound::Included(first_key(range)),
    };
    let upper = if range + 1 < KEY_RANGES {
        Bound::Excluded(first_key(range + 1))
    } else {
        Bound::Unbounded
    };
    (lower, upper)
}

/// Rows read from a flat table in one batch.
struct Slice<K, V> {
    /// The rows to move, grouped by the epoch whose bucket they belong in.
    by_epoch: BTreeMap<EpochId, Vec<(K, V)>>,
    /// The rows to delete: below the floor, or with no epoch.
    expired: Vec<(K, V)>,
    /// Every key read, for the passes that delete what they read.
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

    /// Where its key range stands after this slice.
    fn range_progress(&self) -> RangeProgress<K>
    where
        K: Copy,
    {
        match self.watermark {
            Some(key) if self.sliced => RangeProgress::At(Some(key)),
            _ => RangeProgress::Done,
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
            // Shared by the transaction and effects ranges, which run at once.
            sst_buffer_limit: bulk_ingestion_options().sst_buffer_limit(2 * KEY_RANGES),
        }
    }

    fn run(&mut self) -> IotaResult<()> {
        // Before the drains, so a run interrupted mid-drain never leaves the
        // synced watermark above rows the drains deleted. Skipped once the
        // drains are done, so later starts keep what state sync fetched ahead
        // of execution.
        if self.migration_pending()? {
            self.pin_floor()?;
            if !self.started()? {
                self.check_disk_space()?;
            }
            self.rewind_synced_watermark()?;
        }
        let mut counts = self.drain_in_parallel()?;
        counts.add(&self.drain_contents_without_summary()?);
        if counts.moved > 0 || counts.expired > 0 {
            info!(
                moved = counts.moved,
                expired = counts.expired,
                floor = self.floor,
                "the ledger backlog migration reached the end of the flat tables"
            );
        }

        self.clear_flat_tables()?;

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

    /// Deletes the flat tables the passes left their rows in, once all of them
    /// are done. The passes read rows without deleting them, so that every
    /// pass resolving a transaction's epoch sees the same tables; one range
    /// deletion per table then replaces a deletion per row. Does nothing
    /// once the tables are empty.
    fn clear_flat_tables(&self) -> IotaResult<()> {
        let tables = &self.perpetual_tables;
        tables.transactions.schedule_delete_all()?;
        tables.events_2.schedule_delete_all()?;
        tables.executed_effects.schedule_delete_all()?;
        tables.effects.schedule_delete_all()?;
        tables
            .executed_transactions_to_checkpoint
            .schedule_delete_all()?;
        self.checkpoint_store
            .tables
            .checkpoint_by_digest
            .schedule_delete_all()?;
        Ok(())
    }

    /// About how many rows [`Self::drain_in_parallel`] reads: a transaction and
    /// its effects per transaction, and a summary per checkpoint.
    ///
    /// Counted from the summaries of the checkpoints above the pruned
    /// watermark where they are there, since RocksDB's key estimate subtracts
    /// the deletions a pruned table still holds and can under-report it
    /// several times over. The sequence-keyed summaries are never pruned, so
    /// the watermark, not the oldest of them, marks where the flat tables
    /// start.
    fn rows_to_drain(&self, ledger: bool, summaries: bool) -> IotaResult<u64> {
        let certified = &self.checkpoint_store.tables.certified_checkpoints;
        let pruned = self
            .checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?;
        // How many transactions came before the held ones: those through the
        // pruned checkpoint, as its summary counts them.
        let before_held = match pruned {
            None => Some(0),
            Some(pruned) => certified
                .get(&pruned)?
                .map(|checkpoint| VerifiedCheckpoint::from(checkpoint).network_total_transactions),
        };
        let latest = self.checkpoint_store.get_latest_certified_checkpoint()?;
        if let (Some(before_held), Some(latest)) = (before_held, latest) {
            let held_from = pruned.map_or(0, |pruned| pruned.saturating_add(1));
            let mut rows = 0;
            if ledger {
                rows += 2 * latest
                    .network_total_transactions
                    .saturating_sub(before_held);
            }
            if summaries {
                rows += latest
                    .sequence_number
                    .saturating_add(1)
                    .saturating_sub(held_from);
            }
            return Ok(rows);
        }
        let tables = &self.perpetual_tables;
        let mut rows = 0;
        if ledger {
            rows += tables.transactions.estimated_len()? + tables.effects.estimated_len()?;
        }
        if summaries {
            rows += self
                .checkpoint_store
                .tables
                .checkpoint_by_digest
                .estimated_len()?;
        }
        Ok(rows)
    }

    /// Drains the flat ledger tables and the flat checkpoint summaries, each
    /// table in [`KEY_RANGES`] key ranges with a thread per range. None of
    /// these passes deletes what it reads, so they do not depend on each
    /// other.
    ///
    /// A ledger range writes its rows to SST files and records its progress
    /// only once they are ingested; a summary range records its progress in
    /// the write batch of its rows. The progress writes of a store are
    /// serialized, so its single progress row always holds every range's
    /// latest position.
    fn drain_in_parallel(&self) -> IotaResult<MigrationCounts> {
        let ledger = match self
            .perpetual_tables
            .ledger_backlog_migration_progress
            .get(&())?
        {
            Some(LedgerBacklogMigrationProgress::Done) => None,
            Some(LedgerBacklogMigrationProgress::Draining {
                transactions,
                effects,
            }) if transactions.len() == KEY_RANGES && effects.len() == KEY_RANGES => {
                Some((transactions, effects))
            }
            // A different number of ranges restarts the drain, which is safe
            // since nothing has been deleted yet.
            _ => Some((
                vec![RangeProgress::At(None); KEY_RANGES],
                vec![RangeProgress::At(None); KEY_RANGES],
            )),
        };
        let summaries = match self
            .checkpoint_store
            .tables
            .checkpoint_backlog_migration_progress
            .get(&())?
        {
            Some(CheckpointBacklogMigrationProgress::Summaries(ranges))
                if ranges.len() == KEY_RANGES =>
            {
                Some(ranges)
            }
            None | Some(CheckpointBacklogMigrationProgress::Summaries(_)) => {
                Some(vec![RangeProgress::At(None); KEY_RANGES])
            }
            Some(_) => None,
        };
        if ledger.is_none() && summaries.is_none() {
            return Ok(MigrationCounts::default());
        }

        let total = self.rows_to_drain(ledger.is_some(), summaries.is_some())?;
        let progress = ParallelProgress {
            logger: Mutex::new(ProgressLogger::new(BACKLOG_PASS, "rows", total)),
            moved: AtomicUsize::new(0),
            expired: AtomicUsize::new(0),
            failed: AtomicBool::new(false),
            runtime: tokio::runtime::Handle::try_current().ok(),
        };
        let ledger = ledger.map(|(transactions, effects)| {
            Mutex::new(LedgerRanges {
                transactions,
                effects,
            })
        });
        let summaries = summaries.map(Mutex::new);
        let sst_files = ledger
            .as_ref()
            .map(|_| self.ledger_sst_files())
            .transpose()?;

        let results: Vec<IotaResult<()>> = std::thread::scope(|scope| {
            let mut workers = Vec::new();
            for range in 0..KEY_RANGES {
                let progress = &progress;
                if let (Some(ledger), Some(files)) = (&ledger, &sst_files) {
                    workers.push(scope.spawn(move || {
                        progress.watch(|| {
                            self.drain_ledger_range(
                                ledger,
                                files,
                                progress,
                                |ranges| &mut ranges.transactions[range],
                                |from, pending| self.transactions_slice(range, from, pending),
                            )
                        })
                    }));
                    workers.push(scope.spawn(move || {
                        progress.watch(|| {
                            self.drain_ledger_range(
                                ledger,
                                files,
                                progress,
                                |ranges| &mut ranges.effects[range],
                                |from, pending| self.effects_slice(range, from, pending),
                            )
                        })
                    }));
                }
                if let Some(summaries) = &summaries {
                    workers.push(scope.spawn(move || {
                        progress.watch(|| self.drain_summaries_range(range, summaries, progress))
                    }));
                }
            }
            workers
                .into_iter()
                .map(|worker| {
                    worker.join().unwrap_or_else(|_| {
                        Err(IotaError::Storage(
                            "a backlog migration thread panicked".to_string(),
                        ))
                    })
                })
                .collect()
        });
        results.into_iter().collect::<IotaResult<Vec<()>>>()?;
        let ParallelProgress {
            logger,
            moved,
            expired,
            ..
        } = progress;
        logger.into_inner().finish();

        // Every range finished, as an error would have returned above. Not an
        // error: every row is in its bucket by now, and the ingestion moved
        // the files, so at most an empty directory is left.
        if let Some(files) = sst_files {
            if let Err(e) = std::fs::remove_dir_all(&files.dir) {
                warn!(
                    "unable to remove the SST file directory {:?}: {e}",
                    files.dir
                );
            }
        }
        if ledger.is_some() {
            self.perpetual_tables
                .ledger_backlog_migration_progress
                .insert(&(), &LedgerBacklogMigrationProgress::Done)?;
        }
        if summaries.is_some() {
            self.checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .insert(
                    &(),
                    &CheckpointBacklogMigrationProgress::ContentsWithoutSummary(None),
                )?;
        }
        Ok(MigrationCounts {
            moved: moved.into_inner(),
            expired: expired.into_inner(),
        })
    }

    /// The directory the ledger drain writes its SST files to, emptied of
    /// those an interrupted run left.
    fn ledger_sst_files(&self) -> IotaResult<LedgerSstFiles> {
        let dir = self
            .perpetual_tables
            .objects
            .db
            .path_for_pruning()
            .join(LEDGER_SST_DIR);
        let io_error = |e: std::io::Error| {
            IotaError::Storage(format!(
                "unable to prepare the SST file directory {dir:?}: {e}"
            ))
        };
        // A leftover is never ingested, and a file written again under the
        // same name replaces it, so one that cannot be removed only costs disk.
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                warn!("unable to remove the leftover SST file directory {dir:?}: {e}");
            }
        }
        std::fs::create_dir_all(&dir).map_err(io_error)?;
        Ok(LedgerSstFiles {
            dir,
            options: self.historic_ledger.sst_file_options().clone(),
            next_file: AtomicUsize::new(0),
        })
    }

    /// Drains one ledger key range, whose position `position` selects, one
    /// slice at a time. The rows of the slices are ingested once they reach
    /// [`Self::sst_buffer_limit`], and the position is recorded only after
    /// that, so a run resumed after a crash starts at rows the buckets
    /// already hold or read again.
    fn drain_ledger_range<K: Copy>(
        &self,
        ledger: &Mutex<LedgerRanges>,
        files: &LedgerSstFiles,
        progress: &ParallelProgress,
        position: impl Fn(&mut LedgerRanges) -> &mut RangeProgress<K>,
        read_slice: impl Fn(Option<K>, &mut PendingRows) -> IotaResult<LedgerSlice<K>>,
    ) -> IotaResult<()> {
        let RangeProgress::At(mut from) = *position(&mut ledger.lock()) else {
            return Ok(());
        };
        let mut pending = PendingRows::default();
        loop {
            if progress.stopped() {
                return Ok(());
            }
            let slice = read_slice(from, &mut pending)?;
            progress.record(&slice.counts, slice.read);
            let finished = matches!(slice.next, RangeProgress::Done);
            if finished || pending.size_in_bytes() >= self.sst_buffer_limit {
                pending.ingest(files)?;
                let mut ranges = ledger.lock();
                *position(&mut ranges) = slice.next;
                self.write_ledger_progress(&ranges)?;
            }
            let RangeProgress::At(next) = slice.next else {
                return Ok(());
            };
            from = next;
        }
    }

    /// Drains key range `range` of the flat checkpoint summaries.
    fn drain_summaries_range(
        &self,
        range: usize,
        summaries: &Mutex<Vec<RangeProgress<CheckpointDigest>>>,
        progress: &ParallelProgress,
    ) -> IotaResult<()> {
        loop {
            let RangeProgress::At(from) = summaries.lock()[range] else {
                return Ok(());
            };
            if progress.stopped() {
                return Ok(());
            }
            let mut slice = self.summaries_slice(range, from)?;
            let mut ranges = summaries.lock();
            let mut written = ranges.clone();
            written[range] = slice.next;
            slice.batch.insert_batch(
                &self
                    .checkpoint_store
                    .tables
                    .checkpoint_backlog_migration_progress,
                [(
                    (),
                    CheckpointBacklogMigrationProgress::Summaries(written.clone()),
                )],
            )?;
            slice.batch.write()?;
            // Only once written, or another range could record a position
            // whose rows a failed write left behind.
            *ranges = written;
            drop(ranges);
            progress.record(&slice.counts, slice.read);
        }
    }

    /// Records every ledger range's latest position. The caller holds the
    /// ranges' lock, which serializes these writes.
    fn write_ledger_progress(&self, ranges: &LedgerRanges) -> IotaResult<()> {
        self.perpetual_tables
            .ledger_backlog_migration_progress
            .insert(
                &(),
                &LedgerBacklogMigrationProgress::Draining {
                    transactions: ranges.transactions.clone(),
                    effects: ranges.effects.clone(),
                },
            )?;
        Ok(())
    }

    /// Moves the contents rows the summaries left behind, once the summaries
    /// and the ledger are drained.
    fn drain_contents_without_summary(&self) -> IotaResult<MigrationCounts> {
        use CheckpointBacklogMigrationProgress as Progress;

        let mut counts = MigrationCounts::default();
        let mut logger = None;
        loop {
            let progress = self
                .checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .get(&())?;
            let from = match progress {
                Some(Progress::ContentsWithoutSummary(from)) => from,
                _ => {
                    Self::close_step(&mut logger);
                    return Ok(counts);
                }
            };
            let total = self
                .checkpoint_store
                .tables
                .checkpoint_content
                .estimated_len()?;
            let logger = Self::open_step(&mut logger, CHECKPOINT_PASS, "contents", total);
            let slice = self.move_contents_without_summary(from)?;
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

    /// Settles this run's floor against the one the drains have run with so
    /// far, recording it when this run starts the migration.
    ///
    /// Until the drains have written progress, the configured floor replaces
    /// the recorded one, whichever way the retention changed: nothing has
    /// moved yet, as when the disk space check refused the earlier run.
    /// After that, a raised retention, a lower floor, restarts the drains
    /// while no row of an expired epoch has been deleted yet: they skipped the
    /// rows below the old floor, which are all still in the flat tables. Once
    /// the step that deletes such rows has begun, the recorded floor is kept,
    /// as a resumed run would otherwise keep epochs an earlier run had begun
    /// deleting and report them as held. A lowered retention keeps the
    /// recorded floor too: what the drains moved stays, for the next expiry
    /// to drop.
    fn pin_floor(&mut self) -> IotaResult<()> {
        let pinned = &self.perpetual_tables.ledger_backlog_migration_floor;
        let configured = self.floor;
        match pinned.get(&())? {
            None => pinned.insert(&(), &configured)?,
            Some(_) if !self.started()? => pinned.insert(&(), &configured)?,
            Some(recorded) if configured < recorded && !self.deleting_expired_rows()? => {
                warn!(
                    recorded,
                    configured,
                    "the retention was raised while the ledger migration was running; \
                     restarting its drains to keep the epochs from {configured} on"
                );
                self.perpetual_tables
                    .ledger_backlog_migration_progress
                    .remove(&())?;
                self.checkpoint_store
                    .tables
                    .checkpoint_backlog_migration_progress
                    .remove(&())?;
                pinned.insert(&(), &configured)?;
            }
            Some(recorded) => {
                if recorded != configured {
                    warn!(
                        recorded,
                        configured,
                        "the ledger migration keeps the floor it started with, not the one the \
                         configured retention gives: rows below it are being deleted already, \
                         or the retention was lowered"
                    );
                }
                self.floor = recorded;
            }
        }
        info!(
            floor = self.floor,
            "the ledger migration keeps the transactions and checkpoints of the epochs from {} on",
            self.floor
        );
        Ok(())
    }

    /// Whether an earlier run has written any progress.
    fn started(&self) -> IotaResult<bool> {
        Ok(self
            .perpetual_tables
            .ledger_backlog_migration_progress
            .get(&())?
            .is_some()
            || self
                .checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .get(&())?
                .is_some())
    }

    /// Refuses to start a migration the disk holding the buckets has too
    /// little free space for: the drains copy the retained rows into the
    /// buckets, and the flat tables are freed only once they are done, so a
    /// migration that runs out of space part way can never finish.
    fn check_disk_space(&self) -> IotaResult<()> {
        let perpetual = &self.perpetual_tables;
        let checkpoints = &self.checkpoint_store.tables;
        let sizes = [
            perpetual
                .transactions
                .db
                .sst_files_size(perpetual.transactions.cf_name())?,
            perpetual
                .effects
                .db
                .sst_files_size(perpetual.effects.cf_name())?,
            perpetual
                .events_2
                .db
                .sst_files_size(perpetual.events_2.cf_name())?,
            perpetual
                .executed_effects
                .db
                .sst_files_size(perpetual.executed_effects.cf_name())?,
            perpetual
                .executed_transactions_to_checkpoint
                .db
                .sst_files_size(perpetual.executed_transactions_to_checkpoint.cf_name())?,
            checkpoints
                .checkpoint_content
                .db
                .sst_files_size(checkpoints.checkpoint_content.cf_name())?,
            checkpoints
                .checkpoint_by_digest
                .db
                .sst_files_size(checkpoints.checkpoint_by_digest.cf_name())?,
        ];
        let oldest_held_epoch = match self
            .checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
        {
            Some(pruned) => self
                .checkpoint_store
                .get_checkpoint_by_sequence_number(pruned + 1)?
                .map_or(self.epoch, |checkpoint| checkpoint.epoch()),
            None => 0,
        };
        let needed = estimated_copy_bytes(
            sizes.iter().sum(),
            self.epoch,
            self.floor,
            oldest_held_epoch,
        );
        ensure_free_space(&self.historic_ledger.files_dir(), needed)
    }

    /// Whether the step that deletes the rows of expired epochs has begun.
    /// The drains before it delete only rows they moved.
    fn deleting_expired_rows(&self) -> IotaResult<bool> {
        Ok(matches!(
            self.checkpoint_store
                .tables
                .checkpoint_backlog_migration_progress
                .get(&())?,
            Some(
                CheckpointBacklogMigrationProgress::ContentsWithoutSummary(_)
                    | CheckpointBacklogMigrationProgress::Done
            )
        ))
    }

    /// Reads up to [`Self::keys_per_slice`] rows of `flat` within `range`,
    /// grouped by the epoch `epoch_of` gives them.
    fn read_slice<K, V>(
        &self,
        flat: &DBMap<K, V>,
        range: (Bound<K>, Bound<K>),
        epoch_of: impl Fn(&K, &V) -> IotaResult<Option<EpochId>>,
    ) -> IotaResult<Slice<K, V>>
    where
        K: Serialize + DeserializeOwned + Copy,
        V: Serialize + DeserializeOwned,
    {
        let mut slice = Slice::new();
        for (read, row) in flat.safe_range_iter(range).enumerate() {
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

    /// Reads the next slice of key range `range` of the four tables keyed by
    /// transaction digest into `pending`.
    ///
    /// The tables are walked together, so each transaction's epoch is
    /// resolved once for all of its rows: from its finalizing checkpoint, or
    /// else from its effects. A transaction with neither has no record of
    /// being executed here, as for one state sync fetched but execution has
    /// not reached (see [`Self::rewind_synced_watermark`]), and its rows are
    /// dropped. The exception is a randomness state update a validator stored
    /// before executing it, since its signature cannot always be built again;
    /// it is filed under the epoch it names.
    fn transactions_slice(
        &self,
        range: usize,
        from: Option<TransactionDigest>,
        pending: &mut PendingRows,
    ) -> IotaResult<LedgerSlice<TransactionDigest>> {
        let tables = &self.perpetual_tables;
        let range = key_range(range, from, TransactionDigest::new);
        let mut transactions = tables.transactions.safe_range_iter(range).peekable();
        let mut events = tables.events_2.safe_range_iter(range).peekable();
        let mut executed = tables.executed_effects.safe_range_iter(range).peekable();
        let mut finalized = tables
            .executed_transactions_to_checkpoint
            .safe_range_iter(range)
            .peekable();

        let mut rows = Vec::new();
        let mut sliced = false;
        while let Some(digest) = [
            next_key(&mut transactions)?,
            next_key(&mut events)?,
            next_key(&mut executed)?,
            next_key(&mut finalized)?,
        ]
        .into_iter()
        .flatten()
        .min()
        {
            if rows.len() == self.keys_per_slice {
                sliced = true;
                break;
            }
            rows.push(TransactionRows {
                digest,
                transaction: take_row(&mut transactions, &digest)?,
                events: take_row(&mut events, &digest)?,
                executed: take_row(&mut executed, &digest)?,
                finalized: take_row(&mut finalized, &digest)?,
            });
        }

        // Effects only for the transactions no checkpoint has finalized, as on
        // a validator, which never writes `executed_transactions_to_checkpoint`.
        let effects_digests: Vec<_> = rows
            .iter()
            .filter(|row| row.finalized.is_none())
            .filter_map(|row| row.executed)
            .collect();
        let effects_epochs: BTreeMap<TransactionEffectsDigest, EpochId> = effects_digests
            .iter()
            .copied()
            .zip(tables.effects.multi_get(&effects_digests)?)
            .filter_map(|(digest, effects)| Some((digest, effects?.epoch())))
            .collect();

        let mut counts = MigrationCounts::default();
        for row in rows.iter() {
            let epoch = match (&row.finalized, &row.executed, &row.transaction) {
                (Some((epoch, _)), _, _) => Some(*epoch),
                (None, Some(effects), _) if effects_epochs.contains_key(effects) => {
                    effects_epochs.get(effects).copied()
                }
                (None, _, Some(transaction)) => match transaction.inner().transaction().kind() {
                    TransactionKind::RandomnessStateUpdate(update) => Some(update.epoch),
                    _ => None,
                },
                _ => None,
            };
            let Some(epoch) = epoch.filter(|epoch| *epoch >= self.floor) else {
                counts.expired += row.len();
                continue;
            };
            counts.moved += row.len();
            let bucket = pending.bucket(&self.historic_ledger, epoch)?;
            let target = Arc::clone(&bucket.bucket);
            if let Some(transaction) = &row.transaction {
                bucket.transactions.insert_tagged(
                    &target.transactions,
                    &row.digest,
                    transaction,
                )?;
            }
            if let Some(events) = &row.events {
                bucket
                    .events
                    .insert_tagged(&target.events, &row.digest, events)?;
            }
            if let Some(executed) = &row.executed {
                bucket.executed_effects.insert_tagged(
                    &target.executed_effects,
                    &row.digest,
                    executed,
                )?;
            }
            // The bucket's epoch is the checkpoint's epoch, so only the
            // sequence number is kept.
            if let Some((_, sequence)) = &row.finalized {
                bucket.tx_to_checkpoint.insert_tagged(
                    &target.tx_to_checkpoint,
                    &row.digest,
                    sequence,
                )?;
            }
        }
        let next = match rows.last() {
            Some(last) if sliced => RangeProgress::At(Some(last.digest)),
            _ => RangeProgress::Done,
        };
        Ok(LedgerSlice {
            counts,
            read: rows.len(),
            next,
        })
    }

    /// Reads the next slice of key range `range` of the effects table into
    /// `pending`.
    fn effects_slice(
        &self,
        range: usize,
        from: Option<TransactionEffectsDigest>,
        pending: &mut PendingRows,
    ) -> IotaResult<LedgerSlice<TransactionEffectsDigest>> {
        let flat = &self.perpetual_tables.effects;
        let slice = self.read_slice(
            flat,
            key_range(range, from, TransactionEffectsDigest::new),
            |_, effects| Ok(Some(effects.epoch())),
        )?;
        let mut moved = 0;
        for (epoch, rows) in slice.by_epoch.iter() {
            let bucket = pending.bucket(&self.historic_ledger, *epoch)?;
            moved += rows.len();
            for (digest, effects) in rows {
                bucket
                    .effects
                    .insert_tagged(&bucket.bucket.effects, digest, effects)?;
            }
        }
        Ok(LedgerSlice {
            counts: MigrationCounts {
                moved,
                expired: slice.expired.len(),
            },
            read: slice.keys.len(),
            next: slice.range_progress(),
        })
    }

    /// Builds the write batch moving the next slice of key range `range` of
    /// the flat digest-keyed checkpoint summaries into the buckets, each with
    /// the contents row it names.
    ///
    /// Checkpoints in different epochs can share one contents row (every empty
    /// checkpoint does), so each epoch's bucket gets its own copy; expiring one
    /// epoch then cannot leave another's checkpoint without contents.
    fn summaries_slice(
        &self,
        range: usize,
        from: Option<CheckpointDigest>,
    ) -> IotaResult<SliceBatch<CheckpointDigest>> {
        let tables = &self.checkpoint_store.tables;
        let flat = &tables.checkpoint_by_digest;
        let slice = self.read_slice(
            flat,
            key_range(range, from, CheckpointDigest::new),
            |_, summary| Ok(Some(summary.inner().epoch)),
        )?;

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
        Ok(SliceBatch {
            batch,
            counts: MigrationCounts {
                moved,
                expired: slice.expired.len(),
            },
            read: slice.keys.len(),
            next: slice.range_progress(),
        })
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
        let range = (
            from.map_or(Bound::Unbounded, Bound::Excluded),
            Bound::Unbounded,
        );
        let slice = self.read_slice(flat, range, |_, contents| {
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

/// One transaction's rows in the four flat tables keyed by its digest.
struct TransactionRows {
    digest: TransactionDigest,
    transaction: Option<TrustedTransaction>,
    events: Option<TransactionEvents>,
    executed: Option<TransactionEffectsDigest>,
    finalized: Option<(EpochId, CheckpointSequenceNumber)>,
}

impl TransactionRows {
    /// How many of the four tables hold a row for the transaction.
    fn len(&self) -> usize {
        usize::from(self.transaction.is_some())
            + usize::from(self.events.is_some())
            + usize::from(self.executed.is_some())
            + usize::from(self.finalized.is_some())
    }
}

/// The key of the row `rows` yields next, `None` once it is exhausted.
fn next_key<V>(
    rows: &mut std::iter::Peekable<DbIterator<'_, (TransactionDigest, V)>>,
) -> IotaResult<Option<TransactionDigest>> {
    match rows.peek() {
        Some(Ok((key, _))) => Ok(Some(*key)),
        Some(Err(_)) => match rows.next() {
            Some(Err(e)) => Err(e.into()),
            _ => unreachable!("the peeked row is an error"),
        },
        None => Ok(None),
    }
}

/// Takes the next row of `rows` if it is keyed by `digest`.
fn take_row<V>(
    rows: &mut std::iter::Peekable<DbIterator<'_, (TransactionDigest, V)>>,
    digest: &TransactionDigest,
) -> IotaResult<Option<V>> {
    if next_key(rows)?.as_ref() != Some(digest) {
        return Ok(None);
    }
    Ok(rows.next().transpose()?.map(|(_, value)| value))
}

#[cfg(test)]
#[path = "../unit_tests/ledger_backlog_migration_tests.rs"]
mod tests;
