// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IndexStore supports creation of various ancillary indexes of state in
//! IotaDataStore. The main user of this data is the explorer.

pub mod grpc_api;
pub mod jsonrpc_api;
pub mod live_scan;
pub mod schema;

use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap, HashSet},
    ops::{Bound, RangeBounds},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use either::Either;
use iota_json_rpc_types::{IotaMoveValue, IotaObjectDataFilter, TransactionFilter};
use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, ObjectReference, Owner, StructTag, TransactionDigest,
    TransactionEffects, TransactionEvents, TransactionEventsDigest, TypeTag, Version,
};
use iota_storage::{mutex_table::MutexTable, sharded_lru::ShardedLruCache};
use iota_types::{
    base_types::{EpochId, ObjectInfo, TxSequenceNumber},
    dynamic_field::{DynamicFieldInfo, DynamicFieldName, visitor as DFV},
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult, UserInputError},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    inner_temporary_store::TxCoins,
    iota_sdk_types_conversions::type_tag_core_to_sdk,
    layout_resolver::LayoutResolver,
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, VerifiedCheckpoint},
    object::{Object, bounded_visitor::BoundedVisitor},
    parse_iota_struct_tag,
    storage::{
        ObjectStore,
        error::{Error as StorageError, Kind as StorageErrorKind},
    },
    transaction::{TransactionAPI, TransactionEnvelope},
};
use itertools::Itertools;
pub use jsonrpc_api::*;
use live_scan::*;
use move_core_types::{
    account_address::AccountAddress, annotated_value as A, identifier::Identifier,
    language_storage::ModuleId,
};
use parking_lot::{ArcMutexGuard, Mutex};
use prometheus_filtered::{
    IntCounter, IntGauge, MetricLevel, Registry, register_int_counter_with_registry,
    register_int_gauge_with_registry,
};
pub use schema::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, info, trace, warn};
use typed_store::{
    DBMapUtils, TypedStoreError,
    database::{Database, drop_tolerant_write_options, wait_for_database_close},
    rocks::{
        DBBatch, DBMap, DBOptions, MetricConf, ReadWriteOptions, TaggedDBMap,
        bulk_ingestion_options, bulk_ingestion_options_split_between, bulk_ingestion_write_options,
        default_db_options, list_tables, open_cf_opts, read_size_from_env, safe_drop_db,
    },
    rocksdb,
    traits::Map,
};

use crate::{
    authority::{AuthorityStore, authority_store_pruner::MIN_EPOCHS_TO_RETAIN_FOR_INDEXES},
    checkpoints::CheckpointStore,
    index_rebuild_cancellation::{RebuildCancelled, is_cancelled},
    par_index_live_object_set::{
        LiveObjectIndexer, PROGRESS_REPORT_INTERVAL, ParMakeLiveObjectIndexer, eta_display,
        progress_rate,
    },
    rpc_index_history::{self, EpochBuckets},
};

type OwnerIndexKey = (Address, ObjectId);

type CoinIndexKey = (Address, String, ObjectId);

type DynamicFieldKey = (ObjectId, ObjectId);

type EventId = (TxSequenceNumber, usize);

type EventIndex = (TransactionEventsDigest, TransactionDigest, u64);

pub const MAX_TX_RANGE_SIZE: u64 = 4096;

pub const MAX_GET_OWNED_OBJECT_SIZE: usize = 256;

/// Subdirectory of the node's database path holding the JSON-RPC index
/// store. The formal-snapshot restore builds the store under the same name,
/// so a restored node opens it in place.
pub const JSONRPC_INDEXES_DIR: &str = "jsonrpc_indexes";

/// Removes the JSON-RPC index database of releases that stored it under
/// `indexes` inside the node's database path. Its content cannot be adopted
/// anyway (see [`IndexStore::new`]), and the store now lives under
/// [`JSONRPC_INDEXES_DIR`].
pub fn remove_legacy_jsonrpc_indexes_dir(db_path: &Path) -> std::io::Result<()> {
    let legacy_dir = db_path.join("indexes");
    if legacy_dir.exists() {
        info!("removing the legacy JSON-RPC index database at {legacy_dir:?}");
        std::fs::remove_dir_all(&legacy_dir)?;
    }
    Ok(())
}

/// Bump this when changing the serialization format or layout of an existing
/// table. A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`.
const CURRENT_DB_VERSION: u64 = 1;

const ENV_VAR_COIN_INDEX_BLOCK_CACHE_SIZE_MB: &str = "COIN_INDEX_BLOCK_CACHE_MB";

const ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB: &str = "JSONRPC_HISTORY_BLOCK_CACHE_MB";

const DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB: usize = 512;

pub const DB_PREFIX_HISTORIC_TXS_SEQ: u8 = 1;

pub const DB_PREFIX_HIST_TXS_BY_MUTATED_OBJECT_ID: u8 = 5;

#[derive(Debug)]
pub struct ObjectIndexChanges {
    pub deleted_owners: Vec<OwnerIndexKey>,
    pub deleted_dynamic_fields: Vec<DynamicFieldKey>,
    pub new_owners: Vec<(OwnerIndexKey, ObjectInfo)>,
    pub new_dynamic_fields: Vec<DynamicFieldKey>,
}

/// A staged index update for one checkpoint, waiting for its in-order commit.
struct PendingCheckpointUpdate {
    batch: DBBatch,
    /// Net coin index change per key (`Some` upsert, `None` delete), used at
    /// commit time to derive balance cache updates from the pre-commit
    /// database state.
    coin_changes: BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
}

