// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! IndexStore supports creation of various ancillary indexes of state in
//! IotaDataStore. The main user of this data is the explorer.

use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use bincode::Options;
use either::Either;
use iota_common::try_iterator_ext::TryIteratorExt;
use iota_json_rpc_types::{IotaObjectDataFilter, TransactionFilter};
use iota_storage::{mutex_table::MutexTable, sharded_lru::ShardedLruCache};
use iota_types::{
    base_types::{
        IotaAddress, ObjectDigest, ObjectID, ObjectInfo, ObjectRef, SequenceNumber,
        TransactionDigest, TxSequenceNumber,
    },
    digests::TransactionEventsDigest,
    dynamic_field::{self, DynamicFieldInfo},
    effects::TransactionEvents,
    error::{IotaError, IotaResult, UserInputError},
    inner_temporary_store::TxCoins,
    object::{Object, Owner},
    parse_iota_struct_tag,
};
use itertools::Itertools;
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
use parking_lot::ArcMutexGuard;
use prometheus::{
    IntCounter, IntCounterVec, Registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, info, trace};
use typed_store::{
    DBMapUtils, TypedStoreError,
    rocks::{
        DBBatch, DBMap, DBMapTableConfigMap, DBOptions, MetricConf, default_db_options,
        read_size_from_env,
    },
    rocksdb::compaction_filter::Decision,
    traits::Map,
};

type OwnedMutexGuard<T> = ArcMutexGuard<parking_lot::RawMutex, T>;

type OwnerIndexKey = (IotaAddress, ObjectID);
type CoinIndexKey = (IotaAddress, String, ObjectID);
type DynamicFieldKey = (ObjectID, ObjectID);
type EventId = (TxSequenceNumber, usize);
type EventIndex = (TransactionEventsDigest, TransactionDigest, u64);
type AllBalance = HashMap<TypeTag, TotalBalance>;

pub const MAX_TX_RANGE_SIZE: u64 = 4096;

pub const MAX_GET_OWNED_OBJECT_SIZE: usize = 256;
const ENV_VAR_COIN_INDEX_BLOCK_CACHE_SIZE_MB: &str = "COIN_INDEX_BLOCK_CACHE_MB";
const ENV_VAR_DISABLE_INDEX_CACHE: &str = "DISABLE_INDEX_CACHE";
const ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE: &str = "INVALIDATE_INSTEAD_OF_UPDATE";

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
    pub new_dynamic_fields: Vec<(DynamicFieldKey, DynamicFieldInfo)>,
}

#[derive(Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CoinInfo {
    pub version: SequenceNumber,
    pub digest: ObjectDigest,
    pub balance: u64,
    pub previous_transaction: TransactionDigest,
}

impl CoinInfo {
    pub fn from_object(object: &Object) -> Option<CoinInfo> {
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
    per_coin_type_balance: ShardedLruCache<(IotaAddress, TypeTag), IotaResult<TotalBalance>>,
    all_balances: ShardedLruCache<IotaAddress, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>>,
    locks: MutexTable<IotaAddress>,
}

#[derive(Default)]
pub struct IndexStoreCacheUpdates {
    _locks: Vec<OwnedMutexGuard<()>>,
    per_coin_type_balance_changes: Vec<((IotaAddress, TypeTag), IotaResult<TotalBalance>)>,
    all_balance_changes: Vec<(IotaAddress, IotaResult<Arc<AllBalance>>)>,
}

/// The `IndexStoreTables` struct defines a set of `DBMaps` used to index
/// various aspects of transaction and object data. Each field corresponds to a
/// specific index, with keys such as `IotaAddress`, `TransactionDigest`, etc.
/// Each mapping is configured with custom database options.
#[derive(DBMapUtils)]
pub struct IndexStoreTables {
    /// Index from iota address to transactions initiated by that address.
    transactions_from_addr: DBMap<(IotaAddress, TxSequenceNumber), TransactionDigest>,

