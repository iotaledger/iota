// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transaction history, bucketed by the epoch that executed it.
//!
//! The buckets are column families of the perpetual database (see
//! `epoch_buckets`), as in [`crate::authority::historic_objects`].

use std::{collections::BTreeMap, fmt::Debug, path::Path, sync::Arc};

use iota_sdk_types::{
    TransactionDigest, TransactionEffects, TransactionEffectsDigest, TransactionEvents,
};
use iota_types::{
    committee::EpochId,
    error::{IotaError, IotaResult},
    messages_checkpoint::CheckpointSequenceNumber,
    transaction::TrustedTransaction,
};
use serde::{Serialize, de::DeserializeOwned};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables},
    traits::Map,
};

use crate::epoch_buckets::{
    BucketPaths, BucketReopen, EpochBuckets, absent_if_dropped, bucket_cf_epoch, bucket_cf_options,
    extra_column_family_options,
};

/// Column-family prefix of the historic ledger buckets; a bucket's family
/// is `{prefix}{epoch}`.
const HISTORIC_LEDGER_CF_PREFIX: &str = "hist_ledger_e";

/// The directory of each epoch, under the historic root, that this
/// store's buckets keep their files in.
const BUCKET_DIR: &str = "ledger";

/// Tags of the tables inside a bucket's column family. Do not reuse a tag
/// for a different table: mark it retired in a comment instead, so an older
/// bucket's rows can never be read as the wrong type.
const DB_PREFIX_HISTORIC_TRANSACTIONS: u8 = 0;
const DB_PREFIX_HISTORIC_EFFECTS: u8 = 1;
const DB_PREFIX_HISTORIC_EXECUTED_EFFECTS: u8 = 2;
const DB_PREFIX_HISTORIC_EVENTS: u8 = 3;
const DB_PREFIX_HISTORIC_TX_TO_CHECKPOINT: u8 = 4;

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets`] persists on a prune; empty until the first prune.
///
/// The name must not begin with [`HISTORIC_LEDGER_CF_PREFIX`], since that is
/// how a bucket's column family is told from every other one in this
/// database.
const EARLIEST_RETAINED_CF: &str = "hist_ledger_retention";

/// A value read from a bucket, with that bucket; `None` where no bucket held
/// it.
type FoundIn<V> = Option<(Arc<HistoricLedgerBucket>, V)>;

/// The keys to read from one bucket, each with its position in the batch.
type KeysOfBucket<K> = (Arc<HistoricLedgerBucket>, Vec<(usize, K)>);

/// One epoch's transaction history. All rows of a transaction are in the same
/// bucket.
pub struct HistoricLedgerBucket {
    pub(crate) transactions: TaggedDBMap<TransactionDigest, TrustedTransaction>,
    pub(crate) effects: TaggedDBMap<TransactionEffectsDigest, TransactionEffects>,
    pub(crate) executed_effects: TaggedDBMap<TransactionDigest, TransactionEffectsDigest>,
    pub(crate) events: TaggedDBMap<TransactionDigest, TransactionEvents>,
    pub(crate) tx_to_checkpoint: TaggedDBMap<TransactionDigest, CheckpointSequenceNumber>,
}

impl BucketReopen for HistoricLedgerBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        Ok(Self {
            transactions: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_TRANSACTIONS,
                &ReadWriteOptions::default(),
                true,
            )?,
            effects: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_EFFECTS,
                &ReadWriteOptions::default(),
                true,
            )?,
            executed_effects: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_EXECUTED_EFFECTS,
                &ReadWriteOptions::default(),
                true,
            )?,
            events: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_EVENTS,
                &ReadWriteOptions::default(),
                true,
            )?,
            tx_to_checkpoint: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_TX_TO_CHECKPOINT,
                &ReadWriteOptions::default(),
                true,
            )?,
        })
    }
}

/// Transaction history, bucketed by the epoch that executed it.
///
/// A bucket's existence does **not** mean its epoch has been executed: state
/// sync writes a checkpoint's transactions and effects into the bucket of its
/// epoch ahead of execution, also across epoch boundaries. Retention must
/// therefore count from the epoch being executed, not from the newest bucket;
/// see [`Self::prune`].
pub struct HistoricLedger {
    buckets: EpochBuckets<HistoricLedgerBucket>,

    /// Counts the bucket walks this store has done.
    #[cfg(test)]
    bucket_walks: std::sync::atomic::AtomicU64,
}

impl HistoricLedger {
    /// The `(name, options)` pairs of the column families this store needs,
    /// for the perpetual store to open alongside its own tables. See
    /// `epoch_buckets::extra_column_family_options`.
    pub fn extra_column_family_options(
        perpetual_path: &Path,
        db_options: &DBOptions,
        historic_root: &Path,
    ) -> Vec<(String, DBOptions)> {
        extra_column_family_options(
            perpetual_path,
            db_options,
            HISTORIC_LEDGER_CF_PREFIX,
            EARLIEST_RETAINED_CF,
            Some(&BucketPaths::new(historic_root, BUCKET_DIR)),
        )
    }

    /// Opens the historic-ledger buckets among the column families of `db`,
    /// the perpetual database. `db_options` and `historic_root` must be the
    /// ones `db` was opened with.
    pub fn open(
        db: Arc<Database>,
        db_options: &DBOptions,
        historic_root: &Path,
    ) -> Result<Self, TypedStoreError> {
        let existing_cfs = list_tables(db.path_for_pruning().to_path_buf())
            .map_err(|e| TypedStoreError::RocksDB(format!("failed to list buckets: {e}")))?;

        let mut buckets = BTreeMap::new();
        for cf_name in &existing_cfs {
            if let Some(epoch) = bucket_cf_epoch(HISTORIC_LEDGER_CF_PREFIX, cf_name) {
                buckets.insert(epoch, Arc::new(HistoricLedgerBucket::reopen(&db, cf_name)?));
            }
        }

        let cf_options = bucket_cf_options(db_options).options;
        if db.cf_handle(EARLIEST_RETAINED_CF).is_none() {
            db.create_cf(EARLIEST_RETAINED_CF, &cf_options)?;
        }
        let earliest_retained_table: DBMap<(), EpochId> = DBMap::reopen(
            &db,
            Some(EARLIEST_RETAINED_CF),
            &ReadWriteOptions::default(),
            true,
        )?;

        let buckets = EpochBuckets::open(
            db,
            "historic ledger",
            HISTORIC_LEDGER_CF_PREFIX,
            cf_options,
            Some(BucketPaths::new(historic_root, BUCKET_DIR)),
            earliest_retained_table,
            buckets,
        )?;
        Ok(Self {
            buckets,
            #[cfg(test)]
            bucket_walks: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Records that a read walked the buckets.
    #[cfg(test)]
    fn count_walk(&self) {
        self.bucket_walks
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// How many bucket walks this store has done.
    #[cfg(test)]
    pub(crate) fn bucket_walks(&self) -> u64 {
        self.bucket_walks.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// The oldest epoch this store holds a bucket for, `None` when it holds
    /// none; transactions executed before it are not readable. A node restored
    /// from a formal snapshot starts with no bucket, whatever the retention.
    pub fn earliest_bucket_epoch(&self) -> Option<EpochId> {
        self.buckets.earliest_epoch()
    }

    /// The earliest epoch [`Self::prune`] retains; the transactions of every
    /// epoch below it are gone.
    pub fn earliest_retained_epoch(&self) -> EpochId {
        self.buckets.earliest_retained()
    }

    /// The options SST files for a bucket must be written with.
    pub(crate) fn sst_file_options(&self) -> &typed_store::rocksdb::Options {
        self.buckets.cf_options()
    }

    /// The bucket holding `epoch`'s transaction history, created if absent.
    pub fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<HistoricLedgerBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The bucket holding `epoch`'s transaction history, and `None` once that
    /// epoch has been expired. See
    /// `EpochBuckets::ensure_retained`.
    pub fn ensure_retained(&self, epoch: EpochId) -> IotaResult<Option<Arc<HistoricLedgerBucket>>> {
        self.buckets
            .ensure_retained(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// Keeps the buckets of `current_epoch` and the `epochs_to_retain` epochs
    /// below it, drops older ones, and returns the earliest epoch retained.
    ///
    /// `current_epoch` must be the epoch this node is executing, not the
    /// newest bucket (see [`HistoricLedger`]); buckets above it are left
    /// alone. Blocks for as long as the drops take, so a caller on an async
    /// runtime must use `spawn_blocking`.
    ///
    /// `before_drop` runs before each bucket is dropped, once the earliest
    /// epoch retained has moved past it, so that what advertises the retained
    /// range can be moved first.
    pub fn prune(
        &self,
        current_epoch: EpochId,
        epochs_to_retain: u64,
        before_drop: &dyn Fn(),
    ) -> IotaResult<Option<EpochId>> {
        self.buckets
            // No live table holds rows that depend on a bucket.
            .prune(current_epoch, epochs_to_retain, |_, _| {
                before_drop();
                Ok(())
            })
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// Compacts the buckets of the epochs below `current_epoch` that are not
    /// compacted yet (see
    /// `EpochBuckets::compact_closed`, which asks
    /// `stop` before each bucket). Blocks while
    /// the compactions run, so async callers must use `spawn_blocking`.
    pub fn compact_closed_buckets(
        &self,
        current_epoch: EpochId,
        stop: &dyn Fn() -> bool,
    ) -> IotaResult<usize> {
        self.buckets
            .compact_closed(current_epoch, stop)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// For each of `keys`, the newest bucket holding it in the table `table`
    /// picks, with the value it holds there; `None` where no bucket does.
    ///
    /// One batched read per bucket, newest first, for the keys no newer
    /// bucket held, so a batch costs a read per bucket rather than one per
    /// key and bucket. A bucket dropped meanwhile holds nothing.
    fn multi_find<K, V>(
        &self,
        keys: &[K],
        table: impl Fn(&HistoricLedgerBucket) -> &TaggedDBMap<K, V>,
    ) -> IotaResult<Vec<FoundIn<V>>>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        #[cfg(test)]
        self.count_walk();
        let mut found: Vec<FoundIn<V>> = keys.iter().map(|_| None).collect();
        let mut pending: Vec<usize> = (0..keys.len()).collect();
        for bucket in self.buckets.iter(true) {
            if pending.is_empty() {
                break;
            }
            let values = absent_if_dropped(
                table(&bucket).multi_get(pending.iter().map(|&index| &keys[index])),
            )
            .map_err(|e| IotaError::Storage(e.to_string()))?;
            // Empty for a dropped bucket, which leaves every key pending.
            if values.is_empty() {
                continue;
            }
            let mut still_pending = Vec::new();
            for (index, value) in pending.into_iter().zip(values) {
                match value {
                    Some(value) => found[index] = Some((bucket.clone(), value)),
                    None => still_pending.push(index),
                }
            }
            pending = still_pending;
        }
        Ok(found)
    }

    /// For each `(bucket, key)` of `lookups`, the value `bucket` holds under
    /// `key` in the table `table` picks; `None` for a `None` lookup. One
    /// batched read per distinct bucket.
    fn multi_get_in_buckets<K, V>(
        lookups: Vec<FoundIn<K>>,
        table: impl Fn(&HistoricLedgerBucket) -> &TaggedDBMap<K, V>,
    ) -> IotaResult<Vec<Option<V>>>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let mut values: Vec<Option<V>> = lookups.iter().map(|_| None).collect();
        let mut by_bucket: Vec<KeysOfBucket<K>> = Vec::new();
        for (index, lookup) in lookups.into_iter().enumerate() {
            let Some((bucket, key)) = lookup else {
                continue;
            };
            match by_bucket
                .iter_mut()
                .find(|(other, _)| Arc::ptr_eq(other, &bucket))
            {
                Some((_, keys)) => keys.push((index, key)),
                None => by_bucket.push((bucket, vec![(index, key)])),
            }
        }
        for (bucket, keys) in by_bucket {
            let found =
                absent_if_dropped(table(&bucket).multi_get(keys.iter().map(|(_, key)| key)))
                    .map_err(|e| IotaError::Storage(e.to_string()))?;
            for ((index, _), value) in keys.into_iter().zip(found) {
                values[index] = value;
            }
        }
        Ok(values)
    }

    /// [`Self::get_executed_effects_digest`] for each of `digests`.
    pub fn multi_get_executed_effects_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffectsDigest>>> {
        Ok(self
            .multi_find(digests, |bucket| &bucket.executed_effects)?
            .into_iter()
            .map(|found| found.map(|(_, effects_digest)| effects_digest))
            .collect())
    }

    /// [`Self::multi_get_executed_effects_digests`] read from `epoch`'s bucket
    /// alone, in one batched read. A transaction executed in another epoch is
    /// reported as not executed.
    pub fn multi_get_executed_effects_digests_in_epoch(
        &self,
        epoch: EpochId,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffectsDigest>>> {
        self.multi_get_in_epoch(epoch, digests, |bucket| &bucket.executed_effects)
    }

    /// [`Self::multi_get_transactions`] read from `epoch`'s bucket alone, in
    /// one batched read. A transaction stored under another epoch is reported
    /// as absent.
    pub fn multi_get_transactions_in_epoch(
        &self,
        epoch: EpochId,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TrustedTransaction>>> {
        self.multi_get_in_epoch(epoch, digests, |bucket| &bucket.transactions)
    }

    /// [`Self::multi_get_effects`] read from `epoch`'s bucket alone, in one
    /// batched read. Effects stored under another epoch are reported as
    /// absent.
    pub fn multi_get_effects_in_epoch(
        &self,
        epoch: EpochId,
        digests: &[TransactionEffectsDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        self.multi_get_in_epoch(epoch, digests, |bucket| &bucket.effects)
    }

    /// [`Self::multi_get_events`] read from `epoch`'s bucket alone, in one
    /// batched read. Events stored under another epoch are reported as
    /// absent.
    pub fn multi_get_events_in_epoch(
        &self,
        epoch: EpochId,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        self.multi_get_in_epoch(epoch, digests, |bucket| &bucket.events)
    }

    /// The values `keys` hold in `epoch`'s bucket, in the table `table`
    /// picks; all `None` when there is no such bucket or it was dropped.
    fn multi_get_in_epoch<K, V>(
        &self,
        epoch: EpochId,
        keys: &[K],
        table: impl Fn(&HistoricLedgerBucket) -> &TaggedDBMap<K, V>,
    ) -> IotaResult<Vec<Option<V>>>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let Some(bucket) = self.buckets.get(epoch) else {
            return Ok(keys.iter().map(|_| None).collect());
        };
        let found = absent_if_dropped(table(&bucket).multi_get(keys))
            .map_err(|e| IotaError::Storage(e.to_string()))?;
        // Empty for a dropped bucket.
        if found.is_empty() {
            return Ok(keys.iter().map(|_| None).collect());
        }
        Ok(found)
    }

    /// [`Self::get_executed_effects`] for each of `digests`, read from the
    /// bucket holding each one's execution record.
    pub fn multi_get_executed_effects(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        let records = self.multi_find(digests, |bucket| &bucket.executed_effects)?;
        Self::multi_get_in_buckets(records, |bucket| &bucket.effects)
    }

    /// [`Self::get_events`] for each of `digests`, read from the bucket holding
    /// each one's execution record.
    pub fn multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        let records = self.multi_find(digests, |bucket| &bucket.executed_effects)?;
        let lookups = records
            .into_iter()
            .zip(digests)
            .map(|(record, digest)| record.map(|(bucket, _)| (bucket, *digest)))
            .collect();
        Self::multi_get_in_buckets(lookups, |bucket| &bucket.events)
    }

    /// [`Self::get_transaction`] for each of `digests`.
    pub fn multi_get_transactions(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TrustedTransaction>>> {
        Ok(self
            .multi_find(digests, |bucket| &bucket.transactions)?
            .into_iter()
            .map(|found| found.map(|(_, transaction)| transaction))
            .collect())
    }

    /// [`Self::get_effects`] for each of `digests`.
    pub fn multi_get_effects(
        &self,
        digests: &[TransactionEffectsDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        Ok(self
            .multi_find(digests, |bucket| &bucket.effects)?
            .into_iter()
            .map(|found| found.map(|(_, effects)| effects))
            .collect())
    }

    /// The bucket holding the execution record of `digest`, with its epoch;
    /// `None` if this node has not executed it or the bucket was dropped.
    ///
    /// Read the transaction's effects, events and checkpoint from the returned
    /// bucket rather than probing again. A transaction stored but not yet
    /// executed is not found here; see [`Self::get_transaction`].
    pub fn find_epoch(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<(EpochId, Arc<HistoricLedgerBucket>)>> {
        #[cfg(test)]
        self.count_walk();
        for (epoch, bucket) in self.buckets.iter_with_epoch(true) {
            if absent_if_dropped(bucket.executed_effects.contains_key(digest))
                .map_err(|e| IotaError::Storage(e.to_string()))?
            {
                return Ok(Some((epoch, bucket)));
            }
        }
        Ok(None)
    }

    /// The effects of `digest` as this node executed it, `None` if no bucket
    /// holds an execution record for it.
    pub fn get_executed_effects(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        let Some((_, bucket)) = self.find_epoch(digest)? else {
            return Ok(None);
        };
        let effects = absent_if_dropped(bucket.executed_effects.get(digest))
            .map_err(|e| IotaError::Storage(e.to_string()))?
            .map(|effects_digest| absent_if_dropped(bucket.effects.get(&effects_digest)))
            .transpose()
            .map_err(|e| IotaError::Storage(e.to_string()))?
            .flatten();
        Ok(effects)
    }

    /// The digest of the effects of `digest` as this node executed it, `None`
    /// if no bucket holds an execution record for it.
    pub fn get_executed_effects_digest(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffectsDigest>> {
        let Some((_, bucket)) = self.find_epoch(digest)? else {
            return Ok(None);
        };
        absent_if_dropped(bucket.executed_effects.get(digest))
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The events `digest` emitted as this node executed it, `None` if no
    /// bucket holds an execution record for it.
    pub fn get_events(&self, digest: &TransactionDigest) -> IotaResult<Option<TransactionEvents>> {
        let Some((_, bucket)) = self.find_epoch(digest)? else {
            return Ok(None);
        };
        absent_if_dropped(bucket.events.get(digest)).map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The checkpoint that finalized `digest`, with the epoch that checkpoint
    /// belongs to, `None` while the transaction is executed but not yet
    /// finalized, and for a transaction no bucket holds.
    pub fn get_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<(EpochId, CheckpointSequenceNumber)>> {
        let Some((epoch, bucket)) = self.find_epoch(digest)? else {
            return Ok(None);
        };
        // A bucket's epoch is also the epoch of its checkpoints.
        Ok(absent_if_dropped(bucket.tx_to_checkpoint.get(digest))
            .map_err(|e| IotaError::Storage(e.to_string()))?
            .map(|sequence| (epoch, sequence)))
    }

    /// [`Self::get_transaction_checkpoint`] for each of `digests`, in one
    /// batched read per bucket, newest first, for the digests no newer bucket
    /// held. A checkpoint is recorded in the bucket of the epoch that executed
    /// the transaction, which is also the checkpoint's epoch.
    pub fn multi_get_transaction_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<(EpochId, CheckpointSequenceNumber)>>> {
        #[cfg(test)]
        self.count_walk();
        let mut found = vec![None; digests.len()];
        let mut pending: Vec<usize> = (0..digests.len()).collect();
        for (epoch, bucket) in self.buckets.iter_with_epoch(true) {
            if pending.is_empty() {
                break;
            }
            let values = absent_if_dropped(
                bucket
                    .tx_to_checkpoint
                    .multi_get(pending.iter().map(|&index| &digests[index])),
            )
            .map_err(|e| IotaError::Storage(e.to_string()))?;
            // Empty for a dropped bucket, which leaves every digest pending.
            if values.is_empty() {
                continue;
            }
            let mut still_pending = Vec::with_capacity(pending.len());
            for (index, sequence) in pending.into_iter().zip(values) {
                match sequence {
                    Some(sequence) => found[index] = Some((epoch, sequence)),
                    None => still_pending.push(index),
                }
            }
            pending = still_pending;
        }
        Ok(found)
    }

    /// The transaction stored under `digest`, `None` if no bucket holds it.
    /// Also finds a transaction stored before it was executed.
    pub fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TrustedTransaction>> {
        #[cfg(test)]
        self.count_walk();
        for bucket in self.buckets.iter(true) {
            if let Some(transaction) = absent_if_dropped(bucket.transactions.get(digest))
                .map_err(|e| IotaError::Storage(e.to_string()))?
            {
                return Ok(Some(transaction));
            }
        }
        Ok(None)
    }

    /// The effects stored under `digest`, `None` if no bucket holds them.
    ///
    /// A caller that has the transaction digest should read the effects from
    /// the bucket [`Self::find_epoch`] returns instead.
    pub fn get_effects(
        &self,
        digest: &TransactionEffectsDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        #[cfg(test)]
        self.count_walk();
        for bucket in self.buckets.iter(true) {
            if let Some(effects) = absent_if_dropped(bucket.effects.get(digest))
                .map_err(|e| IotaError::Storage(e.to_string()))?
            {
                return Ok(Some(effects));
            }
        }
        Ok(None)
    }

    /// Whether any bucket holds the effects under `digest`.
    pub fn contains_effects(&self, digest: &TransactionEffectsDigest) -> IotaResult<bool> {
        #[cfg(test)]
        self.count_walk();
        for bucket in self.buckets.iter(true) {
            if absent_if_dropped(bucket.effects.contains_key(digest))
                .map_err(|e| IotaError::Storage(e.to_string()))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// One page of the rows of `cf_name` if it is one of this store's column
    /// families, `None` otherwise. For the `iota-tool` table dump, which
    /// cannot reach these through `AuthorityPerpetualTables`.
    ///
    /// Rows are prefixed by table name. `db` may be a read-only or secondary
    /// handle.
    pub fn dump_column_family(
        db: &Arc<Database>,
        cf_name: &str,
        page_size: u16,
        page_number: usize,
    ) -> Result<Option<BTreeMap<String, String>>, TypedStoreError> {
        fn format_rows<'a, K: Debug + 'a, V: Debug + 'a>(
            prefix: &'static str,
            rows: DbIterator<'a, (K, V)>,
        ) -> impl Iterator<Item = Result<(String, String), TypedStoreError>> + 'a {
            rows.map(move |row| {
                row.map(|(key, value)| (format!("{prefix}{key:?}"), format!("{value:?}")))
            })
        }

        fn page(
            rows: impl Iterator<Item = Result<(String, String), TypedStoreError>>,
            page_size: u16,
            page_number: usize,
        ) -> Result<BTreeMap<String, String>, TypedStoreError> {
            rows.skip(page_number * page_size as usize)
                .take(page_size as usize)
                .collect()
        }

        if bucket_cf_epoch(HISTORIC_LEDGER_CF_PREFIX, cf_name).is_some() {
            let bucket = HistoricLedgerBucket::reopen(db, cf_name)?;
            bucket.transactions.try_catch_up_with_primary()?;
            bucket.effects.try_catch_up_with_primary()?;
            bucket.executed_effects.try_catch_up_with_primary()?;
            bucket.events.try_catch_up_with_primary()?;
            bucket.tx_to_checkpoint.try_catch_up_with_primary()?;
            let rows = format_rows("transaction:", bucket.transactions.safe_iter())
                .chain(format_rows("effects:", bucket.effects.safe_iter()))
                .chain(format_rows(
                    "executed_effects:",
                    bucket.executed_effects.safe_iter(),
                ))
                .chain(format_rows("events:", bucket.events.safe_iter()))
                .chain(format_rows(
                    "tx_to_checkpoint:",
                    bucket.tx_to_checkpoint.safe_iter(),
                ));
            return page(rows, page_size, page_number).map(Some);
        }
        if cf_name == EARLIEST_RETAINED_CF {
            let earliest_retained_table: DBMap<(), EpochId> =
                DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)?;
            earliest_retained_table.try_catch_up_with_primary()?;
            return page(
                format_rows("", earliest_retained_table.safe_iter()),
                page_size,
                page_number,
            )
            .map(Some);
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "../unit_tests/historic_ledger_tests.rs"]
mod tests;
