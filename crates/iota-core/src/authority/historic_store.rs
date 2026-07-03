// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch storage for pruned historic data.
//!
//! When the live/historic split is enabled, the pruner relocates data into
//! this store instead of deleting it:
//!
//! - superseded object versions, bucketed by their *supersession epoch* (the
//!   epoch of the checkpoint whose effects superseded them);
//! - checkpoint-keyed history (transactions, effects, events, checkpoint
//!   contents and summaries), bucketed by the epoch of their checkpoint.
//!
//! Each epoch bucket is a fixed set of column families, so expiring an epoch
//! of history is a constant-time `drop_cf` per family instead of per-key
//! deletes.
//!
//! The store is strictly outside the consensus/execution write and read
//! paths: readers are the gRPC exact-version object lookup and the
//! RocksDbStore fallbacks serving old transactions/effects/checkpoints to
//! gRPC and state sync. Lookups carry no epoch hint, so they probe the
//! per-epoch column families newest to oldest; a miss in a sealed, compacted
//! column family is answered from the in-memory RocksDB bloom filters without
//! touching disk.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use iota_types::{
    base_types::EpochId,
    digests::{TransactionDigest, TransactionEffectsDigest},
    effects::{TransactionEffects, TransactionEvents},
    error::{IotaError, IotaResult},
    messages_checkpoint::{
        CheckpointContents, CheckpointContentsDigest, CheckpointDigest, CheckpointSequenceNumber,
        TrustedCheckpoint,
    },
    object::Object,
    storage::ObjectKey,
    transaction::TrustedTransaction,
};
use prometheus_filtered::{
    Histogram, IntCounter, IntGauge, Registry, register_histogram_with_registry,
    register_int_counter_with_registry, register_int_gauge_with_registry,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
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
const TRANSACTIONS_CF_PREFIX: &str = "hist_tx_e";
const EFFECTS_CF_PREFIX: &str = "hist_fx_e";
const EXECUTED_EFFECTS_CF_PREFIX: &str = "hist_exec_fx_e";
const EVENTS_CF_PREFIX: &str = "hist_ev_e";
const CHECKPOINT_CONTENTS_CF_PREFIX: &str = "hist_ckpt_content_e";
const CHECKPOINT_SEQ_CF_PREFIX: &str = "hist_ckpt_seq_e";
const CHECKPOINTS_CF_PREFIX: &str = "hist_ckpt_e";

/// Every column-family prefix of an epoch bucket. No prefix may be a prefix
/// of another followed by a digit, so parsing an epoch from a name is
/// unambiguous.
const EPOCH_CF_PREFIXES: [&str; 9] = [
    OBJECTS_CF_PREFIX,
    EXPIRY_CF_PREFIX,
    TRANSACTIONS_CF_PREFIX,
    EFFECTS_CF_PREFIX,
    EXECUTED_EFFECTS_CF_PREFIX,
    EVENTS_CF_PREFIX,
    CHECKPOINT_CONTENTS_CF_PREFIX,
    CHECKPOINT_SEQ_CF_PREFIX,
    CHECKPOINTS_CF_PREFIX,
];

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
    /// Lowest checkpoint whose checkpoint-keyed history was relocated into
    /// this bucket. `None` until checkpoint relocation reaches this epoch.
    /// The earliest bucket may cover its epoch only partially (relocation
    /// enabled mid-epoch), so this — not the epoch's first checkpoint — is
    /// the availability horizon.
    pub min_checkpoint: Option<CheckpointSequenceNumber>,
    /// Highest checkpoint whose checkpoint-keyed history was relocated into
    /// this bucket.
    pub max_checkpoint: Option<CheckpointSequenceNumber>,
}

