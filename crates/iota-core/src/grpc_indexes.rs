// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    hash::Hasher,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use iota_sdk_types::{Address, ObjectId, Owner, StructTag, TypeTag};
use iota_types::{
    base_types::SequenceNumber,
    committee::EpochId,
    digests::TransactionDigest,
    error::IotaResult,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CheckpointContents, CheckpointSequenceNumber},
    move_package::MovePackageExt,
    object::Object,
    storage::{
        AccountOwnedObjectInfo, DynamicFieldKey, EpochInfo, OwnedObjectCursor,
        OwnedObjectIteratorItem, PackageVersionInfo, PackageVersionIteratorItem, PackageVersionKey,
        TransactionInfo, error::Error as StorageError,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use typed_store::{
    DBMapUtils, TypedStoreError,
    rocks::{DBMap, MetricConf},
    traits::Map,
};

use crate::{
    authority::AuthorityStore,
    checkpoints::CheckpointStore,
    par_index_live_object_set::{LiveObjectIndexer, ParMakeLiveObjectIndexer},
};

/// Bump this when changing the serialization format of an existing table.
/// A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`.
const CURRENT_DB_VERSION: u64 = 1;

/// On-disk directory name for the gRPC indexes store.
pub const GRPC_INDEXES_DIR: &str = "grpc_indexes";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct MetadataInfo {
    /// Version of the Database
    version: u64,
}

/// Watermark type for the gRPC indexes store.
///
/// The variants are keys into the shared `watermark` column family
/// (`DBMap<Watermark, CheckpointSequenceNumber>`), each storing a checkpoint
/// sequence number.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Watermark {
    /// Highest checkpoint sequence number indexed.
    Indexed,
    /// Highest checkpoint sequence number pruned.
    Pruned,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoinIndexKey {
    coin_type: StructTag,
}

/// Coin index value with regulated coin metadata.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinIndexInfo {
    pub coin_metadata_object_id: Option<ObjectId>,
    pub treasury_object_id: Option<ObjectId>,
    pub regulated_coin_metadata_object_id: Option<ObjectId>,
}

impl From<CoinIndexInfo> for iota_types::storage::CoinInfo {
    fn from(info: CoinIndexInfo) -> Self {
        Self {
            coin_metadata_object_id: info.coin_metadata_object_id,
            treasury_object_id: info.treasury_object_id,
            regulated_coin_metadata_object_id: info.regulated_coin_metadata_object_id,
        }
    }
}

impl CoinIndexInfo {
    fn merge(&mut self, other: Self) {
        self.coin_metadata_object_id = self
            .coin_metadata_object_id
            .or(other.coin_metadata_object_id);
        self.treasury_object_id = self.treasury_object_id.or(other.treasury_object_id);
        self.regulated_coin_metadata_object_id = self
            .regulated_coin_metadata_object_id
            .or(other.regulated_coin_metadata_object_id);
    }
}

/// Insert-or-merge a [`CoinIndexInfo`] into an in-memory HashMap.
fn merge_coin_into(
    index: &mut HashMap<CoinIndexKey, CoinIndexInfo>,
    key: CoinIndexKey,
    info: CoinIndexInfo,
) {
    use std::collections::hash_map::Entry;
    match index.entry(key) {
        Entry::Occupied(mut o) => o.get_mut().merge(info),
        Entry::Vacant(v) => {
            v.insert(info);
        }
    }
}

/// Read-modify-write a [`CoinIndexInfo`] entry in the `coin` DB table.
///
/// Reads the current value (if any), applies `mutate`, and stages the result
/// into `batch`.  Used for incremental indexing where the full value is built
/// across multiple objects (e.g. `CoinMetadata` + `RegulatedCoinMetadata`).
fn read_merge_write_coin(
    table: &DBMap<CoinIndexKey, CoinIndexInfo>,
    batch: &mut typed_store::rocks::DBBatch,
    key: CoinIndexKey,
    mutate: impl FnOnce(&mut CoinIndexInfo),
) -> Result<(), StorageError> {
    let mut entry = table.get(&key).ok().flatten().unwrap_or_default();
    mutate(&mut entry);
    batch.insert_batch(table, [(key, entry)])?;
    Ok(())
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
/// coins.  When serialized, `None` sorts before `Some(...)`, so **non-coin
/// objects sort before coins** within the same `(owner, type_id, type_params)`
/// group.  Among coins, `!balance` inverts the natural order so that **higher
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
    pub version: SequenceNumber,
}