pub struct IndexStoreMetrics {
    balance_lookup_from_db: IntCounter,
    balance_lookup_from_total: IntCounter,
    all_balance_lookup_from_db: IntCounter,
    all_balance_lookup_from_total: IntCounter,
    /// Lowest checkpoint the history backfill has replayed so far. The
    /// value reflects only the backfill's own progress: it keeps its final
    /// value after the backfill stops and is not raised when pruning later
    /// drops replayed epochs.
    history_backfill_lowest_replayed_checkpoint: IntGauge,
    /// 1 while the background history backfill is running, 0 otherwise.
    history_backfill_running: IntGauge,
}

/// The `IndexStoreCaches` struct manages `ShardedLruCache` instances to
/// facilitate balance lookups and ownership queries.
pub struct IndexStoreCaches {
    per_coin_type_balance: ShardedLruCache<(Address, TypeTag), IotaResult<TotalBalance>>,
    all_balances: ShardedLruCache<Address, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>>,
    locks: MutexTable<Address>,
}

/// The live-state and marker tables of the JSON-RPC index — everything that
/// is bounded by the live object set or is a singleton. The history tables
/// live in per-epoch column families of the same database ([`HistoryBucket`])
/// so that pruning drops whole epochs instead of deleting rows.
#[derive(DBMapUtils)]
pub struct IndexStoreTables {
    /// A singleton that stores metadata information on the DB.
    ///
    /// A missing `meta` row (a database from before per-checkpoint indexing)
    /// or a version mismatch triggers a full re-index. During a rebuild,
    /// `meta` is written first and `watermark` last, so a crashed rebuild is
    /// re-detected on the next open.
    meta: DBMap<(), MetadataInfo>,

    /// Highest checkpoint sequence number indexed.
    ///
    /// Written inside each checkpoint's batch, so index data and watermark
    /// land atomically. Falling behind `highest_executed_checkpoint` (e.g.
    /// after a formal-snapshot restore, or a period with indexes disabled)
    /// triggers a full re-index via `needs_to_do_initialization`.
    watermark: DBMap<(), CheckpointSequenceNumber>,

    /// Lowest checkpoint whose transactions are in the history tables.
    ///
    /// A rebuild seeds this to one past the watermark (no history yet); the
    /// background replay then works downwards, committing the marker inside
    /// each checkpoint's batch, until it reaches the checkpoint-contents
    /// pruner. Absent on databases that were never rebuilt: their history
    /// has been indexed continuously and is complete.
    history_watermark: DBMap<(), CheckpointSequenceNumber>,

    /// Earliest epoch retained by the last index pruning pass. History
    /// buckets below it are never recreated, and the backfill stops at it
    /// instead of replaying epochs the pruner would drop again.
    earliest_retained_epoch: DBMap<(), EpochId>,

    /// This is an index of object references to currently existing objects,
    /// indexed by the composite key of the Address of their owner and
    /// the object ID of the object. This composite index allows an
    /// efficient iterator to list all objected currently owned
    /// by a specific user, and their object reference.
    owner_index: DBMap<OwnerIndexKey, ObjectInfo>,

    coin_index: DBMap<CoinIndexKey, CoinInfo>,

    /// An index of the currently existing dynamic fields, keyed by the
    /// object ID of their parent and the object ID of the `Field` object.
    /// Allows an efficient iterator to list all dynamic fields of a specific
    /// parent. Only the key is stored; field metadata is resolved on demand
    /// from the object store at query time, so indexing needs no layout
    /// resolution.
    dynamic_field_index: DBMap<DynamicFieldKey, ()>,
}

/// One epoch's history tables, sharing a single per-epoch column family of
/// the index database, distinguished by a tag byte prefixed to every key.
/// Transactions are numbered by network order and epochs partition that
/// order contiguously, so each bucket is a disjoint, epoch-ordered segment
/// of every history table: chaining per-bucket scans in epoch order
/// preserves the global iteration order, and pruning an epoch is one
/// constant-time column-family drop.
pub(crate) struct HistoryBucket {
    /// Ordering of all indexed transactions.
    tx_order: TaggedDBMap<TxSequenceNumber, TransactionDigest>,

    /// Index from transaction digest to sequence number.
    txs_seq: TaggedDBMap<TransactionDigest, TxSequenceNumber>,

    /// Index from iota address to transactions initiated by that address.
    txs_from_addr: TaggedDBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from iota address to transactions that were sent to that address.
    txs_to_addr: TaggedDBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that used that object id as input.
    txs_by_input_object_id: TaggedDBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that modified/created that object
    /// id.
    txs_by_mutated_object_id: TaggedDBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from package id, module and function identifier to transactions
    /// that used that move function call as input.
    txs_by_move_function:
        TaggedDBMap<(ObjectId, String, String, TxSequenceNumber), TransactionDigest>,

    event_order: TaggedDBMap<EventId, EventIndex>,

    event_by_move_module: TaggedDBMap<(ModuleId, EventId), EventIndex>,

    event_by_move_event: TaggedDBMap<(StructTag, EventId), EventIndex>,

    event_by_event_module: TaggedDBMap<(ModuleId, EventId), EventIndex>,

    event_by_sender: TaggedDBMap<(Address, EventId), EventIndex>,

    event_by_time: TaggedDBMap<(u64, EventId), EventIndex>,
}

/// Prefix of the per-epoch history column families; a bucket's family is
/// `{prefix}{epoch}`. On-disk names are the ground truth for which buckets
/// exist.
const HISTORY_CF_PREFIX: &str = "hist_e";

impl IndexStoreTables {
    pub fn owner_index(&self) -> &DBMap<OwnerIndexKey, ObjectInfo> {
        &self.owner_index
    }

    pub fn coin_index(&self) -> &DBMap<CoinIndexKey, CoinInfo> {
        &self.coin_index
    }

    #[cfg(test)]
    pub(crate) fn dynamic_field_index(&self) -> &DBMap<DynamicFieldKey, ()> {
        &self.dynamic_field_index
    }

