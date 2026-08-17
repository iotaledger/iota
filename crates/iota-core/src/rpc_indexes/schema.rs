// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The unified RPC index store's schema: the tables both the JSON-RPC and
//! gRPC read surfaces share, and the history tables that live in per-epoch
//! column families (see [`super`]).

use std::{
    collections::{BTreeSet, HashMap, hash_map::Entry},
    hash::Hasher,
    sync::Arc,
};

use iota_sdk_types::{
    Address, ObjectId, ObjectReference, Owner, StructTag, TransactionDigest, TransactionEffects,
    TransactionEvents, TransactionEventsDigest, TypeTag, Version,
};
use iota_types::{
    base_types::TxSequenceNumber,
    committee::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::IotaResult,
    messages_checkpoint::CheckpointSequenceNumber,
    move_package::MovePackageExt,
    object::Object,
    storage::{DynamicFieldKey, PackageVersionInfo, PackageVersionKey},
    transaction::{TransactionAPI, TransactionEnvelope},
};
use move_core_types::{
    account_address::AccountAddress, identifier::Identifier, language_storage::ModuleId,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use typed_store::{
    DBMapUtils, TypedStoreError,
    database::Database,
    rocks::{DBBatch, DBMap, ReadWriteOptions, TaggedDBMap},
};

/// The API groups whose read surface the unified store can serve. A store
/// serves whichever of these its node needs; the enabled set is recorded in
/// [`MetadataInfo`] so turning one on rebuilds the store instead of silently
/// leaving its tables empty.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexGroup {
    JsonRpc,
    Grpc,
}

/// A singleton stored in the `meta` table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct MetadataInfo {
    /// Version of the database.
    pub(super) version: u64,
    /// The API groups whose tables this store maintains. An enabled group
    /// missing here means its tables were never filled — the store is
    /// wiped and rebuilt, exactly like a stale watermark.
    pub(super) groups: BTreeSet<IndexGroup>,
}

/// Subdirectory of the node's database path holding the unified RPC index
/// store.
pub const RPC_INDEXES_DIR: &str = "rpc_indexes";

/// Bump this when changing the serialization format or layout of an
/// existing table. A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`. Starts over at 1: the unified store lives
/// under its own database directory, so it carries none of the history of
/// either store it replaces.
pub(super) const CURRENT_DB_VERSION: u64 = 1;

/// Prefix of the per-epoch history column families; a bucket's family is
/// `{prefix}{epoch}`. On-disk names are the ground truth for which buckets
/// exist.
pub(super) const HISTORY_CF_PREFIX: &str = "hist_rpc_e";

// The tag a history table's keys carry inside its bucket's column family.
// Do not reuse a tag for a different table: mark it retired in a comment
// instead, so an older bucket's rows can never be read as the wrong type.
// Public so that the database inspection tooling can scan a bucket without
// reopening the store.
pub const DB_PREFIX_HISTORIC_TX_ORDER: u8 = 0;
/// A transaction's position in the network order, which places a JSON-RPC
/// query cursor. The checkpoint that confirmed the transaction is not here:
/// it is kept with the ledger, so a finality answer cannot expire before
/// the transaction it describes.
pub const DB_PREFIX_HISTORIC_TXS_SEQ: u8 = 1;
pub const DB_PREFIX_HISTORIC_TXS_FROM_ADDR: u8 = 2;
pub const DB_PREFIX_HISTORIC_TXS_TO_ADDR: u8 = 3;
pub const DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID: u8 = 4;
pub const DB_PREFIX_HISTORIC_TXS_BY_MUTATED_OBJECT_ID: u8 = 5;
pub const DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION: u8 = 6;
pub const DB_PREFIX_HISTORIC_EVENT_ORDER: u8 = 7;
pub const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE: u8 = 8;
pub const DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT: u8 = 9;
pub const DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE: u8 = 10;
pub const DB_PREFIX_HISTORIC_EVENT_BY_SENDER: u8 = 11;
pub const DB_PREFIX_HISTORIC_EVENT_BY_TIME: u8 = 12;

