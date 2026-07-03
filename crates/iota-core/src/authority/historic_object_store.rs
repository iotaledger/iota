// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch storage for superseded object versions.
//!
//! When the live-object/historic split is enabled, the pruner relocates
//! superseded object versions into this store instead of deleting them. Rows
//! are bucketed by their *supersession epoch* (the epoch of the checkpoint
//! whose effects superseded them), one pair of column families per epoch, so
//! that expiring an epoch of history is a constant-time `drop_cf` instead of
//! per-key deletes.
//!
//! The store is strictly outside the consensus/execution read paths: the only
//! reader is the gRPC exact-version object lookup. Lookups carry no epoch
//! hint, so they probe the per-epoch column families newest to oldest; a miss
//! in a sealed, compacted column family is answered from the in-memory RocksDB
//! bloom filters without touching disk.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use iota_types::{
    base_types::EpochId,
    error::{IotaError, IotaResult},
    object::Object,
    storage::ObjectKey,
};
use prometheus_filtered::{
    Histogram, IntCounter, IntGauge, Registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
use serde::{Deserialize, Serialize};
use typed_store::{
    Map,
    database::Database,
    metrics::SamplingInterval,
    rocks::{
        DBMap, DBOptions, MetricConf, ReadWriteOptions, default_db_options, list_tables,
        read_size_from_env,
    },
    rocksdb,
};

use crate::authority::authority_store_types::{StoreObject, StoreObjectWrapper};

const HISTORY_DIR_NAME: &str = "history";
const META_CF_NAME: &str = "meta";
const OBJECTS_CF_PREFIX: &str = "hist_obj_e";
const EXPIRY_CF_PREFIX: &str = "hist_exp_e";

const ENV_VAR_HISTORY_BLOCK_CACHE_SIZE: &str = "HISTORY_BLOCK_CACHE_MB";
const DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB: usize = 512;

/// Durable per-epoch bookkeeping, kept in the always-open `meta` column
/// family.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EpochBucketInfo {
    /// Set once the pruner has moved past this epoch; a sealed bucket never
    /// receives writes again.
    pub sealed: bool,
    /// Number of relocated object versions in the bucket.
    pub object_count: u64,
    /// Number of tombstone-head expiry entries in the bucket.
    pub expiry_count: u64,
}

struct EpochBucket {
    /// Superseded object versions relocated out of the live `objects` table.
    objects: DBMap<ObjectKey, StoreObjectWrapper>,
    /// Tombstone heads (`Deleted`/`Wrapped`) whose lineages were superseded in
    /// this epoch. They stay in the live table until this bucket expires, at
    /// which point they are point-deleted from the live table right before
    /// the bucket is dropped.
    expiry: DBMap<ObjectKey, ()>,
}

pub struct HistoricObjectStoreMetrics {
    pub relocated_objects: IntCounter,
    pub relocated_bytes: IntCounter,
    pub lookup_probes: Histogram,
    pub lookup_not_found: IntCounter,
    pub epochs_retained: IntGauge,
    pub earliest_retained_epoch: IntGauge,
}

impl HistoricObjectStoreMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            relocated_objects: register_int_counter_with_registry!(
                "historic_object_store_relocated_objects",
                "Number of superseded object versions relocated into the historic store",
                registry
            )
            .unwrap(),
            relocated_bytes: register_int_counter_with_registry!(
                "historic_object_store_relocated_bytes",
                "Serialized bytes relocated into the historic store",
                registry
            )
            .unwrap(),
            lookup_probes: register_histogram_with_registry!(
                "historic_object_store_lookup_probes",
                "Number of epoch buckets probed per historic lookup",
                vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0],
                registry
            )
            .unwrap(),
            lookup_not_found: register_int_counter_with_registry!(
                "historic_object_store_lookup_not_found",
                "Historic lookups that missed every epoch bucket",
                registry
            )
            .unwrap(),
            epochs_retained: register_int_gauge_with_registry!(
                "historic_object_store_epochs_retained",
                "Number of epoch buckets currently retained",
                registry
            )
            .unwrap(),
            earliest_retained_epoch: register_int_gauge_with_registry!(
                "historic_object_store_earliest_retained_epoch",
                "Earliest epoch with a retained bucket",
                registry
            )
            .unwrap(),
        })
    }

    pub fn new_for_test() -> Arc<Self> {
        Self::new(&Registry::new())
    }
}

