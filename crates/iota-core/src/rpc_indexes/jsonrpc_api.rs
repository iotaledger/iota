// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The JSON-RPC read surface of the unified RPC index store: the reads
//! `IndexStore` (`jsonrpc_index.rs`) used to serve, ported onto the unified
//! schema. Every public read here fails with
//! [`IotaError::IndexStoreNotAvailable`] when this store does not maintain
//! the [`IndexGroup::JsonRpc`] group's tables — the check that replaces the
//! `Option<Arc<IndexStore>>` callers used to match on.
//!
//! Coin balances and pages no longer come from a dedicated coin table: a
//! coin's balance is embedded in its owner-index key
//! (`inverted_balance = Some(!balance)`), so both are prefix scans of the
//! shared `owner` table, narrowed to `0x2::coin::Coin` (every coin type) or
//! `Coin<T>` (one coin type) the same way [`Self::owner_iter`] narrows any
//! other type filter.

use std::{
    cmp::{max, min},
    collections::{HashMap, HashSet, hash_map::Entry},
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use either::Either;
use iota_json_rpc_types::{IotaMoveValue, IotaObjectDataFilter, TransactionFilter};
use iota_sdk_types::{
    Address, ObjectDigest, ObjectId, StructTag, TransactionDigest, TransactionEventsDigest,
    TypeTag, Version,
};
use iota_storage::{mutex_table::MutexTable, sharded_lru::ShardedLruCache};
use iota_types::{
    base_types::{ObjectInfo, TxSequenceNumber},
    dynamic_field::{DynamicFieldInfo, DynamicFieldName, visitor as DFV},
    error::{IotaError, IotaResult, UserInputError},
    iota_sdk_types_conversions::type_tag_core_to_sdk,
    layout_resolver::LayoutResolver,
    object::{Object, bounded_visitor::BoundedVisitor},
    storage::{DynamicFieldKey, ObjectStore},
};
use itertools::Itertools;
use move_core_types::{annotated_value as A, language_storage::ModuleId};
use parking_lot::ArcMutexGuard;
use prometheus_filtered::{IntCounter, Registry, register_int_counter_with_registry};
use serde::{Serialize, de::DeserializeOwned};
use tracing::{error, warn};
use typed_store::{
    TypedStoreError,
    rocks::{TaggedDBMap, read_size_from_env},
    traits::Map,
};

use super::{
    RpcIndexesStore,
    schema::{
        EventId, EventIndex, HistoryBucket, IndexGroup, OwnerIndexInfo, OwnerIndexKey,
        OwnerTypeFilter, TotalBalance, owner_bounds,
    },
};

const ENV_VAR_DISABLE_INDEX_CACHE: &str = "DISABLE_INDEX_CACHE";
const ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE: &str = "INVALIDATE_INSTEAD_OF_UPDATE";

type AllBalance = HashMap<TypeTag, TotalBalance>;
type OwnedMutexGuard<T> = ArcMutexGuard<parking_lot::RawMutex, T>;

/// Whether a commit invalidates the balance caches instead of updating them
/// with the checkpoint's deltas.
pub(super) fn invalidate_balance_caches_instead_of_updating() -> bool {
    read_size_from_env(ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE).unwrap_or(0) > 0
}

/// What one checkpoint does to the coin holdings the balance caches track,
/// per owner and coin object. There is no coin table to compare against, so
/// the balances an owner held before the checkpoint are collected here while
/// the checkpoint's object changes are staged.
#[derive(Default)]
pub(super) struct CoinBalanceChanges(HashMap<(Address, ObjectId), CoinBalanceChange>);

/// The before and after balance of one coin object under one owner.
struct CoinBalanceChange {
    coin_type: TypeTag,
    /// The balance the owner held in this coin before the checkpoint, `None`
    /// when the owner did not hold it. Taken from the first change touching
    /// the pair: only that change sees the state the checkpoint started from,
    /// which is the state the committed owner rows are still in.
    prior: Option<u64>,
    /// The balance the owner holds after the checkpoint, `None` when the coin
    /// is gone or has moved on to another owner.
    current: Option<u64>,
}

impl CoinBalanceChanges {
    /// Records that `owner` no longer holds `object`, whose state is the one
    /// before the change.
    pub(super) fn record_removed(&mut self, owner: Address, object: &Object) {
        let Some((coin_type, balance)) = coin_type_and_balance(object) else {
            return;
        };
        match self.0.entry((owner, object.id())) {
            Entry::Occupied(mut occupied) => occupied.get_mut().current = None,
            Entry::Vacant(vacant) => {
                vacant.insert(CoinBalanceChange {
                    coin_type,
                    prior: Some(balance),
                    current: None,
                });
            }
        }
    }

    /// Records the `object` that `owner` holds after the change.
    pub(super) fn record_written(&mut self, owner: Address, object: &Object) {
        let Some((coin_type, balance)) = coin_type_and_balance(object) else {
            return;
        };
        match self.0.entry((owner, object.id())) {
            Entry::Occupied(mut occupied) => occupied.get_mut().current = Some(balance),
            Entry::Vacant(vacant) => {
                // Nothing has claimed the pair yet, so the owner did not hold
                // this coin before the checkpoint: had it held it, the row
                // would have been deleted first — the deletion path
                // recomputes the row from the object's state before the
                // change, through `record_removed`, for every address-owned
                // input.
                vacant.insert(CoinBalanceChange {
                    coin_type,
                    prior: None,
                    current: Some(balance),
                });
            }
        }
    }
}

/// The coin type and balance of a coin object, `None` for anything else.
/// Uses the balance the way [`OwnerIndexKey::for_object`] does, so a coin's
/// delta and its owner row always carry the same number.
fn coin_type_and_balance(object: &Object) -> Option<(TypeTag, u64)> {
    let coin_type = object.coin_type_opt()?.clone();
    let balance = object
        .as_coin_maybe()
        .map(|coin| coin.balance.value())
        .unwrap_or(0);
    Some((coin_type, balance))
}

/// The balance cache maintenance of one committed checkpoint, holding the
/// affected owners' locks until it is applied and dropped.
#[derive(Default)]
pub(super) struct IndexStoreCacheUpdates {
    _locks: Vec<OwnedMutexGuard<()>>,
    per_coin_type_balance_changes: Vec<((Address, TypeTag), IotaResult<TotalBalance>)>,
    all_balance_changes: Vec<(Address, IotaResult<Arc<AllBalance>>)>,
}

/// Balance caches, keyed the same way regardless of table layout: a coin's
/// balance lives in the owner index either way, so these survived the merge
/// unchanged.
pub(super) struct BalanceCaches {
    pub(super) per_coin_type_balance: ShardedLruCache<(Address, TypeTag), IotaResult<TotalBalance>>,
    pub(super) all_balances: ShardedLruCache<Address, IotaResult<Arc<AllBalance>>>,
    pub(super) locks: MutexTable<Address>,
}

impl BalanceCaches {
    pub(super) fn new() -> Self {
        Self {
            per_coin_type_balance: ShardedLruCache::new(1_000_000, 1000),
            all_balances: ShardedLruCache::new(1_000_000, 1000),
            locks: MutexTable::new(128),
        }
    }
}

pub(super) struct JsonRpcMetrics {
    pub(super) balance_lookup_from_db: IntCounter,
    balance_lookup_from_total: IntCounter,
    pub(super) all_balance_lookup_from_db: IntCounter,
    all_balance_lookup_from_total: IntCounter,
}

impl JsonRpcMetrics {
    pub(super) fn new(registry: &Registry) -> Self {
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

/// Coin details the owner index does not store, resolved from the object
/// store per returned row: mirrors the object store's own view of the coin
/// rather than a value cached alongside the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoinInfo {
    pub version: Version,
    pub digest: ObjectDigest,
    pub balance: u64,
    pub previous_transaction: TransactionDigest,
}

impl CoinInfo {
    /// Returns coin metadata when `object` is a `Coin<T>`, `None` otherwise.
    fn from_object(object: &Object) -> Option<CoinInfo> {
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

/// The base `0x2::coin::Coin` struct tag, with no type parameter: matches
/// every `Coin<T>` under [`OwnerTypeFilter::BaseType`], the way `get_balance`
/// and `get_owned_coins_iterator_with_cursor` scan "every coin type".
fn coin_base_type() -> StructTag {
    let mut coin = StructTag::new_gas_coin();
    coin.type_params_mut().clear();
    coin
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

impl RpcIndexesStore {
    /// Fails fast when this store does not maintain the JSON-RPC group's
    /// tables — the check that replaces the `Option`-ness callers relied on
    /// before the two index stores were unified into one.
    fn require_jsonrpc(&self) -> IotaResult<()> {
        if self.serves(IndexGroup::JsonRpc) {
            Ok(())
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    pub fn get_transactions(
        &self,
        filter: Option<TransactionFilter>,
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.require_jsonrpc()?;
        let cursor = cursor
            .map(|cursor| {
                self.lookup_digest(&cursor)?
                    .map(|(seq, _)| seq)
                    .ok_or(IotaError::TransactionNotFound { digest: cursor })
            })
            .transpose()?;
        match filter {
            Some(TransactionFilter::MoveFunction {
                package,
                module,
                function,
            }) => self.get_transactions_by_move_function(
                package, module, function, cursor, limit, reverse,
            ),
            Some(TransactionFilter::InputObject(object_id)) => {
                self.get_transactions_by_input_object(object_id, cursor, limit, reverse)
            }
            Some(TransactionFilter::ChangedObject(object_id)) => {
                self.get_transactions_by_mutated_object(object_id, cursor, limit, reverse)
            }
            Some(TransactionFilter::FromAddress(address)) => {
                self.get_transactions_from_addr(address, cursor, limit, reverse)
            }
            Some(TransactionFilter::ToAddress(address)) => {
                self.get_transactions_to_addr(address, cursor, limit, reverse)
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        let max_string = "z".repeat(self.max_type_length().try_into().unwrap());
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
        self.require_jsonrpc()?;
        self.get_transactions_from_index(|bucket| &bucket.txs_to_addr, addr, cursor, limit, reverse)
    }

    /// The retained history buckets in scan order: ascending epochs for
    /// forward scans, descending for reverse scans. Buckets are disjoint,
    /// epoch-ordered segments of the global sequence order, so chaining
    /// per-bucket scans in this order preserves it.
    fn history_buckets(&self, reverse: bool) -> Vec<Arc<HistoryBucket>> {
        self.history.iter(reverse)
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

    pub fn all_events(
        &self,
        tx_seq: TxSequenceNumber,
        event_seq: usize,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<(TransactionEventsDigest, TransactionDigest, usize, u64)>> {
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
        let seq = self
            .lookup_digest(digest)?
            .map(|(seq, _)| seq)
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
        select: impl Fn(&HistoryBucket) -> &TaggedDBMap<(KeyT, EventId), EventIndex>,
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
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
        self.require_jsonrpc()?;
        Ok(self
            .tables
            .dynamic_field
            .safe_iter_with_prefix_from(&object, &cursor.unwrap_or(ObjectId::ZERO))
            // The seek is inclusive, so drop the cursor by id: its own row
            // may already be gone.
            .filter_ok(move |(key, ())| Some(key.field_id) != cursor)
            .map_ok(|(key, ())| key.field_id))
    }

    /// Whether `field_id` is an indexed dynamic field of `object`.
    pub fn dynamic_field_exists(&self, object: ObjectId, field_id: ObjectId) -> IotaResult<bool> {
        self.require_jsonrpc()?;
        Ok(self
            .tables
            .dynamic_field
            .contains_key(&DynamicFieldKey::new(object, field_id))?)
    }

    /// Objects owned by `owner` in the unified key order (grouped by type,
    /// coins balance-descending, id-ascending), resolving the response fields
    /// the index does not store from the object store. `cursor` is the last
    /// object of the previous page; its key is rebuilt from the live object,
    /// so a cursor whose object was deleted in between is refused.
    pub fn get_owner_objects(
        &self,
        owner: Address,
        cursor: Option<ObjectId>,
        limit: usize,
        filter: Option<IotaObjectDataFilter>,
        object_store: &dyn ObjectStore,
    ) -> IotaResult<Vec<ObjectInfo>> {
        self.require_jsonrpc()?;
        let cursor_key = cursor
            .map(|id| self.owner_key_for_cursor(owner, id, object_store))
            .transpose()?;
        // The cursor above is still validated for a zero limit; only the
        // scan itself is skipped.
        let mut results = Vec::new();
        if limit == 0 {
            return Ok(results);
        }
        for item in self.owner_iter(owner, cursor_key.as_ref(), OwnerTypeFilter::None)? {
            let (key, _info) = item?;
            if Some(key.object_id) == cursor {
                continue; // the seek is inclusive; drop the cursor row itself
            }
            // The index and the object store are read at different times; an
            // object deleted in between is omitted, like an unresolvable one.
            let Some(object) = object_store.try_get_object(&key.object_id)? else {
                continue;
            };
            let object_info = ObjectInfo::new(&object.object_ref(), &object);
            if filter.as_ref().is_none_or(|f| f.matches(&object_info)) {
                results.push(object_info);
            }
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    /// Rebuilds the cursor object's position in the owner index from its live
    /// state — the same data the gRPC cursor carries explicitly.
    fn owner_key_for_cursor(
        &self,
        owner: Address,
        cursor: ObjectId,
        object_store: &dyn ObjectStore,
    ) -> IotaResult<OwnerIndexKey> {
        let cursor_not_found = || IotaError::UserInput {
            error: UserInputError::ObjectNotFound {
                object_id: cursor,
                version: None,
            },
        };
        let object = object_store
            .try_get_object(&cursor)?
            .ok_or_else(cursor_not_found)?;
        // A package or other non-Move object is never in the owner index —
        // its cursor is exactly as invalid as one whose object is gone.
        let (key, _) = OwnerIndexKey::for_object(owner, &object).ok_or_else(cursor_not_found)?;
        Ok(key)
    }

    /// Owned entries of the owner index for `owner`, narrowed by
    /// `type_filter` and, when given, resuming right after `cursor`. Shared
    /// by every owner and coin read: hash collisions of `type_filter`'s
    /// truncated hash are post-filtered here using the full `StructTag`
    /// carried by each row's value, so callers never see a row of an
    /// unrelated type.
    pub(crate) fn owner_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnerIndexKey>,
        type_filter: OwnerTypeFilter,
    ) -> IotaResult<
        impl Iterator<Item = Result<(OwnerIndexKey, OwnerIndexInfo), TypedStoreError>> + '_,
    > {
        let (lower_bound, upper_bound) = owner_bounds(owner, cursor, &type_filter);
        Ok(self
            .tables
            .owner
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound))
            .filter(move |result| match result {
                // Post-filter out hash collisions based on the full `StructTag` stored in the
                // value.
                Ok((_, info)) => match &type_filter {
                    OwnerTypeFilter::None => true,
                    OwnerTypeFilter::BaseType { tag, .. } => {
                        info.object_type.address() == tag.address()
                            && info.object_type.module() == tag.module()
                            && info.object_type.name() == tag.name()
                    }
                    OwnerTypeFilter::ExactType { tag, .. } => info.object_type == *tag,
                },
                // Don't filter out DB errors — let them pass through to the caller.
                Err(_) => true,
            }))
    }

    /// Owned coins of `owner`, in the unified key's order (balance-descending
    /// within a type). `coin_type` narrows the scan to `Coin<coin_type>`;
    /// `None` scans every coin type, the way [`Self::get_all_balances_from_db`]
    /// does. `cursor` is the last coin of the previous page, rebuilt from the
    /// live object the same way [`Self::get_owner_objects`]'s cursor is.
    pub fn get_owned_coins_iterator_with_cursor(
        &self,
        owner: Address,
        cursor: Option<ObjectId>,
        coin_type: Option<StructTag>,
        limit: usize,
        object_store: &dyn ObjectStore,
    ) -> IotaResult<Vec<(StructTag, ObjectId, CoinInfo)>> {
        self.require_jsonrpc()?;
        let tag = coin_type.unwrap_or_else(coin_base_type);
        let filter = OwnerTypeFilter::from_struct_tag(Some(&tag));
        let cursor_key = cursor
            .map(|id| self.owner_key_for_cursor(owner, id, object_store))
            .transpose()?;
        // The cursor above is still validated for a zero limit; only the
        // scan itself is skipped.
        let mut results = Vec::new();
        if limit == 0 {
            return Ok(results);
        }
        for item in self.owner_iter(owner, cursor_key.as_ref(), filter)? {
            let (key, info) = item?;
            if Some(key.object_id) == cursor {
                continue;
            }
            let Some(object) = object_store.try_get_object(&key.object_id)? else {
                continue;
            };
            let Some(coin) = CoinInfo::from_object(&object) else {
                continue;
            };
            results.push((info.object_type, key.object_id, coin));
            if results.len() >= limit {
                break;
            }
        }
        Ok(results)
    }

    /// This method first gets the balance from `per_coin_type_balance` cache.
    /// On a cache miss, it gets the balance for passed in `coin_type` from
    /// the `all_balance` cache. Only on the second cache miss, we go to the
    /// database (expensive) and update the cache.
    pub fn get_balance(&self, owner: Address, coin_type: TypeTag) -> IotaResult<TotalBalance> {
        self.require_jsonrpc()?;
        self.jsonrpc_metrics.balance_lookup_from_total.inc();
        let force_disable_cache = read_size_from_env(ENV_VAR_DISABLE_INDEX_CACHE).unwrap_or(0) > 0;
        if force_disable_cache {
            return self.get_balance_from_db(owner, &coin_type);
        }

        if let Some(balance) = self
            .caches
            .per_coin_type_balance
            .get(&(owner, coin_type.clone()))
        {
            return balance;
        }
        // Repopulating a missed entry must not interleave with a commit for
        // this owner: a value read between the commit's batch write and its
        // cache merge would get the checkpoint's delta applied twice. The
        // committer holds this lock across both, so the repopulation runs
        // either fully before it (the delta then merges on top) or fully
        // after (the merge skipped the absent key).
        let _lock = self.caches.locks.acquire_lock(owner);
        // A reader ahead of this one may have filled the entry while it
        // waited.
        if let Some(balance) = self
            .caches
            .per_coin_type_balance
            .get(&(owner, coin_type.clone()))
        {
            return balance;
        }
        // cache miss, lookup in all balance cache
        if let Some(Ok(all_balance)) = self.caches.all_balances.get(&owner) {
            if let Some(balance) = all_balance.get(&coin_type) {
                return Ok(*balance);
            }
        }
        // The database read runs before the cache insert, so the cache
        // shard's write lock is not held across the scan and owners of other
        // shard entries stay unblocked.
        let balance = self.get_balance_from_db(owner, &coin_type);
        self.caches
            .per_coin_type_balance
            .get_with((owner, coin_type), move || balance)
    }

    /// This method gets the balance for all coin types from the `all_balance`
    /// cache. On a cache miss, we go to the database (expensive) and update
    /// the cache. This cache is dual purpose in the sense that it not only
    /// serves `get_AllBalance()` calls but is also used for serving
    /// `get_Balance()` queries.
    pub fn get_all_balance(&self, owner: Address) -> IotaResult<Arc<AllBalance>> {
        self.require_jsonrpc()?;
        self.jsonrpc_metrics.all_balance_lookup_from_total.inc();
        let force_disable_cache = read_size_from_env(ENV_VAR_DISABLE_INDEX_CACHE).unwrap_or(0) > 0;
        if force_disable_cache {
            return self.get_all_balances_from_db(owner);
        }

        if let Some(all_balance) = self.caches.all_balances.get(&owner) {
            return all_balance;
        }
        // See `get_balance`: repopulation takes the owner's lock so it
        // cannot interleave with a commit's write-then-merge, and the
        // database read runs before the cache insert.
        let _lock = self.caches.locks.acquire_lock(owner);
        if let Some(all_balance) = self.caches.all_balances.get(&owner) {
            return all_balance;
        }
        let all_balance = self.get_all_balances_from_db(owner);
        self.caches
            .all_balances
            .get_with(owner, move || all_balance)
    }

    /// Sums the owner index's coin rows for `Coin<coin_type>`: `owner_iter`'s
    /// `ExactType` filter already excludes every other coin type, hash
    /// collisions included, so no post-filtering is needed here.
    pub(super) fn get_balance_from_db(
        &self,
        owner: Address,
        coin_type: &TypeTag,
    ) -> IotaResult<TotalBalance> {
        self.jsonrpc_metrics.balance_lookup_from_db.inc();
        let tag = StructTag::new_coin(coin_type.clone());
        let filter = OwnerTypeFilter::from_struct_tag(Some(&tag));
        let mut balance = 0i128;
        let mut num_coins = 0i64;
        for item in self.owner_iter(owner, None, filter)? {
            let (key, _) = item?;
            balance += coin_balance(&key) as i128;
            num_coins += 1;
        }
        Ok(TotalBalance { balance, num_coins })
    }

    /// Sums the owner index's coin rows of every type for `owner`, grouped by
    /// the exact `Coin<T>` each row's value carries: `owner_iter`'s
    /// `BaseType` filter matches every coin type but leaves the collision
    /// check (a `T` that hashes the same as an unrelated one) to the value.
    pub(super) fn get_all_balances_from_db(&self, owner: Address) -> IotaResult<Arc<AllBalance>> {
        self.jsonrpc_metrics.all_balance_lookup_from_db.inc();
        let filter = OwnerTypeFilter::from_struct_tag(Some(&coin_base_type()));
        let mut balances: AllBalance = HashMap::new();
        for item in self.owner_iter(owner, None, filter)? {
            let (key, info) = item?;
            let Some(coin_type) = info.object_type.coin_type_opt().cloned() else {
                continue;
            };
            let entry = balances.entry(coin_type).or_default();
            entry.balance += coin_balance(&key) as i128;
            entry.num_coins += 1;
        }
        Ok(Arc::new(balances))
    }

    /// Turns a committed checkpoint's coin changes into the balance cache
    /// deltas, holding the affected owners' locks for as long as the returned
    /// value lives. Runs entirely off the checkpoint: the balances each owner
    /// held before it were collected while its object changes were staged, so
    /// no table has to be read here.
    pub(super) fn balance_cache_updates(
        &self,
        coin_changes: CoinBalanceChanges,
    ) -> IndexStoreCacheUpdates {
        if coin_changes.0.is_empty() {
            return IndexStoreCacheUpdates::default();
        }

        let addresses: HashSet<Address> = coin_changes.0.keys().map(|(owner, _)| *owner).collect();
        let _locks = self.caches.locks.acquire_locks(addresses.into_iter());

        let mut balance_changes: HashMap<Address, AllBalance> = HashMap::new();
        for ((owner, _), change) in coin_changes.0 {
            let entry = balance_changes
                .entry(owner)
                .or_default()
                .entry(change.coin_type)
                .or_insert(TotalBalance {
                    num_coins: 0,
                    balance: 0,
                });
            match (change.prior, change.current) {
                (Some(prior), Some(current)) => {
                    entry.balance += current as i128 - prior as i128;
                }
                (None, Some(current)) => {
                    entry.num_coins += 1;
                    entry.balance += current as i128;
                }
                (Some(prior), None) => {
                    entry.num_coins -= 1;
                    entry.balance -= prior as i128;
                }
                // The owner neither held the coin before the checkpoint nor
                // holds it after: a coin created and spent within it.
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
                    Ok::<Arc<AllBalance>, IotaError>(Arc::new(balance_map)),
                )
            })
            .collect();
        IndexStoreCacheUpdates {
            _locks,
            per_coin_type_balance_changes,
            all_balance_changes,
        }
    }

    /// Drops the affected entries, so the next read repopulates them from the
    /// database.
    pub(super) fn invalidate_balance_caches(&self, updates: &IndexStoreCacheUpdates) {
        self.caches.per_coin_type_balance.batch_invalidate(
            updates
                .per_coin_type_balance_changes
                .iter()
                .map(|(key, _)| key.clone()),
        );
        self.caches.all_balances.batch_invalidate(
            updates
                .all_balance_changes
                .iter()
                .map(|(address, _)| *address),
        );
    }

    /// Applies the deltas to the entries the caches already hold.
    pub(super) fn merge_balance_cache_updates(&self, updates: IndexStoreCacheUpdates) {
        self.update_per_coin_type_cache(updates.per_coin_type_balance_changes);
        self.update_all_balance_cache(updates.all_balance_changes);
    }

    fn update_per_coin_type_cache(
        &self,
        keys: impl IntoIterator<Item = ((Address, TypeTag), IotaResult<TotalBalance>)>,
    ) {
        self.caches
            .per_coin_type_balance
            .batch_merge(keys, Self::merge_balance);
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
        keys: impl IntoIterator<Item = (Address, IotaResult<Arc<AllBalance>>)>,
    ) {
        self.caches
            .all_balances
            .batch_merge(keys, Self::merge_all_balance);
    }

    fn merge_all_balance(
        old_balance: &IotaResult<Arc<AllBalance>>,
        balance_delta: &IotaResult<Arc<AllBalance>>,
    ) -> IotaResult<Arc<AllBalance>> {
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

/// The balance a coin's owner-index key carries: `inverted_balance` is
/// `Some` for every row `owner_iter`'s coin filters yield, since only coins
/// set it.
fn coin_balance(key: &OwnerIndexKey) -> u64 {
    !key.inverted_balance
        .expect("a coin owner-index row always carries a balance")
}

/// A [`LayoutResolver`] memoizing layouts by struct tag, for callers that
/// resolve many values of few types, e.g. scanning a dynamic-field table
/// whose entries share one type.
pub struct CachingLayoutResolver<'a> {
    resolver: &'a mut dyn LayoutResolver,
    layouts: HashMap<StructTag, A::MoveDatatypeLayout>,
}

impl<'a> CachingLayoutResolver<'a> {
    pub fn new(resolver: &'a mut dyn LayoutResolver) -> Self {
        Self {
            resolver,
            layouts: HashMap::new(),
        }
    }
}

impl LayoutResolver for CachingLayoutResolver<'_> {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &StructTag,
    ) -> Result<A::MoveDatatypeLayout, IotaError> {
        if let Some(layout) = self.layouts.get(struct_tag) {
            return Ok(layout.clone());
        }
        let layout = self.resolver.get_annotated_layout(struct_tag)?;
        self.layouts.insert(struct_tag.clone(), layout.clone());
        Ok(layout)
    }
}

/// Resolves a `Field` object into the [`DynamicFieldInfo`] served by the
/// JSON-RPC API. Runs at query time — the index stores only the field keys.
/// Returns `None` when `o` is not a `Field` object, its layout cannot be
/// resolved, or a dynamic object field's value object no longer exists.
pub fn try_create_dynamic_field_info(
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
            // The wrapper is not rewritten when its child is mutated, so its
            // version is not the child's.
            let Some(object) = object_store.try_get_object(&object_id)? else {
                return Ok(None);
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
