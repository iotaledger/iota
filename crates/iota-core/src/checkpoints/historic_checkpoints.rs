// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Checkpoint history in the checkpoint store's database, bucketed by the
//! epoch that closed the checkpoint.
//!
//! The buckets are extra column families of the checkpoint store's database
//! rather than a store of their own (see [`crate::epoch_buckets`]), the same
//! way [`crate::authority::historic_ledger`] holds the perpetual database's
//! transaction history: written once, in the epoch the checkpoint belongs
//! to, and read back by digest.

use std::{collections::BTreeMap, fmt::Debug, path::Path, sync::Arc};

use iota_sdk_types::{CheckpointContentsDigest, CheckpointDigest, checkpoint::CheckpointContents};
use iota_types::{committee::EpochId, messages_checkpoint::TrustedCheckpoint};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables},
    traits::Map,
};

use crate::epoch_buckets::{EpochBuckets, bucket_cf_epoch};

/// Column-family prefix of the historic checkpoint buckets; a bucket's
/// family is `{prefix}{epoch}`.
const HISTORIC_CHECKPOINTS_CF_PREFIX: &str = "hist_ckpt_e";

/// Tags of the tables inside a bucket's column family. Do not reuse a tag
/// for a different table: mark it retired in a comment instead, so an older
/// bucket's rows can never be read as the wrong type.
const DB_PREFIX_HISTORIC_CHECKPOINT_CONTENT: u8 = 0;
const DB_PREFIX_HISTORIC_CHECKPOINT_BY_DIGEST: u8 = 1;

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets`] persists on a prune. It is empty until the first prune,
/// which is the same as retaining every bucket.
///
/// The name must not begin with [`HISTORIC_CHECKPOINTS_CF_PREFIX`], since
/// that is how a bucket's column family is told from every other one in this
/// database.
const EARLIEST_RETAINED_CF: &str = "hist_ckpt_retention";

/// One epoch's checkpoint history.
pub struct HistoricCheckpointsBucket {
    pub(crate) checkpoint_content: TaggedDBMap<CheckpointContentsDigest, CheckpointContents>,
    pub(crate) checkpoint_by_digest: TaggedDBMap<CheckpointDigest, TrustedCheckpoint>,
}

impl HistoricCheckpointsBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        Ok(Self {
            checkpoint_content: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_CHECKPOINT_CONTENT,
                &ReadWriteOptions::default(),
                true,
            )?,
            checkpoint_by_digest: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_CHECKPOINT_BY_DIGEST,
                &ReadWriteOptions::default(),
                true,
            )?,
        })
    }
}

/// Checkpoint history, bucketed by the epoch that closed the checkpoint.
///
/// The buckets are column families of the checkpoint store's database
/// rather than a store of their own, so a checkpoint's rows can be committed
/// alongside the rest of that commit's batch.
///
/// A bucket's existence does **not** mean its epoch has been executed. State
/// sync inserts a certified checkpoint and its contents before this node
/// executes them, in the bucket of the checkpoint's epoch, and it runs ahead
/// of execution and across epoch boundaries — so the newest bucket here can
/// belong to an epoch whose first checkpoint this node has yet to execute.
/// [`crate::authority::historic_ledger::HistoricLedger`] has the same
/// property, for the same reason.
///
/// Anything that decides how much history to keep must therefore count from
/// the epoch being executed, not from the newest bucket:
/// [`crate::epoch_buckets::EpochBuckets::prune`] derives its floor from the
/// newest bucket, so retaining N epochs that way would spend part of N on
/// epochs synced but not yet executed and drop the history of an epoch still
/// being served. [`Self::prune`] takes the executed epoch for that reason.
pub struct HistoricCheckpoints {
    buckets: EpochBuckets<HistoricCheckpointsBucket>,
}

impl HistoricCheckpoints {
    /// Options for a historic-checkpoint bucket's column family: written
    /// once, while the epoch that closed its checkpoints is current, then
    /// only ever read back by exact-key lookup.
    ///
    /// `db_options` are the checkpoint store's base options. Build this once
    /// and clone it per column family, as
    /// [`crate::authority::historic_objects::HistoricObjects::cf_options`]
    /// does: the clones share the base options' block cache instead of each
    /// allocating one of their own.
    fn cf_options(db_options: &DBOptions) -> DBOptions {
        db_options
            .clone()
            .optimize_for_write_throughput_no_deletion()
    }

    /// The `(name, options)` pairs of the column families this store needs,
    /// for the checkpoint store's open path to list alongside its own
    /// tables: a column family left for auto-discovery would otherwise be
    /// reopened with default options and a block cache of its own.
    pub fn extra_column_family_options(
        checkpoint_db_path: &Path,
        db_options: &DBOptions,
    ) -> Vec<(String, DBOptions)> {
        let cf_options = Self::cf_options(db_options);
        let mut options = vec![(EARLIEST_RETAINED_CF.to_string(), cf_options.clone())];
        if !checkpoint_db_path.join("CURRENT").exists() {
            return options;
        }
        let Ok(existing_cfs) = list_tables(checkpoint_db_path.to_path_buf()) else {
            return options;
        };
        options.extend(
            existing_cfs
                .into_iter()
                .filter(|name| bucket_cf_epoch(HISTORIC_CHECKPOINTS_CF_PREFIX, name).is_some())
                .map(|name| (name, cf_options.clone())),
        );
        options
    }

    /// Opens the historic-checkpoint buckets already present among `db`'s
    /// column families. `db` is the checkpoint store's own handle: the
    /// buckets are its column families, not a database of their own, and
    /// `db_options` are the options its tables were opened with.
    pub fn open(db: Arc<Database>, db_options: &DBOptions) -> Result<Self, TypedStoreError> {
        let existing_cfs = list_tables(db.path_for_pruning().to_path_buf())
            .map_err(|e| TypedStoreError::RocksDB(format!("failed to list buckets: {e}")))?;

        let mut buckets = BTreeMap::new();
        for cf_name in &existing_cfs {
            if let Some(epoch) = bucket_cf_epoch(HISTORIC_CHECKPOINTS_CF_PREFIX, cf_name) {
                buckets.insert(
                    epoch,
                    Arc::new(HistoricCheckpointsBucket::reopen(&db, cf_name)?),
                );
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
            "historic checkpoints",
            HISTORIC_CHECKPOINTS_CF_PREFIX,
            cf_options,
            earliest_retained_table,
            buckets,
            HistoricCheckpointsBucket::reopen,
        )?;
        Ok(Self { buckets })
    }

    /// The oldest epoch this store still holds a bucket for, `None` when it
    /// holds none at all. No checkpoint closed before this epoch is readable
    /// any more.
    ///
    /// This is what the store holds, not what its retention would keep: a node
    /// restored from a formal snapshot starts with no bucket at all, whatever
    /// the retention says.
    pub fn earliest_bucket_epoch(&self) -> Option<EpochId> {
        self.buckets.earliest_epoch()
    }

    /// The bucket holding `epoch`'s checkpoint history, created if absent.
    pub fn ensure(
        &self,
        epoch: EpochId,
    ) -> Result<Arc<HistoricCheckpointsBucket>, TypedStoreError> {
        self.buckets.ensure(epoch)
    }

    /// The bucket holding `epoch`'s checkpoint history, and `None` once that
    /// epoch has been expired. See
    /// [`crate::epoch_buckets::EpochBuckets::ensure_retained`].
    pub fn ensure_retained(
        &self,
        epoch: EpochId,
    ) -> Result<Option<Arc<HistoricCheckpointsBucket>>, TypedStoreError> {
        self.buckets.ensure_retained(epoch)
    }

    /// Drops the buckets of the epochs that have fallen outside
    /// `epochs_to_retain`, counted back from `executed_epoch` and including
    /// it, and returns the earliest epoch retained.
    ///
    /// `executed_epoch` must be an epoch this node has executed rather than
    /// the newest bucket, for the reason given on [`HistoricCheckpoints`];
    /// the buckets above it are left alone. Blocks for as long as the drops
    /// take, so a caller on an async runtime must use `spawn_blocking`.
    pub fn prune(
        &self,
        current_epoch: EpochId,
        epochs_to_retain: u64,
    ) -> Result<Option<EpochId>, TypedStoreError> {
        // Nothing here lives in a live table, so a drop has no side effect to
        // prepare.
        self.buckets
            .prune(current_epoch, epochs_to_retain, |_, _| Ok(()))
    }

    /// The contents stored under `digest`, newest bucket first, `None` if no
    /// bucket holds them.
    ///
    /// A digest no bucket holds belongs to a checkpoint this node never had,
    /// or to one whose epoch has been dropped, and both answer `None`.
    pub fn find_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<CheckpointContents>, TypedStoreError> {
        for bucket in self.buckets.iter(true) {
            if let Some(contents) = bucket.checkpoint_content.get(digest)? {
                return Ok(Some(contents));
            }
        }
        Ok(None)
    }

    /// The certified summary stored under `digest`, newest bucket first,
    /// `None` if no bucket holds it.
    ///
    /// Keyed by checkpoint digest. A caller that has the sequence number
    /// reads `certified_checkpoints` instead, which holds the same summaries
    /// and is never pruned.
    pub fn find_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<TrustedCheckpoint>, TypedStoreError> {
        for bucket in self.buckets.iter(true) {
            if let Some(checkpoint) = bucket.checkpoint_by_digest.get(digest)? {
                return Ok(Some(checkpoint));
            }
        }
        Ok(None)
    }

    /// One page of the rows `cf_name` holds, if it is one of this store's
    /// column families: a bucket of checkpoint history, or the
    /// retention-floor family. `None` for any other name, leaving the caller
    /// to report it as unknown.
    ///
    /// For the table dump of `iota-tool`, which walks the checkpoint store's
    /// column families by name: these are not fields of
    /// `CheckpointStoreTables`, so the dump derived from it cannot read
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

        if bucket_cf_epoch(HISTORIC_CHECKPOINTS_CF_PREFIX, cf_name).is_some() {
            let bucket = HistoricCheckpointsBucket::reopen(db, cf_name)?;
            bucket.checkpoint_content.try_catch_up_with_primary()?;
            bucket.checkpoint_by_digest.try_catch_up_with_primary()?;
            let rows = format_rows("content:", bucket.checkpoint_content.safe_iter()).chain(
                format_rows("by_digest:", bucket.checkpoint_by_digest.safe_iter()),
            );
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
#[path = "../unit_tests/historic_checkpoints_tests.rs"]
mod tests;
