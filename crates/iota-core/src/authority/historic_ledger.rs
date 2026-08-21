// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checkpoint-keyed transaction history in the perpetual database, bucketed
//! by the epoch that executed it.
//!
//! The buckets are extra column families of the perpetual database rather
//! than a store of their own (see [`crate::epoch_buckets`]), the same way
//! [`crate::authority::historic_objects`] holds superseded object versions:
//! written once, in the epoch that produced them, and read back by digest.

use std::{collections::BTreeMap, fmt::Debug, path::Path, sync::Arc};

use iota_sdk_types::{
    TransactionDigest, TransactionEffects, TransactionEffectsDigest, TransactionEvents,
};
use iota_types::{
    committee::EpochId,
    error::{IotaError, IotaResult},
    messages_checkpoint::CheckpointSequenceNumber,
    storage::ObjectKey,
    transaction::TrustedTransaction,
};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables},
    traits::Map,
};

use crate::epoch_buckets::{EpochBuckets, bucket_cf_epoch};

/// Column-family prefix of the historic ledger buckets; a bucket's family
/// is `{prefix}{epoch}`.
const HISTORIC_LEDGER_CF_PREFIX: &str = "hist_ledger_e";

/// Tags of the tables inside a bucket's column family. Do not reuse a tag
/// for a different table: mark it retired in a comment instead, so an older
/// bucket's rows can never be read as the wrong type.
const DB_PREFIX_HISTORIC_TRANSACTIONS: u8 = 0;
const DB_PREFIX_HISTORIC_EFFECTS: u8 = 1;
const DB_PREFIX_HISTORIC_EXECUTED_EFFECTS: u8 = 2;
const DB_PREFIX_HISTORIC_EVENTS: u8 = 3;
const DB_PREFIX_HISTORIC_UNCHANGED_LOADED_RUNTIME_OBJECTS: u8 = 4;
const DB_PREFIX_HISTORIC_TX_TO_CHECKPOINT: u8 = 5;

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets`] persists on a prune. It is empty until the first prune,
/// which is the same as retaining every bucket.
///
/// The name must not begin with [`HISTORIC_LEDGER_CF_PREFIX`], since that is
/// how a bucket's column family is told from every other one in this
/// database.
const EARLIEST_RETAINED_CF: &str = "hist_ledger_retention";

/// One epoch's checkpoint-keyed ledger history.
///
/// Everything about a single transaction is in one bucket, so a caller that
/// has found the epoch for a digest reads the rest directly rather than
/// probing again.
pub struct HistoricLedgerBucket {
    pub(crate) transactions: TaggedDBMap<TransactionDigest, TrustedTransaction>,
    pub(crate) effects: TaggedDBMap<TransactionEffectsDigest, TransactionEffects>,
    pub(crate) executed_effects: TaggedDBMap<TransactionDigest, TransactionEffectsDigest>,
    pub(crate) events: TaggedDBMap<TransactionDigest, TransactionEvents>,
    pub(crate) unchanged_loaded_runtime_objects: TaggedDBMap<TransactionDigest, Vec<ObjectKey>>,
    pub(crate) tx_to_checkpoint: TaggedDBMap<TransactionDigest, CheckpointSequenceNumber>,
}

impl HistoricLedgerBucket {
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
            unchanged_loaded_runtime_objects: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_UNCHANGED_LOADED_RUNTIME_OBJECTS,
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

/// Checkpoint-keyed transaction history, bucketed by the epoch that executed
/// it.
///
/// The buckets are column families of the perpetual database rather than a
/// store of their own, so a transaction's outputs can be committed alongside
/// the rest of that commit's batch.
pub struct HistoricLedger {
    buckets: EpochBuckets<HistoricLedgerBucket>,
}

impl HistoricLedger {
    /// Options for a historic-ledger bucket's column family: written once,
    /// while the epoch that executed its rows is current, then only ever
    /// read back by exact-key lookup.
    ///
    /// `db_options` are the perpetual database's base options. Build this
    /// once and clone it per column family, as
    /// [`crate::authority::historic_objects::HistoricObjects::cf_options`]
    /// does: the clones share the base options' block cache instead of each
    /// allocating one of their own.
    fn cf_options(db_options: &DBOptions) -> DBOptions {
        db_options
            .clone()
            .optimize_for_write_throughput_no_deletion()
    }

    /// The `(name, options)` pairs of the column families this store needs,
    /// for the perpetual store's open path to list alongside its own tables
    /// and the historic-object buckets: a column family left for
    /// auto-discovery would otherwise be reopened with default options and a
    /// block cache of its own.
    pub fn extra_column_family_options(
        perpetual_path: &Path,
        db_options: &DBOptions,
    ) -> Vec<(String, DBOptions)> {
        let cf_options = Self::cf_options(db_options);
        let mut options = vec![(EARLIEST_RETAINED_CF.to_string(), cf_options.clone())];
        if !perpetual_path.join("CURRENT").exists() {
            return options;
        }
        let Ok(existing_cfs) = list_tables(perpetual_path.to_path_buf()) else {
            return options;
        };
        options.extend(
            existing_cfs
                .into_iter()
                .filter(|name| bucket_cf_epoch(HISTORIC_LEDGER_CF_PREFIX, name).is_some())
                .map(|name| (name, cf_options.clone())),
        );
        options
    }

    /// Opens the historic-ledger buckets already present among `db`'s
    /// column families. `db` is the perpetual database's own handle: the
    /// buckets are its column families, not a database of their own, and
    /// `db_options` are the options its tables were opened with.
    pub fn open(db: Arc<Database>, db_options: &DBOptions) -> Result<Self, TypedStoreError> {
        let existing_cfs = list_tables(db.path_for_pruning().to_path_buf())
            .map_err(|e| TypedStoreError::RocksDB(format!("failed to list buckets: {e}")))?;

        let mut buckets = BTreeMap::new();
        for cf_name in &existing_cfs {
            if let Some(epoch) = bucket_cf_epoch(HISTORIC_LEDGER_CF_PREFIX, cf_name) {
                buckets.insert(epoch, Arc::new(HistoricLedgerBucket::reopen(&db, cf_name)?));
            }
        }

        let cf_options = Self::cf_options(db_options).options;
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
            earliest_retained_table,
            buckets,
            HistoricLedgerBucket::reopen,
        )?;
        Ok(Self { buckets })
    }

    /// The bucket holding `epoch`'s transaction history, created if absent.
    pub fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<HistoricLedgerBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// One page of the rows `cf_name` holds, if it is one of this store's
    /// column families: a bucket of transaction history, or the
    /// retention-floor family. `None` for any other name, leaving the caller
    /// to report it as unknown.
    ///
    /// For the table dump of `iota-tool`, which walks the perpetual
    /// database's column families by name: these are not fields of
    /// `AuthorityPerpetualTables`, so the dump derived from it cannot read
    /// them. `db` may be a read-only or secondary handle — nothing here
    /// writes, and no column family is created.
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
            bucket
                .unchanged_loaded_runtime_objects
                .try_catch_up_with_primary()?;
            bucket.tx_to_checkpoint.try_catch_up_with_primary()?;
            let rows = format_rows("transaction:", bucket.transactions.safe_iter())
                .chain(format_rows("effects:", bucket.effects.safe_iter()))
                .chain(format_rows(
                    "executed_effects:",
                    bucket.executed_effects.safe_iter(),
                ))
                .chain(format_rows("events:", bucket.events.safe_iter()))
                .chain(format_rows(
                    "unchanged_loaded_runtime_objects:",
                    bucket.unchanged_loaded_runtime_objects.safe_iter(),
                ))
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