/// Type filter for `owner_iter`.
///
/// - `None` — all objects for the owner.
/// - `BaseType` — all objects whose `address::module::name` matches (e.g. all
///   `Coin<*>`). Post-filters hash collisions via `tag`.
/// - `ExactType` — only objects of the exact `StructTag` (e.g. `Coin<IOTA>`).
///   Post-filters hash collisions via `tag`.
#[derive(Clone)]
pub enum OwnerTypeFilter {
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
    /// If `None`, returns `OwnerTypeFilter::None`.  If `Some(tag)` with no
    /// type params, returns `OwnerTypeFilter::BaseType`.  If `Some(tag)`
    /// with type params, returns `OwnerTypeFilter::ExactType`.
    pub fn from_struct_tag(tag: Option<&StructTag>) -> Self {
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
/// position (inclusive) so that RocksDB can seek directly.
fn owner_bounds(
    owner: Address,
    cursor: Option<&OwnedObjectCursor>,
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

/// Build an `OwnerIndexKey` for an address-owned object.
fn make_owner_key(owner: Address, object: &Object) -> Option<(OwnerIndexKey, OwnerIndexInfo)> {
    let struct_tag: StructTag = object.type_()?.clone().into();
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

/// RocksDB tables for the GrpcIndexesStore
///
/// Anytime a new table is added, or an existing one has its schema changed,
/// make sure to also update the value of `CURRENT_DB_VERSION`.
///
/// NOTE: Authors and Reviewers before adding any new tables ensure that they
/// are either:
/// - bounded in size by the live object set
/// - are prune-able and have corresponding logic in the `prune` function
#[derive(DBMapUtils)]
struct IndexStoreTables {
    /// A singleton that store metadata information on the DB.
    ///
    /// A few uses for this singleton:
    /// - determining if the DB has been initialized (as some tables will still
    ///   be empty post initialization)
    /// - version of the DB. Everytime a new table or schema is changed the
    ///   version number needs to be incremented.
    meta: DBMap<(), MetadataInfo>,

    /// Table used to track watermark for the highest indexed checkpoint
    ///
    /// This is useful to help know the highest checkpoint that was indexed in
    /// the event that the node was running with indexes enabled, then run
    /// for a period of time with indexes disabled, and then run with them
    /// enabled again so that the tables can be reinitialized.
    watermark: DBMap<Watermark, CheckpointSequenceNumber>,

    /// Deprecated: per-epoch metadata moved to the CheckpointStore's
    /// `epoch_info` table. Active on released gRPC nodes, so it is dropped on
    /// open here; not migrated.
    #[allow(dead_code)]
    #[deprecated_db_map]
    epochs: Option<DBMap<EpochId, EpochInfo>>,

    /// Maps transaction digests to the checkpoint that contains them.
    ///
    /// Only contains entries for transactions which have yet to be pruned from
    /// the main database.
    transaction_checkpoints: DBMap<TransactionDigest, CheckpointSequenceNumber>,

    /// An index of object ownership.
    ///
    /// Uses fixed-size u64 hash keys for correct RocksDB byte-order iteration.
    /// Allows an efficient iterator to list all objects currently owned by a
    /// specific user account, optionally filtered by type.
    ///
    /// Full `StructTag` stored in value for collision filtering & API
    /// responses. Bounded by the live object set (one entry per
    /// address-owned object).
    owner: DBMap<OwnerIndexKey, OwnerIndexInfo>,

    /// An index of dynamic fields (children objects).
    ///
    /// Allows an efficient iterator to list all of the dynamic fields owned by
    /// a particular ObjectId. Only the key is stored; field metadata is loaded
    /// on demand from the object store.
    dynamic_field: DBMap<DynamicFieldKey, ()>,

    /// Coin info with regulated coin metadata.
    /// Bounded by the live object set (one entry per coin type).
    coin: DBMap<CoinIndexKey, CoinIndexInfo>,

    /// An index of Package versions.
    ///
    /// Maps original package ID and version to the storage ID of that version.
    /// Allows efficient listing of all versions of a package, including
    /// upgraded user packages that have different storage IDs.
    /// Bounded by the live object set (one entry per package version).
    package_version: DBMap<PackageVersionKey, PackageVersionInfo>,
    // NOTE: Authors and Reviewers before adding any new tables ensure that they are either:
    // - bounded in size by the live object set
    // - are prune-able and have corresponding logic in the `prune` function
}

impl IndexStoreTables {
    fn open<P: Into<PathBuf>>(path: P) -> Self {
        IndexStoreTables::open_tables_read_write(
            path.into(),
            MetricConf::new("grpc-index"),
            None,
            None,
        )
    }

    fn open_with_options<P: Into<PathBuf>>(
        path: P,
        options: typed_store::rocksdb::Options,
    ) -> Self {
        IndexStoreTables::open_tables_read_write(
            path.into(),
            MetricConf::new("grpc-index"),
            Some(options),
            None,
        )
    }

    fn needs_to_do_initialization(&self, checkpoint_store: &CheckpointStore) -> bool {
        // Schema mismatch (or unreadable meta) -> migration may be pending
        // and the watermark CF may be from an incompatible schema.
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
        let watermark = self.watermark.get(&Watermark::Indexed).ok().flatten();
        watermark < highest_executed_checkpoint
    }

    /// Range of checkpoints that transaction-digest indexing can cover.
    /// Returns `None` when there is nothing to do (no executed checkpoints,
    /// or the lower bound has overtaken the upper).
    fn transaction_index_range(
        &self,
        checkpoint_store: &CheckpointStore,
        highest_executed_checkpoint: Option<CheckpointSequenceNumber>,
    ) -> Result<Option<std::ops::RangeInclusive<CheckpointSequenceNumber>>, StorageError> {
        let lowest = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .map(|c| c.saturating_add(1))
            .unwrap_or(0);
        Ok(highest_executed_checkpoint
            .and_then(|highest| (lowest <= highest).then_some(lowest..=highest)))
    }

    /// See [`GrpcIndexesStore::live_object_restorer`].
    fn live_object_restorer(&self) -> GrpcLiveObjectRestorer<'_> {
        GrpcLiveObjectRestorer {
            tables: self,
            coin_index: Mutex::new(HashMap::new()),
        }
    }

    /// Phase 2 of `init`: rebuild the live-state indexes by scanning the
    /// current live object set in parallel. Must re-run on any drift to keep
    /// them consistent.
    fn index_live_object_set(&self, authority_store: &AuthorityStore) -> Result<(), StorageError> {
        let restorer = self.live_object_restorer();
        crate::par_index_live_object_set::par_index_live_object_set(authority_store, &restorer)?;
        restorer.finish()?;
        Ok(())
    }

    /// Runs only when `needs_to_do_initialization` is true (fresh DB, schema
    /// mismatch, crashed mid-init, or the index watermark falling behind
    /// `highest_executed_checkpoint`).
    /// The on-disk DB needs to be wiped before this is called, so `init` always
    /// starts from an empty store.
    #[tracing::instrument(skip_all)]
    fn init(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        info!("Initializing gRPC indexes");

        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;

        // Phase 1 — history-derived indexes. Transactions need only
        // `CheckpointContents`, so they span `transaction_index_range`
        // (checkpoint-store pruning).
        let tx_range =
            self.transaction_index_range(checkpoint_store, highest_executed_checkpoint)?;

        // `tx_range` is `None` only when no checkpoints have ever been executed
        // on this node, so skipping phase-1 indexing entirely is correct.
        if let Some(range) = tx_range {
            self.index_historical_checkpoints(checkpoint_store, range)?;
        }

        // Phase 2 — live-state indexes from the current live object set.
        self.index_live_object_set(authority_store)?;

        self.finalize(highest_executed_checkpoint.unwrap_or(0))?;

        info!("Finished initializing gRPC indexes");

        Ok(())
    }

    /// Mark the store fully initialized: set `Watermark::Indexed` to
    /// `indexed_checkpoint` and write `meta` last, so a crash before the
    /// `meta` write leaves a store the next `new` call wipes and re-inits.
    /// The final step of both `init` and a formal-snapshot restore.
    fn finalize(
        &self,
        indexed_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), TypedStoreError> {
        self.watermark
            .insert(&Watermark::Indexed, &indexed_checkpoint)?;
        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )
    }

    /// Index transaction digests by replaying the `CheckpointContents` of
    /// every checkpoint in `checkpoint_range` in order.
    #[tracing::instrument(skip(self, checkpoint_store))]
    fn index_historical_checkpoints(
        &self,
        checkpoint_store: &CheckpointStore,
        checkpoint_range: std::ops::RangeInclusive<u64>,
    ) -> Result<(), StorageError> {
        info!(
            "Indexing {} checkpoints in range {checkpoint_range:?}",
            checkpoint_range.size_hint().0
        );
        let start_time = Instant::now();

        for checkpoint_sequence_number in checkpoint_range {
            let summary = checkpoint_store
                .get_checkpoint_by_sequence_number(checkpoint_sequence_number)?
                .ok_or_else(|| {
                    StorageError::missing(format!(
                        "missing checkpoint {checkpoint_sequence_number}"
                    ))
                })?;
            let contents = checkpoint_store
                .get_checkpoint_contents(&summary.content_digest)?
                .ok_or_else(|| {
                    StorageError::missing(format!(
                        "missing checkpoint {checkpoint_sequence_number}"
                    ))
                })?;

            let mut batch = self.transaction_checkpoints.batch();
            self.index_transactions(checkpoint_sequence_number, &contents, &mut batch)?;
            batch.write()?;
        }

        info!(
            "Indexing checkpoints took {} seconds",
            start_time.elapsed().as_secs()
        );
        Ok(())
    }

    /// Prune data from this Index
    fn prune(
        &self,
        pruned_checkpoint_watermark: u64,
        checkpoint_contents_to_prune: &[CheckpointContents],
    ) -> Result<(), TypedStoreError> {
        let mut batch = self.transaction_checkpoints.batch();

        let transactions_to_prune = checkpoint_contents_to_prune
            .iter()
            .flat_map(|contents| contents.iter().map(|digests| digests.transaction));

        batch.delete_batch(&self.transaction_checkpoints, transactions_to_prune)?;
        batch.insert_batch(
            &self.watermark,
            [(Watermark::Pruned, pruned_checkpoint_watermark)],
        )?;

        batch.write()
    }

    /// Index a Checkpoint
    fn index_checkpoint(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<typed_store::rocks::DBBatch, StorageError> {
        debug!(
            checkpoint = checkpoint.checkpoint_summary.sequence_number,
            "indexing checkpoint"
        );

        let mut batch = self.transaction_checkpoints.batch();

        self.index_transactions(
            checkpoint.checkpoint_summary.sequence_number,
            &checkpoint.checkpoint_contents,
            &mut batch,
        )?;
        self.index_objects(checkpoint, &mut batch)?;

        batch.insert_batch(
            &self.watermark,
            [(
                Watermark::Indexed,
                checkpoint.checkpoint_summary.sequence_number,
            )],
        )?;

        debug!(
            checkpoint = checkpoint.checkpoint_summary.sequence_number,
            "finished indexing checkpoint"
        );

        Ok(batch)
    }

    fn index_transactions(
        &self,
        checkpoint_seq_number: CheckpointSequenceNumber,
        contents: &CheckpointContents,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        batch.insert_batch(
            &self.transaction_checkpoints,
            contents
                .iter()
                .map(|d| (d.transaction, checkpoint_seq_number)),
        )?;

        Ok(())
    }

    fn index_objects(
        &self,
        checkpoint: &CheckpointData,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        let mut coin_index: HashMap<CoinIndexKey, CoinIndexInfo> = HashMap::new();

        for tx in &checkpoint.transactions {
            // determine changes from removed objects
            for removed_object in tx.removed_objects_pre_version() {
                match removed_object.owner() {
                    Owner::Address(address) => {
                        // owner: delete old entry
                        if let Some((owner_key, _)) = make_owner_key(*address, removed_object) {
                            batch.delete_batch(&self.owner, [owner_key])?;
                        }
                    }
                    Owner::Object(object_id) => {
                        batch.delete_batch(
                            &self.dynamic_field,
                            [DynamicFieldKey::new(*object_id, removed_object.id())],
                        )?;
                    }
                    Owner::Shared(_) | Owner::Immutable => {}
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }

            // determine changes from changed objects
            for (object, old_object) in tx.changed_objects() {
                if let Some(old_object) = old_object {
                    match old_object.owner() {
                        Owner::Address(address) => {
                            // owner: delete old entry
                            if let Some((owner_key, _)) = make_owner_key(*address, old_object) {
                                batch.delete_batch(&self.owner, [owner_key])?;
                            }
                        }
                        Owner::Object(object_id) => {
                            if old_object.owner() != object.owner() {
                                batch.delete_batch(
                                    &self.dynamic_field,
                                    [DynamicFieldKey::new(*object_id, old_object.id())],
                                )?;
                            }
                        }
                        Owner::Shared(_) | Owner::Immutable => {}
                        _ => unimplemented!(
                            "a new Owner enum variant was added and needs to be handled"
                        ),
                    }
                }

                match object.owner() {
                    Owner::Address(owner) => {
                        if let Some((owner_key, owner_info)) = make_owner_key(*owner, object) {
                            batch.insert_batch(&self.owner, [(owner_key, owner_info)])?;
                        }
                    }
                    Owner::Object(parent) => {
                        if should_index_dynamic_field(object) {
                            let field_key = DynamicFieldKey::new(*parent, object.id());
                            batch.insert_batch(&self.dynamic_field, [(field_key, ())])?;
                        }
                    }
                    Owner::Shared(_) | Owner::Immutable => {}
                    _ => {
                        unimplemented!("a new Owner enum variant was added and needs to be handled")
                    }
                }
            }

            // coin indexing
            //
            // coin indexing relies on the fact that CoinMetadata and TreasuryCap are
            // created in the same transaction so we don't need to worry about
            // overriding any older value that may exist in the database
            // (because there necessarily cannot be).
            for (key, value) in tx.created_objects().flat_map(try_create_coin_index_info) {
                merge_coin_into(&mut coin_index, key, value);
            }
        }

        batch.insert_batch(&self.coin, coin_index)?;

        // package version + regulated coin indexing
        // Both use created_objects(): packages and RegulatedCoinMetadata objects are
        // always created, never mutated in-place, so changed_objects() would only add
        // noise from unrelated object mutations.
        let mut package_version_index: Vec<(PackageVersionKey, PackageVersionInfo)> = Vec::new();
        let mut regulated_coin_keys: Vec<(CoinIndexKey, ObjectId)> = Vec::new();
        for tx in &checkpoint.transactions {
            for object in tx.created_objects() {
                if let Some((key, info)) = try_create_package_version_info(object) {
                    package_version_index.push((key, info));
                }
                if let Some((key, object_id)) = try_create_regulated_coin_info(object) {
                    regulated_coin_keys.push((key, object_id));
                }
            }
        }
        batch.insert_batch(&self.package_version, package_version_index)?;
        // Merge regulated coin entries into coin table.
        // These are rare (at most one per regulated coin type per checkpoint),
        // so read-modify-write is acceptable.
        for (key, object_id) in regulated_coin_keys {
            read_merge_write_coin(&self.coin, batch, key, |entry| {
                entry.regulated_coin_metadata_object_id = Some(object_id);
            })?;
        }

        Ok(())
    }

    fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        Ok(self
            .transaction_checkpoints
            .get(digest)?
            .map(|checkpoint| TransactionInfo {
                checkpoint,
                object_types: Default::default(),
            }))
    }

    fn owner_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnedObjectCursor>,
        type_filter: OwnerTypeFilter,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKey, OwnerIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        let (lower_bound, upper_bound) = owner_bounds(owner, cursor, &type_filter);
        Ok(self
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

    fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> Result<impl Iterator<Item = Result<DynamicFieldKey, TypedStoreError>> + '_, TypedStoreError>
    {
        let lower_bound = DynamicFieldKey::new(parent, cursor.unwrap_or(ObjectId::ZERO));
        let upper_bound = DynamicFieldKey::new(parent, ObjectId::MAX);
        let iter = self
            .dynamic_field
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound))
            .map(|r| r.map(|(key, ())| key));
        Ok(iter)
    }

    fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfo>, TypedStoreError> {
        let key = CoinIndexKey {
            coin_type: coin_type.to_owned(),
        };
        self.coin.get(&key)
    }

    fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> Result<impl Iterator<Item = PackageVersionIteratorItem> + '_, TypedStoreError> {
        let lower_bound = PackageVersionKey {
            original_package_id,
            version: cursor.unwrap_or(0),
        };
        let upper_bound = PackageVersionKey {
            original_package_id,
            version: u64::MAX,
        };
        Ok(self
            .package_version
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound)))
    }
}