struct EpochBucket {
    /// Superseded object versions relocated out of the live `objects` table.
    objects: DBMap<ObjectKey, StoreObjectWrapper>,
    /// Tombstone heads (`Deleted`/`Wrapped`) whose lineages were superseded in
    /// this epoch. They stay in the live table until this bucket expires, at
    /// which point they are point-deleted from the live table right before
    /// the bucket is dropped.
    expiry: DBMap<ObjectKey, ()>,
    /// Transactions of this epoch's pruned checkpoints.
    transactions: DBMap<TransactionDigest, TrustedTransaction>,
    /// Effects of this epoch's pruned checkpoints, by effects digest.
    effects: DBMap<TransactionEffectsDigest, TransactionEffects>,
    /// Transaction digest to executed effects digest.
    executed_effects: DBMap<TransactionDigest, TransactionEffectsDigest>,
    /// Events by the digest of the transaction that produced them.
    events: DBMap<TransactionDigest, TransactionEvents>,
    /// Checkpoint contents by contents digest.
    checkpoint_contents: DBMap<CheckpointContentsDigest, CheckpointContents>,
    /// Checkpoint contents digest to checkpoint sequence number.
    checkpoint_seq_by_contents: DBMap<CheckpointContentsDigest, CheckpointSequenceNumber>,
    /// Certified checkpoint summaries by checkpoint digest.
    checkpoints: DBMap<CheckpointDigest, TrustedCheckpoint>,
}

/// One epoch-homogeneous batch of checkpoint-keyed history to relocate.
/// All keys must belong to checkpoints of the target bucket's epoch.
#[derive(Default)]
pub struct CheckpointHistoryBatch {
    pub transactions: Vec<(TransactionDigest, TrustedTransaction)>,
    pub effects: Vec<(TransactionEffectsDigest, TransactionEffects)>,
    pub executed_effects: Vec<(TransactionDigest, TransactionEffectsDigest)>,
    pub events: Vec<(TransactionDigest, TransactionEvents)>,
    pub checkpoint_contents: Vec<(CheckpointContentsDigest, CheckpointContents)>,
    pub checkpoint_seq_by_contents: Vec<(CheckpointContentsDigest, CheckpointSequenceNumber)>,
    pub checkpoints: Vec<(CheckpointDigest, TrustedCheckpoint)>,
    /// Inclusive checkpoint sequence range covered by this batch; drives the
    /// bucket's availability watermark.
    pub checkpoint_range: Option<(CheckpointSequenceNumber, CheckpointSequenceNumber)>,
}

impl EpochBucket {
    fn flush_all(&self) -> IotaResult<()> {
        self.objects.flush()?;
        self.expiry.flush()?;
        self.transactions.flush()?;
        self.effects.flush()?;
        self.executed_effects.flush()?;
        self.events.flush()?;
        self.checkpoint_contents.flush()?;
        self.checkpoint_seq_by_contents.flush()?;
        self.checkpoints.flush()?;
        Ok(())
    }
}

impl CheckpointHistoryBatch {
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
            && self.effects.is_empty()
            && self.executed_effects.is_empty()
            && self.events.is_empty()
            && self.checkpoint_contents.is_empty()
            && self.checkpoint_seq_by_contents.is_empty()
            && self.checkpoints.is_empty()
    }
}

pub struct HistoricStoreMetrics {
    pub relocated_objects: IntCounter,
    pub relocated_transactions: IntCounter,
    pub relocated_bytes: IntCounter,
    pub lookup_probes: Histogram,
    pub lookup_not_found: IntCounter,
    pub epochs_retained: IntGauge,
    pub earliest_retained_epoch: IntGauge,
}