/// Store of superseded object versions, bucketed by supersession epoch.
///
/// Writes come exclusively from the single pruner task; reads may come from
/// any number of RPC threads concurrently.
pub struct HistoricObjectStore {
    db: Arc<Database>,
    /// Template options for per-epoch column families. All clones share one
    /// block cache through the cloned table factory.
    cf_options: rocksdb::Options,
    meta: DBMap<EpochId, EpochBucketInfo>,
    buckets: RwLock<BTreeMap<EpochId, EpochBucket>>,
    disable_wal: bool,
    metrics: Arc<HistoricObjectStoreMetrics>,
}

impl HistoricObjectStore {
    pub fn path(parent_path: &Path) -> PathBuf {
        parent_path.join(HISTORY_DIR_NAME)
    }

    /// Opens (or creates) the store under `<parent_path>/history`,
    /// rediscovering all per-epoch column families present on disk.
    ///
    /// Relocation batches are written without the WAL when `disable_wal` is
    /// set: relocation is idempotent and re-runnable from the pruner
    /// watermark, and the pruner flushes the bucket before deleting the
    /// source rows, so durability is preserved.
    pub fn open(
        parent_path: &Path,
        disable_wal: bool,
        metrics: Arc<HistoricObjectStoreMetrics>,
    ) -> IotaResult<Self> {
        let path = Self::path(parent_path);
        let db_options = default_db_options().disable_write_throttling();
        let cf_options = Self::epoch_cf_options(&db_options);
        let meta_options = db_options.clone().optimize_for_point_lookup(8);

        // Column families must be passed at open with their tuned options;
        // any column family left for auto-discovery would silently get
        // default options (and its own block cache).
        let existing_cfs = list_tables(path.clone()).unwrap_or_default();
        let mut opt_cfs: Vec<(&str, rocksdb::Options)> = vec![(META_CF_NAME, meta_options.options)];
        for cf_name in &existing_cfs {
            if cf_name != META_CF_NAME {
                opt_cfs.push((cf_name, cf_options.clone()));
            }
        }

        let db = typed_store::rocks::open_cf_opts(
            &path,
            Some(db_options.options),
            MetricConf::new("history")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            &opt_cfs,
        )?;

        let meta = DBMap::reopen(&db, Some(META_CF_NAME), &ReadWriteOptions::default(), false)?;

        // Column family names on disk are the ground truth for which buckets
        // exist; `meta` may lag by one crash (bucket created, meta row not yet
        // written) and is backfilled lazily on the next write. A bucket's two
        // column families are created and dropped in separate operations, so
        // a crash can leave one of the pair missing: recreate it (empty)
        // here. A bucket half-dropped this way simply resurfaces and is
        // dropped again by the next retention pass.
        let mut epochs = std::collections::BTreeSet::new();
        for cf_name in &existing_cfs {
            let epoch_str = match (
                cf_name.strip_prefix(OBJECTS_CF_PREFIX),
                cf_name.strip_prefix(EXPIRY_CF_PREFIX),
            ) {
                (Some(epoch_str), _) | (_, Some(epoch_str)) => epoch_str,
                (None, None) => continue,
            };
            let epoch: EpochId = epoch_str.parse().map_err(|_| {
                IotaError::Storage(format!("unparsable historic column family name: {cf_name}"))
            })?;
            epochs.insert(epoch);
        }
        let mut buckets = BTreeMap::new();
        for epoch in epochs {
            for cf_name in [Self::objects_cf_name(epoch), Self::expiry_cf_name(epoch)] {
                if db.cf_handle(&cf_name).is_none() {
                    db.create_cf(&cf_name, &cf_options)
                        .map_err(|e| IotaError::Storage(e.to_string()))?;
                }
            }
            buckets.insert(epoch, Self::reopen_bucket(&db, epoch)?);
        }

        let store = Self {
            db,
            cf_options,
            meta,
            buckets: RwLock::new(buckets),
            disable_wal,
            metrics,
        };
        store.update_retention_metrics();
        Ok(store)
    }

    fn epoch_cf_options(db_options: &DBOptions) -> rocksdb::Options {
        // Relocation writes are append-only per bucket (universal compaction,
        // no deletions), values are large (blob files), and reads are exact
        // key lookups answered by the block-based bloom filters, which the
        // block options pin in RAM. `set_block_options` creates the single
        // block cache that every clone of these options shares.
        db_options
            .clone()
            .optimize_for_write_throughput_no_deletion()
            .optimize_for_large_values_no_scan(1 << 10)
            .set_block_options(
                read_size_from_env(ENV_VAR_HISTORY_BLOCK_CACHE_SIZE)
                    .unwrap_or(DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB),
                16 << 10,
            )
            .options
    }

