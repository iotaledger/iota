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
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use bincode::Options;
use either::Either;
use iota_common::try_iterator_ext::TryIteratorExt;
use iota_json_rpc_types::{IotaMoveValue, IotaObjectDataFilter, TransactionFilter};
use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, ObjectReference, Owner, StructTag, TransactionDigest,
    TransactionEventsDigest, TypeTag, Version,
};
use iota_storage::{mutex_table::MutexTable, sharded_lru::ShardedLruCache};
use iota_types::{
    base_types::{ObjectInfo, TxSequenceNumber},
    dynamic_field::{self, DynamicFieldInfo, DynamicFieldName, visitor as DFV},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEffectsExt, TransactionEvents},
    error::{IotaError, IotaResult, UserInputError},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    inner_temporary_store::TxCoins,
    iota_sdk_types_conversions::type_tag_core_to_sdk,
    layout_resolver::LayoutResolver,
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber},
    object::{Object, bounded_visitor::BoundedVisitor},
    parse_iota_struct_tag,
    storage::{BackingPackageStore, ObjectStore, error::Error as StorageError},
    transaction::{Transaction, TransactionDataAPI},
};
use itertools::Itertools;
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
};
use parking_lot::ArcMutexGuard;
use prometheus_filtered::{
    IntCounter, IntCounterVec, Registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tracing::{debug, error, info, trace, warn};
use typed_store::{
    DBMapUtils, TypedStoreError,
    rocks::{
        DBBatch, DBMap, DBMapTableConfigMap, DBOptions, MetricConf, bulk_ingestion_options,
        bulk_ingestion_write_options, default_db_options, read_size_from_env, safe_drop_db,
    },
    rocksdb::compaction_filter::Decision,
    traits::Map,
};

use crate::{
    authority::{AuthorityStore, authority_per_epoch_store::AuthorityPerEpochStore},
    checkpoints::CheckpointStore,
    par_index_live_object_set::{LiveObjectIndexer, ParMakeLiveObjectIndexer},
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

/// Bump this when changing the serialization format of an existing table.
/// A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`.
const CURRENT_DB_VERSION: u64 = 1;
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

/// The `IndexStoreTables` struct defines a set of `DBMaps` used to index
/// various aspects of transaction and object data. Each field corresponds to a
/// specific index, with keys such as `Address`, `TransactionDigest`, etc.
/// Each mapping is configured with custom database options.
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

    /// Index from iota address to transactions initiated by that address.
    transactions_from_addr: DBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from iota address to transactions that were sent to that address.
    transactions_to_addr: DBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that used that object id as input.
    transactions_by_input_object_id: DBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that modified/created that object
    /// id.
    transactions_by_mutated_object_id: DBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from package id, module and function identifier to transactions
    /// that used that moce function call as input.
    transactions_by_move_function:
        DBMap<(ObjectId, String, String, TxSequenceNumber), TransactionDigest>,

    /// Ordering of all indexed transactions.
    transaction_order: DBMap<TxSequenceNumber, TransactionDigest>,

    /// Index from transaction digest to sequence number.
    transactions_seq: DBMap<TransactionDigest, TxSequenceNumber>,

    /// This is an index of object references to currently existing objects,
    /// indexed by the composite key of the Address of their owner and
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

    event_by_sender: DBMap<(Address, EventId), EventIndex>,

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
        if self.transaction_order.is_empty() && self.owner_index.is_empty() {
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

    /// Range of checkpoints that history replay can cover. Returns `None`
    /// when there is nothing to do (no executed checkpoints, or the lower
    /// bound has overtaken the upper).
    fn transaction_index_range(
        &self,
        checkpoint_store: &CheckpointStore,
        highest_executed_checkpoint: Option<CheckpointSequenceNumber>,
    ) -> Result<Option<std::ops::RangeInclusive<CheckpointSequenceNumber>>, StorageError> {
        // Replay reads each transaction, its effects, and its events, all of
        // which are pruned together with the checkpoint contents — the object
        // pruner does not bound it.
        let lowest = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .map(|c| c.saturating_add(1))
            .unwrap_or(0);
        Ok(highest_executed_checkpoint
            .and_then(|highest| (lowest <= highest).then_some(lowest..=highest)))
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
        epoch_store: &AuthorityPerEpochStore,
        package_store: &Arc<dyn BackingPackageStore + Send + Sync>,
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

        // Phase 1 — history-derived tables, replayed over the checkpoints
        // whose data is still locally available.
        let tx_range =
            self.transaction_index_range(checkpoint_store, highest_executed_checkpoint)?;
        if let Some(range) = tx_range {
            self.index_historical_checkpoints(authority_store, checkpoint_store, range)?;
        }

        // Phase 2 — live-state tables from the current live object set.
        self.index_live_object_set(
            authority_store,
            epoch_store,
            package_store,
            batch_size_limit,
        )?;

        self.watermark
            .insert(&(), &highest_executed_checkpoint.unwrap_or(0))?;

        info!("Finished initializing JSON-RPC indexes");

        Ok(())
    }

    /// Replays every checkpoint in `checkpoint_range` in order, writing only
    /// the history tables; the live-state tables are covered by the
    /// live-object scan. Transactions are numbered by their position in the
    /// network transaction order, derived from each checkpoint's transaction
    /// total, so numbering stays canonical whatever range is locally
    /// available.
    #[tracing::instrument(skip_all)]
    fn index_historical_checkpoints(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        checkpoint_range: std::ops::RangeInclusive<CheckpointSequenceNumber>,
    ) -> Result<(), StorageError> {
        info!(
            "Indexing {} checkpoints in range {checkpoint_range:?}",
            checkpoint_range.size_hint().0
        );
        let start_time = Instant::now();

        for checkpoint_seq in checkpoint_range {
            let summary = checkpoint_store
                .get_checkpoint_by_sequence_number(checkpoint_seq)?
                .ok_or_else(|| {
                    StorageError::missing(format!("missing checkpoint {checkpoint_seq}"))
                })?;
            let contents = checkpoint_store
                .get_checkpoint_contents(&summary.content_digest)?
                .ok_or_else(|| {
                    StorageError::missing(format!("missing checkpoint contents {checkpoint_seq}"))
                })?;
            let first_sequence_number =
                summary.network_total_transactions - contents.iter().len() as u64;

            let mut batch = self.transactions_from_addr.batch();
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
                    Some(authority_store.get_events(&digests.transaction)?.ok_or_else(
                        || StorageError::missing(format!("missing events {}", digests.transaction)),
                    )?)
                } else {
                    None
                };

                let data = transaction_index_data(&transaction, &effects, events.as_ref())
                    .map_err(|e| StorageError::custom(e.to_string()))?;
                self.index_tx(&mut batch, sequence, summary.timestamp_ms, data)
                    .map_err(|e| StorageError::custom(e.to_string()))?;
            }
            batch
                .write_opt(&bulk_ingestion_write_options())
                .map_err(StorageError::from)?;
        }

        info!(
            "Indexing checkpoints took {} seconds",
            start_time.elapsed().as_secs()
        );
        Ok(())
    }

    /// Phase 2 of `init`: rebuild the live-state indexes (owner, coin,
    /// dynamic field) by scanning the current live object set in parallel.
    fn index_live_object_set(
        &self,
        authority_store: &AuthorityStore,
        epoch_store: &AuthorityPerEpochStore,
        package_store: &Arc<dyn BackingPackageStore + Send + Sync>,
        batch_size_limit: usize,
    ) -> Result<(), StorageError> {
        let indexer = JsonRpcLiveObjectSetIndexer {
            tables: self,
            authority_store,
            epoch_store,
            package_store,
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
                let coin_type_tag = object.coin_type_opt().unwrap_or_else(|| {
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
            let coin_type_tag = obj.coin_type_opt().cloned().unwrap_or_else(|| {
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

        batch.insert_batch(&self.transaction_order, std::iter::once((sequence, digest)))?;

        batch.insert_batch(&self.transactions_seq, std::iter::once((digest, sequence)))?;

        batch.insert_batch(
            &self.transactions_from_addr,
            std::iter::once(((sender, sequence), digest)),
        )?;

        batch.insert_batch(
            &self.transactions_by_input_object_id,
            active_inputs.into_iter().map(|id| ((id, sequence), digest)),
        )?;

        batch.insert_batch(
            &self.transactions_by_mutated_object_id,
            mutated_objects
                .iter()
                .map(|(obj_ref, _)| ((obj_ref.object_id, sequence), digest)),
        )?;

        batch.insert_batch(
            &self.transactions_by_move_function,
            move_functions
                .into_iter()
                .map(|(obj_id, module, function)| ((obj_id, module, function, sequence), digest)),
        )?;

        batch.insert_batch(
            &self.transactions_to_addr,
            mutated_objects.iter().filter_map(|(_, owner)| {
                owner
                    .into_opt_address()
                    .map(|addr| ((addr, sequence), digest))
            }),
        )?;

        // events
        let event_digest = events.digest();
        batch.insert_batch(
            &self.event_order,
            events
                .iter()
                .enumerate()
                .map(|(i, _)| ((sequence, i), (event_digest, digest, timestamp_ms))),
        )?;
        batch.insert_batch(
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
        batch.insert_batch(
            &self.event_by_sender,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.sender, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;
        batch.insert_batch(
            &self.event_by_move_event,
            events.iter().enumerate().map(|(i, e)| {
                (
                    (e.type_.clone(), (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch(
            &self.event_by_time,
            events.iter().enumerate().map(|(i, _)| {
                (
                    (timestamp_ms, (sequence, i)),
                    (event_digest, digest, timestamp_ms),
                )
            }),
        )?;

        batch.insert_batch(
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
            object_index_changes.new_dynamic_fields,
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
    caches: IndexStoreCaches,
    metrics: Arc<IndexStoreMetrics>,
    max_type_length: u64,
    pruner_watermark: Arc<AtomicU64>,
    pending_updates: Mutex<BTreeMap<CheckpointSequenceNumber, PendingCheckpointUpdate>>,
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

/// Extracts one transaction's history-table index inputs.
fn transaction_index_data(
    transaction: &Transaction,
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
            .map(|(obj_ref, owner, _kind)| (obj_ref, owner))
            .collect(),
        move_functions: tx_data
            .move_calls()
            .into_iter()
            .map(|(package, module, function)| (*package, module.to_owned(), function.to_owned()))
            .collect(),
        events: events.cloned().unwrap_or_default(),
    })
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

fn process_object_index(
    tx: &CheckpointTransaction,
    object_store: &dyn ObjectStore,
    layout_resolver: &mut dyn LayoutResolver,
) -> IotaResult<ObjectIndexChanges> {
    let written: BTreeMap<_, _> = tx.output_objects.iter().map(|o| (o.id(), o)).collect();

    let mut deleted_owners = vec![];
    let mut deleted_dynamic_fields = vec![];
    for removed_object in tx.removed_objects_pre_version() {
        match removed_object.owner {
            Owner::Address(addr) => deleted_owners.push((addr, removed_object.id())),
            Owner::Object(object_id) => {
                deleted_dynamic_fields.push((object_id, removed_object.id()))
            }
            _ => {}
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
                    _ => {}
                }
            }
        }

        match object.owner {
            Owner::Address(addr) => {
                new_owners.push(((addr, object.id()), ObjectInfo::from_object(object)));
            }
            Owner::Object(parent) => {
                let Some(df_info) = try_create_dynamic_field_info(
                    object,
                    &written,
                    object_store,
                    layout_resolver,
                )
                .unwrap_or_else(|e| {
                    error!(
                        "try_create_dynamic_field_info should not fail, {}, new_object={:?}",
                        e, object
                    );
                    None
                }) else {
                    // Skip indexing for non dynamic field objects.
                    continue;
                };
                new_dynamic_fields.push(((parent, object.id()), df_info))
            }
            _ => {}
        }
    }

    Ok(ObjectIndexChanges {
        deleted_owners,
        deleted_dynamic_fields,
        new_owners,
        new_dynamic_fields,
    })
}

pub(crate) fn try_create_dynamic_field_info(
    o: &Object,
    written: &BTreeMap<ObjectId, &Object>,
    object_store: &dyn ObjectStore,
    resolver: &mut dyn LayoutResolver,
) -> IotaResult<Option<DynamicFieldInfo>> {
    // Skip if not a move object
    let Some(move_object) = o.data.as_opt_struct().cloned() else {
        return Ok(None);
    };

    // We only index dynamic field objects
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
        type_: name_type,
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

            // Try to find the object in the written objects first.
            let (version, digest, object_type) = if let Some(object) = written.get(&object_id) {
                (
                    object.version(),
                    object.digest(),
                    object.data.opt_object_type().unwrap().clone(),
                )
            } else {
                // If not found, try to find it in the database. The child is
                // written at the wrapper's version when the field is added,
                // but that historic version may since have been pruned; the
                // child of a live field is itself live, so fall back to its
                // latest version.
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
                (version, digest, object_type)
            };

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
    authority_store: &'a AuthorityStore,
    epoch_store: &'a AuthorityPerEpochStore,
    package_store: &'a Arc<dyn BackingPackageStore + Send + Sync>,
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
            object_store: self.authority_store,
            layout_resolver: self
                .epoch_store
                .executor()
                .type_layout_resolver(Box::new(self.package_store.clone())),
            batch_size_limit: self.batch_size_limit,
        }
    }
}

/// One worker's indexer within a [`JsonRpcLiveObjectSetIndexer`] run.
struct JsonRpcLiveObjectIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: DBBatch,
    object_store: &'a AuthorityStore,
    layout_resolver: Box<dyn LayoutResolver + 'a>,
    batch_size_limit: usize,
}

impl LiveObjectIndexer for JsonRpcLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                self.batch.insert_batch(
                    &self.tables.owner_index,
                    [((owner, object.id()), ObjectInfo::from_object(&object))],
                )?;
                if let Some(coin_info) = CoinInfo::from_object(&object) {
                    let coin_type = object
                        .coin_type_opt()
                        .expect("coin object must have a coin type")
                        .to_string();
                    self.batch.insert_batch(
                        &self.tables.coin_index,
                        [((owner, coin_type, object.id()), coin_info)],
                    )?;
                }
            }
            Owner::Object(parent) => {
                if let Some(field_info) = try_create_dynamic_field_info(
                    &object,
                    &BTreeMap::new(),
                    self.object_store,
                    self.layout_resolver.as_mut(),
                )
                .unwrap_or_else(|e| {
                    error!(
                        "try_create_dynamic_field_info should not fail, {}, object={:?}",
                        e, object
                    );
                    None
                }) {
                    self.batch.insert_batch(
                        &self.tables.dynamic_field_index,
                        [((parent, object.id()), field_info)],
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

impl IndexStore {
    /// Opens the store, wiping and rebuilding the indexes first when they are
    /// missing or stale (e.g. on the first start after a formal-snapshot
    /// restore, or after running with indexes disabled). Databases written
    /// before per-checkpoint indexing are wiped and rebuilt as well: nodes
    /// restored from a formal snapshot wrote corrupted data into them.
    pub async fn new(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
        authority_store: &Arc<AuthorityStore>,
        checkpoint_store: &CheckpointStore,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        package_store: Arc<dyn BackingPackageStore + Send + Sync>,
    ) -> Self {
        let pruner_watermark = Arc::new(AtomicU64::new(0));
        let compaction_metrics = JsonRpcCompactionMetrics::new(registry);
        let mut tables = Self::open_tables(&path, &pruner_watermark, &compaction_metrics);

        tables
            .seed_meta()
            .expect("failed to initialize index tables");

        if tables.needs_to_do_initialization(checkpoint_store) {
            let batch_size_limit;
            let mut init_tables = {
                drop(tables);
                safe_drop_db(path.clone(), Duration::from_secs(30))
                    .await
                    .expect("unable to destroy old JSON-RPC index db");

                // Open the empty DB with tuned bulk ingestion options to
                // speed up the initial indexing. The DB is reopened with default options
                // afterwards.
                let bulk_options = bulk_ingestion_options();
                batch_size_limit = bulk_options.batch_size_limit;

                // Apply the per-column-family bulk options to every table.
                let mut table_config = BTreeMap::new();
                for table_name in IndexStoreTables::describe_tables().into_keys() {
                    table_config.insert(table_name, bulk_options.column_family_options.clone());
                }

                IndexStoreTables::open_tables_read_write(
                    path.clone(),
                    MetricConf::new("index"),
                    Some(bulk_options.db_options),
                    Some(DBMapTableConfigMap::new(table_config)),
                )
            };

            init_tables
                .init(
                    authority_store,
                    checkpoint_store,
                    epoch_store,
                    &package_store,
                    batch_size_limit,
                )
                .expect("unable to initialize JSON-RPC index");

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
            tables = Self::open_tables(&path, &pruner_watermark, &compaction_metrics);

            // Sanity check: verify the database version was persisted correctly, i.e.
            // the WAL-disabled bulk writes were flushed before the reopen.
            let stored_version = tables
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
        let anchor = tables
            .watermark
            .get(&())
            .expect("failed to initialize index tables")
            .and_then(|watermark| {
                checkpoint_store
                    .get_checkpoint_by_sequence_number(watermark)
                    .expect("checkpoint store read cannot fail")
                    .map(|checkpoint| checkpoint.network_total_transactions)
            })
            .unwrap_or(0);

        Self::finish_open(tables, registry, max_type_length, pruner_watermark, anchor)
    }

    /// Opens the store without the init logic of [`Self::new`] — for tests.
    pub fn new_without_init(
        path: PathBuf,
        registry: &Registry,
        max_type_length: Option<u64>,
    ) -> Self {
        let pruner_watermark = Arc::new(AtomicU64::new(0));
        let compaction_metrics = JsonRpcCompactionMetrics::new(registry);
        let tables = Self::open_tables(&path, &pruner_watermark, &compaction_metrics);
        Self::finish_open(tables, registry, max_type_length, pruner_watermark, 0)
    }

    fn finish_open(
        tables: IndexStoreTables,
        registry: &Registry,
        max_type_length: Option<u64>,
        pruner_watermark: Arc<AtomicU64>,
        next_sequence_number_floor: TxSequenceNumber,
    ) -> Self {
        let metrics = IndexStoreMetrics::new(registry);
        let caches = IndexStoreCaches {
            per_coin_type_balance: ShardedLruCache::new(1_000_000, 1000),
            all_balances: ShardedLruCache::new(1_000_000, 1000),
            locks: MutexTable::new(128),
        };
        let next_sequence_number = tables
            .transaction_order
            .safe_range_iter_reversed(..)
            .next()
            .transpose()
            .expect("failed to initialize indexes")
            .map(|(seq, _)| seq + 1)
            .unwrap_or(0)
            .max(next_sequence_number_floor)
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
            pending_updates: Mutex::new(BTreeMap::new()),
        }
    }

    fn open_tables(
        path: &Path,
        pruner_watermark: &Arc<AtomicU64>,
        compaction_metrics: &Arc<JsonRpcCompactionMetrics>,
    ) -> IndexStoreTables {
        let db_options = default_db_options().disable_write_throttling();
        let compaction_metrics = compaction_metrics.clone();
        let pruner_watermark = pruner_watermark.clone();
        let table_options = DBMapTableConfigMap::new(BTreeMap::from([
            (
                "transactions_from_addr".to_string(),
                compaction_filter_config_by_key(
                    "transactions_from_addr",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, id): (Address, TxSequenceNumber)| id,
                ),
            ),
            (
                "transactions_to_addr".to_string(),
                compaction_filter_config_by_key(
                    "transactions_to_addr",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, sequence_number): (Address, TxSequenceNumber)| sequence_number,
                ),
            ),
            (
                "transactions_by_move_function".to_string(),
                compaction_filter_config_by_key(
                    "transactions_by_move_function",
                    compaction_metrics.clone(),
                    db_options.clone(),
                    pruner_watermark.clone(),
                    |(_, _, _, id): (ObjectId, String, String, TxSequenceNumber)| id,
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
                    |(_, event_id): (Address, EventId)| event_id.0,
                ),
            ),
            (
                "event_by_time".to_string(),
                compaction_filter_config_by_key(
                    "event_by_time",
                    compaction_metrics,
                    db_options.clone(),
                    pruner_watermark,
                    |(_, event_id): (u64, EventId)| event_id.0,
                ),
            ),
        ]));
        IndexStoreTables::open_tables_read_write(
            path.to_path_buf(),
            MetricConf::new("index"),
            Some(db_options.options),
            Some(table_options),
        )
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
    pub fn index_checkpoint(
        &self,
        checkpoint: &CheckpointData,
        object_store: &dyn ObjectStore,
        layout_resolver: &mut dyn LayoutResolver,
        index_coins: bool,
    ) -> IotaResult {
        let checkpoint_seq = checkpoint.checkpoint_summary.sequence_number;
        let timestamp_ms = checkpoint.checkpoint_summary.timestamp_ms;

        let digests: Vec<_> = checkpoint
            .transactions
            .iter()
            .map(|tx| *tx.effects.transaction_digest())
            .collect();
        let already_indexed = self.tables.transactions_seq.multi_get(&digests)?;

        let mut batch = self.tables.transactions_from_addr.batch();
        let mut coin_changes = BTreeMap::new();
        for (tx, indexed_seq) in checkpoint.transactions.iter().zip(already_indexed) {
            if indexed_seq.is_some() {
                continue;
            }
            let data = transaction_index_data(&tx.transaction, &tx.effects, tx.events.as_ref())?;
            let digest = data.digest;
            let sequence = self.next_sequence_number.fetch_add(1, Ordering::SeqCst);
            self.tables
                .index_tx(&mut batch, sequence, timestamp_ms, data)?;

            let object_index_changes = process_object_index(tx, object_store, layout_resolver)?;
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

        let mut pending_updates = self.pending_updates.lock().unwrap();
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
        let next_update = self.pending_updates.lock().unwrap().pop_first();
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
                if reverse {
                    let iter = self
                        .tables
                        .transaction_order
                        .safe_range_iter_reversed(..=cursor.unwrap_or(TxSequenceNumber::MAX))
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
            Either::Left(index.safe_range_iter_reversed(
                ..=(key.clone(), cursor.unwrap_or(TxSequenceNumber::MAX)),
            ))
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
        input_object: ObjectId,
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
        mutated_object: ObjectId,
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
        addr: Address,
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
                    .safe_range_iter_reversed(..=key),
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
        addr: Address,
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
                .safe_range_iter_reversed(..=(tx_seq, event_seq))
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
                    .safe_range_iter_reversed(..=(min(tx_seq, seq), event_seq)),
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
        let iter = if descending {
            Either::Left(index.safe_range_iter_reversed(..=(key.clone(), (tx_seq, event_seq))))
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
        sender: &Address,
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
                .safe_range_iter_reversed(..=(end_time, (tx_seq, event_seq)))
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
            .safe_range_iter_reversed(..=(cut_time_ms, (TxSequenceNumber::MAX, usize::MAX)))
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
        object: ObjectId,
        cursor: Option<ObjectId>,
    ) -> IotaResult<impl Iterator<Item = Result<(ObjectId, DynamicFieldInfo), TypedStoreError>> + '_>
    {
        debug!(?object, "get_dynamic_fields");
        Ok(self
            .tables
            .dynamic_field_index
            .safe_iter_with_prefix_from(&object, &cursor.unwrap_or(ObjectId::ZERO))
            // skip an extra b/c the cursor is exclusive
            .skip(usize::from(cursor.is_some()))
            .map_ok(|((_, c), object_info)| (c, object_info)))
    }

    pub fn get_dynamic_field_object_id(
        &self,
        object: ObjectId,
        name_type: TypeTag,
        name_bcs_bytes: &[u8],
    ) -> IotaResult<Option<ObjectId>> {
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
    use iota_sdk_types::{CheckpointSummary, GasCostSummary, ObjectId, StructTag};
    use iota_types::{
        committee::EpochId, crypto::AuthorityStrongQuorumSignInfo, effects::TransactionEffectsAPI,
        error::IotaError, gas_coin::GAS, in_memory_storage::InMemoryStorage,
        layout_resolver::LayoutResolver, message_envelope::Envelope,
        messages_checkpoint::VerifiedCheckpoint,
        test_checkpoint_data_builder::TestCheckpointDataBuilder,
    };
    use move_core_types::annotated_value::MoveDatatypeLayout;
    use prometheus_filtered::Registry;
    use typed_store::Map;

    use super::IndexStore;
    use crate::checkpoints::CheckpointStore;

    /// The tests only index coin objects, which never need layout resolution.
    struct NoLayoutResolver;

    impl LayoutResolver for NoLayoutResolver {
        fn get_annotated_layout(
            &mut self,
            _struct_tag: &StructTag,
        ) -> Result<MoveDatatypeLayout, IotaError> {
            Err(IotaError::Unknown(
                "no layout resolution in tests".to_string(),
            ))
        }
    }

    /// An executed (non-boundary) checkpoint for seeding a test
    /// `CheckpointStore`, with a placeholder signature.
    fn executed_checkpoint(epoch: EpochId, sequence_number: u64) -> VerifiedCheckpoint {
        let summary = CheckpointSummary {
            epoch,
            sequence_number,
            network_total_transactions: 0,
            content_digest: Default::default(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: GasCostSummary::default(),
            end_of_epoch_data: None,
            timestamp_ms: 0,
            version_specific_data: Vec::new(),
            checkpoint_commitments: Vec::new(),
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        VerifiedCheckpoint::new_unchecked(Envelope::new_from_data_and_sig(summary, sig))
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
        use iota_sdk_types::{Address, Owner, TransactionDigest, Version};
        use iota_types::object::{MoveObject, MoveObjectExt, Object};

        let owner = Owner::Address(Address::ZERO);
        let id = ObjectId::random();
        let contents = iota_types::coin::Coin::new(id, 42).to_bcs_bytes();

        let coin = Object::new_move(
            MoveObject::new_coin(GAS::type_tag(), Version::MIN_VALID_INCL, id, 42),
            owner,
            TransactionDigest::ZERO,
        );
        assert_eq!(super::CoinInfo::from_object(&coin).unwrap().balance, 42);

        let fake = Object::new_move(
            MoveObject::new_from_execution_with_limit(
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

        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );
        let digest = iota_sdk_types::TransactionDigest::random();
        index_store
            .tables
            .transaction_order
            .insert(&0, &digest)
            .unwrap();
        index_store
            .tables
            .transactions_seq
            .insert(&digest, &0)
            .unwrap();

        index_store.tables.seed_meta().unwrap();
        assert!(
            index_store
                .tables
                .needs_to_do_initialization(&checkpoint_store),
            "a database from before per-checkpoint indexing must be rebuilt"
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
        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );
        let object_store = InMemoryStorage::new(vec![]);
        let address = TestCheckpointDataBuilder::derive_address(1);

        let mut builder = TestCheckpointDataBuilder::new(0).start_transaction(0);
        for object_idx in 0..10 {
            builder = builder.create_coin_object(object_idx, 1, 100, GAS::type_tag());
        }
        let mut builder = builder.finish_transaction();
        let checkpoint = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint, &object_store, &mut NoLayoutResolver, true)?;
        index_store.commit_update_for_checkpoint(0)?;

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

        let mut builder = builder.start_transaction(0);
        for object_idx in 0..3 {
            builder = builder.delete_object(object_idx);
        }
        let mut builder = builder.finish_transaction();
        let checkpoint = builder.build_checkpoint();
        index_store.index_checkpoint(&checkpoint, &object_store, &mut NoLayoutResolver, true)?;
        index_store.commit_update_for_checkpoint(1)?;

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
        let object_store = InMemoryStorage::new(vec![]);
        let address = TestCheckpointDataBuilder::derive_address(1);

        let mut builder = TestCheckpointDataBuilder::new(0)
            .start_transaction(0)
            .create_coin_object(0, 1, 100, GAS::type_tag())
            .finish_transaction();
        let checkpoint = builder.build_checkpoint();
        let digest = *checkpoint.transactions[0].effects.transaction_digest();

        index_store.index_checkpoint(&checkpoint, &object_store, &mut NoLayoutResolver, true)?;
        index_store.commit_update_for_checkpoint(0)?;
        assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
        assert_eq!(index_store.tables.watermark.get(&())?, Some(0));

        // Replay the same checkpoint.
        index_store.index_checkpoint(&checkpoint, &object_store, &mut NoLayoutResolver, true)?;
        index_store.commit_update_for_checkpoint(0)?;

        assert_eq!(index_store.get_transaction_seq(&digest)?, Some(0));
        assert_eq!(
            index_store.get_transactions(None, None, None, false)?,
            vec![digest]
        );
        let balance = index_store.get_balance(address, GAS::type_tag())?;
        assert_eq!(balance.balance, 100);
        assert_eq!(balance.num_coins, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_transaction_by_move_function() {
        let tmp_dir = iota_common::tempdir();
        let index_store = IndexStore::new_without_init(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            Some(128),
        );
        let db = &index_store.tables.transactions_by_move_function;
        db.insert(
            &(
                ObjectId::new([1; 32]),
                "mod".to_string(),
                "f".to_string(),
                0,
            ),
            &[0; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectId::new([1; 32]),
                "mod".to_string(),
                "Z".repeat(128),
                0,
            ),
            &[1; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectId::new([1; 32]),
                "mod".to_string(),
                "f".repeat(128),
                0,
            ),
            &[2; 32].into(),
        )
        .unwrap();
        db.insert(
            &(
                ObjectId::new([1; 32]),
                "mod".to_string(),
                "z".repeat(128),
                0,
            ),
            &[3; 32].into(),
        )
        .unwrap();

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
        v.reverse();
        assert_eq!(v, v_rev);
    }
}