    /// Opens the tables with tuned bulk-ingestion options (WAL disabled,
    /// unordered writes) for a full rebuild or a formal-snapshot restore.
    /// Writes must be flushed before the database closes, and serving
    /// queries requires a reopen with default options.
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
            MetricConf::new("index"),
            Some(bulk_options.db_options),
            Some(table_config),
        )
    }

    /// Seeds the `meta` row on the first open of an empty database, so a
    /// fresh store on a node with no executed checkpoints needs no rebuild.
    ///
    /// A database written before per-checkpoint indexing has data but no
    /// `meta` row and is deliberately left unseeded, so
    /// `needs_to_do_initialization` wipes and rebuilds it. Its content cannot
    /// be trusted: nodes restored from a formal snapshot wrote a corrupted
    /// owner index and non-canonical transaction numbering into it.
    fn seed_meta(&self) -> IotaResult {
        if self.meta.get(&())?.is_some() {
            return Ok(());
        }
        if self.owner_index.is_empty() {
            self.meta.insert(
                &(),
                &MetadataInfo {
                    version: CURRENT_DB_VERSION,
                },
            )?;
        }
        Ok(())
    }

    /// Whether the store must be wiped and rebuilt. Read errors propagate:
    /// a transient error must fail the open rather than silently wipe a
    /// healthy store or adopt a stale one.
    fn needs_to_do_initialization(&self, checkpoint_store: &CheckpointStore) -> IotaResult<bool> {
        let schema_mismatch = match self.meta.get(&())? {
            Some(metadata) => metadata.version != CURRENT_DB_VERSION,
            None => true,
        };

        Ok(schema_mismatch || self.is_indexed_watermark_out_of_date(checkpoint_store)?)
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
            // A rebuild and a restore both write the watermark only once
            // their data is durable, so data without one comes from a build
            // that was cut short — including when nothing is executed
            // locally and the comparison below has nothing to outrun. An
            // empty store is the fresh one `seed_meta` covers. Scanned rather
            // than `is_empty`, which reads an unreadable index as non-empty
            // and would wipe a healthy store on a transient read error.
            let has_data = self.owner_index.safe_iter().next().transpose()?.is_some();
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
        // checkpoints writes nothing but the watermark (see the digest check
        // in `index_checkpoint`).
        Ok(watermark < executed)
    }

    /// Rebuilds the live-state tables, for the cases
    /// `needs_to_do_initialization` covers (fresh DB, schema mismatch,
    /// crashed mid-init, or the index watermark falling behind
    /// `highest_executed_checkpoint`). The on-disk DB needs to be wiped
    /// before this is called, so `init` always starts from an empty store.
    ///
    /// Writes only `meta`: the caller adopts the rebuild by writing the
    /// watermarks once the WAL-less bulk writes are flushed. Returns the
    /// highest executed checkpoint to anchor them to.
    #[tracing::instrument(skip_all)]
    fn init(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        batch_size_limit: usize,
        cancelled: &AtomicBool,
    ) -> Result<Option<CheckpointSequenceNumber>, StorageError> {
        info!("Initializing JSON-RPC indexes");

        // Written before the flush, the watermarks would be WAL-durable over
        // unflushed data, and a crash before the flush would leave a store
        // the next open adopts as complete.
        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )?;

        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;

        // Live-state tables from the current live object set. The history
        // tables are not built here: `backfill_history` fills them in the
        // background once the node is up, resuming from `history_watermark`.
        self.index_live_object_set(authority_store, batch_size_limit, cancelled)?;

        info!("Finished initializing JSON-RPC indexes");

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

    /// Rebuilds the live-state indexes (owner, coin, dynamic field) by
    /// scanning the current live object set in parallel.
    fn index_live_object_set(
        &self,
        authority_store: &AuthorityStore,
        batch_size_limit: usize,
        cancelled: &AtomicBool,
    ) -> Result<(), StorageError> {
        let indexer = JsonRpcLiveObjectSetIndexer {
            tables: self,
            batch_size_limit,
        };
        crate::par_index_live_object_set::par_index_live_object_set(
            authority_store,
            &indexer,
            cancelled,
        )
    }

    fn index_coin(
        &self,
        digest: &TransactionDigest,
        batch: &mut DBBatch,
        object_index_changes: &ObjectIndexChanges,
        tx_coins: TxCoins,
        coin_changes: &mut BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
    ) -> IotaResult {
        let (input_coins, written_coins) = tx_coins;
        // 1. Delete old owner if the object is deleted or transferred to a new owner,
        // by looking at `object_index_changes.deleted_owners`.
        let coin_delete_keys = object_index_changes
            .deleted_owners
            .iter()
            .filter_map(|(owner, obj_id)| {
                // Not every deleted owner entry is a coin. Skip the ones that aren't.
                let object = input_coins.get(obj_id).or(written_coins.get(obj_id))?;
                let coin_type_tag = object.opt_coin_type().unwrap_or_else(|| {
                    panic!(
                        "object_id: {obj_id} is not a coin type, input_coins: {input_coins:?}, written_coins: {written_coins:?}, tx_digest: {digest}"
                    )
                });
                let key = (*owner, coin_type_tag.to_string(), *obj_id);
                coin_changes.insert(key.clone(), (coin_type_tag.clone(), None));
                Some(key)
            }).collect::<Vec<_>>();
        trace!(
            tx_digest=?digest,
            "coin_delete_keys: {:?}",
            coin_delete_keys,
        );
        batch.delete_batch(&self.coin_index, coin_delete_keys)?;

        // 2. Upsert new owner, by looking at `object_index_changes.new_owners`.
        // For a object to appear in `new_owners`, it must be owned by `Owner::Address`
        // after the tx. It also must not be deleted, hence appear in
        // written_coins. Here the coin could be transferred to a new address,
        // to simply have the metadata changed (digest, balance etc) due to a
        // successful or failed transaction.
        let coin_add_keys = object_index_changes
        .new_owners
        .iter()
        .filter_map(|((owner, obj_id), obj_info)| {
            // If it's not in written_coins, then it's not a coin. Skip it.
            let obj = written_coins.get(obj_id)?;
            let coin_type_tag = obj.opt_coin_type().cloned().unwrap_or_else(|| {
                panic!(
                    "object_id: {obj_id} in written_coins is not a coin type, written_coins: {written_coins:?}, tx_digest: {digest}"
                )
            });
            let coin = obj.as_coin_maybe().unwrap_or_else(|| {
                panic!(
                    "object_id: {obj_id} in written_coins cannot be deserialized as a Coin, written_coins: {written_coins:?}, tx_digest: {digest}"
                )
            });
            let coin_info = CoinInfo {
                version: obj_info.version,
                digest: obj_info.digest,
                balance: coin.balance.value(),
                previous_transaction: *digest,
            };
            let key = (*owner, coin_type_tag.to_string(), *obj_id);
            coin_changes.insert(key.clone(), (coin_type_tag, Some(coin_info.clone())));
            Some((key, coin_info))
        }).collect::<Vec<_>>();
        trace!(
            tx_digest=?digest,
            "coin_add_keys: {:?}",
            coin_add_keys,
        );

        batch.insert_batch(&self.coin_index, coin_add_keys)?;

        Ok(())
    }

    /// Appends one transaction's owner, dynamic-field, and coin index rows to
    /// a checkpoint's batch.
    fn index_object_changes(
        &self,
        batch: &mut DBBatch,
        coin_changes: &mut BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
        digest: &TransactionDigest,
        object_index_changes: ObjectIndexChanges,
        tx_coins: TxCoins,
    ) -> IotaResult {
        self.index_coin(digest, batch, &object_index_changes, tx_coins, coin_changes)?;

        batch.delete_batch(&self.owner_index, object_index_changes.deleted_owners)?;
        batch.delete_batch(
            &self.dynamic_field_index,
            object_index_changes.deleted_dynamic_fields,
        )?;

        batch.insert_batch(&self.owner_index, object_index_changes.new_owners)?;

        batch.insert_batch(
            &self.dynamic_field_index,
            object_index_changes
                .new_dynamic_fields
                .into_iter()
                .map(|key| (key, ())),
        )?;

        Ok(())
    }
}

