// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    hash::Hasher,
    ops::Bound,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use iota_sdk_types::{
    Address, ObjectId, Owner, StructTag, TransactionDigest, TypeTag, Version,
    checkpoint::CheckpointContents,
};
use iota_types::{
    committee::EpochId,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CheckpointContentsExt, CheckpointSequenceNumber, VerifiedCheckpoint},
    move_package::MovePackageExt,
    object::Object,
    storage::{
        AccountOwnedObjectInfo, DynamicFieldKey, OwnedObjectCursor, OwnedObjectIteratorItem,
        PackageVersionInfo, PackageVersionIteratorItem, PackageVersionKey, TransactionInfo,
        error::{Error as StorageError, Kind as StorageErrorKind},
    },
};
use prometheus_filtered::{IntGauge, MetricLevel, Registry, register_int_gauge_with_registry};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use typed_store::{
    DBMapUtils, TypedStoreError,
    database::{Database, drop_tolerant_write_options, wait_for_database_close},
    rocks::{
        DBMap, DBMapTableConfigMap, MetricConf, ReadWriteOptions, bulk_ingestion_options,
        bulk_ingestion_write_options, default_db_options, list_tables, open_cf_opts,
        read_size_from_env, safe_drop_db,
    },
    rocksdb,
    traits::Map,
};

use crate::{
    authority::AuthorityStore,
    checkpoints::CheckpointStore,
    index_rebuild_cancellation::{RebuildCancelled, is_cancelled},
    par_index_live_object_set::{
        LiveObjectIndexer, PROGRESS_REPORT_INTERVAL, ParMakeLiveObjectIndexer, eta_display,
        progress_rate,
    },
    rpc_index_history::{self, EpochBuckets},
};

/// Bump this when changing the serialization format of an existing table.
/// A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`.
const CURRENT_DB_VERSION: u64 = 2;

/// On-disk directory name for the gRPC indexes store.
pub const GRPC_INDEXES_DIR: &str = "grpc_indexes";

const ENV_VAR_HISTORY_BLOCK_CACHE_SIZE_MB: &str = "GRPC_HISTORY_BLOCK_CACHE_MB";
const DEFAULT_HISTORY_BLOCK_CACHE_SIZE_MB: usize = 512;

/// Prefix of the per-epoch history column families; a bucket's family is
/// `"{prefix}{epoch}"`. On-disk names are the ground truth for which buckets
/// exist.
const HISTORY_CF_PREFIX: &str = "hist_e";

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
    pub version: Version,
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

fn default_table_options() -> typed_store::rocks::DBOptions {
    typed_store::rocks::default_db_options().disable_write_throttling()
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
    #[default_options_override_fn = "default_table_options"]
    watermark: DBMap<Watermark, CheckpointSequenceNumber>,

    /// Lowest checkpoint whose transaction digests are indexed; the
    /// background replay works downwards from it. Absent when the history
    /// is complete.
    #[default_options_override_fn = "default_table_options"]
    history_watermark: DBMap<(), CheckpointSequenceNumber>,

    /// Earliest epoch retained by the last pruning pass; buckets below it
    /// are never recreated and the backfill stops at it.
    #[default_options_override_fn = "default_table_options"]
    earliest_retained_epoch: DBMap<(), EpochId>,

    /// An index of object ownership.
    ///
    /// Uses fixed-size u64 hash keys for correct RocksDB byte-order iteration.
    /// Allows an efficient iterator to list all objects currently owned by a
    /// specific user account, optionally filtered by type.
    ///
    /// Full `StructTag` stored in value for collision filtering & API
    /// responses. Bounded by the live object set (one entry per
    /// address-owned object).
    #[default_options_override_fn = "default_table_options"]
    owner: DBMap<OwnerIndexKey, OwnerIndexInfo>,

    /// An index of dynamic fields (children objects).
    ///
    /// Allows an efficient iterator to list all of the dynamic fields owned by
    /// a particular ObjectId. Only the key is stored; field metadata is loaded
    /// on demand from the object store.
    #[default_options_override_fn = "default_table_options"]
    dynamic_field: DBMap<DynamicFieldKey, ()>,

    /// Coin info with regulated coin metadata.
    /// Bounded by the live object set (one entry per coin type).
    #[default_options_override_fn = "default_table_options"]
    coin: DBMap<CoinIndexKey, CoinIndexInfo>,

    /// An index of Package versions.
    ///
    /// Maps original package ID and version to the storage ID of that version.
    /// Allows efficient listing of all versions of a package, including
    /// upgraded user packages that have different storage IDs.
    /// Bounded by the live object set (one entry per package version).
    #[default_options_override_fn = "default_table_options"]
    package_version: DBMap<PackageVersionKey, PackageVersionInfo>,
    // NOTE: Authors and Reviewers before adding any new tables ensure that they are either:
    // - bounded in size by the live object set
    // - are prune-able and have corresponding logic in the `prune` function
}

impl IndexStoreTables {
    fn open_with_options<P: Into<PathBuf>>(
        path: P,
        options: typed_store::rocksdb::Options,
        table_options: Option<DBMapTableConfigMap>,
    ) -> Self {
        IndexStoreTables::open_tables_read_write(
            path.into(),
            MetricConf::new("grpc-index"),
            Some(options),
            table_options,
        )
    }