    fn objects_cf_name(epoch: EpochId) -> String {
        format!("{OBJECTS_CF_PREFIX}{epoch}")
    }

    fn expiry_cf_name(epoch: EpochId) -> String {
        format!("{EXPIRY_CF_PREFIX}{epoch}")
    }

    fn reopen_bucket(db: &Arc<Database>, epoch: EpochId) -> IotaResult<EpochBucket> {
        // Per-epoch column families skip the periodic metrics reporter task:
        // with ~100 retained epochs the per-table metrics add little insight
        // and one task per column family adds up.
        let objects = DBMap::reopen(
            db,
            Some(&Self::objects_cf_name(epoch)),
            &ReadWriteOptions::default(),
            true,
        )?;
        let expiry = DBMap::reopen(
            db,
            Some(&Self::expiry_cf_name(epoch)),
            &ReadWriteOptions::default(),
            true,
        )?;
        Ok(EpochBucket { objects, expiry })
    }

    fn update_retention_metrics(&self) {
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        self.metrics.epochs_retained.set(buckets.len() as i64);
        if let Some((&earliest, _)) = buckets.first_key_value() {
            self.metrics.earliest_retained_epoch.set(earliest as i64);
        }
    }

    /// Durably persists relocated rows and the tombstone-head expiry list
    /// into the bucket for `supersession_epoch`, creating the bucket on first
    /// use. Idempotent: rewriting the same keys with the same bytes is
    /// harmless.
    ///
    /// Durability of the write is only guaranteed after a subsequent
    /// [`Self::flush_epoch`]; callers must flush before deleting the source
    /// rows from the live table.
    pub fn put_objects(
        &self,
        supersession_epoch: EpochId,
        objects: &[(ObjectKey, StoreObjectWrapper)],
        tombstone_heads: &[ObjectKey],
    ) -> IotaResult<()> {
        if objects.is_empty() && tombstone_heads.is_empty() {
            return Ok(());
        }
        self.ensure_bucket(supersession_epoch)?;
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let bucket = buckets
            .get(&supersession_epoch)
            .expect("bucket was just created");

        let mut batch = bucket.objects.batch();
        batch.insert_batch(&bucket.objects, objects.iter().map(|(k, v)| (k, v)))?;
        batch.insert_batch(&bucket.expiry, tombstone_heads.iter().map(|k| (k, ())))?;

        let mut info = self.meta.get(&supersession_epoch)?.unwrap_or_default();
        info.object_count += objects.len() as u64;
        info.expiry_count += tombstone_heads.len() as u64;
        batch.insert_batch(&self.meta, [(supersession_epoch, info)])?;

        let mut write_options = rocksdb::WriteOptions::default();
        write_options.disable_wal(self.disable_wal);
        batch.write_opt(&write_options)?;

        self.metrics.relocated_objects.inc_by(objects.len() as u64);
        let relocated_bytes: u64 = objects
            .iter()
            .map(|(_, value)| bcs::serialized_size(value).unwrap_or_default() as u64)
            .sum();
        self.metrics.relocated_bytes.inc_by(relocated_bytes);
        Ok(())
    }

    /// Flushes the bucket's memtables to disk. This is the durability barrier
    /// for WAL-less relocation writes: it must complete before the relocated
    /// rows are deleted from the live table.
    pub fn flush_epoch(&self, epoch: EpochId) -> IotaResult<()> {
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let Some(bucket) = buckets.get(&epoch) else {
            return Ok(());
        };
        bucket.objects.flush()?;
        bucket.expiry.flush()?;
        Ok(())
    }

    /// Seals a bucket once the pruner has moved past its epoch: flushes it,
    /// compacts it into its final sorted run, and records the seal. Sealing
    /// is idempotent; a sealed bucket never receives writes again.
    pub fn seal_epoch(&self, epoch: EpochId) -> IotaResult<()> {
        {
            let buckets = self.buckets.read().expect("lock should not be poisoned");
            let Some(bucket) = buckets.get(&epoch) else {
                return Ok(());
            };
            bucket.objects.flush()?;
            bucket.expiry.flush()?;
            // Full-range manual compaction: object keys are 40-byte
            // fix-int-serialized (ObjectId, version) tuples, so these raw
            // bounds cover every possible key.
            let full_range_end = vec![0xffu8; 48];
            bucket.objects.compact_range_raw(
                &Self::objects_cf_name(epoch),
                vec![],
                full_range_end.clone(),
            )?;
            bucket.expiry.compact_range_raw(
                &Self::expiry_cf_name(epoch),
                vec![],
                full_range_end,
            )?;
        }
        let mut info = self.meta.get(&epoch)?.unwrap_or_default();
        if !info.sealed {
            info.sealed = true;
            self.meta.insert(&epoch, &info)?;
        }
        Ok(())
    }