/// The `IndexStore` enables users to access and manage indexed transaction
/// data, including ownership and balance information for different objects and
/// coins.
pub struct IndexStore {
    next_sequence_number: AtomicU64,
    tables: IndexStoreTables,
    /// The retained history buckets.
    history: EpochBuckets<HistoryBucket>,
    caches: IndexStoreCaches,
    metrics: Arc<IndexStoreMetrics>,
    max_type_length: u64,
    pending_updates: Mutex<BTreeMap<CheckpointSequenceNumber, PendingCheckpointUpdate>>,
    history_backfill_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Stops the startup rebuild and the background history backfill.
    cancelled: Arc<AtomicBool>,
    /// How many epochs of history the pruner is configured to retain
    /// (`num_epochs_to_retain_for_indexes`); bounds the history backfill so
    /// it does not replay epochs the next prune pass would drop again.
    /// `None` when index pruning is off.
    epochs_to_retain: Option<u64>,
}

/// The pieces produced by opening the index database.
struct OpenedIndexDb {
    tables: IndexStoreTables,
    db: Arc<Database>,
    history_cf_options: rocksdb::Options,
    /// Every history bucket found on disk, before the retention floor is
    /// applied by [`IndexStore::drop_pruned_buckets`].
    history: BTreeMap<EpochId, Arc<HistoryBucket>>,
}

fn coin_index_table_default_config() -> DBOptions {
    default_db_options()
        .optimize_for_write_throughput()
        .optimize_for_read(
            read_size_from_env(ENV_VAR_COIN_INDEX_BLOCK_CACHE_SIZE_MB).unwrap_or(5 * 1024),
        )
        .disable_write_throttling()
}

/// Coin objects touched by the transaction, as inputs for the coin index.
fn transaction_coins(tx: &CheckpointTransaction) -> TxCoins {
    let input_coins = tx
        .input_objects
        .iter()
        .filter(|o| o.is_coin())
        .map(|o| (o.id(), o.clone()))
        .collect();
    let written_coins = tx
        .output_objects
        .iter()
        .filter(|o| o.is_coin())
        .map(|o| (o.id(), o.clone()))
        .collect();
    (input_coins, written_coins)
}

