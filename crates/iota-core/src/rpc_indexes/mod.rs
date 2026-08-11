// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The unified RPC index store: the on-disk indexes both the JSON-RPC and
//! gRPC APIs read from, replacing the separate `jsonrpc_index` and
//! `grpc_indexes` stores. A store is configured with the [`IndexGroup`]s its
//! node needs; tables of a disabled group stay empty, and the digest history
//! (see [`schema::HistoryBucket`]) is filled from checkpoint contents alone
//! when the JSON-RPC group is off, since gRPC needs only the checkpoint a
//! transaction landed in, not its network sequence number.
//!
//! This module is schema, open, rebuild, backfill, prune, and the
//! per-checkpoint ingest; [`jsonrpc_api`] and [`grpc_api`] add the two read
//! surfaces, and [`live_scan`] fills the live-state tables from a rebuild's
//! object scan or a formal-snapshot restore.

pub mod grpc_api;
pub mod jsonrpc_api;
pub mod live_scan;
pub mod schema;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use iota_sdk_types::{Owner, TransactionDigest};
use iota_types::{
    base_types::TxSequenceNumber,
    committee::EpochId,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, VerifiedCheckpoint},
    storage::{
        DynamicFieldKey, PackageVersionInfo, PackageVersionKey,
        error::{Error as StorageError, Kind as StorageErrorKind},
    },
};
use parking_lot::Mutex;
use prometheus_filtered::{IntGauge, MetricLevel, Registry, register_int_gauge_with_registry};
use tracing::{error, info, warn};
use typed_store::{
    TypedStoreError,
    database::{Database, drop_tolerant_write_options, wait_for_database_close},
    rocks::{
        DBBatch, DBMap, MetricConf, ReadWriteOptions, bulk_ingestion_options,
        bulk_ingestion_options_split_between, default_db_options, list_tables, open_cf_opts,
        read_size_from_env, safe_drop_db,
    },
    rocksdb,
    traits::Map,
};

pub use self::schema::{IndexGroup, TotalBalance};
use self::{
    jsonrpc_api::{
        BalanceCaches, CoinBalanceChanges, JsonRpcMetrics,
        invalidate_balance_caches_instead_of_updating,
    },
    live_scan::LiveObjectSetIndexer,
    schema::{
        CURRENT_DB_VERSION, CoinIndexInfo, CoinIndexKey, HISTORY_CF_PREFIX, HistoryBucket,
        IndexStoreTables, MetadataInfo, OwnerIndexKey, is_dynamic_field, merge_coin_into,
        transaction_index_data, try_create_coin_index_info, try_create_package_version_info,
        try_create_regulated_coin_info,
    },
};
use crate::{
    authority::{AuthorityStore, authority_store_pruner::MIN_EPOCHS_TO_RETAIN_FOR_INDEXES},
    checkpoints::CheckpointStore,
    index_rebuild_cancellation::{RebuildCancelled, is_cancelled},
    par_index_live_object_set::{
        PROGRESS_REPORT_INTERVAL, eta_display, par_index_live_object_set, progress_rate,
    },
    rpc_index_history::{self, EpochBuckets},
};

const ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB: &str = "RPC_INDEX_HISTORY_BLOCK_CACHE_MB";
const DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB: usize = 512;

/// The column-family name of `epoch`'s history bucket.
fn history_cf_name(epoch: EpochId) -> String {
    rpc_index_history::bucket_cf_name(HISTORY_CF_PREFIX, epoch)
}

/// The epoch of a history column family, `None` for other names.
fn history_cf_epoch(cf_name: &str) -> Option<EpochId> {
    rpc_index_history::bucket_cf_epoch(HISTORY_CF_PREFIX, cf_name)
}

/// A staged index update for one checkpoint, waiting for its in-order commit.
struct PendingCheckpointUpdate {
    batch: DBBatch,
    /// The checkpoint's coin balance changes, used at commit time to derive
    /// the JSON-RPC balance cache updates. Empty when that group is off.
    coin_changes: CoinBalanceChanges,
}

struct RpcIndexesMetrics {
    /// Lowest checkpoint the history backfill has replayed so far. The
    /// value reflects only the backfill's own progress: it keeps its final
    /// value after the backfill stops and is not raised when pruning later
    /// drops replayed epochs.
    history_backfill_lowest_replayed_checkpoint: IntGauge,
    /// 1 while the background history backfill is running, 0 otherwise.
    history_backfill_running: IntGauge,
}

impl RpcIndexesMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            // How far the backfill got is visible nowhere else, so keep it
            // above the default metric filter.
            history_backfill_lowest_replayed_checkpoint: register_int_gauge_with_registry!(
                "rpc_index_history_backfill_lowest_replayed_checkpoint",
                "Lowest checkpoint the RPC index history backfill has replayed, keeping its \
                 final value after the backfill stops; unaffected by later pruning",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            history_backfill_running: register_int_gauge_with_registry!(
                "rpc_index_history_backfill_running",
                "1 while the RPC index history backfill is running, 0 otherwise",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
        }
    }
}

/// The pieces produced by opening the index database.
struct OpenedIndexDb {
    tables: IndexStoreTables,
    db: Arc<Database>,
    history_cf_options: rocksdb::Options,
    /// Every history bucket found on disk, before the retention floor is
    /// applied by [`EpochBuckets::open`].
    history: BTreeMap<EpochId, Arc<HistoryBucket>>,
}

/// The unified store backing both the JSON-RPC and gRPC APIs. See the
/// [module docs][self].
pub struct RpcIndexesStore {
    tables: IndexStoreTables,
    /// The API groups this store maintains; a group not in this set never
    /// has its tables filled.
    groups: BTreeSet<IndexGroup>,
    /// The retained history buckets.
    history: EpochBuckets<HistoryBucket>,
    next_sequence_number: AtomicU64,
    metrics: RpcIndexesMetrics,
    /// Balance caches backing the JSON-RPC coin reads; unused, but harmless,
    /// on a store that does not serve [`IndexGroup::JsonRpc`].
    caches: BalanceCaches,
    jsonrpc_metrics: JsonRpcMetrics,
    max_type_length: u64,
    /// The staged updates of the checkpoints indexed but not yet committed,
    /// in checkpoint order.
    pending_updates: Mutex<BTreeMap<CheckpointSequenceNumber, PendingCheckpointUpdate>>,
    history_backfill_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Stops the startup rebuild and the background history backfill.
    cancelled: Arc<AtomicBool>,
    /// How many epochs of history the pruner is configured to retain
    /// (`num_epochs_to_retain_for_indexes`); bounds the history backfill so
    /// it does not replay epochs the next prune pass would drop again, and
    /// is the retention `prune` enforces. Governs every history table,
    /// digests included, since they all live in the one bucket family.
    /// `None` when index pruning is off.
    epochs_to_retain: Option<u64>,
}