    /// Whether the store must be wiped and rebuilt. Read errors propagate:
    /// a transient error must fail the open rather than silently wipe a
    /// healthy store or adopt a stale one.
    fn needs_to_do_initialization(
        &self,
        checkpoint_store: &CheckpointStore,
    ) -> Result<bool, StorageError> {
        // Schema mismatch -> migration may be pending and the watermark CF
        // may be from an incompatible schema.
        let schema_mismatch = match self.meta.get(&()).map_err(StorageError::from)? {
            Some(metadata) => metadata.version != CURRENT_DB_VERSION,
            None => true,
        };

        Ok(schema_mismatch || self.is_indexed_watermark_out_of_date(checkpoint_store)?)
    }

    // Check if the index watermark is behind the highest_executed_checkpoint.
    fn is_indexed_watermark_out_of_date(
        &self,
        checkpoint_store: &CheckpointStore,
    ) -> Result<bool, StorageError> {
        let highest_executed_checkpoint = checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .map_err(|e| StorageError::custom(e.to_string()))?;
        let watermark = self
            .watermark
            .get(&Watermark::Indexed)
            .map_err(StorageError::from)?;
        Ok(watermark < highest_executed_checkpoint)
    }

    /// See [`GrpcIndexesStore::live_object_restorer`].
    fn live_object_restorer(&self, batch_size_limit: usize) -> GrpcLiveObjectRestorer<'_> {
        GrpcLiveObjectRestorer {
            tables: self,
            coin_index: Mutex::new(HashMap::new()),
            batch_size_limit,
        }
    }

    /// Phase 2 of `init`: rebuild the live-state indexes by scanning the
    /// current live object set in parallel. Must re-run on any drift to keep
    /// them consistent.
    fn index_live_object_set(
        &self,
        authority_store: &AuthorityStore,
        batch_size_limit: usize,
        cancelled: &AtomicBool,
    ) -> Result<(), StorageError> {
        let restorer = self.live_object_restorer(batch_size_limit);
        crate::par_index_live_object_set::par_index_live_object_set(
            authority_store,
            &restorer,
            cancelled,
        )?;
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
        batch_size_limit: usize,
        cancelled: &AtomicBool,
    ) -> Result<(), StorageError> {
        info!("Initializing gRPC indexes");

        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;

        // Live-state indexes from the current live object set. The digest
        // history is not built here: `backfill_history` fills it in the
        // background once the node is up, resuming from `history_watermark`.
        self.index_live_object_set(authority_store, batch_size_limit, cancelled)?;

        self.finalize(highest_executed_checkpoint)?;

        info!("Finished initializing gRPC indexes");

        Ok(())
    }

    /// Flushes the bulk-ingested data, then stamps the watermarks and `meta`
    /// last, so a crash in between leaves a store the next open re-inits.
    /// `indexed_checkpoint` is the highest checkpoint the build covers; the
    /// background replay later fills the digest history at and below it,
    /// working downwards from the marker seeded here.
    fn finalize(
        &self,
        indexed_checkpoint: Option<CheckpointSequenceNumber>,
    ) -> Result<(), TypedStoreError> {
        // The watermarks and `meta` are WAL-durable and the bulk writes are not,
        // so flush first; flushing one table flushes every column family.
        self.meta.flush_all()?;
        self.history_watermark
            .insert(&(), &indexed_checkpoint.map_or(0, |c| c.saturating_add(1)))?;
        self.watermark
            .insert(&Watermark::Indexed, &indexed_checkpoint.unwrap_or(0))?;
        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )
    }

    /// Index a Checkpoint. `bucket` is the digest history bucket of the
    /// checkpoint's epoch; the batch spans it and the static tables, which
    /// share one database.
    fn index_checkpoint(
        &self,
        bucket: &TransactionCheckpointsBucket,
        checkpoint: &CheckpointData,
    ) -> Result<typed_store::rocks::DBBatch, StorageError> {
        debug!(
            checkpoint = checkpoint.checkpoint_summary.sequence_number,
            "indexing checkpoint"
        );

        let mut batch = self.meta.batch();

        Self::index_transactions(
            bucket,
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
        bucket: &TransactionCheckpointsBucket,
        checkpoint_seq_number: CheckpointSequenceNumber,
        contents: &CheckpointContents,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        batch.insert_batch(
            bucket,
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
        let iter = self
            .dynamic_field
            .safe_iter_with_prefix_from(&parent, Bound::Included(&cursor.unwrap_or(ObjectId::ZERO)))
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
        Ok(self.package_version.safe_iter_with_prefix_from(
            &original_package_id,
            Bound::Included(&cursor.unwrap_or(0)),
        ))
    }
}

/// One epoch's transaction-digest history: the digests of the checkpoints
/// executed in that epoch, mapped to their checkpoint.
type TransactionCheckpointsBucket = DBMap<TransactionDigest, CheckpointSequenceNumber>;

/// Builds one bucket's view from its column-family name. Per-epoch column
/// families skip the periodic metrics reporter task: with up to ~100
/// retained epochs, one task per column family adds up.
fn reopen_transaction_checkpoints_bucket(
    db: &Arc<Database>,
    cf_name: &str,
) -> Result<TransactionCheckpointsBucket, TypedStoreError> {
    DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)
}

struct GrpcIndexesMetrics {
    /// Lowest checkpoint the digest history backfill has replayed so far.
    /// The value reflects only the backfill's own progress: it keeps its
    /// final value after the backfill stops and is not raised when pruning
    /// later drops replayed epochs.
    history_backfill_lowest_replayed_checkpoint: IntGauge,
    /// 1 while the background digest history backfill is running, 0
    /// otherwise.
    history_backfill_running: IntGauge,
}

