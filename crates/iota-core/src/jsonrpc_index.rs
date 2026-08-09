// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IndexStore supports creation of various ancillary indexes of state in
//! IotaDataStore. The main user of this data is the explorer.

use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap, HashSet},
    ops::{Bound, RangeBounds},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
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
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber},
    object::{Object, bounded_visitor::BoundedVisitor},
    parse_iota_struct_tag,
    storage::{ObjectStore, error::Error as StorageError},
    transaction::{TransactionAPI, TransactionEnvelope},
};
use itertools::Itertools;
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
};
use parking_lot::{ArcMutexGuard, Mutex, RwLock};
use prometheus_filtered::{IntCounter, Registry, register_int_counter_with_registry};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, info, trace, warn};
use typed_store::{
    DBMapUtils, TypedStoreError,
    database::{Database, wait_for_database_close},
    rocks::{
        DBBatch, DBMap, DBOptions, MetricConf, ReadWriteOptions, TaggedDBMap,
        bulk_ingestion_options, bulk_ingestion_write_options, default_db_options, list_tables,
        open_cf_opts, read_size_from_env, safe_drop_db,
    },
    rocksdb,
    traits::Map,
};

use crate::{
    authority::AuthorityStore,
    checkpoints::CheckpointStore,
    par_index_live_object_set::{
        LiveObjectIndexer, PROGRESS_REPORT_INTERVAL, ParMakeLiveObjectIndexer, eta_display,
        progress_rate,
    },
};

type OwnedMutexGuard<T> = ArcMutexGuard<parking_lot::RawMutex, T>;

type OwnerIndexKey = (Address, ObjectId);
type CoinIndexKey = (Address, String, ObjectId);
type DynamicFieldKey = (ObjectId, ObjectId);
type EventId = (TxSequenceNumber, usize);
type EventIndex = (TransactionEventsDigest, TransactionDigest, u64);
type AllBalance = HashMap<TypeTag, TotalBalance>;

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
const ENV_VAR_DISABLE_INDEX_CACHE: &str = "DISABLE_INDEX_CACHE";
const ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE: &str = "INVALIDATE_INSTEAD_OF_UPDATE";
const ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB: &str = "JSONRPC_HISTORY_BLOCK_CACHE_MB";
const DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB: usize = 512;

// Do not reuse these tags. Mark them as deprecated if a table is removed.
const DB_PREFIX_HISTORIC_TX_ORDER: u8 = 0;
const DB_PREFIX_HISTORIC_TXS_SEQ: u8 = 1;
const DB_PREFIX_HISTORIC_TXS_FROM_ADDR: u8 = 2;
const DB_PREFIX_HISTORIC_TXS_TO_ADDR: u8 = 3;
const DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID: u8 = 4;
const DB_PREFIX_HIST_TXS_BY_MUTATED_OBJECT_ID: u8 = 5;
const DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION: u8 = 6;
const DB_PREFIX_HISTORIC_EVENT_ORDER: u8 = 7;
const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE: u8 = 8;
const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT: u8 = 9;
const DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE: u8 = 10;
const DB_PREFIX_HISTORIC_EVENT_BY_SENDER: u8 = 11;
const DB_PREFIX_HISTORIC_EVENT_BY_TIME: u8 = 12;

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct TotalBalance {
    pub balance: i128,
    pub num_coins: i64,
}

#[derive(Debug)]
pub struct ObjectIndexChanges {
    pub deleted_owners: Vec<OwnerIndexKey>,
    pub deleted_dynamic_fields: Vec<DynamicFieldKey>,
    pub new_owners: Vec<(OwnerIndexKey, ObjectInfo)>,
    pub new_dynamic_fields: Vec<DynamicFieldKey>,
}

/// Per-transaction inputs for the history tables of the index batch. Unlike
/// the live-state tables (owner, coin, dynamic field), these need only the
/// transaction, its effects, and its events — no object contents.
struct TransactionIndexData {
    digest: TransactionDigest,
    sender: Address,
    active_inputs: Vec<ObjectId>,
    mutated_objects: Vec<(ObjectReference, Owner)>,
    move_functions: Vec<(ObjectId, String, String)>,
    events: TransactionEvents,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct MetadataInfo {
    /// Version of the Database
    version: u64,
}

/// A staged index update for one checkpoint, waiting for its in-order commit.
struct PendingCheckpointUpdate {
    batch: DBBatch,
    /// Net coin index change per key (`Some` upsert, `None` delete), used at
    /// commit time to derive balance cache updates from the pre-commit
    /// database state.
    coin_changes: BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
}

#[derive(Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CoinInfo {
    pub version: Version,
    pub digest: ObjectDigest,
    pub balance: u64,
    pub previous_transaction: TransactionDigest,
}

impl CoinInfo {
    /// Returns coin metadata when `object` is a `Coin<T>`, `None` otherwise.
    pub fn from_object(object: &Object) -> Option<CoinInfo> {
        // Check the type before parsing: any struct whose BCS layout matches
        // `Coin`'s `{UID, u64}` would otherwise deserialize successfully.
        if !object.is_coin() {
            return None;
        }
        object.as_coin_maybe().map(|coin| CoinInfo {
            version: object.version(),
            digest: object.digest(),
            previous_transaction: object.previous_transaction,
            balance: coin.value(),
        })
    }
}

pub struct IndexStoreMetrics {
    balance_lookup_from_db: IntCounter,
    balance_lookup_from_total: IntCounter,
    all_balance_lookup_from_db: IntCounter,
    all_balance_lookup_from_total: IntCounter,
}

impl IndexStoreMetrics {
    pub fn new(registry: &Registry) -> IndexStoreMetrics {
        Self {
            balance_lookup_from_db: register_int_counter_with_registry!(
                "balance_lookup_from_db",
                "Total number of balance requests served from database",
                registry,
            )
            .unwrap(),
            balance_lookup_from_total: register_int_counter_with_registry!(
                "balance_lookup_from_total",
                "Total number of balance requests served ",
                registry,
            )
            .unwrap(),
            all_balance_lookup_from_db: register_int_counter_with_registry!(
                "all_balance_lookup_from_db",
                "Total number of all balance requests served from database",
                registry,
            )
            .unwrap(),
            all_balance_lookup_from_total: register_int_counter_with_registry!(
                "all_balance_lookup_from_total",
                "Total number of all balance requests served",
                registry,
            )
            .unwrap(),
        }
    }
}

/// The `IndexStoreCaches` struct manages `ShardedLruCache` instances to
/// facilitate balance lookups and ownership queries.
pub struct IndexStoreCaches {
    per_coin_type_balance: ShardedLruCache<(Address, TypeTag), IotaResult<TotalBalance>>,
    all_balances: ShardedLruCache<Address, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>>,
    locks: MutexTable<Address>,
}