pub struct GrpcIndexesStore {
    tables: Arc<IndexStoreTables>,
    pending_updates: Mutex<BTreeMap<u64, typed_store::rocks::DBBatch>>,
}

impl GrpcIndexesStore {
    pub async fn new(
        path: PathBuf,
        authority_store: Arc<AuthorityStore>,
        checkpoint_store: &CheckpointStore,
    ) -> Self {
        let tables = {
            let tables = IndexStoreTables::open(&path);

            // If the index tables are uninitialized or on an older version then we need to
            // populate them
            if tables.needs_to_do_initialization(checkpoint_store) {
                let mut tables = {
                    drop(tables);
                    typed_store::rocks::safe_drop_db(path.clone(), Duration::from_secs(30))
                        .await
                        .expect("unable to destroy old gRPC index db");

                    // Open the empty DB with `unordered_write`s enabled in order to get a ~3x
                    // speedup when indexing
                    let mut options = typed_store::rocksdb::Options::default();
                    options.set_unordered_write(true);
                    IndexStoreTables::open_with_options(&path, options)
                };

                tables
                    .init(&authority_store, checkpoint_store)
                    .expect("unable to initialize gRPC index");

                let weak_db = Arc::downgrade(&tables.meta.db);
                drop(tables);

                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
                loop {
                    if weak_db.strong_count() == 0 {
                        break;
                    }
                    if std::time::Instant::now() > deadline {
                        panic!("unable to reopen DB after indexing");
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }

                // Reopen the DB with default options (eg without `unordered_write`s enabled)
                IndexStoreTables::open(&path)
            } else {
                tables
            }
        };

        let tables = Arc::new(tables);

        Self {
            tables,
            pending_updates: Default::default(),
        }
    }

    /// Open the store without the wipe/init logic of [`Self::new`] — for the
    /// restore tool, which populates and finalizes the store itself.
    pub fn new_without_init(path: PathBuf) -> Self {
        let tables = Arc::new(IndexStoreTables::open(path));

        Self {
            tables,
            pending_updates: Default::default(),
        }
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // We are checkpointing the whole db
        self.tables.meta.checkpoint_db(path).map_err(Into::into)
    }

    pub fn prune(
        &self,
        pruned_checkpoint_watermark: u64,
        checkpoint_contents_to_prune: &[CheckpointContents],
    ) -> Result<(), TypedStoreError> {
        self.tables
            .prune(pruned_checkpoint_watermark, checkpoint_contents_to_prune)
    }

    /// Index a checkpoint and stage the index updated in `pending_updates`.
    ///
    /// Updates will not be committed to the database until
    /// `commit_update_for_checkpoint` is called.
    #[tracing::instrument(
        skip_all,
        fields(checkpoint = checkpoint.checkpoint_summary.sequence_number)
    )]
    pub fn index_checkpoint(&self, checkpoint: &CheckpointData) {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        let batch = self.tables.index_checkpoint(checkpoint).expect("db error");

        self.pending_updates
            .lock()
            .unwrap()
            .insert(sequence_number, batch);
    }

    /// Commits the pending updates for the provided checkpoint number.
    ///
    /// Invariants:
    /// - `index_checkpoint` must have been called for the provided checkpoint
    /// - Callers of this function must ensure that it is called for each
    ///   checkpoint in sequential order. This will panic if the provided
    ///   checkpoint does not match the expected next checkpoint to commit.
    #[tracing::instrument(skip(self))]
    pub fn commit_update_for_checkpoint(&self, checkpoint: u64) -> Result<(), StorageError> {
        let next_batch = self.pending_updates.lock().unwrap().pop_first();

        // Its expected that the next batch exists
        let (next_sequence_number, batch) = next_batch.unwrap();
        assert_eq!(
            checkpoint, next_sequence_number,
            "commit_update_for_checkpoint must be called in order"
        );

        Ok(batch.write()?)
    }

    pub fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        self.tables.get_transaction_info(digest)
    }

    pub fn owner_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnedObjectCursor>,
        type_filter: OwnerTypeFilter,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKey, OwnerIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        self.tables.owner_iter(owner, cursor, type_filter)
    }

    pub fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> Result<impl Iterator<Item = Result<DynamicFieldKey, TypedStoreError>> + '_, TypedStoreError>
    {
        self.tables.dynamic_field_iter(parent, cursor)
    }

    pub fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfo>, TypedStoreError> {
        self.tables.get_coin_info(coin_type)
    }

    pub fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> Result<impl Iterator<Item = PackageVersionIteratorItem> + '_, TypedStoreError> {
        self.tables
            .package_versions_iter(original_package_id, cursor)
    }

    /// Restorer that builds the live-state indexes (owner, coin, dynamic
    /// field, package version) from a stream of live objects. A
    /// formal-snapshot restore feeds it the downloaded partitions; `init`
    /// uses the same machinery fed by a scan of the local store.
    pub fn live_object_restorer(&self) -> GrpcLiveObjectRestorer<'_> {
        self.tables.live_object_restorer()
    }

    /// Mark a restore-built store fully initialized (the same final step as
    /// `init`), so the node's `GrpcIndexesStore::new` opens it in place
    /// instead of wiping and re-indexing. `restore_checkpoint` is the
    /// restore's highest executed checkpoint.
    ///
    /// Callers must have restored the complete live-state indexes first,
    /// through [`Self::live_object_restorer`].
    pub fn finalize_restore(
        &self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), TypedStoreError> {
        self.tables.finalize(restore_checkpoint)
    }
}