fn process_object_index(tx: &CheckpointTransaction) -> ObjectIndexChanges {
    let mut deleted_owners = vec![];
    let mut deleted_dynamic_fields = vec![];
    for removed_object in tx.removed_objects_pre_version() {
        match removed_object.owner {
            Owner::Address(addr) => deleted_owners.push((addr, removed_object.id())),
            Owner::Object(object_id) => {
                deleted_dynamic_fields.push((object_id, removed_object.id()))
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }
    }

    let mut new_owners = vec![];
    let mut new_dynamic_fields = vec![];

    for (object, old_object) in tx.changed_objects() {
        // For mutated objects, delete the old index entry if the owner changed.
        if let Some(old_object) = old_object {
            if old_object.owner != object.owner {
                match old_object.owner {
                    Owner::Address(addr) => deleted_owners.push((addr, old_object.id())),
                    Owner::Object(object_id) => {
                        deleted_dynamic_fields.push((object_id, old_object.id()))
                    }
                    Owner::Shared(_) | Owner::Immutable => {}
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }
        }

        match object.owner {
            Owner::Address(addr) => {
                new_owners.push(((addr, object.id()), ObjectInfo::from_object(object)));
            }
            Owner::Object(parent) => {
                if is_dynamic_field(object) {
                    new_dynamic_fields.push((parent, object.id()))
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }
    }

    ObjectIndexChanges {
        deleted_owners,
        deleted_dynamic_fields,
        new_owners,
        new_dynamic_fields,
    }
}

/// The JSON-RPC index tables opened for a formal-snapshot restore.
///
/// Hands out per-partition indexers that tee the restore's live objects into
/// the live-state tables, and a finalize step that seeds the markers so a
/// node opens the store in place instead of rebuilding. Mirrors the gRPC
/// index restore; the dynamic-field index stores only field keys, so the tee
/// needs no layout resolution and no ordering guarantee within the object
/// stream.
pub struct JsonRpcIndexRestorer {
    tables: IndexStoreTables,
    batch_size_limit: usize,
}

/// Divisor for the JSON-RPC index's share of the bulk-ingestion memtable
/// budget during a formal-snapshot restore, which writes the perpetual
/// tables and the gRPC index store alongside it on default options.
const RESTORE_CONCURRENT_STORES: usize = 2;

impl JsonRpcIndexRestorer {
    /// Returns an indexer for one partition of the snapshot's live objects.
    pub fn partition_indexer(&self) -> JsonRpcPartitionIndexer<'_> {
        JsonRpcPartitionIndexer(JsonRpcLiveObjectIndexer {
            tables: &self.tables,
            batch: self.tables.owner_index.batch(),
            batch_size_limit: self.batch_size_limit,
        })
    }
}

/// Indexer for one partition of a formal-snapshot restore's live objects.
pub struct JsonRpcPartitionIndexer<'a>(JsonRpcLiveObjectIndexer<'a>);

impl IndexStore {
    /// Opens the store, wiping it and rebuilding the live-state tables first
    /// when the indexes are missing or stale (e.g. on the first start after
    /// a formal-snapshot restore, or after running with indexes disabled).
    /// Databases written before per-checkpoint indexing are wiped and
    /// rebuilt as well: nodes restored from a formal snapshot wrote
    /// corrupted data into them.
    ///
    /// The history tables are filled by a background replay after this
    /// returns; until it finishes, history-backed queries cover a growing
    /// range of recent checkpoints, as on a pruned node. When index pruning
    /// is configured, `num_epochs_to_retain` bounds the replay to the epochs
    /// the pruner would retain.
    ///
    /// Setting `cancelled` abandons a rebuild running here and the background
    /// replay, and fails the open: the store is left unadopted for the next
    /// open to rebuild, and must not serve reads in the meantime.
    pub async fn new(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
        num_epochs_to_retain: Option<u64>,
        authority_store: &Arc<AuthorityStore>,
        checkpoint_store: &Arc<CheckpointStore>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Arc<Self>, StorageError> {
        // An unopenable database would crash-loop the node with no way to
        // self-heal; wipe and rebuild it like a stale one — but only after
        // one retry, so a transient error does not destroy a healthy store.
        // Read errors on an openable database stay fatal instead (see
        // `needs_to_do_initialization`): its data is intact, so a restart
        // retries without paying for a rebuild.
        let mut opened = match Self::open_index_db(&path) {
            Ok(opened) => Some(opened),
            Err(first) => {
                warn!("unable to open the JSON-RPC index database, retrying once: {first}");
                match Self::open_index_db(&path) {
                    Ok(opened) => Some(opened),
                    Err(e) => {
                        warn!(
                            "unable to open the JSON-RPC index database, wiping and rebuilding: \
                             {e}"
                        );
                        None
                    }
                }
            }
        };

        if let Some(opened) = &opened {
            opened
                .tables
                .seed_meta()
                .expect("failed to initialize index tables");
        }

        // Node startup blocks on a rebuild before any RPC surface exists;
        // the gauge tells operators (and their probes) that the node is
        // rebuilding, not hung. Registered unconditionally, so "not
        // rebuilding" reads as 0 rather than a missing series.
        let rebuild_gauge = register_int_gauge_with_registry!(
            "jsonrpc_index_rebuild_in_progress",
            "1 while the JSON-RPC index store is being rebuilt at startup",
            registry;
            MetricLevel::Warn,
        )
        .expect("failed to register the JSON-RPC index rebuild gauge");

        let needs_initialization = opened.as_ref().is_none_or(|opened| {
            opened
                .tables
                .needs_to_do_initialization(checkpoint_store)
                .expect("failed to determine whether the JSON-RPC index needs a rebuild")
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
                    warn!("unable to destroy the old JSON-RPC index database ({e}), deleting it");
                    std::fs::remove_dir_all(&path)
                        .expect("unable to delete the old JSON-RPC index database");
                }

                // Open the empty DB with tuned bulk ingestion options to
                // speed up the initial indexing. The DB is reopened with default options
                // afterwards.
                IndexStoreTables::open_for_bulk_ingestion(path.clone(), 1)
            };
            let batch_size_limit = bulk_ingestion_options().batch_size_limit;

            // The rebuild scans and writes RocksDB for a long time; keep it
            // off the async runtime's worker threads.
            let (init_tables, initialized) = tokio::task::spawn_blocking({
                let authority_store = authority_store.clone();
                let checkpoint_store = checkpoint_store.clone();
                let cancelled = cancelled.clone();
                move || {
                    let mut init_tables = init_tables;
                    let initialized = init_tables.init(
                        &authority_store,
                        &checkpoint_store,
                        batch_size_limit,
                        &cancelled,
                    );
                    (init_tables, initialized)
                }
            })
            .await
            .expect("JSON-RPC index initialization task failed");

            match initialized {
                // A crash before this point re-detects the rebuild on the next
                // open (no watermark), never adopts a half-flushed store.
                Ok(highest_executed_checkpoint) => init_tables
                    .adopt_bulk_ingestion(highest_executed_checkpoint)
                    .expect("unable to adopt the rebuilt JSON-RPC index"),
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
                        warn!("the cancelled JSON-RPC index rebuild left its database open");
                    }
                    return Err(RebuildCancelled::error(format!(
                        "the JSON-RPC index rebuild was cancelled by shutdown: {e}"
                    )));
                }
                Err(e) => panic!("unable to initialize JSON-RPC index: {e}"),
            }

            let weak_db = Arc::downgrade(&init_tables.meta.db);
            drop(init_tables);
            if !wait_for_database_close(weak_db).await {
                panic!("unable to reopen DB after indexing");
            }

            // Reopen the DB with default options (eg without `unordered_write`s enabled)
            let reopened = Self::open_index_db(&path)
                .expect("unable to reopen the JSON-RPC index database after the rebuild");

            // Smoke test: the reopened database is readable and carries the
            // schema version the rebuild wrote.
            let stored_version = reopened
                .tables
                .meta
                .get(&())
                .expect("reopened JSON-RPC index DB should expose readable metadata")
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

        // A store rebuilt without local history has no rows to derive the next
        // sequence number from; anchor it to the network transaction total at
        // the indexed watermark so numbering stays canonical.
        let anchor = opened
            .tables
            .watermark
            .get(&())
            .expect("failed to initialize index tables")
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
            num_epochs_to_retain.map(|epochs| epochs.max(MIN_EPOCHS_TO_RETAIN_FOR_INDEXES));

        let store = Arc::new(Self::finish_open(
            opened,
            registry,
            max_type_length,
            anchor,
            cancelled,
            epochs_to_retain,
        )?);
        store.spawn_history_backfill(authority_store.clone(), checkpoint_store.clone());
        Ok(store)
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
                error!("JSON-RPC index history backfill stopped: {e}");
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
            warn!("the JSON-RPC index history backfill task failed: {e}");
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
    /// `history_backfill_lowest_replayed_checkpoint` gauge; where it stopped
    /// and why is in the log.
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

        info!("Backfilling JSON-RPC history tables from checkpoint {next} downwards");
        self.metrics
            .history_backfill_lowest_replayed_checkpoint
            .set(watermark as i64);
        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut replayed: u64 = 0;
        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                info!("Stopping the JSON-RPC history backfill at checkpoint {next}: shutdown");
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
                    "Stopping the JSON-RPC history backfill at checkpoint {next}: epoch {} was \
                     pruned from the index, only epochs from {earliest_retained} on are retained",
                    summary.epoch
                );
                break;
            }
            if let Some(horizon) = self.backfill_retention_horizon(summary.epoch) {
                if summary.epoch < horizon {
                    info!(
                        "Stopping the JSON-RPC history backfill at checkpoint {next}: epoch {} is \
                         past the index retention, the next pruning pass would drop it again",
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
                        "Stopping the JSON-RPC history backfill at checkpoint {next}: its data \
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
                    "Backfilling JSON-RPC history: {:.1}% done (checkpoint {next} down to {lowest}), {rate:.0} checkpoints/s, ETA ~{eta}",
                    fraction * 100.0,
                );
            }
            let Some(n) = next.checked_sub(1) else {
                break;
            };
            next = n;
        }

        info!(
            "Backfilling {replayed} checkpoints of JSON-RPC history took {} seconds",
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
                "Stopping the JSON-RPC history backfill at checkpoint {next}: it was pruned \
                 mid-replay"
            );
            return Ok(true);
        }
        let earliest_retained = self.history.earliest_retained();
        if let Some(epoch) = epoch.filter(|&epoch| epoch < earliest_retained) {
            info!(
                "Stopping the JSON-RPC history backfill at checkpoint {next}: epoch {epoch} was \
                 pruned from the index mid-replay, only epochs from {earliest_retained} on are \
                 retained"
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// Replays one checkpoint into its epoch's history bucket and lowers
    /// `history_watermark` to it, in one atomic batch. Transactions are
    /// numbered by their position in the network transaction order, derived
    /// from the checkpoint's transaction total, so numbering stays canonical
    /// whatever range is locally available.
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
        for (sequence, digests) in (first_sequence_number..).zip(contents.iter()) {
            let transaction = authority_store
                .get_transaction_block(&digests.transaction)?
                .ok_or_else(|| {
                    StorageError::missing(format!("missing transaction {}", digests.transaction))
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
                            StorageError::missing(format!("missing events {}", digests.transaction))
                        })?,
                )
            } else {
                None
            };

            let data = transaction_index_data(&transaction, &effects, events.as_ref())
                .map_err(|e| StorageError::custom(e.to_string()))?;
            bucket
                .index_tx(&mut batch, sequence, summary.timestamp_ms, data)
                .map_err(|e| StorageError::custom(e.to_string()))?;
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

    /// Opens the store without the init logic of [`Self::new`] — for tests.
    pub fn new_without_init(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
    ) -> Self {
        let opened =
            Self::open_index_db(&path).expect("unable to open the JSON-RPC index database");
        Self::finish_open(opened, registry, max_type_length, 0, Arc::default(), None)
            .expect("unable to read the JSON-RPC index retention floor")
    }

    fn finish_open(
        opened: OpenedIndexDb,
        registry: &Registry,
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
        // Assembled before the scan below, so a bucket the retention floor
        // excludes cannot seed the transaction numbering.
        let history = EpochBuckets::open(
            db,
            "JSON-RPC index history",
            HISTORY_CF_PREFIX,
            history_cf_options,
            tables.earliest_retained_epoch.clone(),
            history,
            HistoryBucket::reopen,
        )?;
        let metrics = IndexStoreMetrics::new(registry);
        let caches = IndexStoreCaches {
            per_coin_type_balance: ShardedLruCache::new(1_000_000, 1000),
            all_balances: ShardedLruCache::new(1_000_000, 1000),
            locks: MutexTable::new(128),
        };
        // The newest bucket can be present but empty (a crash between
        // `create_cf` and its first committed batch), so scan the buckets
        // newest to oldest for the last indexed row.
        let mut next_from_history = None;
        for bucket in history.iter(true) {
            if let Some((seq, _)) = bucket
                .tx_order
                .safe_range_iter_reversed(..)
                .next()
                .transpose()?
            {
                next_from_history = Some(seq + 1);
                break;
            }
        }
        let next_sequence_number = next_from_history
            .unwrap_or(0)
            .max(next_sequence_number_floor)
            .into();

        Ok(Self {
            tables,
            history,
            next_sequence_number,
            caches,
            metrics: Arc::new(metrics),
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
        let coin_options = coin_index_table_default_config();
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
        let mut epochs = std::collections::BTreeSet::new();
        let mut opt_cfs: Vec<(String, rocksdb::Options)> = Vec::new();
        for name in static_tables.keys() {
            let options = if name == "coin_index" {
                coin_options.options.clone()
            } else {
                db_options.options.clone()
            };
            opt_cfs.push((name.clone(), options));
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
            MetricConf::new("index"),
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
            owner_index: map(&db, "owner_index", &db_options.rw_options)?,
            coin_index: map(&db, "coin_index", &coin_options.rw_options)?,
            dynamic_field_index: map(&db, "dynamic_field_index", &db_options.rw_options)?,
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

    /// Drops the history of expired epochs, see [`EpochBuckets::prune`]:
    /// with `epochs_to_retain` = N, the buckets of the newest N epochs are
    /// kept and every older bucket is dropped wholesale. Returns the
    /// earliest epoch to retain, `None` when there is no history at all.
    ///
    /// A query racing a drop may report an error for the dropped epoch's
    /// rows; a retry no longer sees the bucket. Queries block for the
    /// duration of the drops, so callers on an async runtime must use
    /// `spawn_blocking`.
    pub fn prune(&self, epochs_to_retain: u64) -> IotaResult<Option<EpochId>> {
        self.history
            .prune(epochs_to_retain)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    pub fn tables(&self) -> &IndexStoreTables {
        &self.tables
    }

    /// The underlying database — for tests that simulate on-disk states.
    #[cfg(test)]
    fn database_for_testing(&self) -> &Arc<Database> {
        &self.tables.meta.db
    }

    /// Builds and stages the index batch for one executed checkpoint.
    ///
    /// Transactions already present in the index (from a checkpoint replayed
    /// during crash recovery) are skipped. Nothing is written to the database
    /// until
    /// [`Self::commit_update_for_checkpoint`] is called.
    ///
    /// Must be called for each checkpoint in sequence order, so that
    /// transaction sequence numbers follow checkpoint order.
    pub fn index_checkpoint(&self, checkpoint: &CheckpointData) -> IotaResult {
        let checkpoint_seq = checkpoint.checkpoint_summary.sequence_number;
        let timestamp_ms = checkpoint.checkpoint_summary.timestamp_ms;
        let bucket = self.ensure_history_bucket(checkpoint.checkpoint_summary.epoch)?;

        let digests: Vec<_> = checkpoint
            .transactions
            .iter()
            .map(|tx| *tx.effects.transaction_digest())
            .collect();
        // A replayed checkpoint's transactions were indexed into the same
        // epoch's bucket, so only that bucket needs to be consulted.
        let already_indexed = bucket.txs_seq.multi_get(&digests)?;
        // The zip below pairs each transaction with its own lookup.
        debug_assert_eq!(digests.len(), already_indexed.len());

        let mut batch = self.tables.watermark.batch();
        let mut coin_changes = BTreeMap::new();
        for (tx, indexed_seq) in checkpoint.transactions.iter().zip(already_indexed) {
            if indexed_seq.is_some() {
                continue;
            }
            let data = transaction_index_data(&tx.transaction, &tx.effects, tx.events.as_ref())?;
            let digest = data.digest;
            let sequence = self.next_sequence_number.fetch_add(1, Ordering::SeqCst);
            bucket.index_tx(&mut batch, sequence, timestamp_ms, data)?;

            let object_index_changes = process_object_index(tx);
            let tx_coins = transaction_coins(tx);
            self.tables.index_object_changes(
                &mut batch,
                &mut coin_changes,
                &digest,
                object_index_changes,
                tx_coins,
            )?;
        }
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

    /// Commits the staged update for the provided checkpoint and applies the
    /// resulting balance cache maintenance.
    ///
    /// Invariants:
    /// - `index_checkpoint` must have been called for the provided checkpoint
    /// - Callers of this function must ensure that it is called for each
    ///   checkpoint in sequential order. This will panic if the provided
    ///   checkpoint does not match the expected next checkpoint to commit.
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

        let cache_updates = self.balance_cache_updates(update.coin_changes)?;

        let invalidate_caches =
            read_size_from_env(ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE).unwrap_or(0) > 0;

        if invalidate_caches {
            // Invalidate cache before writing to db so we always serve latest values
            self.invalidate_per_coin_type_cache(
                cache_updates
                    .per_coin_type_balance_changes
                    .iter()
                    .map(|x| x.0.clone()),
            )?;
            self.invalidate_all_balance_cache(
                cache_updates.all_balance_changes.iter().map(|x| x.0),
            )?;
        }

        // The update may stage rows of a history bucket `prune` drops before
        // this write; those rows are discarded instead of failing the write.
        // Only expired epochs can be lost that way: `index_checkpoint`
        // created the bucket of the epoch being executed, so it is the
        // newest one, and `prune` retains at least the newest seven.
        update.batch.write_opt(&drop_tolerant_write_options())?;

        if !invalidate_caches {
            // We cannot update the cache before updating the db or else on failing to write
            // to db we will update the cache twice). However, this only means
            // cache is eventually consistent with the db (within a very short
            // delay)
            self.update_per_coin_type_cache(cache_updates.per_coin_type_balance_changes)?;
            self.update_all_balance_cache(cache_updates.all_balance_changes)?;
        }
        Ok(())
    }

    /// One past the last indexed transaction's sequence number. Sequence
    /// numbers equal network position and genesis is indexed through
    /// checkpoint 0, so this is the total number of transactions.
    ///
    /// The count covers checkpoints staged but not yet committed; a crash
    /// re-derives it from the committed rows on the next open.
    pub fn next_sequence_number(&self) -> TxSequenceNumber {
        self.next_sequence_number.load(Ordering::SeqCst)
    }

    pub fn get_transaction_seq(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TxSequenceNumber>> {
        // An exact-key probe over the buckets, newest first; a miss in a
        // sealed bucket is answered by its in-memory bloom filters.
        for bucket in self.history_buckets(true) {
            if let Some(seq) = bucket.txs_seq.get(digest)? {
                return Ok(Some(seq));
            }
        }
        Ok(None)
    }

    pub fn get_owned_coins_iterator(
        coin_index: &DBMap<CoinIndexKey, CoinInfo>,
        owner: Address,
        coin_type_tag: Option<String>,
    ) -> IotaResult<impl Iterator<Item = (String, ObjectId, CoinInfo)> + '_> {
        let all_coins = coin_type_tag.is_none();
        let starting_coin_type =
            coin_type_tag.unwrap_or_else(|| String::from_utf8([0u8].to_vec()).unwrap());
        Ok(coin_index
            .safe_iter_with_bounds(
                Some((owner, starting_coin_type.clone(), ObjectId::ZERO)),
                None,
            )
            .map(|result| result.expect("iterator db error"))
            .take_while(move |((addr, coin_type, _), _)| {
                if addr != &owner {
                    return false;
                }
                if !all_coins && &starting_coin_type != coin_type {
                    return false;
                }
                true
            })
            .map(|((_, coin_type, obj_id), coin)| (coin_type, obj_id, coin)))
    }

    pub fn get_owned_coins_iterator_with_cursor(
        &self,
        owner: Address,
        cursor: (String, ObjectId),
        limit: usize,
        one_coin_type_only: bool,
    ) -> IotaResult<impl Iterator<Item = (String, ObjectId, CoinInfo)> + '_> {
        let (starting_coin_type, starting_object_id) = cursor;
        Ok(self
            .tables
            .coin_index
            .safe_iter_with_bounds(
                Some((owner, starting_coin_type.clone(), starting_object_id)),
                None,
            )
            .map(|result| result.expect("iterator db error"))
            .filter(move |((_, _, obj_id), _)| obj_id != &starting_object_id)
            .enumerate()
            .take_while(move |(index, ((addr, coin_type, _), _))| {
                if *index >= limit {
                    return false;
                }
                if addr != &owner {
                    return false;
                }
                if one_coin_type_only && &starting_coin_type != coin_type {
                    return false;
                }
                true
            })
            .map(|(_, ((_, coin_type, obj_id), coin))| (coin_type, obj_id, coin)))
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // We are checkpointing the whole db
        self.tables.meta.checkpoint_db(path).map_err(Into::into)
    }

    /// Read all balances for a `Address` from the backend database
    pub fn get_all_balances_from_db(
        metrics: Arc<IndexStoreMetrics>,
        coin_index: DBMap<CoinIndexKey, CoinInfo>,
        owner: Address,
    ) -> IotaResult<Arc<HashMap<TypeTag, TotalBalance>>> {
        metrics.all_balance_lookup_from_db.inc();
        let mut balances: HashMap<TypeTag, TotalBalance> = HashMap::new();
        let coins = Self::get_owned_coins_iterator(&coin_index, owner, None)?
            .chunk_by(|(coin_type, _obj_id, _coin)| coin_type.clone());
        for (coin_type, coins) in &coins {
            let mut total_balance = 0i128;
            let mut coin_object_count = 0;
            for (_coin_type, _obj_id, coin_info) in coins {
                total_balance += coin_info.balance as i128;
                coin_object_count += 1;
            }

            if coin_object_count == 0 {
                // we do not want to return coins with 0 balance
                continue;
            }

            let coin_type = TypeTag::Struct(Box::new(parse_iota_struct_tag(&coin_type).map_err(
                |e| IotaError::Execution(format!("Failed to parse event sender address: {e:?}")),
            )?));
            balances.insert(
                coin_type,
                TotalBalance {
                    num_coins: coin_object_count,
                    balance: total_balance,
                },
            );
        }

        Ok(Arc::new(balances))
    }

    fn invalidate_per_coin_type_cache(
        &self,
        keys: impl IntoIterator<Item = (Address, TypeTag)>,
    ) -> IotaResult {
        self.caches.per_coin_type_balance.batch_invalidate(keys);
        Ok(())
    }

    fn invalidate_all_balance_cache(
        &self,
        addresses: impl IntoIterator<Item = Address>,
    ) -> IotaResult {
        self.caches.all_balances.batch_invalidate(addresses);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../unit_tests/rpc_indexes_tests.rs"]
mod tests;