impl HistoricStoreMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            relocated_objects: register_int_counter_with_registry!(
                "historic_store_relocated_objects",
                "Number of superseded object versions relocated into the historic store",
                registry
            )
            .unwrap(),
            relocated_transactions: register_int_counter_with_registry!(
                "historic_store_relocated_transactions",
                "Number of transactions whose checkpoint-keyed history was relocated into the \
                 historic store",
                registry
            )
            .unwrap(),
            relocated_bytes: register_int_counter_with_registry!(
                "historic_store_relocated_bytes",
                "Serialized bytes of object versions relocated into the historic store",
                registry
            )
            .unwrap(),
            lookup_probes: register_histogram_with_registry!(
                "historic_store_lookup_probes",
                "Number of epoch buckets probed per historic lookup",
                vec![1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0],
                registry
            )
            .unwrap(),
            lookup_not_found: register_int_counter_with_registry!(
                "historic_store_lookup_not_found",
                "Historic lookups that missed every epoch bucket",
                registry
            )
            .unwrap(),
            epochs_retained: register_int_gauge_with_registry!(
                "historic_store_epochs_retained",
                "Number of epoch buckets currently retained",
                registry
            )
            .unwrap(),
            earliest_retained_epoch: register_int_gauge_with_registry!(
                "historic_store_earliest_retained_epoch",
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
pub struct HistoricStore {
    db: Arc<Database>,
    /// Template options for per-epoch column families. All clones share one
    /// block cache through the cloned table factory.
    cf_options: rocksdb::Options,
    meta: DBMap<EpochId, EpochBucketInfo>,
    buckets: RwLock<BTreeMap<EpochId, EpochBucket>>,
    disable_wal: bool,
    metrics: Arc<HistoricStoreMetrics>,
}

impl HistoricStore {
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
        metrics: Arc<HistoricStoreMetrics>,
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
        // written) and is backfilled lazily on the next write. A bucket's
        // column families are created and dropped in separate operations, so
        // a crash can leave some of the set missing: recreate them (empty)
        // here. A bucket half-dropped this way simply resurfaces and is
        // dropped again by the next retention pass.
        let mut epochs = std::collections::BTreeSet::new();
        for cf_name in &existing_cfs {
            let Some(epoch_str) = EPOCH_CF_PREFIXES
                .iter()
                .find_map(|prefix| cf_name.strip_prefix(prefix))
            else {
                continue;
            };
            let epoch: EpochId = epoch_str.parse().map_err(|_| {
                IotaError::Storage(format!("unparsable historic column family name: {cf_name}"))
            })?;
            epochs.insert(epoch);
        }
        let mut buckets = BTreeMap::new();
        for epoch in epochs {
            for cf_name in Self::epoch_cf_names(epoch) {
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

    fn epoch_cf_names(epoch: EpochId) -> [String; 9] {
        EPOCH_CF_PREFIXES.map(|prefix| format!("{prefix}{epoch}"))
    }

    fn reopen_bucket(db: &Arc<Database>, epoch: EpochId) -> IotaResult<EpochBucket> {
        // Per-epoch column families skip the periodic metrics reporter task:
        // with ~100 retained epochs the per-table metrics add little insight
        // and one task per column family adds up.
        fn map<K, V>(db: &Arc<Database>, cf_name: String) -> IotaResult<DBMap<K, V>> {
            Ok(DBMap::reopen(
                db,
                Some(&cf_name),
                &ReadWriteOptions::default(),
                true,
            )?)
        }
        Ok(EpochBucket {
            objects: map(db, Self::objects_cf_name(epoch))?,
            expiry: map(db, Self::expiry_cf_name(epoch))?,
            transactions: map(db, format!("{TRANSACTIONS_CF_PREFIX}{epoch}"))?,
            effects: map(db, format!("{EFFECTS_CF_PREFIX}{epoch}"))?,
            executed_effects: map(db, format!("{EXECUTED_EFFECTS_CF_PREFIX}{epoch}"))?,
            events: map(db, format!("{EVENTS_CF_PREFIX}{epoch}"))?,
            checkpoint_contents: map(db, format!("{CHECKPOINT_CONTENTS_CF_PREFIX}{epoch}"))?,
            checkpoint_seq_by_contents: map(db, format!("{CHECKPOINT_SEQ_CF_PREFIX}{epoch}"))?,
            checkpoints: map(db, format!("{CHECKPOINTS_CF_PREFIX}{epoch}"))?,
        })
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

    /// Durably persists one epoch-homogeneous batch of checkpoint-keyed
    /// history into the bucket for `epoch`, creating the bucket on first use.
    /// Idempotent: rewriting the same keys with the same bytes is harmless.
    ///
    /// Durability of the write is only guaranteed after a subsequent
    /// [`Self::flush_epoch`]; callers must flush before deleting the source
    /// rows.
    pub fn put_checkpoint_data(
        &self,
        epoch: EpochId,
        data: CheckpointHistoryBatch,
    ) -> IotaResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        self.ensure_bucket(epoch)?;
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let bucket = buckets.get(&epoch).expect("bucket was just created");

        let num_transactions = data.transactions.len() as u64;
        let mut batch = bucket.transactions.batch();
        batch.insert_batch(&bucket.transactions, data.transactions)?;
        batch.insert_batch(&bucket.effects, data.effects)?;
        batch.insert_batch(&bucket.executed_effects, data.executed_effects)?;
        batch.insert_batch(&bucket.events, data.events)?;
        batch.insert_batch(&bucket.checkpoint_contents, data.checkpoint_contents)?;
        batch.insert_batch(
            &bucket.checkpoint_seq_by_contents,
            data.checkpoint_seq_by_contents,
        )?;
        batch.insert_batch(&bucket.checkpoints, data.checkpoints)?;

        let mut info = self.meta.get(&epoch)?.unwrap_or_default();
        if let Some((batch_min, batch_max)) = data.checkpoint_range {
            info.min_checkpoint = Some(info.min_checkpoint.unwrap_or(batch_min).min(batch_min));
            info.max_checkpoint = Some(info.max_checkpoint.unwrap_or(batch_max).max(batch_max));
        }
        batch.insert_batch(&self.meta, [(epoch, info)])?;

        let mut write_options = rocksdb::WriteOptions::default();
        write_options.disable_wal(self.disable_wal);
        batch.write_opt(&write_options)?;

        self.metrics.relocated_transactions.inc_by(num_transactions);
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
        bucket.flush_all()?;
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
            bucket.flush_all()?;
            // Full-range manual compaction: the longest keys are 40-byte
            // fix-int-serialized (ObjectId, version) tuples, so these raw
            // bounds cover every possible key.
            let full_range_end = vec![0xffu8; 48];
            for cf_name in Self::epoch_cf_names(epoch) {
                bucket
                    .objects
                    .compact_range_raw(&cf_name, vec![], full_range_end.clone())?;
            }
        }
        let mut info = self.meta.get(&epoch)?.unwrap_or_default();
        if !info.sealed {
            info.sealed = true;
            self.meta.insert(&epoch, &info)?;
        }
        Ok(())
    }

    /// Exact-key lookup with no epoch hint: probes buckets newest to oldest.
    fn probe_newest_first<K, V>(
        &self,
        select: impl Fn(&EpochBucket) -> &DBMap<K, V>,
        key: &K,
    ) -> IotaResult<Option<V>>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let buckets = self.buckets.read().expect("lock should not be poisoned");
        let mut probes = 0u64;
        for bucket in buckets.values().rev() {
            probes += 1;
            if let Some(value) = select(bucket).get(key)? {
                self.metrics.lookup_probes.observe(probes as f64);
                return Ok(Some(value));
            }
        }
        self.metrics.lookup_probes.observe(probes.max(1) as f64);
        self.metrics.lookup_not_found.inc();
        Ok(None)
    }

    pub fn get_store_object(&self, key: &ObjectKey) -> IotaResult<Option<StoreObjectWrapper>> {
        self.probe_newest_first(|bucket| &bucket.objects, key)
    }

    pub fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TrustedTransaction>> {
        self.probe_newest_first(|bucket| &bucket.transactions, digest)
    }

    pub fn get_effects(
        &self,
        digest: &TransactionEffectsDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        self.probe_newest_first(|bucket| &bucket.effects, digest)
    }

    pub fn get_executed_effects(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffectsDigest>> {
        self.probe_newest_first(|bucket| &bucket.executed_effects, digest)
    }

    pub fn get_events(&self, digest: &TransactionDigest) -> IotaResult<Option<TransactionEvents>> {
        self.probe_newest_first(|bucket| &bucket.events, digest)
    }

    pub fn get_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> IotaResult<Option<CheckpointContents>> {
        self.probe_newest_first(|bucket| &bucket.checkpoint_contents, digest)
    }

    pub fn get_checkpoint_seq_by_contents_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        self.probe_newest_first(|bucket| &bucket.checkpoint_seq_by_contents, digest)
    }

    pub fn get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> IotaResult<Option<TrustedCheckpoint>> {
        self.probe_newest_first(|bucket| &bucket.checkpoints, digest)
    }

    /// The lowest checkpoint whose checkpoint-keyed history is retained, if
    /// any. Coverage is contiguous from here to the pruning watermark:
    /// relocation processes checkpoints strictly in order, and buckets expire
    /// oldest-first.
    pub fn lowest_available_checkpoint(&self) -> IotaResult<Option<CheckpointSequenceNumber>> {
        for entry in self.meta.safe_iter() {
            let (_, info) = entry?;
            if let Some(min_checkpoint) = info.min_checkpoint {
                return Ok(Some(min_checkpoint));
            }
        }
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
            for cf_name in Self::epoch_cf_names(epoch) {
                self.db
                    .drop_cf(&cf_name)
                    .map_err(|e| IotaError::Storage(e.to_string()))?;
            }
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
        for cf_name in Self::epoch_cf_names(epoch) {
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

    fn open_store(path: &Path) -> HistoricStore {
        HistoricStore::open(path, true, HistoricStoreMetrics::new_for_test()).unwrap()
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
    async fn checkpoint_history_roundtrip_and_watermark() {
        use iota_types::{
            base_types::ExecutionDigests,
            digests::CheckpointContentsDigest,
            effects::{TransactionEffectsAPI, TransactionEffectsExtForTesting},
            messages_checkpoint::CheckpointContentsExt,
        };

        let tmp_dir = iota_common::tempdir();
        let store = open_store(tmp_dir.path());

        let effects = TransactionEffects::new_empty_v1_for_testing(TransactionDigest::random());
        let fx_digest = effects.digest();
        let tx_digest = *effects.transaction_digest();
        let contents =
            CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::random()]);
        let contents_digest = contents.digest();

        store
            .put_checkpoint_data(
                4,
                CheckpointHistoryBatch {
                    effects: vec![(fx_digest, effects)],
                    executed_effects: vec![(tx_digest, fx_digest)],
                    events: vec![(tx_digest, TransactionEvents(vec![]))],
                    checkpoint_contents: vec![(contents_digest, contents)],
                    checkpoint_seq_by_contents: vec![(contents_digest, 42)],
                    checkpoint_range: Some((40, 45)),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(
            store.get_effects(&fx_digest).unwrap().map(|e| e.digest()),
            Some(fx_digest)
        );
        assert_eq!(
            store.get_executed_effects(&tx_digest).unwrap(),
            Some(fx_digest)
        );
        assert!(store.get_events(&tx_digest).unwrap().is_some());
        assert!(
            store
                .get_checkpoint_contents(&contents_digest)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            store
                .get_checkpoint_seq_by_contents_digest(&contents_digest)
                .unwrap(),
            Some(42)
        );
        assert_eq!(store.lowest_available_checkpoint().unwrap(), Some(40));

        // An earlier batch of the same bucket lowers the watermark.
        store
            .put_checkpoint_data(
                4,
                CheckpointHistoryBatch {
                    checkpoint_seq_by_contents: vec![(CheckpointContentsDigest::random(), 38)],
                    checkpoint_range: Some((38, 39)),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(store.lowest_available_checkpoint().unwrap(), Some(38));

        // Everything survives a restart.
        drop(store);
        let store = open_store(tmp_dir.path());
        assert!(store.get_effects(&fx_digest).unwrap().is_some());
        assert_eq!(store.lowest_available_checkpoint().unwrap(), Some(38));

        // Dropping the bucket clears the availability watermark.
        store.drop_epoch(4).unwrap();
        assert_eq!(store.lowest_available_checkpoint().unwrap(), None);
        assert!(store.get_effects(&fx_digest).unwrap().is_none());
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
