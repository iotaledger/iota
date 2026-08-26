// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Received-and-deleted object markers, bucketed by the epoch that wrote them.
//!
//! A marker guards a race inside the epoch that wrote it, so only the epoch
//! the node is in needs its markers and every earlier epoch's can go. A column
//! family per epoch is what makes that a `drop_cf`: the flat table this
//! replaces was cleared with a range tombstone that the execution path then
//! read across until compaction caught up.

use std::{collections::BTreeMap, fmt::Debug, path::Path, sync::Arc};

use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::VersionNumber,
    committee::EpochId,
    error::{IotaError, IotaResult},
    storage::{MarkerValue, ObjectKey},
};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, list_tables},
    traits::Map,
};

use crate::{
    epoch_buckets::{EpochBuckets, bucket_cf_epoch},
    progress_logger::ProgressLogger,
};

/// Column-family prefix of the marker buckets; a bucket's family is
/// `{prefix}{epoch}`.
const MARKERS_CF_PREFIX: &str = "marker_e";

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets::prune`] persists.
const EARLIEST_RETAINED_CF: &str = "marker_earliest_retained";

/// Rows one slice of the migration moves before it writes its batch.
const KEYS_PER_SLICE: usize = 5_000;

/// Historic epochs kept at a reconfiguration, on top of the epoch being
/// entered: none. Not configurable — a marker outside the running epoch
/// answers no question anyone asks.
const HISTORIC_EPOCHS_TO_RETAIN: u64 = 0;

/// One epoch's markers.
pub struct EpochMarkersBucket {
    /// The objects received, deleted or wrapped during this epoch, at the
    /// version it happened at. The bucket is this table's own column family,
    /// so unlike the historic stores' buckets it needs no tag byte to tell
    /// its rows from a neighbouring table's.
    pub(crate) markers: DBMap<ObjectKey, MarkerValue>,
}

impl EpochMarkersBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        Ok(Self {
            markers: DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)?,
        })
    }
}

/// The markers of the epochs still retained, one column family each.
pub struct EpochMarkers {
    buckets: EpochBuckets<EpochMarkersBucket>,
}

impl EpochMarkers {
    /// Options for a marker bucket's column family.
    ///
    /// The base options unchanged, which is what the flat table this replaces
    /// used: markers are read on the execution path, by exact key and by
    /// reverse scan over one object id, so none of the write-heavy tuning the
    /// historic buckets take applies. Built once and cloned per column family,
    /// so the clones share the base options' block cache instead of each
    /// allocating one.
    fn cf_options(db_options: &DBOptions) -> DBOptions {
        db_options.clone()
    }

    /// The `(name, options)` pairs of the column families this store needs,
    /// for the perpetual store's open path to list alongside its own tables: a
    /// column family left for auto-discovery would be reopened with default
    /// options and a block cache of its own.
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
                .filter(|name| bucket_cf_epoch(MARKERS_CF_PREFIX, name).is_some())
                .map(|name| (name, cf_options.clone())),
        );
        options
    }

    /// Opens the marker buckets already present among `db`'s column families.
    /// `db` is the perpetual database's own handle — the buckets are its
    /// column families, not a database of their own — and `db_options` are the
    /// options its tables were opened with.
    pub fn open(db: Arc<Database>, db_options: &DBOptions) -> Result<Self, TypedStoreError> {
        let existing_cfs = list_tables(db.path_for_pruning().to_path_buf())
            .map_err(|e| TypedStoreError::RocksDB(format!("failed to list marker buckets: {e}")))?;

        let mut buckets = BTreeMap::new();
        for cf_name in &existing_cfs {
            if let Some(epoch) = bucket_cf_epoch(MARKERS_CF_PREFIX, cf_name) {
                buckets.insert(epoch, Arc::new(EpochMarkersBucket::reopen(&db, cf_name)?));
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

        Ok(Self {
            buckets: EpochBuckets::open(
                db,
                "epoch markers",
                MARKERS_CF_PREFIX,
                cf_options,
                earliest_retained_table,
                buckets,
                EpochMarkersBucket::reopen,
            )?,
        })
    }

    /// The bucket `epoch`'s markers are written to, created if absent.
    pub(crate) fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<EpochMarkersBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The marker written for `object_id` at exactly `version` during `epoch`.
    pub fn get_marker_value(
        &self,
        object_id: &ObjectId,
        version: &VersionNumber,
        epoch: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        let Some(bucket) = self.buckets.get(epoch) else {
            return Ok(None);
        };
        Ok(bucket.markers.get(&ObjectKey(*object_id, *version))?)
    }

    /// The newest version of `object_id` marked during `epoch`, with its
    /// marker.
    ///
    /// The bucket is the epoch, so unlike the flat table this replaces there
    /// is no epoch left in the key for a reader to check against the one it
    /// asked for.
    pub fn get_latest_marker(
        &self,
        object_id: &ObjectId,
        epoch: EpochId,
    ) -> IotaResult<Option<(VersionNumber, MarkerValue)>> {
        let Some(bucket) = self.buckets.get(epoch) else {
            return Ok(None);
        };
        let Some(row) = bucket
            .markers
            .safe_iter_with_prefix_reversed(object_id)
            .next()
            .transpose()?
        else {
            return Ok(None);
        };
        let (key, marker) = row;
        // The iterator bounds cannot yield another object id.
        debug_assert_eq!(key.0, *object_id);
        Ok(Some((key.1, marker)))
    }

    /// The rows of one of this store's column families, for `iota-tool dump`:
    /// the buckets and their retention floor are column families of the
    /// perpetual database that its table struct does not declare, so the dump
    /// derived from that struct cannot reach them. `None` when `cf_name` is
    /// not one of them.
    pub fn dump_column_family(
        db: &Arc<Database>,
        cf_name: &str,
        page_size: u16,
        page_number: usize,
    ) -> Result<Option<BTreeMap<String, String>>, TypedStoreError> {
        fn page<K: Debug, V: Debug>(
            rows: DbIterator<'_, (K, V)>,
            page_size: u16,
            page_number: usize,
        ) -> Result<BTreeMap<String, String>, TypedStoreError> {
            rows.skip(page_number * page_size as usize)
                .take(page_size as usize)
                .map(|row| row.map(|(key, value)| (format!("{key:?}"), format!("{value:?}"))))
                .collect()
        }

        if bucket_cf_epoch(MARKERS_CF_PREFIX, cf_name).is_some() {
            let bucket = EpochMarkersBucket::reopen(db, cf_name)?;
            bucket.markers.try_catch_up_with_primary()?;
            return page(bucket.markers.safe_iter(), page_size, page_number).map(Some);
        }
        if cf_name == EARLIEST_RETAINED_CF {
            let earliest_retained: DBMap<(), EpochId> =
                DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)?;
            earliest_retained.try_catch_up_with_primary()?;
            return page(earliest_retained.safe_iter(), page_size, page_number).map(Some);
        }
        Ok(None)
    }

    /// Moves the markers left in the flat `object_per_epoch_marker_table` into
    /// the bucket of the epoch that wrote them, dropping the ones an earlier
    /// epoch wrote.
    ///
    /// Call this before starting any service: until it returns, a marker
    /// written before the upgrade is unreachable, and a marker missed is a
    /// receive or a delete this node would let happen twice.
    ///
    /// No watermark, unlike the ledger migration: every slice deletes exactly
    /// the keys it read, so an interrupted run leaves the rest of the table
    /// where the next one finds it, and moving the same row twice writes the
    /// same value to the same key.
    ///
    /// The flat table is cleared at every reconfiguration, so what it holds at
    /// upgrade is one epoch of markers at most. Rows below `epoch` are deleted
    /// rather than moved: the old code had already stopped reading them, so
    /// giving them a column family would create one only for the next
    /// reconfiguration to drop.
    // TODO(https://github.com/iotaledger/iota/issues/12712): remove this once
    // every database has moved its markers into the buckets.
    pub fn migrate_flat_markers(
        &self,
        flat: &DBMap<(EpochId, ObjectKey), MarkerValue>,
        epoch: EpochId,
    ) -> IotaResult<()> {
        let mut progress =
            ProgressLogger::new("epoch marker migration", "markers", flat.estimated_len()?);
        loop {
            let mut moved = Vec::new();
            let mut keys = Vec::new();
            for row in flat.safe_iter().take(KEYS_PER_SLICE) {
                let ((row_epoch, key), marker) = row?;
                if row_epoch == epoch {
                    moved.push((key, marker));
                }
                keys.push((row_epoch, key));
            }
            if keys.is_empty() {
                progress.finish();
                return Ok(());
            }
            let mut batch = flat.batch();
            if !moved.is_empty() {
                let bucket = self.ensure(epoch)?;
                batch.insert_batch(&bucket.markers, moved)?;
            }
            let read = keys.len();
            batch.delete_batch(flat, keys)?;
            batch.write()?;
            progress.advance(read as u64);
        }
    }

    /// Drops every bucket below the epoch being entered, after making sure
    /// that epoch has one. Returns the earliest epoch still retained.
    ///
    /// Called from reconfiguration, where execution is halted, so the write
    /// lock the drops take is not contended with the reads on the execution
    /// path.
    pub fn expire(&self, new_epoch: EpochId) -> IotaResult<Option<EpochId>> {
        self.ensure(new_epoch)?;
        self.buckets
            .prune(new_epoch, HISTORIC_EPOCHS_TO_RETAIN, |_, _| Ok(()))
            .map_err(|e| IotaError::Storage(e.to_string()))
    }
}

#[cfg(test)]
#[path = "../unit_tests/epoch_markers_tests.rs"]
mod tests;