// ---------------------------------------------------------------------------
// GrpcIndexes trait implementation
// ---------------------------------------------------------------------------

impl iota_node_storage::GrpcIndexes for GrpcIndexesStore {
    fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionInfo>> {
        self.tables
            .get_transaction_info(digest)
            .map_err(|e| StorageError::custom(e.to_string()))
    }

    fn account_owned_objects_info_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnedObjectCursor>,
        object_type: Option<StructTag>,
    ) -> iota_types::storage::error::Result<Box<dyn Iterator<Item = OwnedObjectIteratorItem> + '_>>
    {
        let type_filter = OwnerTypeFilter::from_struct_tag(object_type.as_ref());
        let iter = self
            .tables
            .owner_iter(owner, cursor, type_filter)
            .map_err(|e| StorageError::custom(e.to_string()))?
            .map(|result| {
                result.map(|(key, info)| {
                    let cursor = OwnedObjectCursor {
                        object_type_identifier: key.object_type_identifier,
                        object_type_params: key.object_type_params,
                        inverted_balance: key.inverted_balance,
                        object_id: key.object_id,
                    };
                    let obj_info = AccountOwnedObjectInfo {
                        owner: key.owner,
                        object_id: key.object_id,
                        version: info.version,
                        type_: info.object_type.into(),
                    };
                    (obj_info, cursor)
                })
            });
        Ok(Box::new(iter))
    }

    fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> iota_types::storage::error::Result<
        Box<dyn Iterator<Item = Result<DynamicFieldKey, TypedStoreError>> + '_>,
    > {
        let iter = self
            .tables
            .dynamic_field_iter(parent, cursor)
            .map_err(|e| StorageError::custom(e.to_string()))?;
        Ok(Box::new(iter))
    }

    fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> iota_types::storage::error::Result<Option<iota_types::storage::CoinInfo>> {
        self.tables
            .get_coin_info(coin_type)
            .map(|opt| opt.map(Into::into))
            .map_err(|e| StorageError::custom(e.to_string()))
    }

    fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> iota_types::storage::error::Result<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>>
    {
        let iter = self
            .tables
            .package_versions_iter(original_package_id, cursor)
            .map_err(|e| StorageError::custom(e.to_string()))?;
        Ok(Box::new(iter))
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Returns `true` if `object` is a `Field<Name, Value>` and should be
/// indexed in the dynamic field table.
fn should_index_dynamic_field(object: &Object) -> bool {
    object
        .data
        .as_struct_opt()
        .is_some_and(|move_object| move_object.struct_tag().is_dynamic_field())
}

fn try_create_coin_index_info(object: &Object) -> Option<(CoinIndexKey, CoinIndexInfo)> {
    use iota_types::coin::{CoinMetadata, TreasuryCap};

    let object_type = object.type_()?;

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

/// Returns `(CoinIndexKey, regulated_coin_metadata_object_id)` if `object` is
/// a `RegulatedCoinMetadata<T>`.  Used to populate the `coin` table.
fn try_create_regulated_coin_info(object: &Object) -> Option<(CoinIndexKey, ObjectId)> {
    let move_object_type = object.type_()?;
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

fn try_create_package_version_info(
    object: &Object,
) -> Option<(PackageVersionKey, PackageVersionInfo)> {
    let package = object.data.as_package_opt()?;
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

// ---------------------------------------------------------------------------
// Live object set indexer
// ---------------------------------------------------------------------------

/// Builds the live-state indexes from a stream of live objects: `init`'s
/// `index_live_object_set` feeds it a parallel scan of the local store, and a
/// formal-snapshot restore feeds it the downloaded partitions.
///
/// Partitions may be indexed concurrently via [`Self::begin_partition`]; call
/// [`Self::finish`] once after all partitions to flush the cross-partition
/// coin aggregation (a restore then ends with
/// [`GrpcIndexesStore::finalize_restore`]).
pub struct GrpcLiveObjectRestorer<'a> {
    tables: &'a IndexStoreTables,
    coin_index: Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
}

impl GrpcLiveObjectRestorer<'_> {
    /// Indexer for one partition's slice of the object stream; feed it every
    /// object of the partition, then call [`GrpcPartitionIndexer::finish`].
    pub fn begin_partition(&self) -> GrpcPartitionIndexer<'_> {
        GrpcPartitionIndexer(self.live_object_indexer())
    }

    fn live_object_indexer(&self) -> GrpcLiveObjectIndexer<'_> {
        GrpcLiveObjectIndexer {
            tables: self.tables,
            batch: self.tables.owner.batch(),
            coin_index: &self.coin_index,
        }
    }

    /// Flush the coin index aggregated across all partitions.
    pub fn finish(&self) -> Result<(), TypedStoreError> {
        let coin_index = std::mem::take(&mut *self.coin_index.lock().unwrap());
        self.tables.coin.multi_insert(coin_index)
    }
}