    /// Exact-key lookup with no epoch hint: probes buckets newest to oldest.
    pub fn get_store_object(&self, key: &ObjectKey) -> IotaResult<Option<StoreObjectWrapper>> {
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let mut probes = 0u64;
        for bucket in buckets.values().rev() {
            probes += 1;
            if let Some(wrapper) = bucket.objects.get(key)? {
                self.metrics.lookup_probes.observe(probes as f64);
                return Ok(Some(wrapper));
            }
        }
        self.metrics.lookup_probes.observe(probes.max(1) as f64);
        self.metrics.lookup_not_found.inc();
        Ok(None)
    }

    /// Like [`Self::get_store_object`], constructing the full object.
    /// Returns `None` for tombstone rows, mirroring the live table's read
    /// semantics.
    pub fn get_object(&self, key: &ObjectKey) -> IotaResult<Option<Object>> {
        let Some(wrapper) = self.get_store_object(key)? else {
            return Ok(None);
        };
        let StoreObject::Value(store_object) = wrapper.migrate().into_inner() else {
            return Ok(None);
        };
        Ok(Some(
            crate::authority::authority_store_types::try_construct_object(key, *store_object)?,
        ))
    }

    /// The tombstone heads recorded for `epoch`. These are the live-table
    /// keys that must be point-deleted when the bucket expires.
    pub fn tombstone_heads(&self, epoch: EpochId) -> IotaResult<Vec<ObjectKey>> {
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let Some(bucket) = buckets.get(&epoch) else {
            return Ok(Vec::new());
        };
        bucket
            .expiry
            .safe_iter()
            .map(|entry| entry.map(|(key, ())| key).map_err(IotaError::from))
            .collect()
    }

    /// Drops the bucket for `epoch` wholesale. Idempotent. Callers must have
    /// already deleted the bucket's tombstone heads from the live table.
    pub fn drop_epoch(&self, epoch: EpochId) -> IotaResult<()> {
        let removed = {
            let mut buckets = self.buckets.write().expect("lock should not be poisoned");
            buckets.remove(&epoch)
        };
        if removed.is_some() {
            self.db
                .drop_cf(&Self::objects_cf_name(epoch))
                .map_err(|e| IotaError::Storage(e.to_string()))?;
            self.db
                .drop_cf(&Self::expiry_cf_name(epoch))
                .map_err(|e| IotaError::Storage(e.to_string()))?;
        }
        self.meta.remove(&epoch)?;
        self.update_retention_metrics();
        Ok(())
    }

    /// The earliest epoch with a retained bucket, i.e. the historic
    /// availability horizon.
    pub fn earliest_epoch(&self) -> Option<EpochId> {
        self.buckets
            .read()
            .expect("lock should not be poisoned")
            .first_key_value()
            .map(|(&epoch, _)| epoch)
    }

    /// All epochs with retained buckets, ascending.
    pub fn list_epochs(&self) -> Vec<EpochId> {
        self.buckets
            .read()
            .expect("lock should not be poisoned")
            .keys()
            .copied()
            .collect()
    }

    /// Whether the bucket for `epoch` has been sealed.
    pub fn is_sealed(&self, epoch: EpochId) -> IotaResult<bool> {
        Ok(self.meta.get(&epoch)?.is_some_and(|info| info.sealed))
    }

