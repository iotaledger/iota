// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transaction history, bucketed by the epoch that executed it.
//!
//! The buckets are column families of the perpetual database (see
//! [`crate::epoch_buckets`]), as in [`crate::authority::historic_objects`].

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
    /// [`extra_column_family_options`](crate::epoch_buckets::extra_column_family_options).
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

    /// The bucket holding `epoch`'s transaction history, created if absent.
    pub fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<HistoricLedgerBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The bucket holding `epoch`'s transaction history, and `None` once that
    /// epoch has been expired. See
    /// [`crate::epoch_buckets::EpochBuckets::ensure_retained`].
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
    pub fn prune(
        &self,
        current_epoch: EpochId,
        epochs_to_retain: u64,
    ) -> IotaResult<Option<EpochId>> {
        self.buckets
            // No live table holds rows that depend on a bucket.
            .prune(current_epoch, epochs_to_retain, |_, _| Ok(()))
            .map_err(|e| IotaError::Storage(e.to_string()))
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