impl IndexStoreTables {
    /// Opens the tables with tuned bulk-ingestion options (WAL disabled,
    /// unordered writes) for a full rebuild. Writes must be flushed before
    /// the database closes, and serving queries requires a reopen with
    /// default options.
    ///
    /// Anything left under `path` is deleted first, so the caller does not
    /// have to clear the directory.
    fn open_for_bulk_ingestion(path: PathBuf, concurrent_stores: usize) -> Self {
        // A column family of an existing database not named here would
        // silently be opened with default options, and `safe_drop_db` can
        // leave files RocksDB does not recognize, so clear the directory
        // rather than fail the recovery.
        if path.exists() && path.read_dir().is_ok_and(|mut dir| dir.next().is_some()) {
            warn!("clearing leftover files under {path:?} before the index rebuild");
            std::fs::remove_dir_all(&path)
                .expect("unable to clear the index database directory for the rebuild");
        }
        let bulk_options = bulk_ingestion_options_split_between(concurrent_stores);
        let table_config = bulk_options.table_config(Self::describe_tables().into_keys());
        Self::open_tables_read_write(
            path,
            MetricConf::new("rpc-index"),
            Some(bulk_options.db_options),
            Some(table_config),
        )
    }

    /// Seeds the `meta` row on the first open of an empty database, so a
    /// fresh store on a node with no executed checkpoints needs no rebuild.
    fn seed_meta(&self, groups: &BTreeSet<IndexGroup>) -> IotaResult {
        if !matches!(self.meta.get(&()), Ok(None)) {
            return Ok(());
        }
        if self.owner.is_empty() {
            self.meta.insert(
                &(),
                &MetadataInfo {
                    version: CURRENT_DB_VERSION,
                    groups: groups.clone(),
                },
            )?;
        }
        Ok(())
    }

    /// Whether the store must be wiped and rebuilt: a schema mismatch, an
    /// enabled group missing from what `meta` last recorded, or the index
    /// watermark falling behind `highest_executed_checkpoint`. Read errors
    /// propagate: a transient error must fail the open rather than silently
    /// wipe a healthy store or adopt a stale one.
    fn needs_to_do_initialization(
        &self,
        checkpoint_store: &CheckpointStore,
        groups: &BTreeSet<IndexGroup>,
    ) -> IotaResult<bool> {
        let stale = match self.meta.get(&())? {
            Some(metadata) => {
                metadata.version != CURRENT_DB_VERSION || !groups.is_subset(&metadata.groups)
            }
            None => true,
        };

        Ok(stale || self.is_indexed_watermark_out_of_date(checkpoint_store)?)
    }

    /// Whether the index watermark is behind `highest_executed_checkpoint`,
    /// absent on a store that already holds data, or points at a checkpoint
    /// the checkpoint store no longer holds.
    fn is_indexed_watermark_out_of_date(
        &self,
        checkpoint_store: &CheckpointStore,
    ) -> IotaResult<bool> {
        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;
        let Some(watermark) = self.watermark.get(&())? else {
            // A rebuild writes the watermark only once its data is durable,
            // so data without one comes from a build that was cut short.
            // Scanned rather than `is_empty`, which reads an unreadable
            // index as non-empty and would wipe a healthy store on a
            // transient read error.
            let has_data = self.owner.safe_iter().next().transpose()?.is_some();
            return Ok(has_data || highest_executed_checkpoint.is_some());
        };
        // The open anchors the transaction numbering to the watermark's
        // checkpoint, so a checkpoint store rolled back to an older backup
        // must rebuild rather than fail every open.
        if checkpoint_store
            .get_checkpoint_by_sequence_number(watermark)?
            .is_none()
        {
            return Ok(true);
        }
        let Some(executed) = highest_executed_checkpoint else {
            return Ok(false);
        };
        // After an unclean stop the watermark can be ahead of the executed
        // checkpoint by up to the execution concurrency, and replaying those
        // checkpoints writes nothing but the watermark.
        Ok(watermark < executed)
    }