    fn ensure_bucket(&self, epoch: EpochId) -> IotaResult<()> {
        {
            let buckets = self.buckets.read().expect("lock should not be poisoned");
            if buckets.contains_key(&epoch) {
                return Ok(());
            }
        }
        let mut buckets = self.buckets.write().expect("lock should not be poisoned");
        if buckets.contains_key(&epoch) {
            return Ok(());
        }
        for cf_name in [Self::objects_cf_name(epoch), Self::expiry_cf_name(epoch)] {
            // The column family may already exist if a previous run crashed
            // between `create_cf` and the first batch write.
            if self.db.cf_handle(&cf_name).is_none() {
                self.db
                    .create_cf(&cf_name, &self.cf_options)
                    .map_err(|e| IotaError::Storage(e.to_string()))?;
            }
        }
        buckets.insert(epoch, Self::reopen_bucket(&self.db, epoch)?);
        drop(buckets);
        self.update_retention_metrics();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::ObjectId;
    use iota_types::base_types::SequenceNumber;

    use super::*;
    use crate::authority::authority_store_types::get_store_object;

    fn open_store(path: &Path) -> HistoricObjectStore {
        HistoricObjectStore::open(path, true, HistoricObjectStoreMetrics::new_for_test()).unwrap()
    }

    fn test_row(version: u64) -> (ObjectKey, StoreObjectWrapper) {
        let object = Object::immutable_with_id_for_testing(ObjectId::random());
        let key = ObjectKey(object.id(), SequenceNumber::from_u64(version));
        (key, get_store_object(object, None))
    }

    #[tokio::test]
    async fn put_get_roundtrip_probes_newest_first() {
        let tmp_dir = iota_common::tempdir();
        let store = open_store(tmp_dir.path());

        let (key_e1, row_e1) = test_row(1);
        let (key_e5, row_e5) = test_row(3);
        store.put_objects(1, &[(key_e1, row_e1)], &[]).unwrap();
        store.put_objects(5, &[(key_e5, row_e5)], &[]).unwrap();

        assert!(store.get_object(&key_e1).unwrap().is_some());
        assert!(store.get_object(&key_e5).unwrap().is_some());
        let (missing_key, _) = test_row(9);
        assert!(store.get_object(&missing_key).unwrap().is_none());
        assert_eq!(store.list_epochs(), vec![1, 5]);
        assert_eq!(store.earliest_epoch(), Some(1));
    }

    #[tokio::test]
    async fn restart_rediscovers_buckets_and_seal_state() {
        let tmp_dir = iota_common::tempdir();
        let (key, row) = test_row(2);
        {
            let store = open_store(tmp_dir.path());
            store
                .put_objects(
                    7,
                    &[(key, row)],
                    &[ObjectKey(key.0, SequenceNumber::from_u64(3))],
                )
                .unwrap();
            store.flush_epoch(7).unwrap();
            store.seal_epoch(7).unwrap();
            assert!(store.is_sealed(7).unwrap());
        }
        let store = open_store(tmp_dir.path());
        assert_eq!(store.list_epochs(), vec![7]);
        assert!(store.is_sealed(7).unwrap());
        assert!(store.get_object(&key).unwrap().is_some());
        assert_eq!(
            store.tombstone_heads(7).unwrap(),
            vec![ObjectKey(key.0, SequenceNumber::from_u64(3))]
        );
    }

    #[tokio::test]
    async fn seal_epoch_is_idempotent() {
        let tmp_dir = iota_common::tempdir();
        let store = open_store(tmp_dir.path());
        let (key, row) = test_row(1);
        store.put_objects(3, &[(key, row)], &[]).unwrap();
        store.seal_epoch(3).unwrap();
        store.seal_epoch(3).unwrap();
        assert!(store.is_sealed(3).unwrap());
        assert!(store.get_object(&key).unwrap().is_some());
    }

    #[tokio::test]
    async fn drop_epoch_removes_bucket_and_is_idempotent() {
        let tmp_dir = iota_common::tempdir();
        let store = open_store(tmp_dir.path());
        let (key_a, row_a) = test_row(1);
        let (key_b, row_b) = test_row(2);
        store.put_objects(1, &[(key_a, row_a)], &[]).unwrap();
        store.put_objects(2, &[(key_b, row_b)], &[]).unwrap();

        store.drop_epoch(1).unwrap();
        assert!(store.get_object(&key_a).unwrap().is_none());
        assert!(store.get_object(&key_b).unwrap().is_some());
        assert_eq!(store.earliest_epoch(), Some(2));
        // Dropping again is a no-op.
        store.drop_epoch(1).unwrap();

        // The dropped bucket stays gone across restarts.
        drop(store);
        let store = open_store(tmp_dir.path());
        assert_eq!(store.list_epochs(), vec![2]);
        assert!(store.get_object(&key_b).unwrap().is_some());
    }

    #[tokio::test]
    async fn tombstone_rows_read_as_none_objects() {
        let tmp_dir = iota_common::tempdir();
        let store = open_store(tmp_dir.path());
        let key = ObjectKey(ObjectId::random(), SequenceNumber::from_u64(4));
        store
            .put_objects(
                2,
                &[(key, StoreObjectWrapper::V2(StoreObject::Deleted))],
                &[],
            )
            .unwrap();
        assert!(store.get_store_object(&key).unwrap().is_some());
        assert!(store.get_object(&key).unwrap().is_none());
    }
}