impl GrpcIndexesMetrics {
    fn new(registry: &Registry) -> Self {
        Self {
            // How far the backfill got is visible nowhere else, so keep it
            // above the default metric filter.
            history_backfill_lowest_replayed_checkpoint: register_int_gauge_with_registry!(
                "grpc_index_history_backfill_lowest_replayed_checkpoint",
                "Lowest checkpoint the gRPC digest history backfill has replayed, keeping its \
                 final value after the backfill stops; unaffected by later pruning",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
            history_backfill_running: register_int_gauge_with_registry!(
                "grpc_index_history_backfill_running",
                "1 while the gRPC digest history backfill is running, 0 otherwise",
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
    history: BTreeMap<EpochId, Arc<TransactionCheckpointsBucket>>,
}

pub struct GrpcIndexesStore {
    tables: Arc<IndexStoreTables>,
    /// The per-epoch transaction-digest history buckets.
    history: EpochBuckets<TransactionCheckpointsBucket>,
    pending_updates: Mutex<BTreeMap<u64, typed_store::rocks::DBBatch>>,
    metrics: GrpcIndexesMetrics,
    /// Stops the startup rebuild and the background history backfill.
    cancelled: Arc<AtomicBool>,
    /// How many epochs of checkpoints the pruner is configured to retain
    /// (`num_epochs_to_retain_for_checkpoints`); bounds the history backfill
    /// so it does not replay epochs the next prune pass would drop again.
    /// `None` when checkpoint pruning is off.
    epochs_to_retain: Option<u64>,
    history_backfill_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl GrpcIndexesStore {
    /// Opens the index database, passing every existing per-epoch history
    /// column family at open with its tuned options: a column family left
    /// for auto-discovery would silently get default options (and its own
    /// block cache).
    fn open_index_db(path: &Path) -> Result<OpenedIndexDb, TypedStoreError> {
        let db_options = default_table_options();
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
            list_tables(path.to_path_buf()).map_err(|e| TypedStoreError::RocksDB(e.to_string()))?
        } else {
            Vec::new()
        };
        let mut epochs = std::collections::BTreeSet::new();
        let mut opt_cfs: Vec<(String, rocksdb::Options)> = Vec::new();
        for name in static_tables.keys() {
            let options = if name == "meta" {
                default_db_options().options
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
            if let Some(epoch) = rpc_index_history::bucket_cf_epoch(HISTORY_CF_PREFIX, cf_name) {
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
            MetricConf::new("grpc-index"),
            &opt_cfs,
        )?;

        fn map<K, V>(
            db: &Arc<Database>,
            cf_name: &str,
            rw: &ReadWriteOptions,
        ) -> Result<DBMap<K, V>, TypedStoreError> {
            DBMap::reopen(db, Some(cf_name), rw, false)
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
            let bucket = reopen_transaction_checkpoints_bucket(
                &db,
                &rpc_index_history::bucket_cf_name(HISTORY_CF_PREFIX, epoch),
            )?;
            history.insert(epoch, Arc::new(bucket));
        }

        Ok(OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        })
    }

    /// Assembles the store from an opened database, applying the retention
    /// floor to the discovered buckets.
    fn from_opened(
        opened: OpenedIndexDb,
        registry: &Registry,
        epochs_to_retain: Option<u64>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, TypedStoreError> {
        let OpenedIndexDb {
            tables,
            db,
            history_cf_options,
            history,
        } = opened;
        let history = EpochBuckets::open(
            db,
            "gRPC index history",
            HISTORY_CF_PREFIX,
            history_cf_options,
            tables.earliest_retained_epoch.clone(),
            history,
            reopen_transaction_checkpoints_bucket,
        )?;
        Ok(Self {
            tables: Arc::new(tables),
            history,
            pending_updates: Default::default(),
            metrics: GrpcIndexesMetrics::new(registry),
            cancelled,
            epochs_to_retain,
            history_backfill_task: Default::default(),
        })
    }

    /// Opens the store, wiping it and rebuilding the live-state tables
    /// first when the indexes are missing or stale. The digest history is
    /// filled by a background replay after this returns; until it finishes,
    /// lookups cover a growing range of recent checkpoints. When checkpoint
    /// pruning is configured, `num_epochs_to_retain` bounds the replay to
    /// the epochs the pruner would retain.
    ///
    /// Setting `cancelled` abandons a rebuild running here and the
    /// background replay, and fails the open: the store is left unfinalized
    /// for the next open to rebuild, and must not serve reads in the
    /// meantime.
    pub async fn new(
        path: PathBuf,
        registry: &Registry,
        num_epochs_to_retain: Option<u64>,
        authority_store: Arc<AuthorityStore>,
        checkpoint_store: &Arc<CheckpointStore>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Arc<Self>, StorageError> {
        let opened = {
            // An unopenable database would crash-loop the node with no way
            // to self-heal; wipe and rebuild it like a stale one — but only
            // after one retry, so a transient error does not destroy a
            // healthy store.
            let mut opened = match Self::open_index_db(&path) {
                Ok(opened) => Some(opened),
                Err(first) => {
                    warn!("unable to open the gRPC index database, retrying once: {first}");
                    match Self::open_index_db(&path) {
                        Ok(opened) => Some(opened),
                        Err(e) => {
                            warn!(
                                "unable to open the gRPC index database, wiping and rebuilding: {e}"
                            );
                            None
                        }
                    }
                }
            };

            // If the index tables are uninitialized or on an older version then we need to
            // populate them
            if opened.as_ref().is_none_or(|opened| {
                opened
                    .tables
                    .needs_to_do_initialization(checkpoint_store)
                    .expect("failed to determine whether the gRPC index needs a rebuild")
            }) {
                let batch_size_limit;
                let tables = {
                    drop(opened.take());
                    // `DB::destroy` fails on a database it cannot parse —
                    // the very state the rebuild recovers from — so fall
                    // back to deleting the directory.
                    if let Err(e) = safe_drop_db(path.clone(), Duration::from_secs(30)).await {
                        warn!("unable to destroy the old gRPC index database ({e}), deleting it");
                        std::fs::remove_dir_all(&path)
                            .expect("unable to delete the old gRPC index database");
                    }

                    // Open the empty DB with tuned bulk ingestion options to
                    // speed up the initial indexing. The DB is reopened with default options
                    // afterwards.
                    let bulk_options = bulk_ingestion_options();
                    batch_size_limit = bulk_options.batch_size_limit;
                    let table_config =
                        bulk_options.table_config(IndexStoreTables::describe_tables().into_keys());

                    IndexStoreTables::open_with_options(
                        &path,
                        bulk_options.db_options,
                        Some(table_config),
                    )
                };

                // The rebuild scans and writes RocksDB for a long time; keep
                // it off the async runtime's worker threads.
                let (tables, initialized) = tokio::task::spawn_blocking({
                    let authority_store = authority_store.clone();
                    let checkpoint_store = checkpoint_store.clone();
                    let cancelled = cancelled.clone();
                    move || {
                        let mut tables = tables;
                        let initialized = tables.init(
                            &authority_store,
                            &checkpoint_store,
                            batch_size_limit,
                            &cancelled,
                        );
                        (tables, initialized)
                    }
                })
                .await
                .expect("gRPC index initialization task failed");

                match initialized {
                    Ok(()) => {}
                    // Unfinalized, so the next open rebuilds it, as after a
                    // crash. The open fails so the truncated store is never
                    // served, and the reopen below never runs on a store
                    // whose `meta` the skipped finalize never wrote.
                    // Keyed on the error, not on the flag: a real failure
                    // that races the shutdown must stay a failure.
                    Err(e) if is_cancelled(&e) => {
                        // Release the database so the next open can rebuild
                        // it.
                        let weak_db = Arc::downgrade(&tables.meta.db);
                        drop(tables);
                        if !wait_for_database_close(weak_db).await {
                            warn!("the cancelled gRPC index rebuild left its database open");
                        }
                        return Err(RebuildCancelled::error(format!(
                            "the gRPC index rebuild was cancelled by shutdown: {e}"
                        )));
                    }
                    Err(e) => panic!("unable to initialize gRPC index: {e}"),
                }

                let weak_db = Arc::downgrade(&tables.meta.db);
                drop(tables);
                if !wait_for_database_close(weak_db).await {
                    panic!("unable to reopen DB after indexing");
                }

                // Reopen the DB with default options (eg without `unordered_write`s enabled)
                let reopened = Self::open_index_db(&path)
                    .expect("unable to reopen the gRPC index database after the rebuild");

                // Sanity check: verify the database version was persisted correctly, i.e.
                // the WAL-disabled bulk writes were flushed before the reopen.
                let stored_version = reopened
                    .tables
                    .meta
                    .get(&())
                    .expect("reopened gRPC index DB should expose readable metadata")
                    .expect("metadata should have been written before flush and reopen");
                assert_eq!(
                    stored_version.version, CURRENT_DB_VERSION,
                    "database version mismatch after flush and reopen: expected {}, found {}",
                    CURRENT_DB_VERSION, stored_version.version
                );

                reopened
            } else {
                opened.expect("the index database is open unless it needs a rebuild")
            }
        };

        let store = Arc::new(Self::from_opened(
            opened,
            registry,
            num_epochs_to_retain,
            cancelled,
        )?);
        store.spawn_history_backfill(checkpoint_store.clone());
        Ok(store)
    }

    /// Open the store without the wipe/init logic of [`Self::new`] — for the
    /// restore tool, which populates and finalizes the store itself.
    pub fn new_without_init(path: PathBuf) -> Self {
        Self::open_index_db(&path)
            .and_then(|opened| {
                Self::from_opened(opened, &Registry::default(), None, Arc::default())
            })
            .expect("unable to open the gRPC index database")
    }

    /// Starts the background replay that fills the digest history below the
    /// watermark, if any is pending.
    fn spawn_history_backfill(self: &Arc<Self>, checkpoint_store: Arc<CheckpointStore>) {
        let store = self.clone();
        let task = tokio::task::spawn_blocking(move || {
            store.metrics.history_backfill_running.set(1);
            if let Err(e) = store.backfill_history(&checkpoint_store) {
                warn!("the gRPC digest history backfill stopped: {e}");
            }
            store.metrics.history_backfill_running.set(0);
        });
        *self.history_backfill_task.lock().unwrap() = Some(task);
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
            warn!("the gRPC digest history backfill task failed: {e}");
        }
    }

    /// Awaits the backfill task, if one is still running.
    async fn join_backfill_task(&self) -> Result<(), tokio::task::JoinError> {
        let task = self.history_backfill_task.lock().unwrap().take();
        match task {
            Some(task) => task.await,
            None => Ok(()),
        }
    }

    /// Fills the digest history for the checkpoints below
    /// `history_watermark`, newest first, until it reaches the
    /// checkpoint-contents pruner, an epoch [`Self::prune`] removed from the
    /// index, or the checkpoint retention. The marker commits atomically
    /// with each checkpoint's digests, so an interrupted run resumes where
    /// it stopped. No-op when the marker is absent (the history was indexed
    /// continuously and is complete). Reports its progress through the
    /// `grpc_index_history_backfill_lowest_replayed_checkpoint` gauge; where
    /// it stopped and why is in the log.
    #[tracing::instrument(skip_all)]
    fn backfill_history(&self, checkpoint_store: &CheckpointStore) -> Result<(), StorageError> {
        let Some(watermark) = self.tables.history_watermark.get(&())? else {
            return Ok(());
        };
        let Some(mut next) = watermark.checked_sub(1) else {
            return Ok(());
        };

        info!("Backfilling the gRPC digest history from checkpoint {next} downwards");
        self.metrics
            .history_backfill_lowest_replayed_checkpoint
            .set(watermark as i64);
        let start_time = Instant::now();
        let mut last_report = Instant::now();
        let mut replayed: u64 = 0;
        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                info!("Stopping the gRPC digest history backfill at checkpoint {next}: shutdown");
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
                    "Stopping the gRPC digest history backfill at checkpoint {next}: epoch {} \
                     was pruned from the index, only epochs from {earliest_retained} on are \
                     retained",
                    summary.epoch
                );
                break;
            }
            if let Some(horizon) = self.backfill_retention_horizon(summary.epoch) {
                if summary.epoch < horizon {
                    info!(
                        "Stopping the gRPC digest history backfill at checkpoint {next}: epoch \
                         {} is past the checkpoint retention, the next pruning pass would drop \
                         it again",
                        summary.epoch
                    );
                    break;
                }
            }
            if let Err(e) = self.replay_checkpoint_history(checkpoint_store, &summary) {
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
                        "Stopping the gRPC digest history backfill at checkpoint {next}: its \
                         data is already gone ({e})"
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
                    "Backfilling the gRPC digest history: {:.1}% done (checkpoint {next} down \
                     to {lowest}), {rate:.0} checkpoints/s, ETA ~{eta}",
                    fraction * 100.0,
                );
            }
            let Some(n) = next.checked_sub(1) else {
                break;
            };
            next = n;
        }

        info!(
            "Backfilling {replayed} checkpoints of gRPC digest history took {} seconds",
            start_time.elapsed().as_secs()
        );
        Ok(())
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
                "Stopping the gRPC digest history backfill at checkpoint {next}: it was pruned \
                 mid-replay"
            );
            return Ok(true);
        }
        let earliest_retained = self.history.earliest_retained();
        if let Some(epoch) = epoch.filter(|&epoch| epoch < earliest_retained) {
            info!(
                "Stopping the gRPC digest history backfill at checkpoint {next}: epoch {epoch} \
                 was pruned from the index mid-replay, only epochs from {earliest_retained} on \
                 are retained"
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// The lowest epoch the backfill may replay when checkpoint pruning is
    /// configured: the horizon [`Self::prune`] enforces, computed against
    /// the newest bucket. The `earliest_retained_epoch` floor alone is not
    /// enough — it is written by the first pruning pass, and until then a
    /// rebuilt store's backfill would replay epochs that pass drops again.
    /// `None` when checkpoint pruning is off.
    ///
    /// `current_epoch` stands in for the newest epoch while no bucket
    /// exists yet, on a rebuilt store whose backfill has not committed its
    /// first checkpoint.
    fn backfill_retention_horizon(&self, current_epoch: EpochId) -> Option<EpochId> {
        let epochs_to_retain = self.epochs_to_retain?;
        let newest = self.history.newest_epoch().unwrap_or(current_epoch);
        Some(newest.saturating_sub(epochs_to_retain.saturating_sub(1)))
    }

    /// Replays one checkpoint's digests into its epoch's history bucket and
    /// lowers `history_watermark` to it, in one atomic batch.
    fn replay_checkpoint_history(
        &self,
        checkpoint_store: &CheckpointStore,
        summary: &VerifiedCheckpoint,
    ) -> Result<(), StorageError> {
        let checkpoint_seq = summary.sequence_number;
        let contents = checkpoint_store
            .get_checkpoint_contents(&summary.contents_digest)?
            .ok_or_else(|| {
                StorageError::missing(format!("missing checkpoint contents {checkpoint_seq}"))
            })?;
        let bucket = self
            .history
            .ensure(summary.epoch)
            .map_err(|e| StorageError::custom(e.to_string()))?;

        let mut batch = self.tables.history_watermark.batch();
        IndexStoreTables::index_transactions(&bucket, checkpoint_seq, &contents, &mut batch)?;
        batch.insert_batch(&self.tables.history_watermark, [((), checkpoint_seq)])?;
        // A plain WAL-enabled write: the database is serving lookups, and
        // the marker must land atomically with the digests.
        // `drop_tolerant_write_options` discards the bucket's rows if
        // `prune` dropped its column family mid-replay; the next loop
        // iteration then stops at the pruned epoch.
        batch
            .write_opt(&drop_tolerant_write_options())
            .map_err(StorageError::from)?;
        Ok(())
    }

    /// Drops the digest history of expired epochs, see
    /// [`EpochBuckets::prune`]: with `epochs_to_retain` = N, the buckets of
    /// the newest N epochs are kept and every older bucket is dropped
    /// wholesale. Returns the earliest epoch to retain, `None` when there
    /// is no history at all.
    ///
    /// A lookup racing a drop may report an error for the dropped epoch's
    /// digests; a retry no longer sees the bucket. Lookups block for the
    /// duration of the drops, so callers on an async runtime must use
    /// `spawn_blocking`.
    pub fn prune(&self, epochs_to_retain: u64) -> Result<Option<EpochId>, TypedStoreError> {
        self.history.prune(epochs_to_retain)
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
        let bucket = self
            .history
            .ensure(checkpoint.checkpoint_summary.epoch)
            .expect("db error");
        let batch = self
            .tables
            .index_checkpoint(&bucket, checkpoint)
            .expect("db error");

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

        // The update may stage rows of a history bucket `prune` drops before
        // this write; those rows are discarded instead of failing the write.
        // Only expired epochs can be lost that way: `index_checkpoint`
        // created the bucket of the epoch being executed, so it is the
        // newest one.
        Ok(batch.write_opt(&drop_tolerant_write_options())?)
    }

    /// The checkpoint containing `digest`, from the digest history buckets.
    ///
    /// An exact-key probe over the buckets, newest first; a miss in a sealed
    /// bucket is answered by its in-memory bloom filters. Digests of
    /// checkpoints pruned mid-epoch stay answerable until the whole epoch's
    /// bucket drops.
    pub fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        for bucket in self.history.iter(true) {
            if let Some(checkpoint) = bucket.get(digest)? {
                return Ok(Some(TransactionInfo {
                    checkpoint,
                    object_types: Default::default(),
                }));
            }
        }
        Ok(None)
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
    pub fn live_object_restorer(&self, batch_size_limit: usize) -> GrpcLiveObjectRestorer<'_> {
        self.tables.live_object_restorer(batch_size_limit)
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
        self.tables.finalize(Some(restore_checkpoint))
    }

    /// Finalizes the restore as [`Self::finalize_restore`] does, then closes
    /// the store and reopens it the way a node does, so a database the node
    /// would wipe and rebuild — or one that carries no restored objects —
    /// fails the restore instead, and so the caller can move the database
    /// directory. `live_object_count` is the number of objects the restore
    /// wrote.
    pub async fn finalize_and_verify_restore(
        self: Arc<Self>,
        path: &Path,
        restore_checkpoint: CheckpointSequenceNumber,
        live_object_count: u64,
    ) -> Result<(), StorageError> {
        self.finalize_restore(restore_checkpoint)?;

        let weak_db = Arc::downgrade(&self.tables.meta.db);
        drop(self);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the gRPC index database after the restore",
            ));
        }