    /// Index from iota address to transactions that were sent to that address.
    transactions_to_addr: DBMap<(IotaAddress, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that used that object id as input.
    transactions_by_input_object_id: DBMap<(ObjectID, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that modified/created that object
    /// id.
    transactions_by_mutated_object_id: DBMap<(ObjectID, TxSequenceNumber), TransactionDigest>,

    /// Index from package id, module and function identifier to transactions
    /// that used that moce function call as input.
    transactions_by_move_function:
        DBMap<(ObjectID, String, String, TxSequenceNumber), TransactionDigest>,

    /// Ordering of all indexed transactions.
    transaction_order: DBMap<TxSequenceNumber, TransactionDigest>,

    /// Index from transaction digest to sequence number.
    transactions_seq: DBMap<TransactionDigest, TxSequenceNumber>,

    /// This is an index of object references to currently existing objects,
    /// indexed by the composite key of the IotaAddress of their owner and
    /// the object ID of the object. This composite index allows an
    /// efficient iterator to list all objected currently owned
    /// by a specific user, and their object reference.
    owner_index: DBMap<OwnerIndexKey, ObjectInfo>,

    coin_index: DBMap<CoinIndexKey, CoinInfo>,

    /// This is an index of object references to currently existing dynamic
    /// field object, indexed by the composite key of the object ID of their
    /// parent and the object ID of the dynamic field object. This composite
    /// index allows an efficient iterator to list all objects currently owned
    /// by a specific object, and their object reference.
    dynamic_field_index: DBMap<DynamicFieldKey, DynamicFieldInfo>,

    event_order: DBMap<EventId, EventIndex>,

    event_by_move_module: DBMap<(ModuleId, EventId), EventIndex>,

    event_by_move_event: DBMap<(StructTag, EventId), EventIndex>,

    event_by_event_module: DBMap<(ModuleId, EventId), EventIndex>,

    event_by_sender: DBMap<(IotaAddress, EventId), EventIndex>,

    event_by_time: DBMap<(u64, EventId), EventIndex>,

    pruner_watermark: DBMap<(), TxSequenceNumber>,
}

impl IndexStoreTables {
    pub fn owner_index(&self) -> &DBMap<OwnerIndexKey, ObjectInfo> {
        &self.owner_index
    }

    pub fn coin_index(&self) -> &DBMap<CoinIndexKey, CoinInfo> {
        &self.coin_index
    }
}

/// The `IndexStore` enables users to access and manage indexed transaction
/// data, including ownership and balance information for different objects and
/// coins.
pub struct IndexStore {
    next_sequence_number: AtomicU64,
    tables: IndexStoreTables,
    caches: IndexStoreCaches,
    metrics: Arc<IndexStoreMetrics>,
    max_type_length: u64,
    pruner_watermark: Arc<AtomicU64>,
}

struct JsonRpcCompactionMetrics {
    key_removed: IntCounterVec,
    key_kept: IntCounterVec,
    key_error: IntCounterVec,
}

impl JsonRpcCompactionMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        Arc::new(Self {
            key_removed: register_int_counter_vec_with_registry!(
                "json_rpc_compaction_filter_key_removed",
                "Compaction key removed",
                &["cf"],
                registry
            )
            .unwrap(),
            key_kept: register_int_counter_vec_with_registry!(
                "json_rpc_compaction_filter_key_kept",
                "Compaction key kept",
                &["cf"],
                registry
            )
            .unwrap(),
            key_error: register_int_counter_vec_with_registry!(
                "json_rpc_compaction_filter_key_error",
                "Compaction error",
                &["cf"],
                registry
            )
            .unwrap(),
        })
    }
}

fn compaction_filter_config<T: DeserializeOwned>(
    name: &str,
    metrics: Arc<JsonRpcCompactionMetrics>,
    mut db_options: DBOptions,
    pruner_watermark: Arc<AtomicU64>,
    extractor: impl Fn(T) -> TxSequenceNumber + Send + Sync + 'static,
    by_key: bool,
) -> DBOptions {
    let cf = name.to_string();
    db_options
        .options
        .set_compaction_filter(name, move |_, key, value| {
            let bytes = if by_key { key } else { value };
            let deserializer = bincode::DefaultOptions::new()
                .with_big_endian()
                .with_fixint_encoding();
            match deserializer.deserialize(bytes) {
                Ok(key_data) => {
                    let sequence_number = extractor(key_data);
                    if sequence_number < pruner_watermark.load(Ordering::Relaxed) {
                        metrics.key_removed.with_label_values(&[&cf]).inc();
                        Decision::Remove
                    } else {
                        metrics.key_kept.with_label_values(&[&cf]).inc();
                        Decision::Keep
                    }
                }
                Err(_) => {
                    metrics.key_error.with_label_values(&[&cf]).inc();
                    Decision::Keep
                }
            }
        });
    db_options
}

fn compaction_filter_config_by_key<T: DeserializeOwned>(
    name: &str,
    metrics: Arc<JsonRpcCompactionMetrics>,
    db_options: DBOptions,
    pruner_watermark: Arc<AtomicU64>,
    extractor: impl Fn(T) -> TxSequenceNumber + Send + Sync + 'static,
) -> DBOptions {
    compaction_filter_config(name, metrics, db_options, pruner_watermark, extractor, true)
}

fn coin_index_table_default_config() -> DBOptions {
    default_db_options()
        .optimize_for_write_throughput()
        .optimize_for_read(
            read_size_from_env(ENV_VAR_COIN_INDEX_BLOCK_CACHE_SIZE_MB).unwrap_or(5 * 1024),
        )
        .disable_write_throttling()
}

impl IndexStore {
    pub fn new(path: PathBuf, registry: &Registry, max_type_length: Option<u64>) -> Self {
        let db_options = default_db_options().disable_write_throttling();
        let pruner_watermark = Arc::new(AtomicU64::new(0));
        let compaction_metrics = JsonRpcCompactionMetrics::new(registry);
        let table_options = DBMapTableConfigMap::new(BTreeMap::from([
            (
                "transactions_from_addr".to_string(),
                compaction_filter_config_by_key(
                    "transactions_from_addr",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, id): (IotaAddress, TxSequenceNumber)| id,
                ),
            ),
            (
                "transactions_to_addr".to_string(),
                compaction_filter_config_by_key(
                    "transactions_to_addr",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, sequence_number): (IotaAddress, TxSequenceNumber)| sequence_number,
                ),
            ),
            (
                "transactions_by_move_function".to_string(),
                compaction_filter_config_by_key(
                    "transactions_by_move_function",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, _, _, id): (ObjectID, String, String, TxSequenceNumber)| id,
                ),
            ),
            (
                "transaction_order".to_string(),
                compaction_filter_config_by_key(
                    "transaction_order",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |sequence_number: TxSequenceNumber| sequence_number,
                ),
            ),
            (
                "transactions_seq".to_string(),
                compaction_filter_config(
                    "transactions_seq",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |sequence_number: TxSequenceNumber| sequence_number,
                    false,
                ),
            ),
            ("coin_index".to_string(), coin_index_table_default_config()),
            (
                "event_order".to_string(),
                compaction_filter_config_by_key(
                    "event_order",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |event_id: EventId| event_id.0,
                ),
            ),
            (
                "event_by_move_module".to_string(),
                compaction_filter_config_by_key(
                    "event_by_move_module",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, event_id): (ModuleId, EventId)| event_id.0,
                ),
            ),
            (
                "event_by_event_module".to_string(),
                compaction_filter_config_by_key(
                    "event_by_event_module",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, event_id): (ModuleId, EventId)| event_id.0,
                ),
            ),
            (
                "event_by_sender".to_string(),
                compaction_filter_config_by_key(
                    "event_by_sender",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, event_id): (IotaAddress, EventId)| event_id.0,
                ),
            ),
            (
                "event_by_time".to_string(),
                compaction_filter_config_by_key(
                    "event_by_time",
                    compaction_metrics,
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, event_id): (u64, EventId)| event_id.0,
                ),
            ),
        ]));
        let tables = IndexStoreTables::open_tables_read_write(
            path,
            MetricConf::new("index"),
            Some(db_options.options),
            Some(table_options),
        );

        let metrics = IndexStoreMetrics::new(registry);
        let caches = IndexStoreCaches {
            per_coin_type_balance: ShardedLruCache::new(1_000_000, 1000),
            all_balances: ShardedLruCache::new(1_000_000, 1000),
            locks: MutexTable::new(128),
        };
        let next_sequence_number = tables
            .transaction_order
            .reversed_safe_iter_with_bounds(None, None)
            .expect("failed to initialize indexes")
            .next()
            .transpose()
            .expect("failed to initialize indexes")
            .map(|(seq, _)| seq + 1)
            .unwrap_or(0)
            .into();
        let pruner_watermark_value = tables
            .pruner_watermark
            .get(&())
            .expect("failed to initialize index tables")
            .unwrap_or(0);
        pruner_watermark.store(pruner_watermark_value, Ordering::Relaxed);

        Self {
            tables,
            next_sequence_number,
            caches,
            metrics: Arc::new(metrics),
            max_type_length: max_type_length.unwrap_or(128),
            pruner_watermark,
        }
    }

    pub fn tables(&self) -> &IndexStoreTables {
        &self.tables
    }

    pub fn index_coin(
        &self,
        digest: &TransactionDigest,
        batch: &mut DBBatch,
        object_index_changes: &ObjectIndexChanges,
        tx_coins: Option<TxCoins>,
    ) -> IotaResult<IndexStoreCacheUpdates> {
        // In production if this code path is hit, we should expect `tx_coins` to not be
        // None. However, in many tests today we do not distinguish validator
        // and/or fullnode, so we gracefully exist here.
        if tx_coins.is_none() {
            return Ok(IndexStoreCacheUpdates::default());
        }
        // Acquire locks on changed coin owners
        let mut addresses: HashSet<IotaAddress> = HashSet::new();
        addresses.extend(
            object_index_changes
                .deleted_owners
                .iter()
                .map(|(owner, _)| *owner),
        );
        addresses.extend(
            object_index_changes
                .new_owners
                .iter()
                .map(|((owner, _), _)| *owner),
        );
        let _locks = self.caches.locks.acquire_locks(addresses.into_iter());
        let mut balance_changes: HashMap<IotaAddress, HashMap<TypeTag, TotalBalance>> =
            HashMap::new();
        // Index coin info
        let (input_coins, written_coins) = tx_coins.unwrap();
        // 1. Delete old owner if the object is deleted or transferred to a new owner,
        // by looking at `object_index_changes.deleted_owners`.
        // Objects in `deleted_owners` must be coin type (see
        // `AuthorityState::commit_certificate`).
        let coin_delete_keys = object_index_changes
            .deleted_owners
            .iter()
            .filter_map(|(owner, obj_id)| {
                let object = input_coins.get(obj_id).or(written_coins.get(obj_id))?;
                let coin_type_tag = object.coin_type_maybe().unwrap_or_else(|| {
                    panic!(
                        "object_id: {obj_id:?} is not a coin type, input_coins: {input_coins:?}, written_coins: {written_coins:?}, tx_digest: {digest:?}"
                    )
                });
                let map = balance_changes.entry(*owner).or_default();
                let entry = map.entry(coin_type_tag.clone()).or_insert(TotalBalance {
                    num_coins: 0,
                    balance: 0
                });
                if let Ok(Some(coin_info)) = &self.tables.coin_index.get(&(*owner, coin_type_tag.to_string(), *obj_id)) {
                    entry.num_coins -= 1;
                    entry.balance -= coin_info.balance as i128;
                }
                Some((*owner, coin_type_tag.to_string(), *obj_id))
            }).collect::<Vec<_>>();
        trace!(
            tx_digest=?digest,
            "coin_delete_keys: {:?}",
            coin_delete_keys,
        );
        batch.delete_batch(&self.tables.coin_index, coin_delete_keys.into_iter())?;

        // 2. Upsert new owner, by looking at `object_index_changes.new_owners`.
        // For a object to appear in `new_owners`, it must be owned by `Owner::Address`
        // after the tx. It also must not be deleted, hence appear in
        // written_coins (see `AuthorityState::commit_certificate`) It also must
        // be a coin type (see `AuthorityState::commit_certificate`).
        // Here the coin could be transferred to a new address, to simply have the
        // metadata changed (digest, balance etc) due to a successful or failed
        // transaction.
        let coin_add_keys = object_index_changes
        .new_owners
        .iter()
        .filter_map(|((owner, obj_id), obj_info)| {
            // If it's in written_coins, then it's not a coin. Skip it.
            let obj = written_coins.get(obj_id)?;
            let coin_type_tag = obj.coin_type_maybe().unwrap_or_else(|| {
                panic!(
                    "object_id: {obj_id:?} in written_coins is not a coin type, written_coins: {written_coins:?}, tx_digest: {digest:?}"
                )
            });
            let coin = obj.as_coin_maybe().unwrap_or_else(|| {
                panic!(
                    "object_id: {obj_id:?} in written_coins cannot be deserialized as a Coin, written_coins: {written_coins:?}, tx_digest: {digest:?}"
                )
            });
            let map = balance_changes.entry(*owner).or_default();
            let entry = map.entry(coin_type_tag.clone()).or_insert(TotalBalance {
                num_coins: 0,
                balance: 0
            });
            let result = self.tables.coin_index.get(&(*owner, coin_type_tag.to_string(), *obj_id));
            if let Ok(Some(coin_info)) = &result {
                entry.balance -= coin_info.balance as i128;
                entry.balance += coin.balance.value() as i128;
            } else if let Ok(None) = &result {
                entry.num_coins += 1;
                entry.balance += coin.balance.value() as i128;
            }
            Some(((*owner, coin_type_tag.to_string(), *obj_id), (CoinInfo {version: obj_info.version, digest: obj_info.digest, balance: coin.balance.value(), previous_transaction: *digest})))
        }).collect::<Vec<_>>();
        trace!(
            tx_digest=?digest,
            "coin_add_keys: {:?}",
            coin_add_keys,
        );

        batch.insert_batch(&self.tables.coin_index, coin_add_keys.into_iter())?;

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
        let cache_updates = IndexStoreCacheUpdates {
            _locks,
            per_coin_type_balance_changes,
            all_balance_changes,
        };
        Ok(cache_updates)
    }

    /// Indexes a transaction by updating various indices in the `IndexStore`
    /// with the provided transaction details.
    pub fn index_tx(
        &self,
        sender: IotaAddress,
        active_inputs: impl Iterator<Item = ObjectID>,
        mutated_objects: impl Iterator<Item = (ObjectRef, Owner)> + Clone,
        move_functions: impl Iterator<Item = (ObjectID, String, String)> + Clone,
        events: &TransactionEvents,
        object_index_changes: ObjectIndexChanges,
        digest: &TransactionDigest,
        timestamp_ms: u64,
        tx_coins: Option<TxCoins>,
    ) -> IotaResult<u64> {
        let sequence = self.next_sequence_number.fetch_add(1, Ordering::SeqCst);
        let mut batch = self.tables.transactions_from_addr.batch();

        batch.insert_batch(
            &self.tables.transaction_order,
            std::iter::once((sequence, *digest)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_seq,
            std::iter::once((*digest, sequence)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_from_addr,
            std::iter::once(((sender, sequence), *digest)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_by_input_object_id,
            active_inputs.map(|id| ((id, sequence), *digest)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_by_mutated_object_id,
            mutated_objects
                .clone()
                .map(|(obj_ref, _)| ((obj_ref.0, sequence), *digest)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_by_move_function,
            move_functions
                .map(|(obj_id, module, function)| ((obj_id, module, function, sequence), *digest)),
        )?;

        batch.insert_batch(
            &self.tables.transactions_to_addr,
            mutated_objects.filter_map(|(_, owner)| {
                owner
                    .get_address_owner_address()
                    .ok()
                    .map(|addr| ((addr, sequence), digest))
            }),
        )?;

        // Coin Index
        let cache_updates = self.index_coin(digest, &mut batch, &object_index_changes, tx_coins)?;

        // Owner index
        batch.delete_batch(
            &self.tables.owner_index,
            object_index_changes.deleted_owners.into_iter(),
        )?;
        batch.delete_batch(
            &self.tables.dynamic_field_index,
            object_index_changes.deleted_dynamic_fields.into_iter(),
        )?;

        batch.insert_batch(
            &self.tables.owner_index,
            object_index_changes.new_owners.into_iter(),
        )?;

        batch.insert_batch(
            &self.tables.dynamic_field_index,
            object_index_changes.new_dynamic_fields.into_iter(),
        )?;

        // events
        let event_digest = events.digest();
        batch.insert_batch(
            &self.tables.event_order,
            events
                .data
                .iter()
                .enumerate()
                .map(|(i, _)| ((sequence, i), (event_digest, *digest, timestamp_ms))),
        )?;
        batch.insert_batch(
            &self.tables.event_by_move_module,
            events
                .data
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    (
                        i,
                        ModuleId::new(e.package_id.into(), e.transaction_module.clone()),
                    )
                })
                .map(|(i, m)| ((m, (sequence, i)), (event_digest, *digest, timestamp_ms))),
        )?;
        batch.insert_batch(
            &self.tables.event_by_sender,
            events.data.iter().enumerate().map(|(i, e)| {
                (
                    (e.sender, (sequence, i)),
                    (event_digest, *digest, timestamp_ms),
                )
            }),
        )?;
        batch.insert_batch(
            &self.tables.event_by_move_event,
            events.data.iter().enumerate().map(|(i, e)| {
                (
                    (e.type_.clone(), (sequence, i)),
                    (event_digest, *digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch(
            &self.tables.event_by_time,
            events.data.iter().enumerate().map(|(i, _)| {
                (
                    (timestamp_ms, (sequence, i)),
                    (event_digest, *digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch(
            &self.tables.event_by_event_module,
            events.data.iter().enumerate().map(|(i, e)| {
                (
                    (
                        ModuleId::new(e.type_.address, e.type_.module.clone()),
                        (sequence, i),
                    ),
                    (event_digest, *digest, timestamp_ms),
                )
            }),
        )?;

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

        batch.write()?;

        if !invalidate_caches {
            // We cannot update the cache before updating the db or else on failing to write
            // to db we will update the cache twice). However, this only means
            // cache is eventually consistent with the db (within a very short
            // delay)
            self.update_per_coin_type_cache(cache_updates.per_coin_type_balance_changes)?;
            self.update_all_balance_cache(cache_updates.all_balance_changes)?;
        }
        Ok(sequence)
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
                if reverse {
                    let iter = self
                        .tables
                        .transaction_order
                        .reversed_safe_iter_with_bounds(
                            None,
                            Some(cursor.unwrap_or(TxSequenceNumber::MAX)),
                        )?
                        .skip(usize::from(cursor.is_some()))
                        .map(|result| result.map(|(_, digest)| digest));
                    if let Some(limit) = limit {
                        Ok(iter.take(limit).collect::<Result<Vec<_>, _>>()?)
                    } else {
                        Ok(iter.collect::<Result<Vec<_>, _>>()?)
                    }
                } else {
                    let iter = self
                        .tables
                        .transaction_order
                        .safe_iter_with_bounds(Some(cursor.unwrap_or(TxSequenceNumber::MIN)), None)
                        .skip(usize::from(cursor.is_some()))
                        .map(|result| result.map(|(_, digest)| digest));
                    if let Some(limit) = limit {
                        Ok(iter.take(limit).collect::<Result<Vec<_>, _>>()?)
                    } else {
                        Ok(iter.collect::<Result<Vec<_>, _>>()?)
                    }
                }
            }
        }
    }

    fn get_transactions_from_index<KeyT: Clone + Serialize + DeserializeOwned + PartialEq>(
        index: &DBMap<(KeyT, TxSequenceNumber), TransactionDigest>,
        key: KeyT,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        let iter = if reverse {
            Either::Left(index.reversed_safe_iter_with_bounds(
                None,
                Some((key.clone(), cursor.unwrap_or(TxSequenceNumber::MAX))),
            )?)
        } else {
            Either::Right(index.safe_iter_with_bounds(
                Some((key.clone(), cursor.unwrap_or(TxSequenceNumber::MIN))),
                None,
            ))
        };
        iter
            // skip one more if exclusive cursor is Some
            .skip(usize::from(cursor.is_some()))
            .try_take_map_while_and_collect(limit, |((id, _), _)| *id == key, |(_, digest)| digest)
            .map_err(Into::into)
    }

    pub fn get_transactions_by_input_object(
        &self,
        input_object: ObjectID,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        Self::get_transactions_from_index(
            &self.tables.transactions_by_input_object_id,
            input_object,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_by_mutated_object(
        &self,
        mutated_object: ObjectID,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        Self::get_transactions_from_index(
            &self.tables.transactions_by_mutated_object_id,
            mutated_object,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_from_addr(
        &self,
        addr: IotaAddress,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        Self::get_transactions_from_index(
            &self.tables.transactions_from_addr,
            addr,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transactions_by_move_function(
        &self,
        package: ObjectID,
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

        let cursor_val = cursor.unwrap_or(if reverse {
            TxSequenceNumber::MAX
        } else {
            TxSequenceNumber::MIN
        });

        let max_string = "z".repeat(self.max_type_length.try_into().unwrap());
        let module_val = module.clone().unwrap_or(if reverse {
            max_string.clone()
        } else {
            "".to_string()
        });

        let function_val =
            function
                .clone()
                .unwrap_or(if reverse { max_string } else { "".to_string() });

        let key = (package, module_val, function_val, cursor_val);
        let iter = if reverse {
            Either::Left(
                self.tables
                    .transactions_by_move_function
                    .reversed_safe_iter_with_bounds(None, Some(key))?,
            )
        } else {
            Either::Right(
                self.tables
                    .transactions_by_move_function
                    .safe_iter_with_bounds(Some(key), None),
            )
        };
        iter
            // skip one more if exclusive cursor is Some
            .skip(usize::from(cursor.is_some()))
            .try_take_map_while_and_collect(
                limit,
                |((id, m, f, _), _)| {
                    *id == package
                        && module.as_ref().map(|x| x == m).unwrap_or(true)
                        && function.as_ref().map(|x| x == f).unwrap_or(true)
                },
                |(_, digest)| digest,
            )
            .map_err(Into::into)
    }

    pub fn get_transactions_to_addr(
        &self,
        addr: IotaAddress,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        Self::get_transactions_from_index(
            &self.tables.transactions_to_addr,
            addr,
            cursor,
            limit,
            reverse,
        )
    }

    pub fn get_transaction_seq(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TxSequenceNumber>> {
        Ok(self.tables.transactions_seq.get(digest)?)
    }

    pub fn all_events(
        &self,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        Ok(if descending {
            self.tables
                .event_order
                .reversed_safe_iter_with_bounds(None, Some((tx_seq, event_seq)))?
                .take(limit)
                .map(|result| {
                    result.map(|((_, event_seq), (digest, tx_digest, time))| {
                        (digest, tx_digest, event_seq, time)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            self.tables
                .event_order
                .safe_iter_with_bounds(Some((tx_seq, event_seq)), None)
                .take(limit)
                .map(|result| {
                    result.map(|((_, event_seq), (digest, tx_digest, time))| {
                        (digest, tx_digest, event_seq, time)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        })
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
        let iter = if descending {
            Either::Left(
                self.tables
                    .event_order
                    .reversed_safe_iter_with_bounds(None, Some((min(tx_seq, seq), event_seq)))?,
            )
        } else {
            Either::Right(
                self.tables
                    .event_order
                    .safe_iter_with_bounds(Some((max(tx_seq, seq), event_seq)), None),
            )
        };
        iter.try_take_map_while_and_collect(
            Some(limit),
            |((tx, _), _)| tx == &seq,
            |((_, event_seq), (digest, tx_digest, time))| (digest, tx_digest, event_seq, time),
        )
        .map_err(Into::into)
    }

    fn get_event_from_index<KeyT: Clone + PartialEq + Serialize + DeserializeOwned>(
        index: &DBMap<(KeyT, EventId), (TransactionEventsDigest, TransactionDigest, u64)>,
        key: &KeyT,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        let iter =
            if descending {
                Either::Left(index.reversed_safe_iter_with_bounds(
                    None,
                    Some((key.clone(), (tx_seq, event_seq))),
                )?)
            } else {
                Either::Right(
                    index.safe_iter_with_bounds(Some((key.clone(), (tx_seq, event_seq))), None),
                )
            };
        iter.try_take_map_while_and_collect(
            Some(limit),
            |((m, _), _)| m == key,
            |((_, (_, event_seq)), (digest, tx_digest, time))| (digest, tx_digest, event_seq, time),
        )
        .map_err(Into::into)
    }

    pub fn events_by_module_id(
        &self,
        module: &ModuleId,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        Self::get_event_from_index(
            &self.tables.event_by_move_module,
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
        Self::get_event_from_index(
            &self.tables.event_by_move_event,
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
        Self::get_event_from_index(
            &self.tables.event_by_event_module,
            module_id,
            tx_seq,
            event_seq,
            limit,
            descending,
        )
    }

    pub fn events_by_sender(
        &self,
        sender: &IotaAddress,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        Self::get_event_from_index(
            &self.tables.event_by_sender,
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
        if descending {
            self.tables
                .event_by_time
                .reversed_safe_iter_with_bounds(None, Some((end_time, (tx_seq, event_seq))))?
                .try_take_map_while_and_collect(
                    Some(limit),
                    |((m, _), _)| m >= &start_time,
                    |((_, (_, event_seq)), (digest, tx_digest, time))| {
                        (digest, tx_digest, event_seq, time)
                    },
                )
                .map_err(Into::into)
        } else {
            self.tables
                .event_by_time
                .safe_iter_with_bounds(Some((start_time, (tx_seq, event_seq))), None)
                .try_take_map_while_and_collect(
                    Some(limit),
                    |((m, _), _)| m <= &end_time,
                    |((_, (_, event_seq)), (digest, tx_digest, time))| {
                        (digest, tx_digest, event_seq, time)
                    },
                )
                .map_err(Into::into)
        }
    }

    pub fn prune(&self, cut_time_ms: u64) -> IotaResult<TxSequenceNumber> {
        match self
            .tables
            .event_by_time
            .reversed_safe_iter_with_bounds(
                None,
                Some((cut_time_ms, (TxSequenceNumber::MAX, usize::MAX))),
            )?
            .next()
            .transpose()?
        {
            Some(((_, (watermark, _)), _)) => {
                if let Some(digest) = self.tables.transaction_order.get(&watermark)? {
                    info!(
                        "json rpc index pruning. Watermark is {} with digest {}",
                        watermark, digest
                    );
                }
                self.pruner_watermark.store(watermark, Ordering::Relaxed);
                self.tables.pruner_watermark.insert(&(), &watermark)?;
                Ok(watermark)
            }
            None => Ok(0),
        }
    }

    pub fn get_dynamic_fields_iterator(
        &self,
        object: ObjectID,
        cursor: Option<ObjectID>,
    ) -> IotaResult<impl Iterator<Item = Result<(ObjectID, DynamicFieldInfo), TypedStoreError>> + '_>
    {
        debug!(?object, "get_dynamic_fields");
        // The object id 0 is the smallest possible
        let iter_lower_bound = (object, cursor.unwrap_or(ObjectID::ZERO));
        let iter_upper_bound = (object, ObjectID::MAX);
        Ok(self
            .tables
            .dynamic_field_index
            .safe_iter_with_bounds(Some(iter_lower_bound), Some(iter_upper_bound))
            // skip an extra b/c the cursor is exclusive
            .skip(usize::from(cursor.is_some()))
            .take_while(move |result| result.is_err() || (result.as_ref().unwrap().0.0 == object))
            .map_ok(|((_, c), object_info)| (c, object_info)))
    }

    pub fn get_dynamic_field_object_id(
        &self,
        object: ObjectID,
        name_type: TypeTag,
        name_bcs_bytes: &[u8],
    ) -> IotaResult<Option<ObjectID>> {
        debug!(?object, "get_dynamic_field_object_id");
        let dynamic_field_id =
            dynamic_field::derive_dynamic_field_id(object, &name_type, name_bcs_bytes).map_err(
                |e| {
                    IotaError::Unknown(format!(
                        "Unable to generate dynamic field id. Got error: {e:?}"
                    ))
                },
            )?;

        if let Some(info) = self
            .tables
            .dynamic_field_index
            .get(&(object, dynamic_field_id))?
        {
            // info.object_id != dynamic_field_id ==> is_wrapper
            debug_assert!(
                info.object_id == dynamic_field_id
                    || matches!(name_type, TypeTag::Struct(tag) if DynamicFieldInfo::is_dynamic_object_field_wrapper(&tag))
            );
            return Ok(Some(info.object_id));
        }

        let dynamic_object_field_struct = DynamicFieldInfo::dynamic_object_field_wrapper(name_type);
        let dynamic_object_field_type = TypeTag::Struct(Box::new(dynamic_object_field_struct));
        let dynamic_object_field_id = dynamic_field::derive_dynamic_field_id(
            object,
            &dynamic_object_field_type,
            name_bcs_bytes,
        )
        .map_err(|e| {
            IotaError::Unknown(format!(
                "Unable to generate dynamic field id. Got error: {e:?}"
            ))
        })?;
        if let Some(info) = self
            .tables
            .dynamic_field_index
            .get(&(object, dynamic_object_field_id))?
        {
            return Ok(Some(info.object_id));
        }

        Ok(None)
    }

    pub fn get_owner_objects(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
        limit: usize,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<Vec<ObjectInfo>> {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => ObjectID::ZERO,
        };
        Ok(self
            .get_owner_objects_iterator(owner, cursor, filter)?
            .take(limit)
            .collect())
    }

    pub fn get_owned_coins_iterator(
        coin_index: &DBMap<CoinIndexKey, CoinInfo>,
        owner: IotaAddress,
        coin_type_tag: Option<String>,
    ) -> IotaResult<impl Iterator<Item = (String, ObjectID, CoinInfo)> + '_> {
        let all_coins = coin_type_tag.is_none();
        let starting_coin_type =
            coin_type_tag.unwrap_or_else(|| String::from_utf8([0u8].to_vec()).unwrap());
        Ok(coin_index
            .safe_iter_with_bounds(
                Some((owner, starting_coin_type.clone(), ObjectID::ZERO)),
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
        owner: IotaAddress,
        cursor: (String, ObjectID),
        limit: usize,
        one_coin_type_only: bool,
    ) -> IotaResult<impl Iterator<Item = (String, ObjectID, CoinInfo)> + '_> {
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
        owner: IotaAddress,
        starting_object_id: ObjectID,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<impl Iterator<Item = ObjectInfo> + '_> {
        Ok(self
            .tables
            .owner_index
            // The object id 0 is the smallest possible
            .safe_iter_with_bounds(Some((owner, starting_object_id)), None)
            .map(|result| result.expect("iterator db error"))
            .skip(usize::from(starting_object_id != ObjectID::ZERO))
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

    pub fn insert_genesis_objects(&self, object_index_changes: ObjectIndexChanges) -> IotaResult {
        let mut batch = self.tables.owner_index.batch();
        batch.insert_batch(
            &self.tables.owner_index,
            object_index_changes.new_owners.into_iter(),
        )?;
        batch.insert_batch(
            &self.tables.dynamic_field_index,
            object_index_changes.new_dynamic_fields.into_iter(),
        )?;
        batch.write()?;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.tables.owner_index.is_empty()
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // We are checkpointing the whole db
        self.tables
            .transactions_from_addr
            .checkpoint_db(path)
            .map_err(Into::into)
    }

    /// This method first gets the balance from `per_coin_type_balance` cache.
    /// On a cache miss, it gets the balance for passed in `coin_type` from
    /// the `all_balance` cache. Only on the second cache miss, we go to the
    /// database (expensive) and update the cache. Notice that db read is
    /// done with `spawn_blocking` as that is expected to block
    pub fn get_balance(&self, owner: IotaAddress, coin_type: TypeTag) -> IotaResult<TotalBalance> {
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
        owner: IotaAddress,
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

    /// Read balance for a `IotaAddress` and `CoinType` from the backend
    /// database
    pub fn get_balance_from_db(
        metrics: Arc<IndexStoreMetrics>,
        coin_index: DBMap<CoinIndexKey, CoinInfo>,
        owner: IotaAddress,
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

    /// Read all balances for a `IotaAddress` from the backend database
    pub fn get_all_balances_from_db(
        metrics: Arc<IndexStoreMetrics>,
        coin_index: DBMap<CoinIndexKey, CoinInfo>,
        owner: IotaAddress,
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
        keys: impl IntoIterator<Item = (IotaAddress, TypeTag)>,
    ) -> IotaResult {
        self.caches.per_coin_type_balance.batch_invalidate(keys);
        Ok(())
    }

    fn invalidate_all_balance_cache(
        &self,
        addresses: impl IntoIterator<Item = IotaAddress>,
    ) -> IotaResult {
        self.caches.all_balances.batch_invalidate(addresses);
        Ok(())
    }

    fn update_per_coin_type_cache(
        &self,
        keys: impl IntoIterator<Item = ((IotaAddress, TypeTag), IotaResult<TotalBalance>)>,
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
        keys: impl IntoIterator<Item = (IotaAddress, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>)>,
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
    use std::{collections::BTreeMap, env::temp_dir};

    use iota_types::{
        base_types::{IotaAddress, ObjectInfo, ObjectType},
        digests::TransactionDigest,
        effects::TransactionEvents,
        gas_coin::GAS,
        object,
        object::Owner,
    };
    use move_core_types::account_address::AccountAddress;
    use prometheus::Registry;

    use super::{IndexStore, ObjectIndexChanges};

    #[tokio::test]
    async fn test_index_cache() -> anyhow::Result<()> {
        // This test is going to invoke `index_tx()`where 10 coins each with balance 100
        // are going to be added to an address. The balance is then going to be read
        // from the db and the cache. It should be 1000. Then, we are going to
        // delete 3 of those coins from the address and invoke `index_tx()`
        // again and read balance. The balance should be 700 and verified from
        // both db and cache. This tests make sure we are invalidating entries
        // in the cache and always reading latest balance.
        let index_store = IndexStore::new(temp_dir(), &Registry::default(), Some(128));
        let address: IotaAddress = AccountAddress::random().into();
        let mut written_objects = BTreeMap::new();
        let mut object_map = BTreeMap::new();

        let mut new_objects = vec![];
        for _i in 0..10 {
            let object = object::Object::new_gas_with_balance_and_owner_for_testing(100, address);
            new_objects.push((
                (address, object.id()),
                ObjectInfo {
                    object_id: object.id(),
                    version: object.version(),
                    digest: object.digest(),
                    type_: ObjectType::Struct(object.type_().unwrap().clone()),
                    owner: Owner::AddressOwner(address),
                    previous_transaction: object.previous_transaction,
                },
            ));
            object_map.insert(object.id(), object.clone());
            written_objects.insert(object.data.id(), object);
        }
        let object_index_changes = ObjectIndexChanges {
            deleted_owners: vec![],
            deleted_dynamic_fields: vec![],
            new_owners: new_objects,
            new_dynamic_fields: vec![],
        };

        let tx_coins = (object_map.clone(), written_objects.clone());
        index_store.index_tx(
            address,
            vec![].into_iter(),
            vec![].into_iter(),
            vec![].into_iter(),
            &TransactionEvents { data: vec![] },
            object_index_changes,
            &TransactionDigest::random(),
            1234,
            Some(tx_coins),
        )?;

        let balance_from_db = IndexStore::get_balance_from_db(
            index_store.metrics.clone(),
            index_store.tables.coin_index.clone(),
            address,
            GAS::type_tag(),
        )?;
        let balance = index_store.get_balance(address, GAS::type_tag())?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.num_coins, 10);

        let all_balance = index_store.get_all_balance(address)?;
        let balance = all_balance.get(&GAS::type_tag()).unwrap();
        assert_eq!(*balance, balance_from_db);
        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.num_coins, 10);

        written_objects.clear();
        let mut deleted_objects = vec![];
        for (id, object) in object_map.iter().take(3) {
            deleted_objects.push((address, *id));
            written_objects.insert(object.data.id(), object.clone());
        }
        let object_index_changes = ObjectIndexChanges {
            deleted_owners: deleted_objects,
            deleted_dynamic_fields: vec![],
            new_owners: vec![],
            new_dynamic_fields: vec![],
        };
        let tx_coins = (object_map, written_objects);
        index_store.index_tx(
            address,
            vec![].into_iter(),
            vec![].into_iter(),
            vec![].into_iter(),
            &TransactionEvents { data: vec![] },
            object_index_changes,
            &TransactionDigest::random(),
            1234,
            Some(tx_coins),
        )?;
        let balance_from_db = IndexStore::get_balance_from_db(
            index_store.metrics.clone(),
            index_store.tables.coin_index.clone(),
            address,
            GAS::type_tag(),
        )?;
        let balance = index_store.get_balance(address, GAS::type_tag())?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 700);
        assert_eq!(balance.num_coins, 7);
        // Invalidate per coin type balance cache and read from all balance cache to
        // ensure the balance matches
        index_store
            .caches
            .per_coin_type_balance
            .invalidate(&(address, GAS::type_tag()));
        let all_balance = index_store.get_all_balance(address)?;
        assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().balance, 700);
        assert_eq!(all_balance.get(&GAS::type_tag()).unwrap().num_coins, 7);
        let balance = index_store.get_balance(address, GAS::type_tag())?;
        assert_eq!(balance, balance_from_db);
        assert_eq!(balance.balance, 700);
        assert_eq!(balance.num_coins, 7);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_transaction_by_move_function() {
        use iota_types::base_types::ObjectID;
        use typed_store::Map;

        let index_store = IndexStore::new(temp_dir(), &Registry::default(), Some(128));
        let db = &index_store.tables.transactions_by_move_function;
        db.insert(
            &(
                ObjectID::new([1; 32]),
                "mod".to_string(),
                "f".to_string(),
                0,
            ),
            &[0; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectID::new([1; 32]),
                "mod".to_string(),
                "Z".repeat(128),
                0,
            ),
            &[1; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectID::new([1; 32]),
                "mod".to_string(),
                "f".repeat(128),
                0,
            ),
            &[2; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectID::new([1; 32]),
                "mod".to_string(),
                "z".repeat(128),
                0,
            ),
            &[3; 32].into(),
        )
        .unwrap();

        let mut v = index_store
            .get_transactions_by_move_function(
                ObjectID::new([1; 32]),
                Some("mod".to_string()),
                None,
                None,
                None,
                false,
            )
            .unwrap();
        let v_rev = index_store
            .get_transactions_by_move_function(
                ObjectID::new([1; 32]),
                Some("mod".to_string()),
                None,
                None,
                None,
                true,
            )
            .unwrap();
        v.reverse();
        assert_eq!(v, v_rev);
    }
}