/// The column-family name of `epoch`'s history bucket.
pub fn history_cf_name(epoch: EpochId) -> String {
    crate::rpc_index_history::bucket_cf_name(HISTORY_CF_PREFIX, epoch)
}

/// The epoch of a history column family, `None` for other names.
pub fn history_cf_epoch(cf_name: &str) -> Option<EpochId> {
    crate::rpc_index_history::bucket_cf_epoch(HISTORY_CF_PREFIX, cf_name)
}

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct TotalBalance {
    pub balance: i128,
    pub num_coins: i64,
}

/// Hash-based owner index key with fixed-size layout for correct RocksDB
/// byte-order iteration.
///
/// ## Sort order (bincode big-endian serialization)
///
/// Keys are ordered by `(owner, object_type_identifier, object_type_params,
/// inverted_balance, object_id)`.
///
/// `inverted_balance` is `None` for non-coin objects and `Some(!balance)` for
/// coins. When serialized, `None` sorts before `Some(...)`, so **non-coin
/// objects sort before coins** within the same `(owner, type_id, type_params)`
/// group. Among coins, `!balance` inverts the natural order so that **higher
/// balances sort first** (richest first).
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OwnerIndexKey {
    pub owner: Address,
    pub object_type_identifier: u64,
    pub object_type_params: u64,
    pub inverted_balance: Option<u64>,
    pub object_id: ObjectId,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerIndexInfo {
    pub object_type: StructTag,
    pub version: Version,
}

/// Type filter for the owner index's range scan.
///
/// - `None` — all objects for the owner.
/// - `BaseType` — all objects whose `address::module::name` matches (e.g. all
///   `Coin<*>`). Post-filters hash collisions via `tag`.
/// - `ExactType` — only objects of the exact `StructTag` (e.g. `Coin<IOTA>`).
///   Post-filters hash collisions via `tag`.
#[derive(Clone)]
pub(crate) enum OwnerTypeFilter {
    None,
    BaseType {
        id_hash: u64,
        tag: StructTag,
    },
    ExactType {
        id_hash: u64,
        params_hash: u64,
        tag: StructTag,
    },
}

impl OwnerTypeFilter {
    /// Construct an `OwnerTypeFilter` from an optional `StructTag` filter.
    ///
    /// If `None`, returns `OwnerTypeFilter::None`. If `Some(tag)` with no
    /// type params, returns `OwnerTypeFilter::BaseType`. If `Some(tag)`
    /// with type params, returns `OwnerTypeFilter::ExactType`.
    pub(crate) fn from_struct_tag(tag: Option<&StructTag>) -> Self {
        if let Some(tag) = tag {
            if tag.type_params().is_empty() {
                Self::BaseType {
                    id_hash: hash_type_identifier(tag),
                    tag: tag.clone(),
                }
            } else {
                Self::ExactType {
                    id_hash: hash_type_identifier(tag),
                    params_hash: hash_type_params(tag),
                    tag: tag.clone(),
                }
            }
        } else {
            Self::None
        }
    }
}

fn hash_type_identifier(tag: &StructTag) -> u64 {
    let mut hasher = twox_hash::XxHash64::with_seed(0);
    hasher.write(tag.address().as_ref());
    hasher.write(tag.module().as_bytes());
    hasher.write(tag.name().as_bytes());
    hasher.finish()
}

fn hash_type_params(tag: &StructTag) -> u64 {
    let mut hasher = twox_hash::XxHash64::with_seed(1);
    let bytes = bcs::to_bytes(&tag.type_params()).expect("type_params serialization cannot fail");
    hasher.write(&bytes);
    hasher.finish()
}

/// Compute inclusive lower and upper `OwnerIndexKey` bounds for a
/// `safe_iter_with_bounds` range scan, narrowed by `type_filter`.
///
/// When `cursor` is `Some`, the lower bound is set to the cursor's exact
/// position (inclusive) so that RocksDB can seek directly. `cursor`'s own
/// `owner` is ignored in favor of the explicit `owner` argument: callers
/// resume a scan for `owner` from a key they already know belongs to it.
pub(super) fn owner_bounds(
    owner: Address,
    cursor: Option<&OwnerIndexKey>,
    filter: &OwnerTypeFilter,
) -> (OwnerIndexKey, OwnerIndexKey) {
    let lower_bound = if let Some(c) = cursor {
        // Resume from the exact cursor position.
        OwnerIndexKey {
            owner,
            object_type_identifier: c.object_type_identifier,
            object_type_params: c.object_type_params,
            inverted_balance: c.inverted_balance,
            object_id: c.object_id,
        }
    } else {
        let (lower_id, _, lower_params, _) = match filter {
            OwnerTypeFilter::None => (0, u64::MAX, 0, u64::MAX),
            OwnerTypeFilter::BaseType { id_hash, .. } => (*id_hash, *id_hash, 0, u64::MAX),
            OwnerTypeFilter::ExactType {
                id_hash,
                params_hash,
                ..
            } => (*id_hash, *id_hash, *params_hash, *params_hash),
        };
        OwnerIndexKey {
            owner,
            object_type_identifier: lower_id,
            object_type_params: lower_params,
            inverted_balance: None,
            object_id: ObjectId::ZERO,
        }
    };

    let (_, upper_bound_id, _, upper_bound_params) = match filter {
        OwnerTypeFilter::None => (0, u64::MAX, 0, u64::MAX),
        OwnerTypeFilter::BaseType { id_hash, .. } => (*id_hash, *id_hash, 0, u64::MAX),
        OwnerTypeFilter::ExactType {
            id_hash,
            params_hash,
            ..
        } => (*id_hash, *id_hash, *params_hash, *params_hash),
    };

    let upper_bound = OwnerIndexKey {
        owner,
        object_type_identifier: upper_bound_id,
        object_type_params: upper_bound_params,
        inverted_balance: Some(u64::MAX),
        object_id: ObjectId::MAX,
    };

    (lower_bound, upper_bound)
}

impl OwnerIndexKey {
    /// Builds the key and value an address-owned `object` occupies in the
    /// owner index — shared by the live indexer, the cursor rebuild, and the
    /// deletion path, so all three agree on where an object sorts. `None`
    /// when `object` has no Move type (e.g. a package).
    pub(crate) fn for_object(
        owner: Address,
        object: &Object,
    ) -> Option<(OwnerIndexKey, OwnerIndexInfo)> {
        let struct_tag: StructTag = object.data.opt_object_type()?.clone().into();
        let id_hash = hash_type_identifier(&struct_tag);
        let params_hash = hash_type_params(&struct_tag);

        // For coins, extract the balance for inverted sorting (richest first).
        let inverted_balance = if object.is_coin() {
            let balance = object
                .as_coin_maybe()
                .map(|c| c.balance.value())
                .unwrap_or(0);
            Some(!balance)
        } else {
            None
        };

        let key = OwnerIndexKey {
            owner,
            object_type_identifier: id_hash,
            object_type_params: params_hash,
            inverted_balance,
            object_id: object.id(),
        };
        let info = OwnerIndexInfo {
            object_type: struct_tag,
            version: object.version(),
        };
        Some((key, info))
    }
}

/// Key of the `coin` table: regulated coin metadata, one entry per coin
/// type.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct CoinIndexKey {
    pub(super) coin_type: StructTag,
}