        let reopened = Self::open_index_db(path).map_err(|e| {
            StorageError::custom(format!(
                "unable to reopen the restored gRPC index database: {e}"
            ))
        })?;
        let stored_version = reopened
            .tables
            .meta
            .get(&())?
            .ok_or_else(|| {
                StorageError::custom("the restored gRPC index database has no metadata")
            })?
            .version;
        if stored_version != CURRENT_DB_VERSION {
            return Err(StorageError::custom(format!(
                "restored gRPC index database version mismatch: expected {CURRENT_DB_VERSION}, \
                 found {stored_version}"
            )));
        }
        let watermark = reopened.tables.watermark.get(&Watermark::Indexed)?;
        if watermark != Some(restore_checkpoint) {
            return Err(StorageError::custom(format!(
                "the restored gRPC index is watermarked at {watermark:?}, expected \
                 {restore_checkpoint}"
            )));
        }
        // The version and the watermark are written by the finalize itself;
        // only the live state proves the object stream landed. `is_empty`
        // has no error channel and reads an unreadable index as non-empty,
        // so the scan is run here and its failure fails the restore.
        let owner_is_empty = reopened
            .tables
            .owner
            .safe_iter()
            .next()
            .transpose()?
            .is_none();
        if live_object_count > 0 && owner_is_empty {
            return Err(StorageError::custom(format!(
                "the restored gRPC index has an empty owner index after {live_object_count} live \
                 objects"
            )));
        }