    /// Rebuilds the live-state tables of whichever `groups` are enabled from
    /// a parallel scan of the live object set, for the cases
    /// `needs_to_do_initialization` covers. The on-disk DB needs to be
    /// wiped before this is called, so `init` always starts from an empty
    /// store.
    ///
    /// Writes only `meta`: the caller adopts the rebuild by writing the
    /// watermarks once the WAL-less bulk writes are flushed. Returns the
    /// highest executed checkpoint to anchor them to.
    #[tracing::instrument(skip_all)]
    fn init(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        groups: &BTreeSet<IndexGroup>,
        batch_size_limit: usize,
        cancelled: &AtomicBool,
    ) -> Result<Option<CheckpointSequenceNumber>, StorageError> {
        info!("Initializing RPC indexes");

        // Written before the flush, the watermarks would be WAL-durable over
        // unflushed data, and a crash before the flush would leave a store
        // the next open adopts as complete.
        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
                groups: groups.clone(),
            },
        )?;

        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;

        // Live-state tables from the current live object set. The history
        // tables are not built here: `backfill_history` fills them in the
        // background once the node is up, resuming from `history_watermark`.
        let indexer = LiveObjectSetIndexer::new(self, groups, batch_size_limit);
        par_index_live_object_set(authority_store, &indexer, cancelled)?;
        indexer.finish()?;

        info!("Finished initializing RPC indexes");

        Ok(highest_executed_checkpoint)
    }

    /// Makes the bulk-ingested data durable and writes the watermarks that
    /// let a node open the store in place instead of rebuilding it.
    /// `highest_executed` is the highest checkpoint the build covers.
    ///
    /// With nothing executed no watermark is written: an absent watermark
    /// already means "nothing indexed", while writing 0 would claim
    /// checkpoint 0 was indexed and shift the numbering anchor past the
    /// genesis transaction.
    fn adopt_bulk_ingestion(
        &self,
        highest_executed: Option<CheckpointSequenceNumber>,
    ) -> Result<(), TypedStoreError> {
        // The watermarks are WAL-durable while the bulk writes are not, so
        // flushing first keeps them from landing over unflushed data, where
        // a crash would leave a store the next open adopts as complete.
        // Flushing any table flushes every column family of the shared
        // database, so one call covers all tables.
        self.meta.flush_all()?;
        self.history_watermark
            .insert(&(), &highest_executed.map_or(0, |c| c.saturating_add(1)))?;
        if let Some(highest_executed) = highest_executed {
            self.watermark.insert(&(), &highest_executed)?;
        }
        Ok(())
    }

    /// Appends the live-state deltas of a checkpoint's `transactions` to its
    /// batch: the owner and dynamic-field rows both API groups share, and the
    /// gRPC group's coin metadata and package versions. `coin_changes`
    /// collects what the JSON-RPC balance caches are updated from and stays
    /// empty when that group is off.
    ///
    /// The rows a change replaces are recomputed from the object as it was
    /// before it: an owner key carries the object's type and, for a coin, its
    /// balance, so the row to delete cannot be derived from the new state.
    fn index_objects(
        &self,
        transactions: &[&CheckpointTransaction],
        groups: &BTreeSet<IndexGroup>,
        batch: &mut DBBatch,
        coin_changes: &mut CoinBalanceChanges,
    ) -> IotaResult {
        let index_jsonrpc = groups.contains(&IndexGroup::JsonRpc);
        let index_grpc = groups.contains(&IndexGroup::Grpc);
        let mut coin_metadata: HashMap<CoinIndexKey, CoinIndexInfo> = HashMap::new();
        let mut package_versions: Vec<(PackageVersionKey, PackageVersionInfo)> = Vec::new();

        for tx in transactions {
            for removed_object in tx.removed_objects_pre_version() {
                match removed_object.owner {
                    Owner::Address(address) => {
                        if let Some((owner_key, _)) =
                            OwnerIndexKey::for_object(address, removed_object)
                        {
                            batch.delete_batch(&self.owner, [owner_key])?;
                        }
                        if index_jsonrpc {
                            coin_changes.record_removed(address, removed_object);
                        }
                    }
                    Owner::Object(object_id) => {
                        batch.delete_batch(
                            &self.dynamic_field,
                            [DynamicFieldKey::new(object_id, removed_object.id())],
                        )?;
                    }
                    Owner::Shared(_) | Owner::Immutable => {}
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }

            for (object, old_object) in tx.changed_objects() {
                if let Some(old_object) = old_object {
                    match old_object.owner {
                        Owner::Address(address) => {
                            if let Some((owner_key, _)) =
                                OwnerIndexKey::for_object(address, old_object)
                            {
                                batch.delete_batch(&self.owner, [owner_key])?;
                            }
                            if index_jsonrpc {
                                coin_changes.record_removed(address, old_object);
                            }
                        }
                        Owner::Object(object_id) => {
                            if old_object.owner != object.owner {
                                batch.delete_batch(
                                    &self.dynamic_field,
                                    [DynamicFieldKey::new(object_id, old_object.id())],
                                )?;
                            }
                        }
                        Owner::Shared(_) | Owner::Immutable => {}
                        _ => unimplemented!(
                            "a new Owner enum variant was added and needs to be handled"
                        ),
                    }
                }

                match object.owner {
                    Owner::Address(owner) => {
                        if let Some((owner_key, owner_info)) =
                            OwnerIndexKey::for_object(owner, object)
                        {
                            batch.insert_batch(&self.owner, [(owner_key, owner_info)])?;
                        }
                        if index_jsonrpc {
                            coin_changes.record_written(owner, object, old_object);
                        }
                    }
                    Owner::Object(parent) => {
                        if is_dynamic_field(object) {
                            batch.insert_batch(
                                &self.dynamic_field,
                                [(DynamicFieldKey::new(parent, object.id()), ())],
                            )?;
                        }
                    }
                    Owner::Shared(_) | Owner::Immutable => {}
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }

            if index_grpc {
                // Packages, coin metadata, treasury caps and regulated coin
                // metadata are always created, never mutated in place, so the
                // changed objects would only add noise from unrelated
                // mutations.
                for object in tx.created_objects() {
                    if let Some((key, info)) = try_create_coin_index_info(object) {
                        merge_coin_into(&mut coin_metadata, key, info);
                    }
                    if let Some((key, object_id)) = try_create_regulated_coin_info(object) {
                        merge_coin_into(
                            &mut coin_metadata,
                            key,
                            CoinIndexInfo {
                                regulated_coin_metadata_object_id: Some(object_id),
                                ..Default::default()
                            },
                        );
                    }
                    if let Some((key, info)) = try_create_package_version_info(object) {
                        package_versions.push((key, info));
                    }
                }
            }
        }

        batch.insert_batch(&self.package_version, package_versions)?;
        // A coin type's metadata, treasury cap and regulated metadata are
        // separate objects, so each row is merged onto what the table already
        // holds rather than replacing it. The read sees committed rows only,
        // which is enough: those objects are created together, in one
        // transaction, so no earlier checkpoint can still have a row of the
        // same coin type staged. Coin types are created rarely, so these
        // reads are rare too.
        for (key, info) in coin_metadata {
            let mut entry = self.coin.get(&key)?.unwrap_or_default();
            entry.merge(info);
            batch.insert_batch(&self.coin, [(key, entry)])?;
        }

        Ok(())
    }
}

impl RpcIndexesStore {
    /// Opens the store, wiping it and rebuilding the live-state tables first
    /// when the indexes are missing or stale (schema mismatch, a newly
    /// enabled group, or the watermark falling behind
    /// `highest_executed_checkpoint`).
    ///
    /// The history tables are filled by a background replay after this
    /// returns; until it finishes, history-backed queries cover a growing
    /// range of recent checkpoints, as on a pruned node. When index pruning
    /// is configured, `epochs_to_retain` bounds the replay to the epochs
    /// the pruner would retain.
    ///
    /// Setting `cancelled` abandons a rebuild running here and the
    /// background replay, and fails the open: the store is left unadopted
    /// for the next open to rebuild, and must not serve reads in the
    /// meantime.
    pub async fn new(
        path: PathBuf,
        registry: &Registry,
        groups: BTreeSet<IndexGroup>,
        max_type_length: Option<u64>,
        epochs_to_retain: Option<u64>,
        authority_store: &Arc<AuthorityStore>,
        checkpoint_store: &Arc<CheckpointStore>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Arc<Self>, StorageError> {
        // An unopenable database would crash-loop the node with no way to
        // self-heal; wipe and rebuild it like a stale one — but only after
        // one retry, so a transient error does not destroy a healthy store.
        let mut opened = match Self::open_index_db(&path) {
            Ok(opened) => Some(opened),
            Err(first) => {
                warn!("unable to open the RPC index database, retrying once: {first}");
                match Self::open_index_db(&path) {
                    Ok(opened) => Some(opened),
                    Err(e) => {
                        warn!("unable to open the RPC index database, wiping and rebuilding: {e}");
                        None
                    }
                }
            }
        };

        if let Some(opened) = &opened {
            opened
                .tables
                .seed_meta(&groups)
                .expect("failed to initialize RPC index tables");
        }

        // Node startup blocks on a rebuild before any RPC surface exists;
        // the gauge tells operators (and their probes) that the node is
        // rebuilding, not hung. Registered unconditionally, so "not
        // rebuilding" reads as 0 rather than a missing series.
        let rebuild_gauge = register_int_gauge_with_registry!(
            "rpc_index_rebuild_in_progress",
            "1 while the RPC index store is being rebuilt at startup",
            registry;
            MetricLevel::Warn,
        )
        .expect("failed to register the RPC index rebuild gauge");

        let needs_initialization = opened.as_ref().is_none_or(|opened| {
            opened
                .tables
                .needs_to_do_initialization(checkpoint_store, &groups)
                .expect("failed to determine whether the RPC index needs a rebuild")
        });
        if needs_initialization {
            rebuild_gauge.set(1);
            let init_tables = {
                drop(opened);
                // `DB::destroy` fails on a database it cannot parse — the
                // very state the rebuild recovers from — so fall back to
                // deleting the directory. The database was already closed
                // above, so a short wait covers its background threads.
                if let Err(e) = safe_drop_db(path.clone(), Duration::from_secs(30)).await {
                    warn!("unable to destroy the old RPC index database ({e}), deleting it");
                    std::fs::remove_dir_all(&path)
                        .expect("unable to delete the old RPC index database");
                }

                // Open the empty DB with tuned bulk ingestion options to
                // speed up the initial indexing. The DB is reopened with
                // default options afterwards.
                IndexStoreTables::open_for_bulk_ingestion(path.clone(), 1)
            };
            let batch_size_limit = bulk_ingestion_options().batch_size_limit;

            // The rebuild scans and writes RocksDB for a long time; keep it
            // off the async runtime's worker threads.
            let (init_tables, initialized) = tokio::task::spawn_blocking({
                let authority_store = authority_store.clone();
                let checkpoint_store = checkpoint_store.clone();
                let cancelled = cancelled.clone();
                let groups = groups.clone();
                move || {
                    let mut init_tables = init_tables;
                    let initialized = init_tables.init(
                        &authority_store,
                        &checkpoint_store,
                        &groups,
                        batch_size_limit,
                        &cancelled,
                    );
                    (init_tables, initialized)
                }
            })
            .await
            .expect("RPC index initialization task failed");

            match initialized {
                // A crash before this point re-detects the rebuild on the
                // next open (no watermark), never adopts a half-flushed
                // store.
                Ok(highest_executed_checkpoint) => init_tables
                    .adopt_bulk_ingestion(highest_executed_checkpoint)
                    .expect("unable to adopt the rebuilt RPC index"),
                // Unadopted, so the next open rebuilds it, as after a crash.
                // The open fails so the truncated store is never served and
                // never stamped with a watermark.
                // Keyed on the error, not on the flag: a real failure that
                // races the shutdown must stay a failure.
                Err(e) if is_cancelled(&e) => {
                    // Release the database so the next open can rebuild it.
                    let weak_db = Arc::downgrade(&init_tables.meta.db);
                    drop(init_tables);
                    if !wait_for_database_close(weak_db).await {
                        warn!("the cancelled RPC index rebuild left its database open");
                    }
                    return Err(RebuildCancelled::error(format!(
                        "the RPC index rebuild was cancelled by shutdown: {e}"
                    )));
                }
                Err(e) => panic!("unable to initialize RPC index: {e}"),
            }

            let weak_db = Arc::downgrade(&init_tables.meta.db);
            drop(init_tables);
            if !wait_for_database_close(weak_db).await {
                panic!("unable to reopen DB after indexing");
            }

            // Reopen the DB with default options (e.g. without
            // `unordered_write`s enabled).
            let reopened = Self::open_index_db(&path)
                .expect("unable to reopen the RPC index database after the rebuild");

            // Smoke test: the reopened database is readable and carries the
            // schema version the rebuild wrote.
            let stored_version = reopened
                .tables
                .meta
                .get(&())
                .expect("reopened RPC index DB should expose readable metadata")
                .expect("metadata should have been written before flush and reopen");
            assert_eq!(
                stored_version.version, CURRENT_DB_VERSION,
                "database version mismatch after flush and reopen: expected {}, found {}",
                CURRENT_DB_VERSION, stored_version.version
            );
            opened = Some(reopened);
            rebuild_gauge.set(0);
        }
        let opened = opened.expect("the index database is open on both paths above");

        // A store rebuilt without local history has no rows to derive the
        // next sequence number from; anchor it to the network transaction
        // total at the indexed watermark so numbering stays canonical.
        let anchor = opened
            .tables
            .watermark
            .get(&())
            .expect("failed to initialize RPC index tables")
            .map(|watermark| {
                checkpoint_store
                    .get_checkpoint_by_sequence_number(watermark)
                    .expect("checkpoint store read cannot fail")
                    // Certified checkpoints are never pruned, and a rebuild
                    // would anchor to the same one.
                    .unwrap_or_else(|| {
                        panic!(
                            "the indexed watermark checkpoint {watermark} is missing from the \
                             checkpoint store"
                        )
                    })
                    .network_total_transactions
            })
            .unwrap_or(0);

        // The pruner never retains fewer epochs than its floor, so the
        // backfill must not stop above it either.
        let epochs_to_retain =
            epochs_to_retain.map(|epochs| epochs.max(MIN_EPOCHS_TO_RETAIN_FOR_INDEXES));

        let store = Arc::new(Self::finish_open(
            opened,
            registry,
            groups,
            max_type_length,
            anchor,
            cancelled,
            epochs_to_retain,
        )?);
        store.spawn_history_backfill(authority_store.clone(), checkpoint_store.clone());
        Ok(store)
    }

    /// Opens the store without the init logic of [`Self::new`] — for tests.
    pub fn new_without_init(path: PathBuf, groups: BTreeSet<IndexGroup>) -> Self {
        let opened = Self::open_index_db(&path).expect("unable to open the RPC index database");
        Self::finish_open(
            opened,
            &Registry::default(),
            groups,
            None,
            0,
            Arc::default(),
            None,
        )
        .expect("unable to open the RPC index database")
    }

    /// Whether this store maintains `group`'s tables.
    pub fn serves(&self, group: IndexGroup) -> bool {
        self.groups.contains(&group)
    }

    /// One past the last indexed transaction's sequence number. Sequence
    /// numbers equal network position and genesis is indexed through
    /// checkpoint 0, so this is the total number of transactions.
    ///
    /// The count covers checkpoints staged but not yet committed; a crash
    /// re-derives it from the watermark's checkpoint on the next open.
    pub fn next_sequence_number(&self) -> TxSequenceNumber {
        self.next_sequence_number.load(Ordering::SeqCst)
    }

    /// The `max_type_length` this store was opened with, defaulting to 128.
    pub fn max_type_length(&self) -> u64 {
        self.max_type_length
    }

    fn finish_open(
        opened: OpenedIndexDb,
        registry: &Registry,
        groups: BTreeSet<IndexGroup>,
        max_type_length: Option<u64>,
        next_sequence_number_floor: TxSequenceNumber,
        cancelled: Arc<AtomicBool>,
        epochs_to_retain: Option<u64>,
    ) -> Result<Self, TypedStoreError> {
        let OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        } = opened;
        let history = EpochBuckets::open(
            db,
            HISTORY_CF_PREFIX,
            history_cf_options,
            tables.earliest_retained_epoch.clone(),
            history,
            HistoryBucket::reopen,
        )?;
        let metrics = RpcIndexesMetrics::new(registry);
        let jsonrpc_metrics = JsonRpcMetrics::new(registry);

        Ok(Self {
            tables,
            groups,
            history,
            next_sequence_number: next_sequence_number_floor.into(),
            metrics,
            caches: BalanceCaches::new(),
            jsonrpc_metrics,
            max_type_length: max_type_length.unwrap_or(128),
            pending_updates: Mutex::new(BTreeMap::new()),
            history_backfill_task: Mutex::new(None),
            cancelled,
            epochs_to_retain,
        })
    }

    /// Opens the index database, passing every existing per-epoch history
    /// column family at open with its tuned options: a column family left
    /// for auto-discovery would silently get default options (and its own
    /// block cache).
    fn open_index_db(path: &Path) -> IotaResult<OpenedIndexDb> {
        let db_options = default_db_options().disable_write_throttling();
        let history_cf_options = rpc_index_history::history_cf_options(
            &db_options,
            read_size_from_env(ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB)
                .unwrap_or(DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB),
        );

        let static_tables = IndexStoreTables::describe_tables();
        // A listing failure on an existing database must not pass for "no
        // history": the history buckets would silently be lost to queries
        // and to retention until the next reopen. `CURRENT` marks a
        // directory holding a database rather than a fresh path.
        let existing_cfs = if path.join("CURRENT").exists() {
            list_tables(path.to_path_buf()).map_err(|e| IotaError::Storage(e.to_string()))?
        } else {
            Vec::new()
        };
        let mut epochs = BTreeSet::new();
        let mut opt_cfs: Vec<(String, rocksdb::Options)> = Vec::new();
        for name in static_tables.keys() {
            opt_cfs.push((name.clone(), db_options.options.clone()));
        }
        // Tables of another schema version need no entry here: `open_cf_opts`
        // appends any remaining on-disk column family with default options so
        // RocksDB can open the database at all, and the version mismatch
        // wipes the whole database afterwards.
        for cf_name in &existing_cfs {
            if let Some(epoch) = history_cf_epoch(cf_name) {
                epochs.insert(epoch);
                opt_cfs.push((cf_name.clone(), history_cf_options.clone()));
            }
        }
        let opt_cfs: Vec<(&str, rocksdb::Options)> = opt_cfs
            .iter()
            .map(|(name, options)| (name.as_str(), options.clone()))
            .collect();
        let db = open_cf_opts(
            path,
            Some(db_options.options.clone()),
            MetricConf::new("rpc-index"),
            &opt_cfs,
        )
        .map_err(|e| IotaError::Storage(e.to_string()))?;

        fn map<K, V>(
            db: &Arc<Database>,
            cf_name: &str,
            rw: &ReadWriteOptions,
        ) -> IotaResult<DBMap<K, V>> {
            DBMap::reopen(db, Some(cf_name), rw, false)
                .map_err(|e| IotaError::Storage(format!("cannot open the {cf_name} table: {e}")))
        }
        let tables = IndexStoreTables {
            meta: map(&db, "meta", &db_options.rw_options)?,
            watermark: map(&db, "watermark", &db_options.rw_options)?,
            history_watermark: map(&db, "history_watermark", &db_options.rw_options)?,
            earliest_retained_epoch: map(&db, "earliest_retained_epoch", &db_options.rw_options)?,
            owner: map(&db, "owner", &db_options.rw_options)?,
            dynamic_field: map(&db, "dynamic_field", &db_options.rw_options)?,
            coin: map(&db, "coin", &db_options.rw_options)?,
            package_version: map(&db, "package_version", &db_options.rw_options)?,
        };

        let mut history = BTreeMap::new();
        for epoch in epochs {
            let bucket = HistoryBucket::reopen(&db, &history_cf_name(epoch))?;
            history.insert(epoch, Arc::new(bucket));
        }

        Ok(OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        })
    }

    /// The bucket holding `epoch`'s history, created if absent. Pruned
    /// epochs are refused, see [`EpochBuckets::ensure`].
    fn ensure_history_bucket(&self, epoch: EpochId) -> IotaResult<Arc<HistoryBucket>> {
        self.history
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The transaction's position in the network order and the checkpoint
    /// that committed it, from the newest bucket holding it, `None` if the
    /// digest is not indexed (or its epoch has been pruned). Shared by both
    /// API surfaces: JSON-RPC uses the sequence number, gRPC uses the
    /// checkpoint.
    pub fn lookup_digest(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<(TxSequenceNumber, CheckpointSequenceNumber)>> {
        for bucket in self.history.iter(true) {
            if let Some(found) = bucket.digests.get(digest)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Drops the history of expired epochs, clamped to
    /// [`MIN_EPOCHS_TO_RETAIN_FOR_INDEXES`] — the one pruning entry point,
    /// covering every history table, digests included, since they all live
    /// in the one bucket family. Returns the earliest epoch to retain,
    /// `None` when index pruning is off or there is no history at all.
    ///
    /// A query racing a drop may report an error for the dropped epoch's
    /// rows; a retry no longer sees the bucket. Queries block for the
    /// duration of the drops, so callers on an async runtime must use
    /// `spawn_blocking`.
    pub fn prune(&self) -> IotaResult<Option<EpochId>> {
        let Some(epochs_to_retain) = self.epochs_to_retain else {
            return Ok(None);
        };
        self.history
            .prune(epochs_to_retain)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// Builds and stages the index update of one executed checkpoint, for
    /// every group this store maintains. Nothing is written to the database
    /// until [`Self::commit_update_for_checkpoint`] is called.
    ///
    /// Transactions already present in the index — from a checkpoint replayed
    /// during crash recovery, or one the history backfill got to first — are
    /// skipped, so nothing is counted twice.
    ///
    /// Must be called for each checkpoint in sequence order, so that
    /// transaction sequence numbers follow checkpoint order.
    #[tracing::instrument(
        skip_all,
        fields(checkpoint = checkpoint.checkpoint_summary.sequence_number)
    )]
    pub fn index_checkpoint(&self, checkpoint: &CheckpointData) -> IotaResult {
        let summary = &checkpoint.checkpoint_summary;
        let checkpoint_seq = summary.sequence_number;
        let bucket = self.ensure_history_bucket(summary.epoch)?;

        let digests: Vec<_> = checkpoint
            .transactions
            .iter()
            .map(|tx| *tx.effects.transaction_digest())
            .collect();
        // A transaction's digest row is written whatever the enabled groups
        // are, and always into the bucket of its own epoch, so this one
        // lookup decides for every table whether the transaction is new.
        let already_indexed = bucket.digests.multi_get(&digests)?;
        // The zip below pairs each transaction with its own lookup.
        debug_assert_eq!(digests.len(), already_indexed.len());
        let transactions: Vec<&CheckpointTransaction> = checkpoint
            .transactions
            .iter()
            .zip(already_indexed)
            .filter_map(|(tx, indexed)| indexed.is_none().then_some(tx))
            .collect();

        let index_jsonrpc = self.serves(IndexGroup::JsonRpc);
        let mut batch = self.tables.watermark.batch();
        for tx in &transactions {
            let sequence = self.next_sequence_number.fetch_add(1, Ordering::SeqCst);
            if index_jsonrpc {
                let data =
                    transaction_index_data(&tx.transaction, &tx.effects, tx.events.as_ref())?;
                bucket.index_tx(
                    &mut batch,
                    sequence,
                    checkpoint_seq,
                    summary.timestamp_ms,
                    data,
                )?;
            } else {
                // A gRPC-only store needs nothing beyond the digest row that
                // `index_tx` would write alongside the JSON-RPC history.
                batch.insert_batch_tagged(
                    &bucket.digests,
                    [(*tx.effects.transaction_digest(), (sequence, checkpoint_seq))],
                )?;
            }
        }

        let mut coin_changes = CoinBalanceChanges::default();
        self.tables
            .index_objects(&transactions, &self.groups, &mut batch, &mut coin_changes)?;
        batch.insert_batch(&self.tables.watermark, [((), checkpoint_seq)])?;

        let mut pending_updates = self.pending_updates.lock();
        assert!(
            pending_updates
                .last_key_value()
                .is_none_or(|(seq, _)| *seq < checkpoint_seq),
            "index_checkpoint must be called in order"
        );
        pending_updates.insert(
            checkpoint_seq,
            PendingCheckpointUpdate {
                batch,
                coin_changes,
            },
        );
        Ok(())
    }

    /// Commits the staged update of `checkpoint_seq` and applies the
    /// resulting balance cache maintenance.
    ///
    /// Invariants:
    /// - [`Self::index_checkpoint`] must have been called for the checkpoint.
    /// - Callers must commit each checkpoint in sequence order. This panics if
    ///   `checkpoint_seq` is not the next checkpoint to commit.
    #[tracing::instrument(skip(self))]
    pub fn commit_update_for_checkpoint(
        &self,
        checkpoint_seq: CheckpointSequenceNumber,
    ) -> IotaResult {
        let next_update = self.pending_updates.lock().pop_first();
        let (staged_seq, update) =
            next_update.expect("commit_update_for_checkpoint called without a staged update");
        assert_eq!(
            checkpoint_seq, staged_seq,
            "commit_update_for_checkpoint must be called in order"
        );

        // Holds the affected owners' locks until it is dropped, so a
        // cache-miss read cannot observe the write below without the update
        // that goes with it.
        let cache_updates = self.balance_cache_updates(update.coin_changes);
        let invalidate_caches = invalidate_balance_caches_instead_of_updating();
        if invalidate_caches {
            // Invalidate before the write, so the caches never serve a value
            // older than the database.
            self.invalidate_balance_caches(&cache_updates);
        }

        // The update may stage rows of a history bucket `prune` drops before
        // this write; those rows are discarded instead of failing the write.
        // Only expired epochs can be lost that way: `index_checkpoint`
        // created the bucket of the epoch being executed, so it is the
        // newest one, and `prune` retains at least the newest seven.
        update.batch.write_opt(&drop_tolerant_write_options())?;

        if !invalidate_caches {
            // Merging before the write would apply the delta twice if the
            // write then failed, so the caches trail the database by the
            // duration of the write.
            self.merge_balance_cache_updates(cache_updates);
        }
        Ok(())
    }

    /// Starts the background replay that fills the history tables below the
    /// watermark, if any is pending.
    fn spawn_history_backfill(
        self: &Arc<Self>,
        authority_store: Arc<AuthorityStore>,
        checkpoint_store: Arc<CheckpointStore>,
    ) {
        let store = self.clone();
        let task = tokio::task::spawn_blocking(move || {
            store.metrics.history_backfill_running.set(1);
            if let Err(e) = store.backfill_history(&authority_store, &checkpoint_store) {
                error!("RPC index history backfill stopped: {e}");
            }
            store.metrics.history_backfill_running.set(0);
        });
        *self.history_backfill_task.lock() = Some(task);
    }

    /// Waits for the background history replay to finish — for tests.
    pub async fn wait_for_history_backfill_for_testing(&self) {
        self.join_backfill_task()
            .await
            .expect("history backfill task failed");
    }

    /// Stops the background history replay at its next checkpoint boundary
    /// and waits for it to finish, so shutdown does not block on a full
    /// replay.
    pub async fn shutdown(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Err(e) = self.join_backfill_task().await {
            warn!("the RPC index history backfill task failed: {e}");
        }
    }

    /// Awaits the backfill task, if one is still running.
    async fn join_backfill_task(&self) -> Result<(), tokio::task::JoinError> {
        let task = self.history_backfill_task.lock().take();
        match task {
            Some(task) => task.await,
            None => Ok(()),
        }
    }

    /// Fills the history tables for the checkpoints below
    /// `history_watermark`, newest first, until it reaches the
    /// checkpoint-contents pruner, an epoch [`Self::prune`] removed from the
    /// index, or the configured index retention. The marker commits
    /// atomically with each checkpoint's rows, so an interrupted run resumes
    /// where it stopped.
    /// No-op when the marker is absent (the history was indexed continuously
    /// and is complete). Reports its progress through the
    /// `rpc_index_history_backfill_lowest_replayed_checkpoint` gauge; where
    /// it stopped and why is in the log.
    #[tracing::instrument(skip_all)]
    fn backfill_history(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        let Some(watermark) = self.tables.history_watermark.get(&())? else {
            return Ok(());
        };
        let Some(mut next) = watermark.checked_sub(1) else {
            return Ok(());
        };

        info!("Backfilling RPC index history tables from checkpoint {next} downwards");
        self.metrics
            .history_backfill_lowest_replayed_checkpoint
            .set(watermark as i64);
        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut replayed: u64 = 0;
        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                info!("Stopping the RPC index history backfill at checkpoint {next}: shutdown");
                break;
            }
            // The pruner advances while the backfill runs; re-check the
            // bound so the replay stops before data that is about to
            // disappear.
            let lowest = checkpoint_store
                .get_highest_pruned_checkpoint_seq_number()?
                .map(|c| c.saturating_add(1))
                .unwrap_or(0);
            if next < lowest {
                break;
            }
            let summary = match checkpoint_store.get_checkpoint_by_sequence_number(next)? {
                Some(summary) => summary,
                None => {
                    // The checkpoint pruner can pass the bound check above
                    // mid-iteration; reaching pruned data is a terminal
                    // condition, not a failure.
                    if self.backfill_reached_pruned_data(checkpoint_store, next, None)? {
                        break;
                    }
                    return Err(StorageError::missing(format!("missing checkpoint {next}")));
                }
            };
            let earliest_retained = self.history.earliest_retained();
            if summary.epoch < earliest_retained {
                info!(
                    "Stopping the RPC index history backfill at checkpoint {next}: epoch {} was \
                     pruned from the index, only epochs from {earliest_retained} on are retained",
                    summary.epoch
                );
                break;
            }
            if let Some(horizon) = self.backfill_retention_horizon(summary.epoch) {
                if summary.epoch < horizon {
                    info!(
                        "Stopping the RPC index history backfill at checkpoint {next}: epoch {} \
                         is past the index retention, the next pruning pass would drop it again",
                        summary.epoch
                    );
                    break;
                }
            }
            if let Err(e) =
                self.replay_checkpoint_history(authority_store, checkpoint_store, &summary)
            {
                // See above: the pruners advance while the backfill runs.
                if self.backfill_reached_pruned_data(checkpoint_store, next, Some(summary.epoch))? {
                    break;
                }
                // A pruner deletes a checkpoint's data before it advances
                // the watermark checked above, so the replay can find the
                // data already gone. That is the end of the locally
                // available history, not a failure.
                if e.kind() == StorageErrorKind::Missing {
                    info!(
                        "Stopping the RPC index history backfill at checkpoint {next}: its data \
                         is already gone ({e})"
                    );
                    break;
                }
                return Err(e);
            }
            replayed += 1;
            self.metrics
                .history_backfill_lowest_replayed_checkpoint
                .set(next as i64);
            if last_report.elapsed() >= PROGRESS_REPORT_INTERVAL {
                last_report = Instant::now();
                let remaining = next - lowest;
                let fraction = replayed as f64 / (replayed + remaining) as f64;
                let elapsed = start_time.elapsed();
                let rate = progress_rate(replayed, elapsed);
                let eta = eta_display(elapsed, fraction);
                info!(
                    "Backfilling RPC index history: {:.1}% done (checkpoint {next} down to \
                     {lowest}), {rate:.0} checkpoints/s, ETA ~{eta}",
                    fraction * 100.0,
                );
            }
            let Some(n) = next.checked_sub(1) else {
                break;
            };
            next = n;
        }

        info!(
            "Backfilling {replayed} checkpoints of RPC index history took {} seconds",
            start_time.elapsed().as_secs()
        );
        Ok(())
    }

    /// The lowest epoch the backfill may replay when index pruning is
    /// configured: the horizon [`Self::prune`] enforces, computed against
    /// the newest bucket. The `earliest_retained_epoch` floor alone is not
    /// enough — it is written by the first pruning pass, and until then a
    /// rebuilt store's backfill would replay epochs that pass drops again.
    /// `None` when index pruning is off.
    ///
    /// `current_epoch` stands in for the newest epoch while no bucket
    /// exists yet, on a rebuilt store whose backfill has not committed its
    /// first checkpoint.
    fn backfill_retention_horizon(&self, current_epoch: EpochId) -> Option<EpochId> {
        let epochs_to_retain = self.epochs_to_retain?;
        let newest = self.history.newest_epoch().unwrap_or(current_epoch);
        Some(newest.saturating_sub(epochs_to_retain.saturating_sub(1)))
    }

    /// Whether a pruner removed checkpoint `next`, or the history bucket of
    /// its epoch, while the backfill was working on it — the same bounds the
    /// loop checks before each checkpoint, re-read once the work on it has
    /// failed. `epoch` is the checkpoint's epoch, where it is known. Logs
    /// the reason the backfill stops.
    fn backfill_reached_pruned_data(
        &self,
        checkpoint_store: &CheckpointStore,
        next: CheckpointSequenceNumber,
        epoch: Option<EpochId>,
    ) -> Result<bool, StorageError> {
        if checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .is_some_and(|pruned| next <= pruned)
        {
            info!(
                "Stopping the RPC index history backfill at checkpoint {next}: it was pruned \
                 mid-replay"
            );
            return Ok(true);
        }
        let earliest_retained = self.history.earliest_retained();
        if let Some(epoch) = epoch.filter(|&epoch| epoch < earliest_retained) {
            info!(
                "Stopping the RPC index history backfill at checkpoint {next}: epoch {epoch} was \
                 pruned from the index mid-replay, only epochs from {earliest_retained} on are \
                 retained"
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// Replays one checkpoint into its epoch's history bucket and lowers
    /// `history_watermark` to it, in one atomic batch. The digest rows are
    /// always written, either way: from the transactions, effects and
    /// events when the JSON-RPC group is enabled, which also fills the
    /// other history tables, or straight from the checkpoint's contents
    /// otherwise — a gRPC-only store needs nothing beyond that.
    ///
    /// Transactions are numbered by their position in the network
    /// transaction order, derived from the checkpoint's transaction total,
    /// so numbering stays canonical whatever range is locally available.
    fn replay_checkpoint_history(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        summary: &VerifiedCheckpoint,
    ) -> Result<(), StorageError> {
        let checkpoint_seq = summary.sequence_number;
        let contents = checkpoint_store
            .get_checkpoint_contents(&summary.contents_digest)?
            .ok_or_else(|| {
                StorageError::missing(format!("missing checkpoint contents {checkpoint_seq}"))
            })?;
        let first_sequence_number = summary
            .network_total_transactions
            .checked_sub(contents.iter().len() as u64)
            .ok_or_else(|| {
                StorageError::custom(format!(
                    "checkpoint {checkpoint_seq} has more transactions ({}) than the network \
                     total ({})",
                    contents.iter().len(),
                    summary.network_total_transactions
                ))
            })?;
        let bucket = self
            .ensure_history_bucket(summary.epoch)
            .map_err(|e| StorageError::custom(e.to_string()))?;

        let mut batch = self.tables.watermark.batch();

        if self.serves(IndexGroup::JsonRpc) {
            for (sequence, digests) in (first_sequence_number..).zip(contents.iter()) {
                let transaction = authority_store
                    .get_transaction_block(&digests.transaction)?
                    .ok_or_else(|| {
                        StorageError::missing(format!(
                            "missing transaction {}",
                            digests.transaction
                        ))
                    })?
                    .into_inner();
                let effects = authority_store
                    .get_effects(&digests.effects)
                    .map_err(|e| StorageError::custom(e.to_string()))?
                    .ok_or_else(|| {
                        StorageError::missing(format!("missing effects {}", digests.effects))
                    })?;
                let events = if effects.events_digest().is_some() {
                    Some(
                        authority_store
                            .get_events(&digests.transaction)?
                            .ok_or_else(|| {
                                StorageError::missing(format!(
                                    "missing events {}",
                                    digests.transaction
                                ))
                            })?,
                    )
                } else {
                    None
                };

                let data = transaction_index_data(&transaction, &effects, events.as_ref())
                    .map_err(|e| StorageError::custom(e.to_string()))?;
                bucket
                    .index_tx(
                        &mut batch,
                        sequence,
                        checkpoint_seq,
                        summary.timestamp_ms,
                        data,
                    )
                    .map_err(|e| StorageError::custom(e.to_string()))?;
            }
        } else {
            // A gRPC-only store needs nothing beyond the checkpoint's
            // contents, already local to every node: `index_tx` above would
            // write the same digest rows, but only after fetching
            // transactions, effects and events it has no other use for.
            batch.insert_batch_tagged(
                &bucket.digests,
                (first_sequence_number..)
                    .zip(contents.iter())
                    .map(|(sequence, digests)| (digests.transaction, (sequence, checkpoint_seq))),
            )?;
        }

        batch.insert_batch(&self.tables.history_watermark, [((), checkpoint_seq)])?;
        // A plain WAL-enabled write, not a bulk-ingestion one: the database
        // is serving queries, and the marker must land atomically with the
        // rows. `drop_tolerant_write_options` discards the bucket's rows if
        // `prune` dropped its column family mid-replay; the next loop
        // iteration then stops at the pruned epoch.
        batch
            .write_opt(&drop_tolerant_write_options())
            .map_err(StorageError::from)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../unit_tests/rpc_indexes_tests.rs"]
mod tests;