/// Coin index value with regulated coin metadata.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub(crate) struct CoinIndexInfo {
    pub(super) coin_metadata_object_id: Option<ObjectId>,
    pub(super) treasury_object_id: Option<ObjectId>,
    pub(super) regulated_coin_metadata_object_id: Option<ObjectId>,
}

impl CoinIndexInfo {
    /// Fills in the object ids `self` does not have yet from `other`. A coin
    /// type's metadata, treasury and regulated metadata are separate objects,
    /// so each contributes one field of the same row.
    pub(super) fn merge(&mut self, other: Self) {
        self.coin_metadata_object_id = self
            .coin_metadata_object_id
            .or(other.coin_metadata_object_id);
        self.treasury_object_id = self.treasury_object_id.or(other.treasury_object_id);
        self.regulated_coin_metadata_object_id = self
            .regulated_coin_metadata_object_id
            .or(other.regulated_coin_metadata_object_id);
    }
}

/// Adds `info` to the coin metadata gathered so far, merging it into an
/// entry that is already there.
pub(super) fn merge_coin_into(
    index: &mut HashMap<CoinIndexKey, CoinIndexInfo>,
    key: CoinIndexKey,
    info: CoinIndexInfo,
) {
    match index.entry(key) {
        Entry::Occupied(mut occupied) => occupied.get_mut().merge(info),
        Entry::Vacant(vacant) => {
            vacant.insert(info);
        }
    }
}