        let weak_db = Arc::downgrade(&reopened.tables.meta.db);
        drop(reopened);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the gRPC index database after verifying the restore",
            ));
        }
        Ok(())
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
        GrpcIndexesStore::get_transaction_info(self, digest)
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
                        object_type: info.object_type.into(),
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
        .as_opt_struct()
        .is_some_and(|move_object| move_object.struct_tag().is_dynamic_field())
}

fn try_create_coin_index_info(object: &Object) -> Option<(CoinIndexKey, CoinIndexInfo)> {
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

/// Returns `(CoinIndexKey, regulated_coin_metadata_object_id)` if `object` is
/// a `RegulatedCoinMetadata<T>`.  Used to populate the `coin` table.
fn try_create_regulated_coin_info(object: &Object) -> Option<(CoinIndexKey, ObjectId)> {
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

fn try_create_package_version_info(
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
    batch_size_limit: usize,
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
            batch_size_limit: self.batch_size_limit,
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
    pub fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        self.0.index_object(object)
    }

    /// Write out this partition's staged index batch.
    pub fn finish(self) -> Result<(), StorageError> {
        self.0.finish()
    }
}

impl LiveObjectIndexer for GrpcPartitionIndexer<'_> {
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
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
    batch_size_limit: usize,
}

impl LiveObjectIndexer for GrpcLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                if let Some((owner_key, owner_info)) = make_owner_key(owner, object) {
                    self.batch
                        .insert_batch(&self.tables.owner, [(owner_key, owner_info)])?;
                }
            }
            // Dynamic Field Index
            Owner::Object(parent) => {
                if should_index_dynamic_field(object) {
                    let field_key = DynamicFieldKey::new(parent, object.id());
                    self.batch
                        .insert_batch(&self.tables.dynamic_field, [(field_key, ())])?;
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }

        // Look for CoinMetadata<T> and TreasuryCap<T> objects
        if let Some((key, value)) = try_create_coin_index_info(object) {
            merge_coin_into(&mut self.coin_index.lock().unwrap(), key, value);
        }

        // Package version index
        if let Some((key, info)) = try_create_package_version_info(object) {
            self.batch
                .insert_batch(&self.tables.package_version, [(key, info)])?;
        }

        // Regulated coin index
        if let Some((key, object_id)) = try_create_regulated_coin_info(object) {
            merge_coin_into(
                &mut self.coin_index.lock().unwrap(),
                key,
                CoinIndexInfo {
                    regulated_coin_metadata_object_id: Some(object_id),
                    ..Default::default()
                },
            );
        }

        // If the batch size grows beyond the limit then write out to the DB so
        // that the data we need to hold in memory doesn't grow unbounded.
        if self.batch.size_in_bytes() >= self.batch_size_limit {
            std::mem::replace(&mut self.batch, self.tables.owner.batch())
                .write_opt(&bulk_ingestion_write_options())?;
        }

        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        self.batch.write_opt(&bulk_ingestion_write_options())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::executed_checkpoint;

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

        let restorer = grpc.live_object_restorer(100);
        let mut partition = restorer.begin_partition();
        partition.index_object(&object).unwrap();
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

    /// `finalize` must make the bulk-ingested data durable before it stamps
    /// the watermark and `meta`, so a crash in between cannot leave a store
    /// the next open adopts as complete.
    #[tokio::test]
    async fn finalize_flushes_before_stamping_the_watermark() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());

        let restorer = grpc.live_object_restorer(100);
        let mut partition = restorer.begin_partition();
        partition
            .index_object(&Object::with_owner_for_testing(Address::from_u16(42)))
            .unwrap();
        partition.finish().unwrap();
        restorer.finish().unwrap();

        assert!(
            grpc.tables.meta.db.live_files().unwrap().is_empty(),
            "the restored rows must still be unflushed before finalize"
        );

        grpc.finalize_restore(5).unwrap();

        assert!(
            !grpc.tables.meta.db.live_files().unwrap().is_empty(),
            "finalize must flush the restored rows before stamping the watermark"
        );
    }

    /// A database that cannot be opened is wiped and rebuilt, instead of
    /// crash-looping the node with no way to self-heal.
    #[tokio::test]
    async fn unopenable_database_is_wiped_and_rebuilt() {
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

        let owner = Address::from_u16(42);
        let object = Object::with_owner_for_testing(owner);
        authority_state.insert_genesis_objects(std::slice::from_ref(&object));

        let tmp_dir = iota_common::tempdir();
        std::fs::write(tmp_dir.path().join("CURRENT"), b"bogus").unwrap();

        let grpc = GrpcIndexesStore::new(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            None,
            authority_state.database_for_testing(),
            checkpoint_store,
            Arc::default(),
        )
        .await
        .unwrap();

        let owned: Vec<_> = grpc
            .owner_iter(owner, None, OwnerTypeFilter::None)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(owned.len(), 1, "the rebuild must repopulate the live state");
    }

    /// A cancelled rebuild fails the open instead of reopening a store the
    /// skipped finalize never stamped, and the next open rebuilds it.
    #[tokio::test]
    async fn cancelled_rebuild_fails_the_open() {
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

        let owner = Address::from_u16(42);
        let object = Object::with_owner_for_testing(owner);
        authority_state.insert_genesis_objects(std::slice::from_ref(&object));

        let tmp_dir = iota_common::tempdir();
        let opened = GrpcIndexesStore::new(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            None,
            authority_state.database_for_testing(),
            checkpoint_store,
            Arc::new(AtomicBool::new(true)),
        )
        .await;
        let Err(error) = opened else {
            panic!("a cancelled rebuild must not return a usable store");
        };
        assert!(
            error.to_string().contains("cancelled by shutdown"),
            "unexpected error: {error}"
        );
        assert!(
            is_cancelled(&error),
            "the node's exit path must still recognize the rewrapped cancellation"
        );

        let grpc = GrpcIndexesStore::new(
            tmp_dir.path().to_path_buf(),
            &Registry::default(),
            None,
            authority_state.database_for_testing(),
            checkpoint_store,
            Arc::default(),
        )
        .await
        .expect("the next open must rebuild the store the cancelled one left behind");
        let owned: Vec<_> = grpc
            .owner_iter(owner, None, OwnerTypeFilter::None)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(owned.len(), 1, "the rebuild must repopulate the live state");
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
        assert!(
            grpc.tables
                .needs_to_do_initialization(&checkpoint_store)
                .unwrap()
        );

        grpc.finalize_restore(5).unwrap();
        assert!(
            !grpc
                .tables
                .needs_to_do_initialization(&checkpoint_store)
                .unwrap(),
            "a finalized restore must open in place"
        );
        assert_eq!(
            grpc.tables.history_watermark.get(&()).unwrap(),
            Some(6),
            "the restore leaves no local history below the restore checkpoint, so the replay \
             marker sits one past it"
        );

        // A finalize behind the executed watermark still triggers re-init.
        let newer = executed_checkpoint(0, 6);
        checkpoint_store.insert_verified_checkpoint(&newer).unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&newer)
            .unwrap();
        assert!(
            grpc.tables
                .needs_to_do_initialization(&checkpoint_store)
                .unwrap(),
            "a stale restore watermark must not suppress re-init"
        );
    }

    /// The restore's finalize must leave a closed, readable store: the
    /// verify's own reopen and this one both need every handle released.
    #[tokio::test]
    async fn finalize_and_verify_restore_closes_the_store() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path().to_path_buf();
        let grpc = Arc::new(GrpcIndexesStore::new_without_init(path.clone()));

        let restorer = grpc.live_object_restorer(100);
        let mut partition = restorer.begin_partition();
        partition
            .index_object(&Object::with_owner_for_testing(Address::from_u16(42)))
            .unwrap();
        partition.finish().unwrap();
        restorer.finish().unwrap();

        grpc.finalize_and_verify_restore(&path, 5, 1).await.unwrap();

        let reopened = GrpcIndexesStore::new_without_init(path);
        assert_eq!(
            reopened.tables.watermark.get(&Watermark::Indexed).unwrap(),
            Some(5)
        );
    }

    /// The finalize writes the version and the watermark whether or not any
    /// object landed, so an empty store must fail the restore instead of
    /// being served as a complete index.
    #[tokio::test]
    async fn finalize_and_verify_restore_rejects_an_empty_store() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path().to_path_buf();
        let grpc = Arc::new(GrpcIndexesStore::new_without_init(path.clone()));

        let error = grpc
            .finalize_and_verify_restore(&path, 5, 1)
            .await
            .expect_err("an empty restore must not pass verification");
        assert!(
            error.to_string().contains("empty owner index"),
            "unexpected error: {error}"
        );
    }

    /// Buckets are rediscovered from the on-disk column-family names on
    /// reopen.
    #[tokio::test]
    async fn digest_buckets_survive_a_reopen() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        let (digest, checkpoint) = (TransactionDigest::random(), 7);
        grpc.history
            .ensure(3)
            .unwrap()
            .insert(&digest, &checkpoint)
            .unwrap();

        let weak_db = Arc::downgrade(&grpc.tables.meta.db);
        drop(grpc);
        assert!(wait_for_database_close(weak_db).await);

        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        assert_eq!(grpc.history.newest_epoch(), Some(3));
        let bucket = grpc.history.ensure(3).unwrap();
        assert_eq!(bucket.get(&digest).unwrap(), Some(checkpoint));
    }
    /// Digest lookups probe every retained epoch's bucket, newest first.
    #[tokio::test]
    async fn digest_lookup_probes_across_epoch_buckets() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        let (old_digest, new_digest) = (TransactionDigest::random(), TransactionDigest::random());
        grpc.history
            .ensure(0)
            .unwrap()
            .insert(&old_digest, &5)
            .unwrap();
        grpc.history
            .ensure(1)
            .unwrap()
            .insert(&new_digest, &9)
            .unwrap();

        assert_eq!(
            grpc.get_transaction_info(&old_digest)
                .unwrap()
                .unwrap()
                .checkpoint,
            5
        );
        assert_eq!(
            grpc.get_transaction_info(&new_digest)
                .unwrap()
                .unwrap()
                .checkpoint,
            9
        );
        assert!(
            grpc.get_transaction_info(&TransactionDigest::random())
                .unwrap()
                .is_none()
        );
    }
    /// Pruning drops whole epoch buckets and the floor survives a reopen,
    /// so dropped epochs are never recreated.
    #[tokio::test]
    async fn digest_pruning_drops_expired_epoch_buckets() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        let old_digest = TransactionDigest::random();
        grpc.history
            .ensure(0)
            .unwrap()
            .insert(&old_digest, &5)
            .unwrap();
        grpc.history
            .ensure(1)
            .unwrap()
            .insert(&TransactionDigest::random(), &9)
            .unwrap();

        assert_eq!(grpc.prune(1).unwrap(), Some(1));
        assert_eq!(grpc.get_transaction_info(&old_digest).unwrap(), None);
        assert!(
            grpc.history.ensure(0).is_err(),
            "a pruned epoch must not be recreated"
        );

        let weak_db = Arc::downgrade(&grpc.tables.meta.db);
        drop(grpc);
        assert!(wait_for_database_close(weak_db).await);
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        assert!(
            grpc.history.ensure(0).is_err(),
            "the retention floor must survive a reopen"
        );
    }
    /// The digest backfill records its progress atomically with each
    /// checkpoint's rows, so an interrupted replay resumes instead of
    /// starting over.
    #[tokio::test]
    async fn digest_backfill_resumes_from_its_marker() {
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
        let genesis_tx_digest = checkpoint_store
            .get_checkpoint_contents(&genesis_checkpoint.contents_digest)
            .unwrap()
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .transaction;

        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        grpc.tables.history_watermark.insert(&(), &1).unwrap();

        grpc.backfill_history(checkpoint_store).unwrap();

        assert_eq!(grpc.tables.history_watermark.get(&()).unwrap(), Some(0));
        assert_eq!(
            grpc.get_transaction_info(&genesis_tx_digest)
                .unwrap()
                .unwrap()
                .checkpoint,
            0
        );
        assert_eq!(
            grpc.metrics
                .history_backfill_lowest_replayed_checkpoint
                .get(),
            0,
            "the gauge must report how far down the replay got"
        );
    }
}