impl ParMakeLiveObjectIndexer for GrpcLiveObjectRestorer<'_> {
    type ObjectIndexer<'a>
        = GrpcPartitionIndexer<'a>
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        self.begin_partition()
    }
}

/// One partition's indexer within a [`GrpcLiveObjectRestorer`] run.
pub struct GrpcPartitionIndexer<'a>(GrpcLiveObjectIndexer<'a>);

impl GrpcPartitionIndexer<'_> {
    pub fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        self.0.index_object(object)
    }

    /// Write out this partition's staged index batch.
    pub fn finish(self) -> Result<(), StorageError> {
        self.0.finish()
    }
}

impl LiveObjectIndexer for GrpcPartitionIndexer<'_> {
    fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        GrpcPartitionIndexer::index_object(self, object)
    }

    fn finish(self) -> Result<(), StorageError> {
        GrpcPartitionIndexer::finish(self)
    }
}

struct GrpcLiveObjectIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: typed_store::rocks::DBBatch,
    coin_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
}

impl LiveObjectIndexer for GrpcLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                if let Some((owner_key, owner_info)) = make_owner_key(owner, &object) {
                    self.batch
                        .insert_batch(&self.tables.owner, [(owner_key, owner_info)])?;
                }
            }
            // Dynamic Field Index
            Owner::Object(parent) => {
                if should_index_dynamic_field(&object) {
                    let field_key = DynamicFieldKey::new(parent, object.id());
                    self.batch
                        .insert_batch(&self.tables.dynamic_field, [(field_key, ())])?;
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }

        // Look for CoinMetadata<T> and TreasuryCap<T> objects
        if let Some((key, value)) = try_create_coin_index_info(&object) {
            merge_coin_into(&mut self.coin_index.lock().unwrap(), key, value);
        }

        // Package version index
        if let Some((key, info)) = try_create_package_version_info(&object) {
            self.batch
                .insert_batch(&self.tables.package_version, [(key, info)])?;
        }

        // Regulated coin index
        if let Some((key, object_id)) = try_create_regulated_coin_info(&object) {
            merge_coin_into(
                &mut self.coin_index.lock().unwrap(),
                key,
                CoinIndexInfo {
                    regulated_coin_metadata_object_id: Some(object_id),
                    ..Default::default()
                },
            );
        }

        // If the batch size grows to greater that 128MB then write out to the DB so
        // that the data we need to hold in memory doesn't grown unbounded.
        if self.batch.size_in_bytes() >= 1 << 27 {
            std::mem::replace(&mut self.batch, self.tables.owner.batch()).write()?;
        }

        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        self.batch.write()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::GasCostSummary;
    use iota_types::{
        crypto::AuthorityStrongQuorumSignInfo,
        iota_system_state::IotaSystemState,
        message_envelope::Envelope,
        messages_checkpoint::{CheckpointSummary, VerifiedCheckpoint},
    };
    use typed_store::rocks::{MetricConf, ReadWriteOptions, open_cf_opts};

    use super::*;

    /// An executed (non-boundary) checkpoint for seeding a test
    /// `CheckpointStore`, with a placeholder signature and no end-of-epoch
    /// data.
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

    /// The live-object restorer must derive the same live-state indexes from
    /// an external object stream that `init` derives from a store scan: an
    /// address-owned object lands in the `owner` index, and the coin
    /// aggregation only hits the `coin` table on the final cross-partition
    /// `finish`.
    #[tokio::test]
    async fn live_object_restorer_builds_live_state_indexes() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());

        let owner = Address::from_u16(42);
        let object = Object::with_owner_for_testing(owner);
        let object_id = object.id();

        let restorer = grpc.live_object_restorer();
        let mut partition = restorer.begin_partition();
        partition.index_object(object).unwrap();
        partition.finish().unwrap();
        restorer.finish().unwrap();

        let owned: Vec<_> = grpc
            .owner_iter(owner, None, OwnerTypeFilter::None)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(owned.len(), 1, "restored object must be owner-indexed");
        assert_eq!(owned[0].0.object_id, object_id);
    }

    /// `finalize_restore` must leave a store that `GrpcIndexesStore::new`
    /// opens in place: `meta` is current and `Watermark::Indexed` matches the
    /// restore checkpoint, so `needs_to_do_initialization` is false and the
    /// restored contents survive. Without it, the store is wiped and
    /// re-initialized.
    #[tokio::test]
    async fn finalize_restore_makes_initialization_unnecessary() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));

        // The restore's highest executed checkpoint.
        let restore_checkpoint = executed_checkpoint(0, 5);
        checkpoint_store
            .insert_verified_checkpoint(&restore_checkpoint)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&restore_checkpoint)
            .unwrap();

        // Before finalize: no `meta`, so the store would be wiped + re-inited.
        assert!(grpc.tables.needs_to_do_initialization(&checkpoint_store));

        grpc.finalize_restore(5).unwrap();
        assert!(
            !grpc.tables.needs_to_do_initialization(&checkpoint_store),
            "a finalized restore must open in place"
        );

        // A finalize behind the executed watermark still triggers re-init.
        let newer = executed_checkpoint(0, 6);
        checkpoint_store.insert_verified_checkpoint(&newer).unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&newer)
            .unwrap();
        assert!(
            grpc.tables.needs_to_do_initialization(&checkpoint_store),
            "a stale restore watermark must not suppress re-init"
        );
    }

    /// On open, the released `epochs` column family is dropped without
    /// migration and stays absent on reopen. (`epochs_v2` never shipped —
    /// no such CF to drop.)
    #[tokio::test]
    async fn deprecated_epochs_cf_is_dropped_without_migration() {
        let tmp_dir = iota_common::tempdir();
        let db_dir = tmp_dir.path().to_path_buf();

        // Open RocksDB with the released `epochs` CF on disk and write one row.
        {
            let opt_cfs: Vec<(&str, typed_store::rocksdb::Options)> =
                vec![("epochs", typed_store::rocks::default_db_options().options)];
            let db = open_cf_opts(&db_dir, None, MetricConf::default(), &opt_cfs)
                .expect("open DB with the old CF");
            let epochs = DBMap::<EpochId, EpochInfo>::reopen(
                &db,
                Some("epochs"),
                &ReadWriteOptions::default(),
                false,
            )
            .unwrap();
            let old_info = EpochInfo {
                epoch: 7,
                protocol_version: 1,
                start_timestamp_ms: 1_000_000,
                end_timestamp_ms: Some(2_000_000),
                start_checkpoint: 42,
                end_checkpoint: Some(99),
                reference_gas_price: 1_000,
                system_state: IotaSystemState::for_testing(7, 1),
            };
            epochs.insert(&old_info.epoch, &old_info).unwrap();
        }

        // Open via the current schema: the deprecated CF must be dropped.
        let tables = IndexStoreTables::open(db_dir.clone());
        drop(tables);

        let listed = typed_store::rocks::list_tables(db_dir.clone()).unwrap();
        assert!(
            !listed.contains(&"epochs".to_string()),
            "the deprecated epochs CF should have been dropped; saw: {listed:?}"
        );

        // Reopening must not panic.
        let _tables = IndexStoreTables::open(db_dir);
    }
}