/// Whether the object is a `Field` object of a dynamic field — the only
/// objects the dynamic-field index stores.
pub(super) fn is_dynamic_field(object: &Object) -> bool {
    object
        .data
        .as_opt_struct()
        .is_some_and(|move_object| move_object.struct_tag().is_dynamic_field())
}

/// The coin metadata row `object` contributes, when it is a `CoinMetadata<T>`
/// or a `TreasuryCap<T>`.
pub(super) fn try_create_coin_index_info(object: &Object) -> Option<(CoinIndexKey, CoinIndexInfo)> {
    use iota_types::coin::{CoinMetadata, TreasuryCap};

    let object_type = object.data.opt_object_type()?;

    if let Some(coin_type) = CoinMetadata::is_coin_metadata_with_coin_type(object_type).cloned() {
        return Some((
            CoinIndexKey { coin_type },
            CoinIndexInfo {
                coin_metadata_object_id: Some(object.id()),
                ..Default::default()
            },
        ));
    }

    if let Some(coin_type) = TreasuryCap::is_treasury_with_coin_type(object_type).cloned() {
        return Some((
            CoinIndexKey { coin_type },
            CoinIndexInfo {
                treasury_object_id: Some(object.id()),
                ..Default::default()
            },
        ));
    }

    None
}

/// The coin type and object id of a `RegulatedCoinMetadata<T>`, for the
/// coin table's third field.
pub(super) fn try_create_regulated_coin_info(object: &Object) -> Option<(CoinIndexKey, ObjectId)> {
    let move_object_type = object.data.opt_object_type()?;
    if !move_object_type.is_regulated_coin_metadata() {
        return None;
    }
    // RegulatedCoinMetadata<T> has one type parameter: the coin type
    let coin_type = match move_object_type.type_params().first()? {
        TypeTag::Struct(s) => *s.clone(),
        _ => return None,
    };
    Some((CoinIndexKey { coin_type }, object.id()))
}

/// The package-version row `object` occupies, when it is a Move package.
pub(super) fn try_create_package_version_info(
    object: &Object,
) -> Option<(PackageVersionKey, PackageVersionInfo)> {
    let package = object.data.as_opt_package()?;
    Some((
        PackageVersionKey {
            original_package_id: package.original_package_id(),
            version: object.version().as_u64(),
        },
        PackageVersionInfo {
            storage_id: object.id(),
        },
    ))
}

pub(super) type EventId = (TxSequenceNumber, usize);
pub(super) type EventIndex = (TransactionEventsDigest, TransactionDigest, u64);

/// Per-transaction inputs for the history tables of the index batch. Unlike
/// the live-state tables (owner, coin, dynamic field), these need only the
/// transaction, its effects, and its events — no object contents.
pub(super) struct TransactionIndexData {
    digest: TransactionDigest,
    sender: Address,
    active_inputs: Vec<ObjectId>,
    mutated_objects: Vec<(ObjectReference, Owner)>,
    move_functions: Vec<(ObjectId, String, String)>,
    events: TransactionEvents,
}

