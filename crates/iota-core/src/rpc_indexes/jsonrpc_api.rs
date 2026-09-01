// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::*;

pub(crate) const ENV_VAR_DISABLE_INDEX_CACHE: &str = "DISABLE_INDEX_CACHE";

pub(crate) const ENV_VAR_INVALIDATE_INSTEAD_OF_UPDATE: &str = "INVALIDATE_INSTEAD_OF_UPDATE";

pub(crate) type AllBalance = HashMap<TypeTag, TotalBalance>;

pub(crate) type OwnedMutexGuard<T> = ArcMutexGuard<parking_lot::RawMutex, T>;

#[derive(Default)]
pub struct IndexStoreCacheUpdates {
    pub(crate) _locks: Vec<OwnedMutexGuard<()>>,
    pub(crate) per_coin_type_balance_changes: Vec<((Address, TypeTag), IotaResult<TotalBalance>)>,
    pub(crate) all_balance_changes: Vec<(Address, IotaResult<Arc<AllBalance>>)>,
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
            // How far the backfill got is visible nowhere else, so keep it
            // above the default metric filter.
            history_backfill_lowest_replayed_checkpoint: register_int_gauge_with_registry!(
                "jsonrpc_index_history_backfill_lowest_replayed_checkpoint",
                "Lowest checkpoint the JSON-RPC index history backfill has replayed, keeping its \
                 final value after the backfill stops; unaffected by later pruning",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            history_backfill_running: register_int_gauge_with_registry!(
                "jsonrpc_index_history_backfill_running",
                "1 while the JSON-RPC index history backfill is running, 0 otherwise",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
        }
    }
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

/// Scan bounds excluding `cursor`: the inclusive lower bound for forward
/// scans and the inclusive upper bound for reverse scans. `None` when the
/// cursor leaves nothing to scan.
pub(crate) fn sequence_bounds_after_cursor(
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

impl IndexStore {
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

    pub(crate) fn get_transactions_from_index<KeyT: Clone + Serialize + DeserializeOwned>(
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

    pub fn get_transactions_to_addr(
        &self,
        addr: Address,
        cursor: Option<TxSequenceNumber>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        self.get_transactions_from_index(|bucket| &bucket.txs_to_addr, addr, cursor, limit, reverse)
    }

    /// The retained history buckets in scan order: ascending epochs for
    /// forward scans, descending for reverse scans. Buckets are disjoint,
    /// epoch-ordered segments of the global sequence order, so chaining
    /// per-bucket scans in this order preserves it.
    pub(crate) fn history_buckets(&self, reverse: bool) -> Vec<Arc<HistoryBucket>> {
        self.history.iter(reverse)
    }

    /// Maps an `event_order` row to the query result shape.
    pub(crate) fn event_order_row(
        ((_, event_seq), (digest, tx_digest, time)): (EventId, EventIndex),
    ) -> (TransactionEventsDigest, TransactionDigest, usize, u64) {
        (digest, tx_digest, event_seq, time)
    }

    /// Maps a keyed event-table row to the query result shape.
    pub(crate) fn keyed_event_row<K>(
        ((_, (_, event_seq)), (digest, tx_digest, time)): ((K, EventId), EventIndex),
    ) -> (TransactionEventsDigest, TransactionDigest, usize, u64) {
        (digest, tx_digest, event_seq, time)
    }

    /// Chains one range scan per retained history bucket, in
    /// global sequence order, collecting up to `limit` mapped rows.
    pub(crate) fn scan_history_buckets<K, V, R>(
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

    pub(crate) fn get_event_from_index<KeyT: Clone + Serialize + DeserializeOwned>(
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
            .safe_iter_with_prefix_from(&object, &cursor.unwrap_or(ObjectId::ZERO))
            // The seek is inclusive, so drop the cursor by id: its own row
            // may already be gone.
            .filter_ok(move |((_, field_id), ())| Some(*field_id) != cursor)
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
        let all_balance = self.caches.all_balances.get(&owner.clone());
        if let Some(Ok(all_balance)) = all_balance {
            if let Some(balance) = all_balance.get(&coin_type) {
                return Ok(*balance);
            }
        }
        // The database read runs before the cache insert, so the cache
        // shard's write lock is not held across the scan and owners of other
        // shard entries stay unblocked.
        let balance = Self::get_balance_from_db(
            self.metrics.clone(),
            self.tables.coin_index.clone(),
            owner,
            coin_type.clone(),
        )
        .map_err(|e| IotaError::Execution(format!("Failed to read balance frm DB: {e:?}")));
        self.caches
            .per_coin_type_balance
            .get_with((owner, coin_type), move || balance)
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
        let all_balance = Self::get_all_balances_from_db(metrics_cloned, coin_index_cloned, owner)
            .map_err(|e| {
                IotaError::Execution(format!("Failed to read all balance from DB: {e:?}"))
            });
        self.caches
            .all_balances
            .get_with(owner, move || all_balance)
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

    /// Derives the balance cache updates for a checkpoint's net coin changes
    /// by comparing them against the pre-commit database state, holding the
    /// affected owners' locks. Must run before the checkpoint's batch is
    /// written.
    pub(crate) fn balance_cache_updates(
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

    pub(crate) fn update_per_coin_type_cache(
        &self,
        keys: impl IntoIterator<Item = ((Address, TypeTag), IotaResult<TotalBalance>)>,
    ) -> IotaResult {
        self.caches
            .per_coin_type_balance
            .batch_merge(keys, Self::merge_balance);
        Ok(())
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
        let cursor = (starting_object_id != ObjectId::ZERO).then_some(starting_object_id);
        Ok(self
            .tables
            .owner_index
            // The object id 0 is the smallest possible
            .safe_iter_with_bounds(Some((owner, starting_object_id)), None)
            .map(|result| result.expect("iterator db error"))
            // The seek is inclusive, so drop the cursor by id: its own row
            // may already be gone.
            .filter(move |((_, object_id), _)| Some(*object_id) != cursor)
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

    pub(crate) fn merge_balance(
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

    pub(crate) fn update_all_balance_cache(
        &self,
        keys: impl IntoIterator<Item = (Address, IotaResult<Arc<HashMap<TypeTag, TotalBalance>>>)>,
    ) -> IotaResult {
        self.caches
            .all_balances
            .batch_merge(keys, Self::merge_all_balance);
        Ok(())
    }

    pub(crate) fn merge_all_balance(
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

/// A [`LayoutResolver`] memoizing layouts by struct tag, for callers that
/// resolve many values of few types, e.g. scanning a dynamic-field table
/// whose entries share one type.
pub(crate) struct CachingLayoutResolver<'a> {
    pub(crate) resolver: &'a mut dyn LayoutResolver,
    pub(crate) layouts: HashMap<StructTag, A::MoveDatatypeLayout>,
}

impl<'a> CachingLayoutResolver<'a> {
    pub(crate) fn new(resolver: &'a mut dyn LayoutResolver) -> Self {
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
