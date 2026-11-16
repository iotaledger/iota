// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Received-and-deleted object markers, bucketed by the epoch that wrote them.
//!
//! A marker guards a race inside the epoch that wrote it, so only the current
//! epoch's markers are kept, and an earlier epoch's are removed with a
//! `drop_cf` rather than a range delete the execution path would read across.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use iota_sdk_types::ObjectId;
use iota_types::{
    base_types::VersionNumber,
    committee::EpochId,
    error::{IotaError, IotaResult},
    storage::{MarkerValue, ObjectKey},
};
use typed_store::{
    TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, list_tables},
    traits::Map,
};

use crate::epoch_buckets::{
    BucketReopen, EpochBuckets, PrunedBuckets, absent_if_dropped, bucket_cf_epoch,
    bucket_metrics_label, dump_page, dump_retention_floor, dump_rows,
};

/// Column-family prefix of the marker buckets; a bucket's family is
/// `{prefix}{epoch}`.
const MARKERS_CF_PREFIX: &str = "marker_e";

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets::prune`] persists.
const EARLIEST_RETAINED_CF: &str = "marker_earliest_retained";

/// Historic epochs kept at a reconfiguration, on top of the epoch being
/// entered: none, as nothing reads a marker outside the running epoch.
const HISTORIC_EPOCHS_TO_RETAIN: u64 = 0;

/// One epoch's markers.
pub struct EpochMarkersBucket {
    /// The objects received, deleted or wrapped during this epoch, at the
    /// version it happened at.
    pub(crate) markers: DBMap<ObjectKey, MarkerValue>,
}

impl BucketReopen for EpochMarkersBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        Ok(Self {
            markers: DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)?
                .with_metrics_label(bucket_metrics_label(cf_name)),
        })
    }
}

/// The markers of the epochs still retained, one column family each.
pub struct EpochMarkers {
    buckets: EpochBuckets<EpochMarkersBucket>,
}

impl EpochMarkers {
    /// Options for a marker bucket's column family: the base options, without
    /// the write-heavy tuning of the historic buckets, as markers are read on
    /// the execution path. Clone the result per column family so they share
    /// one block cache.
    fn cf_options(db_options: &DBOptions) -> DBOptions {
        db_options.clone()
    }

    /// The `(name, options)` pairs of this store's column families, for the
    /// perpetual store to open alongside its own tables; one left out would be
    /// reopened with default options.
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
    /// `db` is the perpetual database's handle and `db_options` the options its
    /// tables were opened with.
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
                // Markers are working data of the epochs being executed rather
                // than history, so they stay in the database directory.
                None,
                earliest_retained_table,
                buckets,
            )?,
        })
    }

    /// The bucket `epoch`'s markers are written to, created if absent.
    pub(crate) fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<EpochMarkersBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The marker written for `object_id` at exactly `version` during `epoch`,
    /// `None` also once that epoch's bucket has been dropped.
    pub fn get_marker_value(
        &self,
        object_id: &ObjectId,
        version: &VersionNumber,
        epoch: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        let Some(bucket) = self.buckets.get(epoch) else {
            return Ok(None);
        };
        Ok(absent_if_dropped(
            bucket.markers.get(&ObjectKey(*object_id, *version)),
        )?)
    }

    /// The newest version of `object_id` marked during `epoch`, with its
    /// marker, `None` also once that epoch's bucket has been dropped.
    pub fn get_latest_marker(
        &self,
        object_id: &ObjectId,
        epoch: EpochId,
    ) -> IotaResult<Option<(VersionNumber, MarkerValue)>> {
        let Some(bucket) = self.buckets.get(epoch) else {
            return Ok(None);
        };
        let Some(row) = absent_if_dropped(
            bucket
                .markers
                .safe_iter_with_prefix_reversed(object_id)
                .next()
                .transpose(),
        )?
        else {
            return Ok(None);
        };
        let (key, marker) = row;
        // The iterator bounds cannot yield another object id.
        debug_assert_eq!(key.0, *object_id);
        Ok(Some((key.1, marker)))
    }

    /// The rows of one of this store's column families, for `iota-tool dump`,
    /// which cannot reach them through the perpetual table struct. `None` when
    /// `cf_name` is not one of them.
    pub fn dump_column_family(
        db: &Arc<Database>,
        cf_name: &str,
        page_size: u16,
        page_number: usize,
    ) -> Result<Option<BTreeMap<String, String>>, TypedStoreError> {
        if bucket_cf_epoch(MARKERS_CF_PREFIX, cf_name).is_some() {
            let bucket = EpochMarkersBucket::reopen(db, cf_name)?;
            bucket.markers.try_catch_up_with_primary()?;
            return dump_page(
                dump_rows("", bucket.markers.safe_iter()),
                page_size,
                page_number,
            )
            .map(Some);
        }
        if cf_name == EARLIEST_RETAINED_CF {
            return dump_retention_floor(db, cf_name, page_size, page_number).map(Some);
        }
        Ok(None)
    }

    /// Drops every bucket below the epoch being entered, after making sure
    /// that epoch has one. Returns the earliest epoch still retained and how
    /// many buckets were dropped.
    ///
    /// Call only while execution is halted, as at reconfiguration: the drops
    /// take a write lock the execution path's reads contend on.
    pub fn expire(&self, new_epoch: EpochId) -> IotaResult<Option<PrunedBuckets>> {
        self.ensure(new_epoch)?;
        self.buckets
            .prune(new_epoch, HISTORIC_EPOCHS_TO_RETAIN, |_, _| Ok(()))
            .map_err(|e| IotaError::Storage(e.to_string()))
    }
}

#[cfg(test)]
#[path = "../unit_tests/epoch_markers_tests.rs"]
mod tests;