/// Extracts one transaction's history-table index inputs.
pub(super) fn transaction_index_data(
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

/// One epoch's history tables, sharing a single per-epoch column family of
/// the index database, distinguished by a tag byte prefixed to every key.
/// Transactions are numbered by network order and epochs partition that
/// order contiguously, so each bucket is a disjoint, epoch-ordered segment
/// of every history table: chaining per-bucket scans in epoch order
/// preserves the global iteration order, and pruning an epoch is one
/// constant-time column-family drop.
///
/// Every field below is query acceleration, pruned by the indexes retention
/// knob: losing a bucket means this node cannot *find* a transaction or
/// event through these tables, not that the transaction is gone. The
/// indexes window must therefore never exceed the ledger's, or a query can
/// return a digest whose transaction has already been pruned from the
/// ledger.
pub(super) struct HistoryBucket {
    /// Ordering of all indexed transactions. Filled only when the JSON-RPC
    /// group is enabled.
    pub(super) tx_order: TaggedDBMap<TxSequenceNumber, TransactionDigest>,

    /// Index from transaction digest to its position in the network order,
    /// which the JSON-RPC queries read to place a cursor. It is written
    /// whatever the enabled groups are, because checkpoint ingest looks a
    /// transaction up here to tell a replayed checkpoint from a new one.
    /// The checkpoint that confirmed a transaction is *not* here: that
    /// answer must not be able to expire before the transaction it
    /// describes, so it lives with the ledger in
    /// `AuthorityPerpetualTables::executed_transactions_to_checkpoint`.
    pub(super) txs_seq: TaggedDBMap<TransactionDigest, TxSequenceNumber>,

    /// Index from iota address to transactions initiated by that address.
    pub(super) txs_from_addr: TaggedDBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from iota address to transactions that were sent to that address.
    pub(super) txs_to_addr: TaggedDBMap<(Address, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that used that object id as input.
    pub(super) txs_by_input_object_id: TaggedDBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from object id to transactions that modified/created that object
    /// id.
    pub(super) txs_by_mutated_object_id:
        TaggedDBMap<(ObjectId, TxSequenceNumber), TransactionDigest>,

    /// Index from package id, module and function identifier to transactions
    /// that used that move function call as input.
    pub(super) txs_by_move_function:
        TaggedDBMap<(ObjectId, String, String, TxSequenceNumber), TransactionDigest>,

    pub(super) event_order: TaggedDBMap<EventId, EventIndex>,

    pub(super) event_by_move_module: TaggedDBMap<(ModuleId, EventId), EventIndex>,

    pub(super) event_by_move_event: TaggedDBMap<(StructTag, EventId), EventIndex>,

    pub(super) event_by_event_module: TaggedDBMap<(ModuleId, EventId), EventIndex>,

    pub(super) event_by_sender: TaggedDBMap<(Address, EventId), EventIndex>,

    pub(super) event_by_time: TaggedDBMap<(u64, EventId), EventIndex>,
}

impl HistoryBucket {
    pub(super) fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
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
        Ok(Self {
            tx_order: map(db, cf_name, DB_PREFIX_HISTORIC_TX_ORDER)?,
            txs_seq: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_SEQ)?,
            txs_from_addr: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_FROM_ADDR)?,
            txs_to_addr: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_TO_ADDR)?,
            txs_by_input_object_id: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_BY_INPUT_OBJECT_ID)?,
            txs_by_mutated_object_id: map(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_TXS_BY_MUTATED_OBJECT_ID,
            )?,
            txs_by_move_function: map(db, cf_name, DB_PREFIX_HISTORIC_TXS_BY_MOVE_FUNCTION)?,
            event_order: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_ORDER)?,
            event_by_move_module: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_MODULE)?,
            event_by_move_event: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_MOVE_EVENT)?,
            event_by_event_module: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_EVENT_MODULE)?,
            event_by_sender: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_SENDER)?,
            event_by_time: map(db, cf_name, DB_PREFIX_HISTORIC_EVENT_BY_TIME)?,
        })
    }

    /// Appends one transaction's history-table rows, digest included, to a
    /// checkpoint's batch. Only called for checkpoints replayed or indexed
    /// while the JSON-RPC group is enabled; a gRPC-only store fills
    /// `txs_seq` directly from the checkpoint's contents instead (see
    /// `RpcIndexesStore::replay_checkpoint_history`).
    pub(super) fn index_tx(
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
                    (e.struct_tag.clone(), (sequence, i)),
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
                            AccountAddress::new(e.struct_tag.address().into_bytes()),
                            Identifier::new(e.struct_tag.module().as_str()).unwrap(),
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

/// The live-state and marker tables of the unified RPC index — everything
/// that is bounded by the live object set or is a singleton. The history
/// tables live in per-epoch column families of the same database
/// ([`HistoryBucket`]) so that pruning drops whole epochs instead of
/// deleting rows.
///
/// `owner` and `dynamic_field` are shared by both API groups; `coin` and
/// `package_version` are gRPC-only for now. There is no JSON-RPC coin table:
/// coin balances are read from `owner` (see the design notes on
/// [`super::RpcIndexesStore`]).
#[derive(DBMapUtils)]
pub struct IndexStoreTables {
    /// A singleton that stores metadata information on the DB.
    ///
    /// A missing `meta` row or a version mismatch triggers a full re-index.
    /// During a rebuild, `meta` is written first and `watermark` last, so a
    /// crashed rebuild is re-detected on the next open.
    pub(super) meta: DBMap<(), MetadataInfo>,

    /// Highest checkpoint sequence number indexed.
    ///
    /// Written inside each checkpoint's batch, so index data and watermark
    /// land atomically. Falling behind `highest_executed_checkpoint`
    /// triggers a full re-index via `needs_to_do_initialization`.
    pub(super) watermark: DBMap<(), CheckpointSequenceNumber>,

    /// Lowest checkpoint whose transactions are in the history tables.
    ///
    /// A rebuild seeds this to one past the watermark (no history yet); the
    /// background replay then works downwards, committing the marker inside
    /// each checkpoint's batch, until it reaches the checkpoint-contents
    /// pruner. Absent on databases that were never rebuilt: their history
    /// has been indexed continuously and is complete. Backfill, digests
    /// included, lives entirely in the one history bucket family, so this
    /// is the only history marker the store needs.
    pub(super) history_watermark: DBMap<(), CheckpointSequenceNumber>,

    /// Earliest epoch retained by the last index pruning pass. History
    /// buckets below it are never recreated, and the backfill stops at it
    /// instead of replaying epochs the pruner would drop again.
    pub(super) earliest_retained_epoch: DBMap<(), EpochId>,

    /// This is an index of object references to currently existing objects,
    /// indexed by the composite key of the address of their owner and the
    /// object ID of the object. Shared by both API groups.
    pub owner: DBMap<OwnerIndexKey, OwnerIndexInfo>,

    /// An index of the currently existing dynamic fields, keyed by the
    /// object ID of their parent and the object ID of the `Field` object.
    /// Only the key is stored; field metadata is resolved on demand from the
    /// object store at query time. Shared by both API groups.
    pub dynamic_field: DBMap<DynamicFieldKey, ()>,

    /// Regulated coin metadata, one entry per coin type. gRPC-only.
    pub(super) coin: DBMap<CoinIndexKey, CoinIndexInfo>,

    /// Maps original package ID and version to the storage ID of that
    /// version, allowing efficient listing of all versions of a package.
    /// gRPC-only.
    pub package_version: DBMap<PackageVersionKey, PackageVersionInfo>,
}