#[derive(Default)]
pub struct IndexStoreCacheUpdates {
    _locks: Vec<OwnedMutexGuard<()>>,
    per_coin_type_balance_changes: Vec<((Address, TypeTag), IotaResult<TotalBalance>)>,
    all_balance_changes: Vec<(Address, IotaResult<Arc<AllBalance>>)>,
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
struct HistoryBucket {
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

fn history_cf_name(epoch: EpochId) -> String {
    format!("{HISTORY_CF_PREFIX}{epoch}")
}

/// The epoch of a history column family, `None` for other names.
fn history_cf_epoch(cf_name: &str) -> Option<EpochId> {
    cf_name
        .strip_prefix(HISTORY_CF_PREFIX)
        .and_then(|epoch| epoch.parse().ok())
}

impl HistoryBucket {
    fn reopen(db: &Arc<Database>, epoch: EpochId) -> Result<Self, TypedStoreError> {
        // The tags are each table's identity within the shared column
        // family; never change or reuse them for existing data. Per-epoch
        // column families skip the periodic metrics reporter task: with up
        // to ~100 retained epochs, one task per column family adds up.
        fn map<K, V>(
            db: &Arc<Database>,
            cf_name: &str,
            tag: u8,
        ) -> Result<TaggedDBMap<K, V>, TypedStoreError>
        where
            K: Clone + Serialize + DeserializeOwned,
            V: Serialize + DeserializeOwned,
        {
            TaggedDBMap::reopen(db, cf_name, tag, &ReadWriteOptions::default(), true)
        }
        let cf = history_cf_name(epoch);
        Ok(Self {
            tx_order: map(db, &cf, DB_PREFIX_HISTORIC_TX_ORDER)?,
            txs_seq: map(db, &cf, DB_PREFIX_HISTORIC_TXS_SEQ)?,
            txs_from_addr: map(db, &cf, DB_PREFIX_HISTORIC_TXS_FROM_ADDR)?,
            txs_to_addr: map(db, &cf, DB_PREFIX_HISTORIC_TXS_TO_ADDR)?,
            txs_by_input_object_id: map(db, &cf, DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID)?,
            txs_by_mutated_object_id: map(db, &cf, DB_PREFIX_HIST_TXS_BY_MUTATED_OBJECT_ID)?,
            txs_by_move_function: map(db, &cf, DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION)?,
            event_order: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_ORDER)?,
            event_by_move_module: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE)?,
            event_by_move_event: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT)?,
            event_by_event_module: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE)?,
            event_by_sender: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_BY_SENDER)?,
            event_by_time: map(db, &cf, DB_PREFIX_HISTORIC_EVENT_BY_TIME)?,
        })
    }

    /// Appends one transaction's history-table rows to a checkpoint's batch.
    fn index_tx(
        &self,
        batch: &mut DBBatch,
        sequence: TxSequenceNumber,
        timestamp_ms: u64,
        tx: TransactionIndexData,
    ) -> IotaResult {
        let TransactionIndexData {
            digest,
            sender,
            active_inputs,
            mutated_objects,
            move_functions,
            events,
        } = tx;

        batch.insert_batch_tagged(&self.tx_order, std::iter::once((sequence, digest)))?;

        batch.insert_batch_tagged(&self.txs_seq, std::iter::once((digest, sequence)))?;

        batch.insert_batch_tagged(
            &self.txs_from_addr,
            std::iter::once(((sender, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_input_object_id,
            active_inputs.into_iter().map(|id| ((id, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_mutated_object_id,
            mutated_objects
                .iter()
                .map(|(obj_ref, _)| ((obj_ref.object_id, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_by_move_function,
            move_functions
                .into_iter()
                .map(|(obj_id, module, function)| ((obj_id, module, function, sequence), digest)),
        )?;

        batch.insert_batch_tagged(
            &self.txs_to_addr,
            mutated_objects.iter().filter_map(|(_, owner)| {
                owner
                    .into_opt_address()
                    .map(|addr| ((addr, sequence), digest))
            }),
        )?;

        // events
        let event_digest = events.digest();
        batch.insert_batch_tagged(
            &self.event_order,
            events
                .iter()
                .enumerate()
                .map(|(i, _)| ((sequence, i), (event_digest, digest, timestamp_ms))),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_move_module,
            events
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        i,
                        ModuleId::new(
                            AccountAddress::new(e.package_id.into_bytes()),
                            Identifier::new(e.module.as_str()).unwrap(),
                        ),
                    )
                })
                .map(|(i, m)| ((m, (sequence, i)), (event_digest, digest, timestamp_ms))),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_sender,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.sender, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;
        batch.insert_batch_tagged(
            &self.event_by_move_event,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.type_.clone(), (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch_tagged(
            &self.event_by_time,
            events.iter().enumerate().map(|(i, _)| {
                (
                    (timestamp_ms, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch_tagged(
            &self.event_by_event_module,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (
                        ModuleId::new(
                            AccountAddress::new(e.type_.address().into_bytes()),
                            Identifier::new(e.type_.module().as_str()).unwrap(),
                        ),
                        (sequence, i),
                    ),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        Ok(())
    }
}

impl IndexStoreTables {
    pub fn owner_index(&self) -> &DBMap<OwnerIndexKey, ObjectInfo> {
        &self.owner_index
    }

    pub fn coin_index(&self) -> &DBMap<CoinIndexKey, CoinInfo> {
        &self.coin_index
    }

    /// Opens the tables with tuned bulk-ingestion options (WAL disabled,
    /// unordered writes) for a full rebuild or a formal-snapshot restore.
    /// Writes must be flushed before the database closes, and serving
    /// queries requires a reopen with default options.
    fn open_for_bulk_ingestion(path: PathBuf) -> Self {
        let bulk_options = bulk_ingestion_options();
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
        if !matches!(self.meta.get(&()), Ok(None)) {
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

    fn needs_to_do_initialization(&self, checkpoint_store: &CheckpointStore) -> bool {
        let schema_mismatch = match self.meta.get(&()) {
            Ok(Some(metadata)) => metadata.version != CURRENT_DB_VERSION,
            Ok(None) => true,
            Err(_) => true,
        };

        schema_mismatch || self.is_indexed_watermark_out_of_date(checkpoint_store)
    }

    // Check if the index watermark is behind the highest_executed_checkpoint.
    fn is_indexed_watermark_out_of_date(&self, checkpoint_store: &CheckpointStore) -> bool {
        let highest_executed_checkpoint = checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .ok()
            .flatten();
        let watermark = self.watermark.get(&()).ok().flatten();
        watermark < highest_executed_checkpoint
    }

    /// Runs only when `needs_to_do_initialization` is true (fresh DB, schema
    /// mismatch, crashed mid-init, or the index watermark falling behind
    /// `highest_executed_checkpoint`).
    /// The on-disk DB needs to be wiped before this is called, so `init`
    /// always starts from an empty store.
    #[tracing::instrument(skip_all)]
    fn init(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        batch_size_limit: usize,
    ) -> Result<(), StorageError> {
        info!("Initializing JSON-RPC indexes");

        // `meta` first, `watermark` last: a crash in between leaves a store
        // the next open wipes and re-initializes.
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
        self.index_live_object_set(authority_store, batch_size_limit)?;

        self.history_watermark.insert(
            &(),
            &highest_executed_checkpoint.map_or(0, |c| c.saturating_add(1)),
        )?;
        self.watermark
            .insert(&(), &highest_executed_checkpoint.unwrap_or(0))?;

        info!("Finished initializing JSON-RPC indexes");

        Ok(())
    }

    /// Rebuilds the live-state indexes (owner, coin, dynamic field) by
    /// scanning the current live object set in parallel.
    fn index_live_object_set(
        &self,
        authority_store: &AuthorityStore,
        batch_size_limit: usize,
    ) -> Result<(), StorageError> {
        let indexer = JsonRpcLiveObjectSetIndexer {
            tables: self,
            batch_size_limit,
        };
        crate::par_index_live_object_set::par_index_live_object_set(authority_store, &indexer)
    }

    fn index_coin(
        &self,
        digest: &TransactionDigest,
        batch: &mut DBBatch,
        object_index_changes: &ObjectIndexChanges,
        tx_coins: Option<TxCoins>,
        coin_changes: &mut BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
    ) -> IotaResult {
        // In production if this code path is hit, we should expect `tx_coins` to not be
        // None. However, in many tests today we do not distinguish validator
        // and/or fullnode, so we gracefully exist here.
        let Some((input_coins, written_coins)) = tx_coins else {
            return Ok(());
        };
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
        tx_coins: Option<TxCoins>,
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
    /// The database holding both the static tables and the per-epoch history
    /// column families; used to create and drop the latter at runtime.
    db: Arc<Database>,
    /// Template options for per-epoch history column families. All clones
    /// share one block cache through the cloned table factory.
    history_cf_options: rocksdb::Options,
    /// The retained history buckets. On-disk column-family names are the
    /// ground truth; this map mirrors them for reads.
    history: RwLock<BTreeMap<EpochId, Arc<HistoryBucket>>>,
    caches: IndexStoreCaches,
    metrics: Arc<IndexStoreMetrics>,
    max_type_length: u64,
    pending_updates: Mutex<BTreeMap<CheckpointSequenceNumber, PendingCheckpointUpdate>>,
    history_backfill_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// The pieces produced by opening the index database.
struct OpenedIndexDb {
    tables: IndexStoreTables,
    db: Arc<Database>,
    history_cf_options: rocksdb::Options,
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

/// Options for the per-epoch history column families. Each bucket is
/// write-once (appended during its epoch or the backfill, then only read)
/// and queried by bounded range scans plus exact-key digest probes, which
/// the block-based bloom filters answer from RAM. `set_block_options`
/// creates the single block cache that every clone of these options shares.
fn history_cf_options(db_options: &DBOptions) -> rocksdb::Options {
    db_options
        .clone()
        .optimize_for_write_throughput_no_deletion()
        .set_block_options(
            read_size_from_env(ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB)
                .unwrap_or(DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB),
            16 << 10,
        )
        .options
}

/// Extracts one transaction's history-table index inputs.
fn transaction_index_data(
    transaction: &TransactionEnvelope,
    effects: &TransactionEffects,
    events: Option<&TransactionEvents>,
) -> IotaResult<TransactionIndexData> {
    let tx_data = &transaction.intent_message().value;

    Ok(TransactionIndexData {
        digest: *effects.transaction_digest(),
        sender: tx_data.sender(),
        active_inputs: tx_data
            .input_objects()?
            .iter()
            .map(|o| o.object_id())
            .collect(),
        mutated_objects: effects
            .all_changed_objects()
            .into_iter()
            .map(|(changed, _kind)| (changed.reference, changed.owner))
            .collect(),
        move_functions: tx_data
            .move_calls()
            .into_iter()
            .map(|(package, module, function)| (*package, module.to_owned(), function.to_owned()))
            .collect(),
        events: events.cloned().unwrap_or_default(),
    })
}

/// Scan bounds excluding `cursor`: the inclusive lower bound for forward
/// scans and the inclusive upper bound for reverse scans. `None` when the
/// cursor leaves nothing to scan.
fn sequence_bounds_after_cursor(
    cursor: Option<TxSequenceNumber>,
    reverse: bool,
) -> Option<(TxSequenceNumber, TxSequenceNumber)> {
    let lower = match cursor {
        Some(cursor) if !reverse => cursor.checked_add(1)?,
        _ => TxSequenceNumber::MIN,
    };
    let upper = match cursor {
        Some(cursor) if reverse => cursor.checked_sub(1)?,
        _ => TxSequenceNumber::MAX,
    };
    Some((lower, upper))
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

/// Whether the object is a `Field` object of a dynamic field — the only
/// objects the dynamic-field index stores.
fn is_dynamic_field(object: &Object) -> bool {
    object
        .data
        .as_opt_struct()
        .is_some_and(|move_object| move_object.struct_tag().is_dynamic_field())
}

/// Resolves a `Field` object into the [`DynamicFieldInfo`] served by the
/// JSON-RPC API. Runs at query time — the index stores only the field keys.
/// Returns `None` when `o` is not a `Field` object or its layout cannot be
/// resolved.
pub(crate) fn try_create_dynamic_field_info(
    o: &Object,
    object_store: &dyn ObjectStore,
    resolver: &mut dyn LayoutResolver,
) -> IotaResult<Option<DynamicFieldInfo>> {
    // Skip if not a move object
    let Some(move_object) = o.data.as_opt_struct().cloned() else {
        return Ok(None);
    };

    // Only dynamic field objects are resolvable
    if !move_object.struct_tag().is_dynamic_field() {
        return Ok(None);
    }

    let layout = match resolver.get_annotated_layout(move_object.struct_tag()) {
        Ok(annotated_layout) => annotated_layout.into_layout(),
        Err(e) => {
            error!(
                "unable to load layout for type `{:?}`: {e}",
                move_object.struct_tag()
            );
            return Ok(None);
        }
    };

    let field = DFV::FieldVisitor::deserialize(move_object.contents(), &layout).map_err(|e| {
        IotaError::ObjectDeserialization {
            error: e.to_string(),
        }
    })?;

    let type_ = field.kind;
    let name_type: TypeTag = type_tag_core_to_sdk(&field.name_layout.into());
    let bcs_name = field.name_bytes.to_owned();

    let name_value = BoundedVisitor::deserialize_value(field.name_bytes, field.name_layout)
        .map_err(|e| {
            warn!("{e}");
            IotaError::ObjectDeserialization {
                error: e.to_string(),
            }
        })?;

    let name = DynamicFieldName {
        type_tag: name_type,
        value: IotaMoveValue::from(name_value).to_json_value(),
    };

    let value_metadata = field.value_metadata().map_err(|e| {
        warn!("{e}");
        IotaError::ObjectDeserialization {
            error: e.to_string(),
        }
    })?;

    Ok(Some(match value_metadata {
        DFV::ValueMetadata::DynamicField(object_type) => DynamicFieldInfo {
            name,
            bcs_name,
            type_,
            object_type: object_type.to_canonical_string(/* with_prefix */ true),
            object_id: o.id(),
            version: o.version(),
            digest: o.digest(),
        },

        DFV::ValueMetadata::DynamicObjectField(object_id) => {
            // Find the actual object from storage using the object id obtained from the
            // wrapper.

            // The child is written at the wrapper's version when the field
            // is added, but that historic version may since have been
            // pruned; the child of a live field is itself live, so fall
            // back to its latest version.
            let object = match object_store.try_get_object_by_key(&object_id, o.version())? {
                Some(object) => object,
                None => object_store.try_get_object(&object_id)?.ok_or(
                    UserInputError::ObjectNotFound {
                        object_id,
                        version: None,
                    },
                )?,
            };
            let version = object.version();
            let digest = object.digest();
            let object_type = object.data.opt_object_type().unwrap().clone();

            DynamicFieldInfo {
                name,
                bcs_name,
                type_,
                object_type: object_type.to_string(),
                object_id,
                version,
                digest,
            }
        }
    }))
}

/// Builds the live-state indexes (owner, coin, dynamic field) from a parallel
/// scan of the live object set during `init`.
struct JsonRpcLiveObjectSetIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch_size_limit: usize,
}

impl ParMakeLiveObjectIndexer for JsonRpcLiveObjectSetIndexer<'_> {
    type ObjectIndexer<'a>
        = JsonRpcLiveObjectIndexer<'a>
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        JsonRpcLiveObjectIndexer {
            tables: self.tables,
            batch: self.tables.owner_index.batch(),
            batch_size_limit: self.batch_size_limit,
        }
    }
}

/// One worker's indexer within a [`JsonRpcLiveObjectSetIndexer`] run, and the
/// per-partition indexer of a formal-snapshot restore.
struct JsonRpcLiveObjectIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: DBBatch,
    batch_size_limit: usize,
}

impl LiveObjectIndexer for JsonRpcLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                self.batch.insert_batch(
                    &self.tables.owner_index,
                    [((owner, object.id()), ObjectInfo::from_object(object))],
                )?;
                if let Some(coin_info) = CoinInfo::from_object(object) {
                    let coin_type = object
                        .opt_coin_type()
                        .expect("coin object must have a coin type")
                        .to_string();
                    self.batch.insert_batch(
                        &self.tables.coin_index,
                        [((owner, coin_type, object.id()), coin_info)],
                    )?;
                }
            }
            Owner::Object(parent) => {
                if is_dynamic_field(object) {
                    self.batch.insert_batch(
                        &self.tables.dynamic_field_index,
                        [((parent, object.id()), ())],
                    )?;
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }

        // If the batch size grows beyond the limit then write out to the DB so
        // that the data we need to hold in memory doesn't grow unbounded.
        if self.batch.size_in_bytes() >= self.batch_size_limit {
            std::mem::replace(&mut self.batch, self.tables.owner_index.batch())
                .write_opt(&bulk_ingestion_write_options())?;
        }

        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        self.batch.write_opt(&bulk_ingestion_write_options())?;
        Ok(())
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

impl JsonRpcIndexRestorer {
    /// Opens the store with bulk-ingestion options and stamps it with this
    /// schema version. `meta` is written now and `watermark` only in
    /// [`Self::finalize`], so a node opening a store from a restore that
    /// crashed in between wipes and rebuilds it.
    pub fn open(path: PathBuf) -> Result<Self, TypedStoreError> {
        let tables = IndexStoreTables::open_for_bulk_ingestion(path);
        tables.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )?;
        Ok(Self {
            tables,
            batch_size_limit: bulk_ingestion_options().batch_size_limit,
        })
    }

    /// Returns an indexer for one partition of the snapshot's live objects.
    pub fn partition_indexer(&self) -> JsonRpcPartitionIndexer<'_> {
        JsonRpcPartitionIndexer(JsonRpcLiveObjectIndexer {
            tables: &self.tables,
            batch: self.tables.owner_index.batch(),
            batch_size_limit: self.batch_size_limit,
        })
    }

    /// Seeds the markers so a node opens the store in place, flushes the
    /// WAL-less bulk writes, and closes the database. `restore_checkpoint`
    /// is the restore's highest executed checkpoint; no history below it
    /// exists locally, so there is nothing for the background replay to
    /// backfill.
    ///
    /// Callers must have restored the complete live object set first,
    /// through [`Self::partition_indexer`].
    pub async fn finalize(
        self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), StorageError> {
        let Self { tables, .. } = self;
        tables
            .history_watermark
            .insert(&(), &restore_checkpoint.saturating_add(1))?;
        tables.watermark.insert(&(), &restore_checkpoint)?;
        // WAL is disabled for the bulk writes; make them durable before the
        // database closes.
        tables.meta.flush_all()?;

        // Release every RocksDB handle before returning, so the caller can
        // move the database directory.
        let weak_db = Arc::downgrade(&tables.meta.db);
        drop(tables);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the JSON-RPC index database after the restore",
            ));
        }
        Ok(())
    }
}

/// Indexer for one partition of a formal-snapshot restore's live objects.
pub struct JsonRpcPartitionIndexer<'a>(JsonRpcLiveObjectIndexer<'a>);

impl JsonRpcPartitionIndexer<'_> {
    pub fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        self.0.index_object(object)
    }

    /// Writes the partition's remaining batch.
    pub fn finish(self) -> Result<(), StorageError> {
        self.0.finish()
    }
}

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
    /// range of recent checkpoints, as on a pruned node.
    pub async fn new(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
        authority_store: &Arc<AuthorityStore>,
        checkpoint_store: &Arc<CheckpointStore>,
    ) -> Arc<Self> {
        let mut opened = Self::open_index_db(&path);

        opened
            .tables
            .seed_meta()
            .expect("failed to initialize index tables");

        if opened.tables.needs_to_do_initialization(checkpoint_store) {
            let mut init_tables = {
                drop(opened);
                safe_drop_db(path.clone(), Duration::from_secs(30))
                    .await
                    .expect("unable to destroy old JSON-RPC index db");

                // Open the empty DB with tuned bulk ingestion options to
                // speed up the initial indexing. The DB is reopened with default options
                // afterwards.
                IndexStoreTables::open_for_bulk_ingestion(path.clone())
            };
            let batch_size_limit = bulk_ingestion_options().batch_size_limit;

            // The rebuild scans and writes RocksDB for a long time; keep it
            // off the async runtime's worker threads.
            let init_tables = tokio::task::spawn_blocking({
                let authority_store = authority_store.clone();
                let checkpoint_store = checkpoint_store.clone();
                move || {
                    init_tables
                        .init(&authority_store, &checkpoint_store, batch_size_limit)
                        .expect("unable to initialize JSON-RPC index");
                    init_tables
                }
            })
            .await
            .expect("JSON-RPC index initialization task failed");

            // Flush all data to disk before dropping tables. This is critical because
            // WAL is disabled for the bulk writes during initialization. Flushing any
            // table flushes every column family of the shared underlying database, so
            // one call covers all tables.
            init_tables
                .meta
                .flush_all()
                .expect("JSON-RPC index DB should be flushable after bulk ingestion");

            let weak_db = Arc::downgrade(&init_tables.meta.db);
            drop(init_tables);

            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if weak_db.strong_count() == 0 {
                    break;
                }
                if Instant::now() > deadline {
                    panic!("unable to reopen DB after indexing");
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Reopen the DB with default options (eg without `unordered_write`s enabled)
            opened = Self::open_index_db(&path);

            // Sanity check: verify the database version was persisted correctly, i.e.
            // the WAL-disabled bulk writes were flushed before the reopen.
            let stored_version = opened
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
        }

        // A store rebuilt without local history has no rows to derive the next
        // sequence number from; anchor it to the network transaction total at
        // the indexed watermark so numbering stays canonical.
        let anchor = opened
            .tables
            .watermark
            .get(&())
            .expect("failed to initialize index tables")
            .and_then(|watermark| {
                let checkpoint = checkpoint_store
                    .get_checkpoint_by_sequence_number(watermark)
                    .expect("checkpoint store read cannot fail");
                if checkpoint.is_none() {
                    warn!(
                        watermark,
                        "indexed watermark checkpoint not found; transaction numbering falls \
                         back to the local index rows"
                    );
                }
                checkpoint.map(|checkpoint| checkpoint.network_total_transactions)
            })
            .unwrap_or(0);

        let store = Arc::new(Self::finish_open(opened, registry, max_type_length, anchor));
        store.spawn_history_backfill(authority_store.clone(), checkpoint_store.clone());
        store
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
            if let Err(e) = store.backfill_history(&authority_store, &checkpoint_store) {
                error!("JSON-RPC index history backfill stopped: {e}");
            }
        });
        *self.history_backfill_task.lock() = Some(task);
    }

    /// Waits for the background history replay to finish — for tests.
    pub async fn wait_for_history_backfill_for_testing(&self) {
        let task = self.history_backfill_task.lock().take();
        if let Some(task) = task {
            task.await.expect("history backfill task failed");
        }
    }

    /// Fills the history tables for the checkpoints below
    /// `history_watermark`, newest first, until it reaches the
    /// checkpoint-contents pruner. The marker commits atomically with each
    /// checkpoint's rows, so an interrupted run resumes where it stopped.
    /// No-op when the marker is absent (the history was indexed continuously
    /// and is complete).
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
        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut replayed: u64 = 0;
        loop {
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
            self.replay_checkpoint_history(authority_store, checkpoint_store, next)?;
            replayed += 1;
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

    /// Replays one checkpoint into its epoch's history bucket and lowers
    /// `history_watermark` to it, in one atomic batch. Transactions are
    /// numbered by their position in the network transaction order, derived
    /// from the checkpoint's transaction total, so numbering stays canonical
    /// whatever range is locally available.
    fn replay_checkpoint_history(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        checkpoint_seq: CheckpointSequenceNumber,
    ) -> Result<(), StorageError> {
        let summary = checkpoint_store
            .get_checkpoint_by_sequence_number(checkpoint_seq)?
            .ok_or_else(|| StorageError::missing(format!("missing checkpoint {checkpoint_seq}")))?;
        let contents = checkpoint_store
            .get_checkpoint_contents(&summary.contents_digest)?
            .ok_or_else(|| {
                StorageError::missing(format!("missing checkpoint contents {checkpoint_seq}"))
            })?;
        let first_sequence_number =
            summary.network_total_transactions - contents.iter().len() as u64;
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
        // A plain write, not a bulk-ingestion one: the database is serving
        // queries, and the marker must land atomically with the rows.
        batch.write().map_err(StorageError::from)?;
        Ok(())
    }

    /// Opens the store without the init logic of [`Self::new`] — for tests.
    pub fn new_without_init(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
    ) -> Self {
        let opened = Self::open_index_db(&path);
        Self::finish_open(opened, registry, max_type_length, 0)
    }

    fn finish_open(
        opened: OpenedIndexDb,
        registry: &Registry,
        max_type_length: Option<u64>,
        next_sequence_number_floor: TxSequenceNumber,
    ) -> Self {
        let OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        } = opened;
        let metrics = IndexStoreMetrics::new(registry);
        let caches = IndexStoreCaches {
            per_coin_type_balance: ShardedLruCache::new(1_000_000, 1000),
            all_balances: ShardedLruCache::new(1_000_000, 1000),
            locks: MutexTable::new(128),
        };
        let next_sequence_number = history
            .last_key_value()
            .map(|(_, bucket)| {
                bucket
                    .tx_order
                    .safe_range_iter_reversed(..)
                    .next()
                    .transpose()
                    .expect("failed to initialize indexes")
                    .map(|(seq, _)| seq + 1)
                    .unwrap_or(0)
            })
            .unwrap_or(0)
            .max(next_sequence_number_floor)
            .into();

        Self {
            tables,
            db,
            history_cf_options,
            history: RwLock::new(history),
            next_sequence_number,
            caches,
            metrics: Arc::new(metrics),
            max_type_length: max_type_length.unwrap_or(128),
            pending_updates: Mutex::new(BTreeMap::new()),
            history_backfill_task: Mutex::new(None),
        }
    }

    /// Opens the index database, passing every existing per-epoch history
    /// column family at open with its tuned options: a column family left
    /// for auto-discovery would silently get default options (and its own
    /// block cache).
    fn open_index_db(path: &Path) -> OpenedIndexDb {
        let db_options = default_db_options().disable_write_throttling();
        let coin_options = coin_index_table_default_config();
        let history_cf_options = history_cf_options(&db_options);

        let static_tables = IndexStoreTables::describe_tables();
        let existing_cfs = list_tables(path.to_path_buf()).unwrap_or_default();
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
        for cf_name in &existing_cfs {
            if static_tables.contains_key(cf_name) || cf_name == "default" {
                continue;
            }
            if let Some(epoch) = history_cf_epoch(cf_name) {
                epochs.insert(epoch);
                opt_cfs.push((cf_name.clone(), history_cf_options.clone()));
            } else {
                // A table of another schema version. It must still be opened
                // for RocksDB to open the database at all; the version
                // mismatch wipes the whole database afterwards.
                opt_cfs.push((cf_name.clone(), rocksdb::Options::default()));
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
        .expect("unable to open the JSON-RPC index database");

        fn map<K, V>(db: &Arc<Database>, cf_name: &str, rw: &ReadWriteOptions) -> DBMap<K, V> {
            DBMap::reopen(db, Some(cf_name), rw, false)
                .unwrap_or_else(|e| panic!("cannot open the {cf_name} column family: {e}"))
        }
        let tables = IndexStoreTables {
            meta: map(&db, "meta", &db_options.rw_options),
            watermark: map(&db, "watermark", &db_options.rw_options),
            history_watermark: map(&db, "history_watermark", &db_options.rw_options),
            owner_index: map(&db, "owner_index", &db_options.rw_options),
            coin_index: map(&db, "coin_index", &coin_options.rw_options),
            dynamic_field_index: map(&db, "dynamic_field_index", &db_options.rw_options),
        };

        let mut history = BTreeMap::new();
        for epoch in epochs {
            let bucket =
                HistoryBucket::reopen(&db, epoch).expect("unable to open a history column family");
            history.insert(epoch, Arc::new(bucket));
        }

        OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        }
    }

    /// The retained history buckets in scan order: ascending epochs for
    /// forward scans, descending for reverse scans. Buckets are disjoint,
    /// epoch-ordered segments of the global sequence order, so chaining
    /// per-bucket scans in this order preserves it.
    fn history_buckets(&self, reverse: bool) -> Vec<Arc<HistoryBucket>> {
        let history = self.history.read();
        if reverse {
            history.values().rev().cloned().collect()
        } else {
            history.values().cloned().collect()
        }
    }

    /// Maps an `event_order` row to the query result shape.
    fn event_order_row(
        ((_, event_seq), (digest, tx_digest, time)): (EventId, EventIndex),
    ) -> (TransactionEventsDigest, TransactionDigest, usize, u64) {
        (digest, tx_digest, event_seq, time)
    }

    /// Maps a keyed event-table row to the query result shape.
    fn keyed_event_row<K>(
        ((_, (_, event_seq)), (digest, tx_digest, time)): ((K, EventId), EventIndex),
    ) -> (TransactionEventsDigest, TransactionDigest, usize, u64) {
        (digest, tx_digest, event_seq, time)
    }

    /// Chains one range scan per retained history bucket, in
    /// global sequence order, collecting up to `limit` mapped rows.
    fn scan_history_buckets<K, V, R>(
        &self,
        select: impl Fn(&HistoryBucket) -> &TaggedDBMap<K, V>,
        range: impl RangeBounds<K> + Clone,
        limit: Option<usize>,
        reverse: bool,
        row: impl Fn((K, V)) -> R,
    ) -> IotaResult<Vec<R>>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let mut results = Vec::new();
        for bucket in self.history_buckets(reverse) {
            if limit.is_some_and(|l| results.len() >= l) {
                break;
            }
            let remaining = limit.map_or(usize::MAX, |l| l - results.len());
            let index = select(&bucket);
            let iter = if reverse {
                Either::Left(index.safe_range_iter_reversed(range.clone()))
            } else {
                Either::Right(index.safe_range_iter(range.clone()))
            };
            for result in iter.take(remaining) {
                results.push(row(result?));
            }
        }
        Ok(results)
    }

    /// The bucket holding `epoch`'s history, created if absent.
    fn ensure_history_bucket(&self, epoch: EpochId) -> IotaResult<Arc<HistoryBucket>> {
        if let Some(bucket) = self.history.read().get(&epoch) {
            return Ok(bucket.clone());
        }
        let mut history = self.history.write();
        if let Some(bucket) = history.get(&epoch) {
            return Ok(bucket.clone());
        }
        let cf_name = history_cf_name(epoch);
        // The column family may already exist if a previous run crashed
        // between `create_cf` and the first batch write.
        if self.db.cf_handle(&cf_name).is_none() {
            self.db
                .create_cf(&cf_name, &self.history_cf_options)
                .map_err(|e| IotaError::Storage(e.to_string()))?;
        }
        let bucket = Arc::new(HistoryBucket::reopen(&self.db, epoch)?);
        history.insert(epoch, bucket.clone());
        Ok(bucket)
    }

    /// Drops the history of epochs past the retention horizon: with
    /// `epochs_to_retain` = N, the buckets of the newest N epochs are kept
    /// and every older bucket is dropped wholesale — one constant-time
    /// column-family drop each, with no per-row deletes and no compaction
    /// churn. Returns the earliest retained epoch.
    ///
    /// A query racing a drop may report an error for the dropped epoch's
    /// rows; a retry no longer sees the bucket.
    pub fn prune(&self, epochs_to_retain: u64) -> IotaResult<Option<EpochId>> {
        let (expired, earliest_retained) = {
            let mut history = self.history.write();
            let Some((&newest, _)) = history.last_key_value() else {
                return Ok(None);
            };
            let earliest_retained = newest.saturating_sub(epochs_to_retain.saturating_sub(1));
            let expired: Vec<EpochId> = history
                .range(..earliest_retained)
                .map(|(&e, _)| e)
                .collect();
            history.retain(|&epoch, _| epoch >= earliest_retained);
            (expired, earliest_retained)
        };
        for epoch in expired {
            info!(
                epoch,
                "dropping the JSON-RPC index history of an expired epoch"
            );
            self.db
                .drop_cf(&history_cf_name(epoch))
                .map_err(|e| IotaError::Storage(e.to_string()))?;
        }
        Ok(Some(earliest_retained))
    }

    pub fn tables(&self) -> &IndexStoreTables {
        &self.tables
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
    pub fn index_checkpoint(&self, checkpoint: &CheckpointData, index_coins: bool) -> IotaResult {
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
            let tx_coins = index_coins.then(|| transaction_coins(tx));
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

        update.batch.write()?;

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

    /// Derives the balance cache updates for a checkpoint's net coin changes
    /// by comparing them against the pre-commit database state, holding the
    /// affected owners' locks. Must run before the checkpoint's batch is
    /// written.
    fn balance_cache_updates(
        &self,
        coin_changes: BTreeMap<CoinIndexKey, (TypeTag, Option<CoinInfo>)>,
    ) -> IotaResult<IndexStoreCacheUpdates> {
        if coin_changes.is_empty() {
            return Ok(IndexStoreCacheUpdates::default());
        }

        let addresses: HashSet<Address> = coin_changes.keys().map(|(owner, _, _)| *owner).collect();
        let _locks = self.caches.locks.acquire_locks(addresses.into_iter());

        let mut balance_changes: HashMap<Address, HashMap<TypeTag, TotalBalance>> = HashMap::new();
        for (key, (coin_type, change)) in &coin_changes {
            let entry = balance_changes
                .entry(key.0)
                .or_default()
                .entry(coin_type.clone())
                .or_insert(TotalBalance {
                    num_coins: 0,
                    balance: 0,
                });
            match (self.tables.coin_index.get(key)?, change) {
                (Some(prior), Some(new)) => {
                    entry.balance += new.balance as i128 - prior.balance as i128;
                }
                (None, Some(new)) => {
                    entry.num_coins += 1;
                    entry.balance += new.balance as i128;
                }
                (Some(prior), None) => {
                    entry.num_coins -= 1;
                    entry.balance -= prior.balance as i128;
                }
                (None, None) => {}
            }
        }

        let per_coin_type_balance_changes: Vec<_> = balance_changes
            .iter()
            .flat_map(|(address, balance_map)| {
                balance_map.iter().map(|(type_tag, balance)| {
                    (
                        (*address, type_tag.clone()),
                        Ok::<TotalBalance, IotaError>(*balance),
                    )
                })
            })
            .collect();
        let all_balance_changes: Vec<_> = balance_changes
            .into_iter()
            .map(|(address, balance_map)| {
                (
                    address,
                    Ok::<Arc<HashMap<TypeTag, TotalBalance>>, IotaError>(Arc::new(balance_map)),
                )
            })
            .collect();
        Ok(IndexStoreCacheUpdates {
            _locks,
            per_coin_type_balance_changes,
            all_balance_changes,
        })
    }

    pub fn next_sequence_number(&self) -> TxSequenceNumber {
        self.next_sequence_number.load(Ordering::SeqCst) + 1
    }

    pub fn get_transactions(
        &self,
        filter: Option<TransactionFilter>,
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        // Lookup TransactionDigest sequence number,
        let cursor = if let Some(cursor) = cursor {
            Some(
                self.get_transaction_seq(&cursor)?
                    .ok_or(IotaError::TransactionNotFound { digest: cursor })?,
            )
        } else {
            None
        };
        match filter {
            Some(TransactionFilter::MoveFunction {
                package,
                module,
                function,
            }) => Ok(self.get_transactions_by_move_function(
                package, module, function, cursor, limit, reverse,
            )?),
            Some(TransactionFilter::InputObject(object_id)) => {
                Ok(self.get_transactions_by_input_object(object_id, cursor, limit, reverse)?)
            }
            Some(TransactionFilter::ChangedObject(object_id)) => {
                Ok(self.get_transactions_by_mutated_object(object_id, cursor, limit, reverse)?)
            }
            Some(TransactionFilter::FromAddress(address)) => {
                Ok(self.get_transactions_from_addr(address, cursor, limit, reverse)?)
            }
            Some(TransactionFilter::ToAddress(address)) => {
                Ok(self.get_transactions_to_addr(address, cursor, limit, reverse)?)
            }
            // NOTE: filter via checkpoint sequence number is implemented in
            // `get_transactions` of authority.rs.
            Some(_) => Err(IotaError::UserInput {
                error: UserInputError::Unsupported(format!("{filter:?}")),
            }),
            None => {
                let Some((lower, upper)) = sequence_bounds_after_cursor(cursor, reverse) else {
                    return Ok(vec![]);
                };
                self.scan_history_buckets(
                    |bucket| &bucket.tx_order,
                    lower..=upper,
                    limit,
                    reverse,
                    |(_, digest)| digest,
                )
            }
        }
    }

    fn get_transactions_from_index<KeyT: Clone + Serialize + DeserializeOwned>(
        &self,
        select: impl Fn(&HistoryBucket) -> &TaggedDBMap<(KeyT, TxSequenceNumber), TransactionDigest>,
        key: KeyT,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        // The cursor is exclusive. Applying it through the scan bounds (rather
        // than by skipping the first row) makes it compose across buckets:
        // every bucket gets the same bounds, and only the bucket containing
        // the cursor's sequence range yields adjacent rows.
        let Some((lower, upper)) = sequence_bounds_after_cursor(cursor, reverse) else {
            return Ok(vec![]);
        };
        self.scan_history_buckets(
            select,
            (key.clone(), lower)..=(key, upper),
            limit,
            reverse,
            |(_, digest)| digest,
        )
    }

    pub fn get_transactions_by_input_object(
        &self,
        input_object: ObjectId,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.get_transactions_from_index(
            |bucket| &bucket.txs_by_input_object_id,
            input_object,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_by_mutated_object(
        &self,
        mutated_object: ObjectId,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.get_transactions_from_index(
            |bucket| &bucket.txs_by_mutated_object_id,
            mutated_object,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_from_addr(
        &self,
        addr: Address,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.get_transactions_from_index(
            |bucket| &bucket.txs_from_addr,
            addr,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_by_move_function(
        &self,
        package: ObjectId,
        module: Option<String>,
        function: Option<String>,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        // If we are passed a function with no module return a UserInputError
        if function.is_some() && module.is_none() {
            return Err(IotaError::UserInput {
                error: UserInputError::MoveFunctionInput(
                    "Cannot supply function without supplying module".to_string(),
                ),
            });
        }

        // We cannot have a cursor without filling out the other keys.
        if cursor.is_some() && (module.is_none() || function.is_none()) {
            return Err(IotaError::UserInput {
                error: UserInputError::MoveFunctionInput(
                    "Cannot supply cursor without supplying module and function".to_string(),
                ),
            });
        }

        let Some((lower, upper)) = sequence_bounds_after_cursor(cursor, reverse) else {
            return Ok(vec![]);
        };

        // An unset module or function spans its whole range: identifiers are
        // at most `max_type_length` characters from an alphabet that sorts
        // at or below `z`.
        let max_string = "z".repeat(self.max_type_length.try_into().unwrap());
        let module_lower = module.clone().unwrap_or_default();
        let module_upper = module.unwrap_or_else(|| max_string.clone());
        let function_lower = function.clone().unwrap_or_default();
        let function_upper = function.unwrap_or(max_string);
        let lower_key = (package, module_lower, function_lower, lower);
        let upper_key = (package, module_upper, function_upper, upper);

        self.scan_history_buckets(
            |bucket| &bucket.txs_by_move_function,
            lower_key..=upper_key,
            limit,
            reverse,
            |(_, digest)| digest,
        )
    }

    pub fn get_transactions_to_addr(
        &self,
        addr: Address,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.get_transactions_from_index(|bucket| &bucket.txs_to_addr, addr, cursor, limit, reverse)
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

    pub fn all_events(
        &self,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        let range = if descending {
            (Bound::Unbounded, Bound::Included((tx_seq, event_seq)))
        } else {
            (Bound::Included((tx_seq, event_seq)), Bound::Unbounded)
        };
        self.scan_history_buckets(
            |bucket| &bucket.event_order,
            range,
            Some(limit),
            descending,
            Self::event_order_row,
        )
    }

    pub fn events_by_transaction(
        &self,
        digest: &TransactionDigest,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        let seq = self
            .get_transaction_seq(digest)?
            .ok_or(IotaError::TransactionNotFound { digest: *digest })?;
        let range = if descending {
            (seq, 0)..=(min(tx_seq, seq), event_seq)
        } else {
            (max(tx_seq, seq), event_seq)..=(seq, usize::MAX)
        };
        self.scan_history_buckets(
            |bucket| &bucket.event_order,
            range,
            Some(limit),
            descending,
            Self::event_order_row,
        )
    }

    fn get_event_from_index<KeyT: Clone + Serialize + DeserializeOwned>(
        &self,
        select: impl Fn(
            &HistoryBucket,
        ) -> &TaggedDBMap<
            (KeyT, EventId),
            (TransactionEventsDigest, TransactionDigest, u64),
        >,
        key: &KeyT,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        let range = if descending {
            (key.clone(), (TxSequenceNumber::MIN, 0))..=(key.clone(), (tx_seq, event_seq))
        } else {
            (key.clone(), (tx_seq, event_seq))..=(key.clone(), (TxSequenceNumber::MAX, usize::MAX))
        };
        self.scan_history_buckets(
            select,
            range,
            Some(limit),
            descending,
            Self::keyed_event_row,
        )
    }

    pub fn events_by_module_id(
        &self,
        module: &ModuleId,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        self.get_event_from_index(
            |bucket| &bucket.event_by_move_module,
            module,
            tx_seq,
            event_seq,
            limit,
            descending,
        )
    }

    pub fn events_by_move_event_struct_name(
        &self,
        struct_name: &StructTag,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        self.get_event_from_index(
            |bucket| &bucket.event_by_move_event,
            struct_name,
            tx_seq,
            event_seq,
            limit,
            descending,
        )
    }

    pub fn events_by_move_event_module(
        &self,
        module_id: &ModuleId,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        self.get_event_from_index(
            |bucket| &bucket.event_by_event_module,
            module_id,
            tx_seq,
            event_seq,
            limit,
            descending,
        )
    }

    pub fn events_by_sender(
        &self,
        sender: &Address,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        self.get_event_from_index(
            |bucket| &bucket.event_by_sender,
            sender,
            tx_seq,
            event_seq,
            limit,
            descending,
        )
    }

    pub fn event_iterator(
        &self,
        start_time: u64,
        end_time: u64,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        let range = if descending {
            (start_time, (TxSequenceNumber::MIN, 0))..=(end_time, (tx_seq, event_seq))
        } else {
            (start_time, (tx_seq, event_seq))..=(end_time, (TxSequenceNumber::MAX, usize::MAX))
        };
        self.scan_history_buckets(
            |bucket| &bucket.event_by_time,
            range,
            Some(limit),
            descending,
            Self::keyed_event_row,
        )
    }

    pub fn get_dynamic_field_ids_iterator(
        &self,
        object: ObjectId,
        cursor: Option<ObjectId>,
    ) -> IotaResult<impl Iterator<Item = Result<ObjectId, TypedStoreError>> + '_> {
        debug!(?object, "get_dynamic_fields");
        Ok(self
            .tables
            .dynamic_field_index
            .safe_iter_with_prefix_from(
                &object,
                std::ops::Bound::Included(&cursor.unwrap_or(ObjectId::ZERO)),
            )
            // skip an extra b/c the cursor is exclusive
            .skip(usize::from(cursor.is_some()))
            .map_ok(|((_, field_id), ())| field_id))
    }

    /// Whether `field_id` is an indexed dynamic field of `object`.
    pub fn dynamic_field_exists(&self, object: ObjectId, field_id: ObjectId) -> IotaResult<bool> {
        Ok(self
            .tables
            .dynamic_field_index
            .contains_key(&(object, field_id))?)
    }

    pub fn get_owner_objects(
        &self,
        owner: Address,
        cursor: Option<ObjectId>,
        limit: usize,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<Vec<ObjectInfo>> {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => ObjectId::ZERO,
        };
        Ok(self
            .get_owner_objects_iterator(owner, cursor, filter)?
            .take(limit)
            .collect())
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

    /// starting_object_id can be used to implement pagination, where a client
    /// remembers the last object id of each page, and use it to query the
    /// next page.
    pub fn get_owner_objects_iterator(
        &self,
        owner: Address,
        starting_object_id: ObjectId,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<impl Iterator<Item = ObjectInfo> + '_> {
        Ok(self
            .tables
            .owner_index
            // The object id 0 is the smallest possible
            .safe_iter_with_bounds(Some((owner, starting_object_id)), None)
            .map(|result| result.expect("iterator db error"))
            .skip(usize::from(starting_object_id != ObjectId::ZERO))
            .take_while(move |((address_owner, _), _)| address_owner == &owner)
            .filter(move |(_, o)| {
                if let Some(filter) = filter.as_ref() {
                    filter.matches(o)
                } else {
                    true
                }
            })
            .map(|(_, object_info)| object_info))
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // We are checkpointing the whole db
        self.tables.meta.checkpoint_db(path).map_err(Into::into)
    }

    /// This method first gets the balance from `per_coin_type_balance` cache.
    /// On a cache miss, it gets the balance for passed in `coin_type` from
    /// the `all_balance` cache. Only on the second cache miss, we go to the
    /// database (expensive) and update the cache. Notice that db read is
    /// done with `spawn_blocking` as that is expected to block
    pub fn get_balance(&self, owner: Address, coin_type: TypeTag) -> IotaResult<TotalBalance> {
        self.metrics.balance_lookup_from_total.inc();
        let force_disable_cache = read_size_from_env(ENV_VAR_DISABLE_INDEX_CACHE).unwrap_or(0) > 0;
        let cloned_coin_type = coin_type.clone();
        let metrics_cloned = self.metrics.clone();
        let coin_index_cloned = self.tables.coin_index.clone();
        if force_disable_cache {
            return Self::get_balance_from_db(
                metrics_cloned,
                coin_index_cloned,
                owner,
                cloned_coin_type,
            )
            .map_err(|e| IotaError::Execution(format!("Failed to read balance frm DB: {e:?}")));
        }

        let balance = self
            .caches
            .per_coin_type_balance
            .get(&(owner, coin_type.clone()));
        if let Some(balance) = balance {
            return balance;
        }
        // cache miss, lookup in all balance cache
        let all_balance = self.caches.all_balances.get(&owner.clone());
        if let Some(Ok(all_balance)) = all_balance {
            if let Some(balance) = all_balance.get(&coin_type) {
                return Ok(*balance);
            }
        }
        let cloned_coin_type = coin_type.clone();
        let metrics_cloned = self.metrics.clone();
        let coin_index_cloned = self.tables.coin_index.clone();
        self.caches
            .per_coin_type_balance
            .get_with((owner, coin_type), move || {
                Self::get_balance_from_db(
                    metrics_cloned,
                    coin_index_cloned,
                    owner,
                    cloned_coin_type,
                )
                .map_err(|e| IotaError::Execution(format!("Failed to read balance frm DB: {e:?}")))
            })
    }

    /// This method gets the balance for all coin types from the `all_balance`
    /// cache. On a cache miss, we go to the database (expensive) and update
    /// the cache. This cache is dual purpose in the sense that it not only
    /// serves `get_AllBalance()` calls but is also used for serving
    /// `get_Balance()` queries. Notice that db read is performed with
    /// `spawn_blocking` as that is expected to block
    pub fn get_all_balance(
        &self,
        owner: Address,
    ) -> IotaResult<Arc<HashMap<TypeTag, TotalBalance>>> {
        self.metrics.all_balance_lookup_from_total.inc();
        let force_disable_cache = read_size_from_env(ENV_VAR_DISABLE_INDEX_CACHE).unwrap_or(0) > 0;
        let metrics_cloned = self.metrics.clone();
        let coin_index_cloned = self.tables.coin_index.clone();

        if force_disable_cache {
            return Self::get_all_balances_from_db(metrics_cloned, coin_index_cloned, owner)
                .map_err(|e| {
                    IotaError::Execution(format!("Failed to read all balance from DB: {e:?}"))
                });
        }

        self.caches.all_balances.get_with(owner, move || {
            Self::get_all_balances_from_db(metrics_cloned, coin_index_cloned, owner).map_err(|e| {
                IotaError::Execution(format!("Failed to read all balance from DB: {e:?}"))
            })
        })
    }

    /// Read balance for a `Address` and `CoinType` from the backend
    /// database
    pub fn get_balance_from_db(
        metrics: Arc<IndexStoreMetrics>,
        coin_index: DBMap<CoinIndexKey, CoinInfo>,
        owner: Address,
        coin_type: TypeTag,
    ) -> IotaResult<TotalBalance> {
        metrics.balance_lookup_from_db.inc();
        let coin_type_str = coin_type.to_string();
        let coins =
            Self::get_owned_coins_iterator(&coin_index, owner, Some(coin_type_str.clone()))?
                .map(|(_coin_type, obj_id, coin)| (coin_type_str.clone(), obj_id, coin));

        let mut balance = 0i128;
        let mut num_coins = 0;
        for (_coin_type, _obj_id, coin_info) in coins {
            balance += coin_info.balance as i128;
            num_coins += 1;
        }
        Ok(TotalBalance { balance, num_coins })
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

    fn update_per_coin_type_cache(
        &self,
        keys: impl IntoIterator<Item = ((Address, TypeTag), IotaResult<TotalBalance>)>,
    ) -> IotaResult {
        self.caches
            .per_coin_type_balance
            .batch_merge(keys, Self::merge_balance);
        Ok(())
    }

    fn merge_balance(
        old_balance: &IotaResult<TotalBalance>,
        balance_delta: &IotaResult<TotalBalance>,
    ) -> IotaResult<TotalBalance> {
        if let Ok(old_balance) = old_balance {
            if let Ok(balance_delta) = balance_delta {
                Ok(TotalBalance {
                    balance: old_balance.balance + balance_delta.balance,
                    num_coins: old_balance.num_coins + balance_delta.num_coins,
                })
            } else {
                balance_delta.clone()
            }
        } else {
            old_balance.clone()
        }
    }

    fn update_all_balance_cache(
        &self,
        keys: impl IntoIterator<Item = (Address, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>)>,
    ) -> IotaResult {
        self.caches
            .all_balances
            .batch_merge(keys, Self::merge_all_balance);
        Ok(())
    }

    fn merge_all_balance(
        old_balance: &IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>,
        balance_delta: &IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>,
    ) -> IotaResult<Arc<HashMap<TypeTag, TotalBalance>>> {
        if let Ok(old_balance) = old_balance {
            if let Ok(balance_delta) = balance_delta {
                // create a deep copy of the old balance hashmap
                let mut new_balance = old_balance.as_ref().clone();
                for (key, delta) in balance_delta.iter() {
                    let old = new_balance.get(key).unwrap_or(&TotalBalance {
                        balance: 0,
                        num_coins: 0,
                    });
                    let new_total = TotalBalance {
                        balance: old.balance + delta.balance,
                        num_coins: old.num_coins + delta.num_coins,
                    };

                    // Remove entries where num_coins becomes zero to prevent cache bloat
                    if new_total.num_coins == 0 {
                        new_balance.remove(key);
                    } else {
                        new_balance.insert(key.clone(), new_total);
                    }
                }
                Ok(Arc::new(new_balance))
            } else {
                balance_delta.clone()
            }
        } else {
            old_balance.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectId, StructTag, TransactionDigest, TypeTag};
    use iota_types::{
        effects::TransactionEffectsAPI, messages_checkpoint::CheckpointContentsExt,
        test_checkpoint_data_builder::TestCheckpointDataBuilder,
    };
    use prometheus_filtered::Registry;
    use typed_store::Map;

    use super::IndexStore;
    use crate::{checkpoints::CheckpointStore, test_utils::executed_checkpoint};

    /// Opens an `IndexStore` at `path` without running the rebuild path.
    fn open_index_store(path: std::path::PathBuf) -> IndexStore {
        IndexStore::new_without_init(path, &Registry::default(), Some(128))
    }

    /// Closes the store's database, waiting until every handle is released
    /// so the same path can be reopened. Accepts the store owned or in an
    /// `Arc`, as long as the passed handle is the last one.
    async fn close_index_store(index_store: impl std::borrow::Borrow<IndexStore>) {
        let weak_db = std::sync::Arc::downgrade(&index_store.borrow().tables.meta.db);
        drop(index_store);
        assert!(super::wait_for_database_close(weak_db).await);
    }

    /// Closes the store and reopens the same path, as a restart does.
    async fn reopen_index_store(index_store: IndexStore, path: std::path::PathBuf) -> IndexStore {
        close_index_store(index_store).await;
        open_index_store(path)
    }

    /// An empty authority store under `dir`, for driving the rebuild and
    /// backfill paths.
    fn open_authority_store(dir: &std::path::Path) -> std::sync::Arc<super::AuthorityStore> {
        crate::authority::AuthorityStore::open_no_genesis(
            std::sync::Arc::new(
                crate::authority::authority_store_tables::AuthorityPerpetualTables::open(dir, None),
            ),
            false,
            &Registry::default(),
        )
        .unwrap()
    }

    /// An authority state whose genesis checkpoint is executed, plus the
    /// genesis transaction's digest.
    async fn genesis_authority_state() -> (
        std::sync::Arc<crate::authority::AuthorityState>,
        TransactionDigest,
    ) {
        let authority_state = crate::authority::test_authority_builder::TestAuthorityBuilder::new()
            .insert_genesis_checkpoint()
            .build()
            .await;
        let checkpoint_store = &authority_state.checkpoint_store;
        let genesis_checkpoint = checkpoint_store
            .get_checkpoint_by_sequence_number(0)
            .unwrap()
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&genesis_checkpoint)
            .unwrap();
        let genesis_contents = checkpoint_store
            .get_checkpoint_contents(&genesis_checkpoint.contents_digest)
            .unwrap()
            .unwrap();
        let genesis_tx_digest = genesis_contents.iter().next().unwrap().transaction;
        (authority_state, genesis_tx_digest)
    }

    fn mark_checkpoint_executed(checkpoint_store: &CheckpointStore, sequence_number: u64) {
        let checkpoint = executed_checkpoint(0, sequence_number);
        checkpoint_store
            .insert_verified_checkpoint(&checkpoint)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&checkpoint)
            .unwrap();
    }

    /// `CoinInfo::from_object` must reject non-coin objects even when their
    /// BCS contents happen to match `Coin`'s `{UID, u64}` layout.
    #[test]
    fn test_coin_info_from_object_requires_coin_type() {
        use iota_sdk_types::{Address, MoveStruct, Owner, TransactionDigest, Version};
        use iota_types::object::{MoveStructExt, Object};

        let owner = Owner::Address(Address::ZERO);
        let id = ObjectId::random();
        let contents = iota_types::coin::Coin::new(id, 42).to_bcs_bytes();

        let coin = Object::new_move(
            MoveStruct::new_coin(TypeTag::from(StructTag::new_gas()), Version::MIN_VALID_INCL, id, 42),
            owner,
            TransactionDigest::ZERO,
        );
        assert_eq!(super::CoinInfo::from_object(&coin).unwrap().balance, 42);

        let fake = Object::new_move(
            MoveStruct::new_from_execution_with_limit(
                "0x2::not_coin::NotCoin".parse::<StructTag>().unwrap(),
                Version::MIN_VALID_INCL,
                contents,
                256,
            )
            .unwrap(),
            owner,
            TransactionDigest::ZERO,
        );
        assert_eq!(super::CoinInfo::from_object(&fake), None);
    }

    /// A brand-new store is seeded with `meta` and needs no rebuild; once the
    /// executed watermark moves past the (missing) indexed watermark — the
    /// state after a formal-snapshot restore — a rebuild is required.
    #[tokio::test]
    async fn test_missing_watermark_triggers_initialization() {
        let tmp_dir = iota_common::tempdir();
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );

        index_store.tables.seed_meta().unwrap();
        assert!(
            !index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store),
            "a brand-new store on a node with no executed checkpoints needs no rebuild"
        );

        mark_checkpoint_executed(&checkpoint_store, 5);
        assert!(
            index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store),
            "an executed checkpoint past the indexed watermark must trigger a rebuild"
        );

        index_store.tables.watermark.insert(&(), &5).unwrap();
        assert!(
            !index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store)
        );

        // A schema version bump also triggers a rebuild.
        index_store
            .tables
            .meta
            .insert(
                &(),
                &super::MetadataInfo {
                    version: super::CURRENT_DB_VERSION + 1,
                },
            )
            .unwrap();
        assert!(
            index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store)
        );
    }

    /// The JSON-RPC index database of releases that stored it under
    /// `indexes` is removed; its content cannot be adopted anyway.
    #[test]
    fn test_remove_legacy_jsonrpc_indexes_dir() {
        let db_path = iota_common::tempdir();
        let legacy_dir = db_path.path().join("indexes");
        std::fs::create_dir(&legacy_dir).unwrap();
        std::fs::write(legacy_dir.join("CURRENT"), b"stale").unwrap();

        super::remove_legacy_jsonrpc_indexes_dir(db_path.path()).unwrap();
        assert!(!legacy_dir.exists());

        // A second call is a no-op.
        super::remove_legacy_jsonrpc_indexes_dir(db_path.path()).unwrap();
    }

    /// A database written before per-checkpoint indexing (data, but no `meta`
    /// row) must be wiped and rebuilt: nodes restored from a formal snapshot
    /// had a corrupted owner index and non-canonical transaction numbering,
    /// and a database without a watermark cannot prove it is not one of them.
    #[tokio::test]
    async fn test_pre_meta_database_triggers_initialization() {
        let tmp_dir = iota_common::tempdir();
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
        mark_checkpoint_executed(&checkpoint_store, 5);

        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        let owner = iota_types::base_types::dbg_addr(1);
        let object =
            iota_types::object::Object::with_id_owner_for_testing(ObjectId::random(), owner);
        index_store
            .tables
            .owner_index
            .insert(
                &(owner, object.id()),
                &iota_types::base_types::ObjectInfo::from_object(&object),
            )
            .unwrap();

        index_store.tables.seed_meta().unwrap();
        assert!(
            index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store),
            "a database from before per-checkpoint indexing must be rebuilt"
        );
    }

    /// After a rebuild, the history tables are filled by a background replay
    /// that works downwards from the watermark and records its progress
    /// atomically with each checkpoint's rows, so an interrupted replay
    /// resumes where it stopped instead of starting over.
    #[tokio::test]
    async fn test_history_backfill_after_rebuild() {
        let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
        let checkpoint_store = &authority_state.checkpoint_store;

        let index_dir = iota_common::tempdir();
        let index_store = IndexStore::new(
            index_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
            &authority_state.database_for_testing(),
            checkpoint_store,
        )
        .await;
        index_store.wait_for_history_backfill_for_testing().await;

        assert_eq!(
            index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
            Some(0)
        );
        assert_eq!(
            index_store.tables.history_watermark.get(&()).unwrap(),
            Some(0),
            "the backfill must have reached the lowest replayable checkpoint"
        );

        // Simulate a replay interrupted before reaching checkpoint 0:
        // resuming replays it and lowers the marker again.
        index_store
            .tables
            .history_watermark
            .insert(&(), &1)
            .unwrap();
        index_store
            .backfill_history(&authority_state.database_for_testing(), checkpoint_store)
            .unwrap();
        assert_eq!(
            index_store.tables.history_watermark.get(&()).unwrap(),
            Some(0)
        );
        assert_eq!(
            index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
            Some(0)
        );
    }

    /// A formal-snapshot restore builds the JSON-RPC index from the restored
    /// live object set (`JsonRpcIndexRestorer`); a node then opens it in
    /// place instead of rebuilding, and the history backfill has nothing to
    /// do. Dynamic fields are indexed by key only, so the tee needs no
    /// layouts and no particular object order.
    #[tokio::test]
    async fn test_restore_built_store_is_adopted_on_open() {
        use iota_sdk_types::{MoveStruct, Owner, TransactionDigest, Version};
        use iota_types::{
            base_types::dbg_addr,
            object::{MoveStructExt, Object},
        };

        let dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&dir.path().join("checkpoints"));
        // The restore marks the restore checkpoint both executed and pruned.
        let restore_checkpoint = executed_checkpoint(0, 5);
        checkpoint_store
            .insert_verified_checkpoint(&restore_checkpoint)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&restore_checkpoint)
            .unwrap();
        checkpoint_store
            .update_highest_pruned_checkpoint(&restore_checkpoint)
            .unwrap();

        let owner = dbg_addr(1);
        let gas_object = Object::new_gas_with_balance_and_owner_for_testing(100, owner);
        let parent = ObjectId::random();
        let field_id = ObjectId::random();
        let mut field_contents = field_id.into_bytes().to_vec();
        field_contents.extend_from_slice(&7u64.to_le_bytes()); // name
        field_contents.extend_from_slice(&8u64.to_le_bytes()); // value
        let field_object = Object::new_move(
            MoveStruct::new_from_execution_with_limit(
                "0x2::dynamic_field::Field<u64,u64>"
                    .parse::<StructTag>()
                    .unwrap(),
                Version::MIN_VALID_INCL,
                field_contents,
                256,
            )
            .unwrap(),
            Owner::Object(parent),
            TransactionDigest::ZERO,
        );

        // Tee the objects into the restorer, as the snapshot's partition
        // downloads do.
        let index_dir = dir.path().join(super::JSONRPC_INDEXES_DIR);
        let restorer = super::JsonRpcIndexRestorer::open(index_dir.clone()).unwrap();
        let mut partition = restorer.partition_indexer();
        partition.index_object(&gas_object).unwrap();
        partition.index_object(&field_object).unwrap();
        partition.finish().unwrap();
        restorer.finalize(5).await.unwrap();

        // Plant a sentinel row: if it survives the open below, the store was
        // adopted rather than wiped and rebuilt into equal-looking data.
        let sentinel = (ObjectId::random(), ObjectId::random());
        {
            let built = open_index_store(index_dir.clone());
            assert!(
                !built.tables.needs_to_do_initialization(&checkpoint_store),
                "a restore-built store must need no rebuild"
            );
            built
                .tables
                .dynamic_field_index
                .insert(&sentinel, &())
                .unwrap();
            close_index_store(built).await;
        }

        let authority_store = open_authority_store(&dir.path().join("store"));
        let index_store = IndexStore::new(
            index_dir,
            &Registry::default(),
            Some(128),
            &authority_store,
            &checkpoint_store,
        )
        .await;
        index_store.wait_for_history_backfill_for_testing().await;

        assert!(
            index_store
                .dynamic_field_exists(sentinel.0, sentinel.1)
                .unwrap(),
            "the restored database must be opened in place, not rebuilt"
        );

        // The owner and coin tables were built from the teed objects.
        let owned: Vec<_> = index_store
            .get_owner_objects(owner, None, 10, None)
            .unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].object_id, gas_object.id());
        let balance = index_store
            .get_balance(owner, TypeTag::from(StructTag::new_gas()))
            .unwrap();
        assert_eq!(balance.num_coins, 1);

        // The dynamic field was indexed by key, without layout resolution.
        let field_ids: Vec<_> = index_store
            .get_dynamic_field_ids_iterator(parent, None)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(field_ids, vec![field_id]);

        // Watermark at the restore checkpoint, history one past it — nothing
        // for the backfill to replay.
        assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(5));
        assert_eq!(
            index_store.tables.history_watermark.get(&()).unwrap(),
            Some(6)
        );
    }

    /// A stale database (here: written by another schema version) is wiped
    /// and rebuilt through the full open path — bulk-ingestion open, flush,
    /// reopen with default options — and none of its rows survive.
    #[tokio::test]
    async fn test_stale_database_is_wiped_and_rebuilt_on_open() {
        let (authority_state, genesis_tx_digest) = genesis_authority_state().await;
        let checkpoint_store = &authority_state.checkpoint_store;

        let index_dir = iota_common::tempdir();
        let index_store = IndexStore::new(
            index_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
            &authority_state.database_for_testing(),
            checkpoint_store,
        )
        .await;
        index_store.wait_for_history_backfill_for_testing().await;

        // Poison the store and mark it as written by another schema version.
        let poison_field = (ObjectId::random(), ObjectId::random());
        index_store
            .tables
            .dynamic_field_index
            .insert(&poison_field, &())
            .unwrap();
        index_store
            .tables
            .meta
            .insert(
                &(),
                &super::MetadataInfo {
                    version: super::CURRENT_DB_VERSION + 1,
                },
            )
            .unwrap();

        // Release the database before reopening the same path.
        close_index_store(index_store).await;

        let index_store = IndexStore::new(
            index_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
            &authority_state.database_for_testing(),
            checkpoint_store,
        )
        .await;
        index_store.wait_for_history_backfill_for_testing().await;

        assert!(
            !index_store
                .dynamic_field_exists(poison_field.0, poison_field.1)
                .unwrap(),
            "stale rows must not survive the rebuild"
        );
        assert_eq!(
            index_store.get_transaction_seq(&genesis_tx_digest).unwrap(),
            Some(0)
        );
        assert_eq!(
            index_store
                .tables
                .meta
                .get(&())
                .unwrap()
                .map(|meta| meta.version),
            Some(super::CURRENT_DB_VERSION)
        );
        assert_eq!(index_store.tables.watermark.get(&()).unwrap(), Some(0));
        assert_eq!(
            index_store.tables.history_watermark.get(&()).unwrap(),
            Some(0)
        );
    }

    /// After a crash between an index commit and the executed-watermark bump,
    /// the index watermark is ahead of `highest_executed_checkpoint` on
    /// restart. That must not trigger a rebuild: the replayed checkpoint is
    /// skipped through the already-indexed check instead.
    #[tokio::test]
    async fn test_watermark_ahead_of_executed_needs_no_rebuild() {
        let tmp_dir = iota_common::tempdir();
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));
        mark_checkpoint_executed(&checkpoint_store, 5);

        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        index_store.tables.seed_meta().unwrap();
        index_store.tables.watermark.insert(&(), &6).unwrap();

        assert!(
            !index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store),
            "an index watermark ahead of the executed watermark must not trigger a rebuild"
        );
    }

    #[tokio::test]
    async fn test_index_cache() -> anyhow::Result<()> {
        // This test indexes a checkpoint where 10 coins each with balance 100
        // are created for an address. The balance is then going to be read
        // from the db and the cache. It should be 1000. Then, a second
        // checkpoint deletes 3 of those coins, and the balance should be 700,
        // verified from both db and cache. This tests make sure we are
        // invalidating entries in the cache and always reading latest balance.
        let tmp_dir = iota_common::tempdir();
        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        let address = TestCheckpointDataBuilder::derive_address(1);

        let mut builder = TestCheckpointDataBuilder::new(0).start_transaction(0);
        for object_idx in 0..10 {
            builder = builder.create_coin_object(object_idx, 1, 100, TypeTag::from(StructTag::new_gas()));
        }
        let mut builder = builder.finish_transaction();
        let checkpoint = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint, true)?;
        index_store.commit_update_for_checkpoint(0)?;

        let balance_from_db = IndexStore::get_balance_from_db(
            index_store.metrics.clone(),
            index_store.tables.coin_index.clone(),
            address,
            TypeTag::from(StructTag::new_gas()),
        )?;
        let balance = index_store.get_balance(address, TypeTag::from(StructTag::new_gas()))?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.num_coins, 10);

        let all_balance = index_store.get_all_balance(address)?;
        let balance = all_balance
            .get(&TypeTag::from(StructTag::new_gas()))
            .unwrap();
        assert_eq!(*balance, balance_from_db);
        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.num_coins, 10);

        let mut builder = builder.start_transaction(0);
        for object_idx in 0..3 {
            builder = builder.delete_object(object_idx);
        }
        let mut builder = builder.finish_transaction();
        let checkpoint = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint, true)?;
        index_store.commit_update_for_checkpoint(1)?;

        let balance_from_db = IndexStore::get_balance_from_db(
            index_store.metrics.clone(),
            index_store.tables.coin_index.clone(),
            address,
            TypeTag::from(StructTag::new_gas()),
        )?;
        let balance = index_store.get_balance(address, TypeTag::from(StructTag::new_gas()))?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 700);
        assert_eq!(balance.num_coins, 7);
        // Invalidate per coin type balance cache and read from all balance cache to
        // ensure the balance matches
        index_store
            .caches
            .per_coin_type_balance
            .invalidate(&(address, TypeTag::from(StructTag::new_gas())));
        let all_balance = index_store.get_all_balance(address)?;
        assert_eq!(
            all_balance
                .get(&TypeTag::from(StructTag::new_gas()))
                .unwrap()
                .balance,
            700
        );
        assert_eq!(
            all_balance
                .get(&TypeTag::from(StructTag::new_gas()))
                .unwrap()
                .num_coins,
            7
        );
        let balance = index_store.get_balance(address, TypeTag::from(StructTag::new_gas()))?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 700);
        assert_eq!(balance.num_coins, 7);

        Ok(())
    }

    /// Replaying a committed checkpoint (crash recovery before the executed
    /// watermark advanced, or the upgrade to per-checkpoint indexing) must
    /// skip its already-indexed transactions: no new sequence numbers, no
    /// duplicate rows, no double-counted balances.
    #[tokio::test]
    async fn test_index_checkpoint_skips_already_indexed() -> anyhow::Result<()> {
        let tmp_dir = iota_common::tempdir();
        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );
        let address = TestCheckpointDataBuilder::derive_address(1);

        let mut builder = TestCheckpointDataBuilder::new(0)
            .start_transaction(0)
            .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
            .finish_transaction();
        let checkpoint = builder.build_checkpoint();
        let digest = *checkpoint.transactions[0].effects.transaction_digest();

        index_store.index_checkpoint(&checkpoint, true)?;
        index_store.commit_update_for_checkpoint(0)?;
        assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
        assert_eq!(index_store.tables.watermark.get(&())?, Some(0));

        // Replay the same checkpoint.
        index_store.index_checkpoint(&checkpoint, true)?;
        index_store.commit_update_for_checkpoint(0)?;

        assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![digest]
        );
        let balance = index_store.get_balance(address, TypeTag::from(StructTag::new_gas()))?;
        assert_eq!(balance.balance, 100);
        assert_eq!(balance.num_coins, 1);

        Ok(())
    }

    /// Checkpoints of different epochs land in separate history buckets:
    /// queries and cursors chain across them in order, reopening rediscovers
    /// the buckets from the column-family names, and pruning drops whole
    /// epochs wholesale.
    #[tokio::test]
    async fn test_history_epoch_buckets_chain_and_prune() -> anyhow::Result<()> {
        let tmp_dir = iota_common::tempdir();
        let index_store = open_index_store(tmp_dir.path().to_path_buf());

        // One transaction in epoch 0, one in epoch 1.
        let mut builder = TestCheckpointDataBuilder::new(0)
            .with_epoch(0)
            .start_transaction(0)
            .create_coin_object(0, 1, 100, TypeTag::from(StructTag::new_gas()))
            .finish_transaction();
        let checkpoint_epoch_0 = builder.build_checkpoint();
        let tx_0 = *checkpoint_epoch_0.transactions[0]
            .effects
            .transaction_digest();
        index_store.index_checkpoint(&checkpoint_epoch_0, true)?;
        index_store.commit_update_for_checkpoint(0)?;

        let mut builder = builder
            .with_epoch(1)
            .start_transaction(1)
            .create_coin_object(1, 1, 100, TypeTag::from(StructTag::new_gas()))
            .finish_transaction();
        let checkpoint_epoch_1 = builder.build_checkpoint();
        let tx_1 = *checkpoint_epoch_1.transactions[0]
            .effects
            .transaction_digest();
        index_store.index_checkpoint(&checkpoint_epoch_1, true)?;
        index_store.commit_update_for_checkpoint(1)?;

        // Forward and reverse iteration chain across the buckets in order.
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![tx_0, tx_1]
        );
        assert_eq!(
            index_store.get_transactions(None, None, None, true)?,
            vec![tx_1, tx_0]
        );
        // An exclusive cursor crosses the bucket boundary.
        assert_eq!(
            index_store.get_transactions(None, Some(tx_1), None, true)?,
            vec![tx_0]
        );
        assert_eq!(
            index_store.get_transactions(None, Some(tx_0), None, false)?,
            vec![tx_1]
        );
        // A limit landing exactly on the bucket boundary stops there.
        assert_eq!(
            index_store.get_transactions(None, None, Some(1), false)?,
            vec![tx_0]
        );
        assert_eq!(
            index_store.get_transactions(None, None, Some(1), true)?,
            vec![tx_1]
        );

        // Reopening rediscovers the buckets from the column-family names.
        let index_store = reopen_index_store(index_store, tmp_dir.path().to_path_buf()).await;
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![tx_0, tx_1]
        );

        // Pruning to one retained epoch drops epoch 0's bucket wholesale,
        // and pruning again is a no-op.
        assert_eq!(index_store.prune(1)?, Some(1));
        assert_eq!(index_store.get_transaction_seq(&tx_0)?, None);
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![tx_1]
        );
        assert_eq!(index_store.prune(1)?, Some(1));

        // The dropped bucket stays gone after another reopen.
        let weak_db = std::sync::Arc::downgrade(&index_store.tables.meta.db);
        drop(index_store);
        while weak_db.strong_count() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![tx_1]
        );

        Ok(())
    }

    /// Tables of one bucket share a column family, separated only by their
    /// tag byte: a full-range scan of one table must not yield a neighboring
    /// table's rows, whose bytes do not deserialize under its types.
    #[tokio::test]
    async fn test_history_tables_do_not_bleed_across_tags() {
        use iota_sdk_types::TransactionDigest;

        let tmp_dir = iota_common::tempdir();
        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        let bucket = index_store.ensure_history_bucket(0).unwrap();

        let digest = TransactionDigest::random();
        let mut batch = index_store.tables.meta.batch();
        // Adjacent tags: `tx_order` and `txs_seq`.
        batch
            .insert_batch_tagged(&bucket.tx_order, [(7u64, digest)])
            .unwrap();
        batch
            .insert_batch_tagged(&bucket.txs_seq, [(digest, 7u64)])
            .unwrap();
        batch.write().unwrap();

        let rows: Vec<_> = bucket
            .tx_order
            .safe_range_iter(u64::MIN..=u64::MAX)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![(7, digest)]);
        let rows: Vec<_> = bucket
            .tx_order
            .safe_range_iter_reversed(u64::MIN..=u64::MAX)
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![(7, digest)]);
        let rows: Vec<_> = bucket
            .txs_seq
            .safe_range_iter_reversed(TransactionDigest::ZERO..=[0xff; 32].into())
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![(digest, 7)]);
    }

    #[tokio::test]
    async fn test_get_transaction_by_move_function() {
        use iota_sdk_types::TransactionDigest;

        let tmp_dir = iota_common::tempdir();
        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        let bucket = index_store.ensure_history_bucket(0).unwrap();
        let mut batch = index_store.tables.meta.batch();
        batch
            .insert_batch_tagged(
                &bucket.txs_by_move_function,
                [
                    (
                        (
                            ObjectId::new([1; 32]),
                            "mod".to_string(),
                            "f".to_string(),
                            0,
                        ),
                        TransactionDigest::from([0; 32]),
                    ),
                    (
                        (
                            ObjectId::new([1; 32]),
                            "mod".to_string(),
                            "Z".repeat(128),
                            0,
                        ),
                        TransactionDigest::from([1; 32]),
                    ),
                    (
                        (
                            ObjectId::new([1; 32]),
                            "mod".to_string(),
                            "f".repeat(128),
                            0,
                        ),
                        TransactionDigest::from([2; 32]),
                    ),
                    (
                        (
                            ObjectId::new([1; 32]),
                            "mod".to_string(),
                            "z".repeat(128),
                            0,
                        ),
                        TransactionDigest::from([3; 32]),
                    ),
                ],
            )
            .unwrap();
        batch.write().unwrap();

        let mut v = index_store
            .get_transactions_by_move_function(
                ObjectId::new([1; 32]),
                Some("mod".to_string()),
                None,
                None,
                None,
                false,
            )
            .unwrap();
        let v_rev = index_store
            .get_transactions_by_move_function(
                ObjectId::new([1; 32]),
                Some("mod".to_string()),
                None,
                None,
                None,
                true,
            )
            .unwrap();
        assert_eq!(
            v.len(),
            4,
            "an unset function must span the whole identifier range"
        );
        v.reverse();
        assert_eq!(v, v_rev);
    }

    /// Events chain across epoch buckets in global sequence order: with all
    /// checkpoint timestamps equal, ordering falls through to the sequence
    /// key, so correctness depends entirely on scanning the buckets in epoch
    /// order.
    #[tokio::test]
    async fn test_events_chain_across_epoch_buckets() -> anyhow::Result<()> {
        use iota_sdk_types::Event;

        let tmp_dir = iota_common::tempdir();
        let index_store = open_index_store(tmp_dir.path().to_path_buf());
        let event = || Event {
            package_id: ObjectId::ZERO,
            module: iota_sdk_types::Identifier::from_static("test"),
            sender: TestCheckpointDataBuilder::derive_address(0),
            type_: StructTag::new_gas(),
            contents: vec![],
        };

        let mut builder = TestCheckpointDataBuilder::new(0)
            .with_epoch(0)
            .start_transaction(0)
            .with_events(vec![event()])
            .finish_transaction();
        let checkpoint_epoch_0 = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint_epoch_0, true)?;
        index_store.commit_update_for_checkpoint(0)?;

        let mut builder = builder
            .with_epoch(1)
            .start_transaction(1)
            .with_events(vec![event()])
            .finish_transaction();
        let checkpoint_epoch_1 = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint_epoch_1, true)?;
        index_store.commit_update_for_checkpoint(1)?;

        let forward = index_store.event_iterator(0, u64::MAX, 0, 0, 10, false)?;
        assert_eq!(forward.len(), 2);
        let descending = index_store.event_iterator(0, u64::MAX, u64::MAX, usize::MAX, 10, true)?;
        assert_eq!(
            descending,
            forward.iter().rev().cloned().collect::<Vec<_>>(),
            "descending must mirror the forward chain across the buckets"
        );

        Ok(())
    }
}
