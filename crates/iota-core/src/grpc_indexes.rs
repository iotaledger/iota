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

use iota_sdk_types::{ObjectId, Owner, StructTag, TypeTag};
use iota_types::{
    base_types::{IotaAddress, SequenceNumber},
    committee::EpochId,
    digests::TransactionDigest,
    error::IotaResult,
    full_checkpoint_content::CheckpointData,
    iota_system_state::IotaSystemStateTrait,
    messages_checkpoint::{
        CheckpointContents, CheckpointSequenceNumber, CheckpointSummary, VerifiedCheckpoint,
    },
    move_package::MovePackageExt,
    object::Object,
    storage::{
        AccountOwnedObjectInfo, DynamicFieldKey, EpochInfo, EpochInfoV1Entry, EpochInfoV2,
        OwnedObjectCursor, OwnedObjectIteratorItem, PackageVersionInfo, PackageVersionIteratorItem,
        PackageVersionKey, TransactionInfo,
        error::{Error as StorageError, Kind as StorageErrorKind},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
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
/// (`DBMap<Watermark, CheckpointSequenceNumber>`). `Indexed` and `Pruned`
/// store a checkpoint sequence number; `EpochIndexed` stores an epoch id.
/// They share the same `u64` value type because RocksDB column families
/// require a single value schema — interpretation depends on the variant
/// used as the key.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Watermark {
    /// Highest checkpoint sequence number indexed.
    Indexed,
    /// Highest checkpoint sequence number pruned.
    Pruned,
    /// Highest epoch whose `epochs_v2` row is fully populated. An `EpochId`,
    /// not a checkpoint sequence number. Advanced atomically with the
    /// close-of-epoch upsert in `index_epoch`, and recomputed over the seeded
    /// prefix by `insert_epoch_info`; only ever raised. The snapshot V2 writer
    /// refuses to publish when this is `< snapshot_epoch` (or absent).
    EpochIndexed,
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
    pub owner: IotaAddress,
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
    owner: IotaAddress,
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
fn make_owner_key(owner: IotaAddress, object: &Object) -> Option<(OwnerIndexKey, OwnerIndexInfo)> {
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

    /// Deprecated: superseded by `epochs_v2`. Not migrated — old rows lack the
    /// end-of-epoch fields, so they could never satisfy `EpochIndexed`; the
    /// table is rebuilt by genesis replay or the snapshot backfill instead.
    #[allow(dead_code)]
    #[deprecated_db_map]
    epochs: Option<DBMap<EpochId, EpochInfo>>,

    /// An index of extra metadata for Epochs.
    ///
    /// Intentionally not pruned: the snapshot writer needs full
    /// `[0, snapshot_epoch]` coverage, so this table grows unboundedly
    /// with epoch count (one row per epoch, ever) by design. Do not add
    /// it to the `prune` function.
    ///
    /// Completeness is tracked by `Watermark::EpochIndexed`.
    epochs_v2: DBMap<EpochId, EpochInfoV2>,

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

    /// Advance `Watermark::EpochIndexed` only if there is no gap.
    fn try_advance_epoch_indexed_watermark(
        &self,
        prev_epoch: EpochId,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        let next_expected = self
            .watermark
            .get(&Watermark::EpochIndexed)?
            .map_or(0, |e| e.saturating_add(1));
        if prev_epoch == next_expected {
            batch.insert_batch(&self.watermark, [(Watermark::EpochIndexed, prev_epoch)])?;
        }
        Ok(())
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

    /// Precondition for local epoch indexing: returns true if the on-disk
    /// history reaches back to genesis for both the checkpoint store and the
    /// object store.
    fn epoch_history_reaches_genesis(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<bool, StorageError> {
        // Both watermarks are `None` -> nothing has been pruned.
        // Some(0) would mean that the genesis checkpoint is pruned.
        let contents_pruned = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .is_some();
        let objects_pruned = authority_store
            .perpetual_tables
            .get_highest_pruned_checkpoint()?
            .is_some();
        Ok(!contents_pruned && !objects_pruned)
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

        // Phase 1 — history-derived indexes. The two index families have
        // different pruning constraints:
        //   - Transactions need only `CheckpointContents`, so they span
        //     `transaction_index_range` (checkpoint-store pruning).
        //   - Epoch boundaries additionally need object data, so they're only indexed
        //     locally when `epoch_history_reaches_genesis`. Otherwise a partial replay
        //     would leave unfillable gaps in `epochs_v2`, so we skip them here and
        //     leave those rows to the node's startup backfill from a formal snapshot.
        let tx_range =
            self.transaction_index_range(checkpoint_store, highest_executed_checkpoint)?;
        let epoch_history_reaches_genesis =
            self.epoch_history_reaches_genesis(authority_store, checkpoint_store)?;

        // `tx_range` is `None` only when no checkpoints have ever been executed
        // on this node — and in that state there are no epoch transitions to
        // observe either, so skipping phase-1 indexing entirely is correct.
        if let Some(range) = tx_range {
            self.index_historical_checkpoints(
                authority_store,
                checkpoint_store,
                range,
                epoch_history_reaches_genesis,
            )?;
        }
        self.initialize_current_epoch_info(authority_store, checkpoint_store)?;

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

    /// Index history-derived indexes by replaying every checkpoint in
    /// `checkpoint_range` in order. When `index_epochs_locally` is true, epoch
    /// boundaries are processed via `index_epoch`.
    #[tracing::instrument(skip(self, authority_store, checkpoint_store))]
    fn index_historical_checkpoints(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        checkpoint_range: std::ops::RangeInclusive<u64>,
        index_epochs_locally: bool,
    ) -> Result<(), StorageError> {
        info!(
            "Indexing {} checkpoints in range {checkpoint_range:?} (index_epochs_locally={index_epochs_locally})",
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

            let is_epoch_boundary =
                summary.is_last_checkpoint_of_epoch() || checkpoint_sequence_number == 0;

            let mut batch = self.transaction_checkpoints.batch();
            if index_epochs_locally && is_epoch_boundary {
                // Boundary (or genesis): `index_epoch` needs the EndOfEpoch /
                // Genesis transaction's output objects (for system state) and
                // its events, so load the full sparse `CheckpointData`.
                let checkpoint_data =
                    assemble_sparse_checkpoint_data(authority_store, summary, contents)?;
                self.index_epoch(&checkpoint_data, &mut batch)?;
                self.index_transactions(
                    checkpoint_sequence_number,
                    &checkpoint_data.checkpoint_contents,
                    &mut batch,
                )?;
            } else {
                // Fast path: only transaction digests are needed.
                self.index_transactions(checkpoint_sequence_number, &contents, &mut batch)?;
            }

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

        self.index_epoch(checkpoint, &mut batch)?;
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

    fn index_epoch(
        &self,
        checkpoint: &CheckpointData,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        // Early return if this checkpoint doesn't have epoch info (non-boundary
        // checkpoint).
        let Some((epoch_info, end_of_epoch_events)) = checkpoint.epoch_info()? else {
            return Ok(());
        };
        let new_epoch_id = epoch_info.epoch;

        // Finalize `prev_epoch`'s row with the close-of-epoch proof bundle and
        // advance `Watermark::EpochIndexed`. Genesis has no previous epoch
        // to finalize; the close-of-epoch write for epoch 0 fires when
        // epoch 1 is seeded, so `EpochIndexed` stays absent until then.
        if new_epoch_id > 0 {
            let prev_epoch = new_epoch_id - 1;
            // If no row exists for `prev_epoch`, this node didn't see its
            // start (e.g. bootstrapped mid-epoch). Skip the upsert; the
            // row stays absent and the watermark stays behind, so the
            // snapshot writer correctly refuses to publish until an
            // external backfill fills the gap.
            if let Some(mut previous_epoch) = self.epochs_v2.get(&prev_epoch)? {
                // The closing checkpoint's last tx is `prev_epoch`'s
                // epoch-change tx; its effects and the system-state objects it
                // wrote (object `0x5` and its inner state object) are this
                // boundary's proof material.
                let change_epoch_tx = checkpoint.end_of_epoch_transaction().ok_or_else(|| {
                    StorageError::custom(format!(
                        "checkpoint {} closes epoch {prev_epoch} but carries no \
                         epoch-change transaction",
                        checkpoint.checkpoint_summary.sequence_number,
                    ))
                })?;
                // Serialized to raw bytes, not the decoded view, so they verify
                // against the effects' written-object digests at restore time
                // (see `get_iota_system_state_objects` for why only these two).
                let next_epoch_start_system_state_objects =
                    iota_types::iota_system_state::get_iota_system_state_objects(
                        &change_epoch_tx.output_objects.as_slice(),
                    )
                    .map_err(|e| {
                        StorageError::custom(format!(
                            "extracting next-epoch start-state objects: {e}"
                        ))
                    })?
                    .iter()
                    .map(bcs::to_bytes)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        StorageError::custom(format!("serializing start-state objects: {e}"))
                    })?;

                previous_epoch.epoch_close_proof = Some(EpochInfoV1Entry {
                    last_checkpoint_summary: checkpoint.checkpoint_summary.clone(),
                    last_checkpoint_contents: checkpoint.checkpoint_contents.clone(),
                    end_of_epoch_tx_effects: change_epoch_tx.effects.clone(),
                    // Safe-mode boundaries run `advance_epoch_safe_mode`, which
                    // mutates `0x5` but emits no events, so the list is empty.
                    end_of_epoch_tx_events: end_of_epoch_events.unwrap_or_default(),
                    next_epoch_start_system_state_objects,
                });
                batch.insert_batch(&self.epochs_v2, [(prev_epoch, previous_epoch)])?;
                self.try_advance_epoch_indexed_watermark(prev_epoch, batch)?;
            }
        }

        // seed the new epoch's row; its close-of-epoch proof is filled in when
        // the next epoch's boundary is indexed.
        let new_info = EpochInfoV2 {
            epoch: epoch_info.epoch,
            start_checkpoint: epoch_info.start_checkpoint,
            start_timestamp_ms: epoch_info.start_timestamp_ms,
            system_state: epoch_info.system_state,
            epoch_close_proof: None,
        };
        batch.insert_batch(&self.epochs_v2, [(new_epoch_id, new_info)])?;

        Ok(())
    }

    /// Read `Watermark::EpochIndexed`: the highest epoch whose `epochs_v2` row
    /// is finalized (the full close-of-epoch proof bundle present, see
    /// `EpochInfoV2::is_finalized`). `None` if no epoch has been fully
    /// indexed yet.
    fn highest_indexed_epoch(&self) -> Result<Option<EpochId>, TypedStoreError> {
        self.watermark.get(&Watermark::EpochIndexed)
    }

    /// Recompute `Watermark::EpochIndexed` = the highest epoch whose contiguous
    /// prefix `[0, epoch]` is finalized (see `EpochInfoV2::is_finalized`).
    /// Only ever raises it.
    ///
    /// Unlike `try_advance_epoch_indexed_watermark` (the live +1 step), this
    /// can jump the watermark across a whole seeded prefix, which is what a
    /// snapshot backfill needs.
    fn reconcile_epoch_indexed_watermark(&self) -> Result<(), TypedStoreError> {
        // `[0, watermark]` is already known complete, so resume the scan from
        // `watermark + 1` rather than re-scanning the whole table.
        let current = self.watermark.get(&Watermark::EpochIndexed)?;
        let mut next = current.map_or(0, |w| w + 1);
        for entry in self.epochs_v2.safe_iter_with_bounds(Some(next), None) {
            let (epoch_id, info) = entry?;
            if epoch_id != next || !info.is_finalized() {
                break;
            }
            next += 1;
        }
        if let Some(highest) = next.checked_sub(1) {
            if Some(highest) > current {
                self.watermark.insert(&Watermark::EpochIndexed, &highest)?;
            }
        }
        Ok(())
    }

    /// Persist fully-populated epoch rows (e.g. restored from a snapshot) and
    /// advance the `EpochIndexed` watermark over the now-contiguous prefix.
    fn insert_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<(), StorageError> {
        let mut batch = self.epochs_v2.batch();
        batch.insert_batch(
            &self.epochs_v2,
            rows.into_iter().map(|row| (row.epoch, row)),
        )?;
        batch.write()?;
        self.reconcile_epoch_indexed_watermark()?;
        Ok(())
    }

    // Seed the open epoch's `epochs_v2` row if missing. No-op if already
    // present or its start checkpoint can't be derived locally (pruned).
    fn initialize_current_epoch_info(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        let Some(checkpoint) = checkpoint_store.get_highest_executed_checkpoint()? else {
            return Ok(());
        };

        // Seed the *open* epoch, not the highest executed checkpoint's epoch.
        // The two differ when that checkpoint closed its epoch — the state a
        // snapshot restore always lands in. The closed epoch's row comes from
        // EPOCH_INFO; the next epoch's row has no other writer until its own
        // close, where `index_epoch` would find it missing, skip the finalize,
        // and wedge `EpochIndexed` below it for good.
        let current_epoch = open_epoch_of(checkpoint.data());

        if self.epochs_v2.get(&current_epoch)?.is_some() {
            // no need to initialize if it already exists
            return Ok(());
        }

        let Some(start_checkpoint) =
            self.current_epoch_start_checkpoint(checkpoint_store, current_epoch)?
        else {
            // Skip rather than fail init; `ensure_current_epoch_info`
            // re-seeds it once the backfill lands the previous epoch's row.
            warn!(
                epoch = current_epoch,
                "skipping current-epoch seed: previous epoch's last checkpoint is \
                 unknown locally; deferring to the snapshot backfill"
            );
            return Ok(());
        };

        let system_state = iota_types::iota_system_state::get_iota_system_state(authority_store)
            .map_err(|e| StorageError::custom(format!("Failed to find system state: {e}")))?;

        let epoch_info = EpochInfoV2 {
            epoch: current_epoch,
            start_checkpoint,
            start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
            system_state,
            epoch_close_proof: None,
        };

        self.epochs_v2.insert(&epoch_info.epoch, &epoch_info)?;

        Ok(())
    }

    /// The current epoch's start checkpoint, or `None` when it can't be derived
    /// from local data (no previous-epoch `epochs_v2` row and no
    /// `epoch_last_checkpoint_map` entry). `None` means "skip for now", not an
    /// error.
    fn current_epoch_start_checkpoint(
        &self,
        checkpoint_store: &CheckpointStore,
        current_epoch: EpochId,
    ) -> Result<Option<CheckpointSequenceNumber>, StorageError> {
        // The first epoch starts at checkpoint 0.
        if current_epoch == 0 {
            return Ok(Some(0));
        }
        let previous_epoch = current_epoch - 1;

        // Prefer the previous epoch's recorded end checkpoint.
        if let Some(end_checkpoint) = self
            .epochs_v2
            .get(&previous_epoch)?
            .and_then(|info| info.end_checkpoint())
        {
            return Ok(Some(end_checkpoint + 1));
        }

        // Unlike the checkpoint summaries, the map is never pruned.
        Ok(checkpoint_store
            .get_epoch_last_checkpoint_seq_number(previous_epoch)?
            .map(|seq| seq + 1))
    }

    /// See [`GrpcIndexesStore::index_missing_epochs_locally`].
    fn index_missing_epochs_locally(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        let Some(last_executed) =
            first_open_epoch(checkpoint_store)?.and_then(|open| open.checked_sub(1))
        else {
            return Ok(()); // no closed epoch yet
        };
        let Some(highest_indexed) = self.highest_indexed_epoch()? else {
            // Without a seeded prefix the replay would have to start at the
            // genesis checkpoint, which the nodes relying on the backfill
            // don't have; `init` covers the unpruned case.
            return Ok(());
        };
        if highest_indexed >= last_executed {
            return Ok(());
        }

        // Replay the closing checkpoints of epochs `[highest_indexed,
        // last_executed]` in order: the first one re-finalizes the already
        // complete prefix end and creates the next epoch's row (an epoch's
        // start state only exists in the previous epoch's closing
        // checkpoint), each subsequent one finalizes a missing row and
        // creates the next.
        for epoch in highest_indexed..=last_executed {
            // The map is never pruned, but the checkpoint data it points to
            // may be: missing data ends what we can rebuild locally.
            let checkpoint_data = (|| -> Result<Option<CheckpointData>, StorageError> {
                let Some(seq) = checkpoint_store.get_epoch_last_checkpoint_seq_number(epoch)?
                else {
                    return Ok(None);
                };
                let Some(summary) = checkpoint_store.get_checkpoint_by_sequence_number(seq)? else {
                    return Ok(None);
                };
                let Some(contents) =
                    checkpoint_store.get_checkpoint_contents(&summary.content_digest)?
                else {
                    return Ok(None);
                };
                match assemble_sparse_checkpoint_data(authority_store, summary, contents) {
                    Ok(data) => Ok(Some(data)),
                    // Pruned-away transactions/effects/objects are the
                    // expected end of what can be rebuilt locally; anything
                    // else is a real storage failure and must propagate.
                    Err(e) if e.kind() == StorageErrorKind::Missing => Ok(None),
                    Err(e) => Err(e),
                }
            })()?;
            let Some(checkpoint_data) = checkpoint_data else {
                warn!(
                    epoch,
                    "cannot index epoch locally (its closing checkpoint's data is pruned); \
                     leaving the remaining epochs to a newer snapshot"
                );
                return Ok(());
            };

            let mut batch = self.transaction_checkpoints.batch();
            self.index_epoch(&checkpoint_data, &mut batch)?;
            batch.write()?;
        }
        info!(
            "locally indexed epochs ({highest_indexed}, {last_executed}] not covered by the \
             snapshot backfill"
        );
        Ok(())
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

    fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfoV2>, TypedStoreError> {
        self.epochs_v2.get(&epoch)
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
        owner: IotaAddress,
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

    pub fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfoV2>, TypedStoreError> {
        self.tables.get_epoch_info(epoch)
    }

    pub fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        self.tables.get_transaction_info(digest)
    }

    pub fn owner_iter(
        &self,
        owner: IotaAddress,
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

    pub fn highest_indexed_epoch(&self) -> Result<Option<EpochId>, TypedStoreError> {
        self.tables.highest_indexed_epoch()
    }

    /// `None` when `epochs_v2` is contiguously populated from genesis through
    /// at least the last closed epoch this node executed; otherwise
    /// `Some((highest_indexed, last_executed_epoch))` describing the gap a
    /// startup guard must reject. Measured against the last executed closed
    /// epoch, not a target epoch, so a node still catching up isn't flagged.
    pub fn epochs_v2_gap(
        &self,
        checkpoint_store: &CheckpointStore,
    ) -> Result<Option<(Option<EpochId>, EpochId)>, StorageError> {
        let Some(open_epoch) = first_open_epoch(checkpoint_store)? else {
            return Ok(None); // nothing executed yet
        };
        let Some(last_executed) = open_epoch.checked_sub(1) else {
            return Ok(None); // still in the genesis epoch; no closed epoch
        };
        let highest_indexed = self.tables.highest_indexed_epoch()?;
        // `<`, not `!=`: a backfill seeded past local execution is a superset.
        Ok((highest_indexed < Some(last_executed)).then_some((highest_indexed, last_executed)))
    }

    /// Seed epoch rows from a snapshot's `EPOCH_INFO`. Must not run
    /// concurrently with live indexing (it recomputes `EpochIndexed` outside
    /// the per-checkpoint batch): callers are the restore tool (which builds
    /// the complete store and then calls [`Self::finalize_restore`]) and the
    /// node's synchronous startup backfill, both before live indexing starts.
    pub fn insert_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<(), StorageError> {
        self.tables.insert_epoch_info(rows)
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
    /// Callers must have restored the complete state first: every live object
    /// through [`Self::live_object_restorer`] and the epoch rows through
    /// [`Self::insert_epoch_info`].
    pub fn finalize_restore(
        &self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), TypedStoreError> {
        self.tables.finalize(restore_checkpoint)
    }

    /// Index locally any closed epochs still missing above the `EpochIndexed`
    /// watermark, by replaying their closing checkpoints from local data.
    ///
    /// Only needed when the latest published formal snapshot lags this node's
    /// executed history by more than one epoch (a delayed snapshot pipeline):
    /// the backfill then seeds a prefix that ends below the locally executed
    /// epochs, and the rows in between can only come from their own closing
    /// checkpoints. Best-effort: stops at the first epoch whose checkpoint
    /// data is already pruned, leaving the rest to a newer snapshot. Must not
    /// run concurrently with live indexing, like `insert_epoch_info`.
    pub fn index_missing_epochs_locally(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        self.tables
            .index_missing_epochs_locally(authority_store, checkpoint_store)
    }

    /// Seed the current (open) epoch's `epochs_v2` row if still missing. No-op
    /// if already seeded or the start checkpoint can't yet be determined; call
    /// again once the closed-epoch rows are seeded.
    pub fn ensure_current_epoch_info(
        &self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        self.tables
            .initialize_current_epoch_info(authority_store, checkpoint_store)
    }
}

// ---------------------------------------------------------------------------
// GrpcIndexes trait implementation
// ---------------------------------------------------------------------------

impl iota_node_storage::GrpcIndexes for GrpcIndexesStore {
    fn get_epoch_info(
        &self,
        epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<EpochInfoV2>> {
        self.tables
            .get_epoch_info(epoch)
            .map_err(|e| StorageError::custom(e.to_string()))
    }

    fn highest_indexed_epoch(&self) -> iota_types::storage::error::Result<Option<EpochId>> {
        self.tables
            .highest_indexed_epoch()
            .map_err(|e| StorageError::custom(e.to_string()))
    }

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
        owner: IotaAddress,
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

// ---------------------------------------------------------------------------

/// The first not-yet-closed epoch: the highest executed checkpoint's epoch,
/// plus one if that checkpoint already closed its epoch.
fn first_open_epoch(checkpoint_store: &CheckpointStore) -> Result<Option<EpochId>, StorageError> {
    Ok(checkpoint_store
        .get_highest_executed_checkpoint()?
        .map(|highest| open_epoch_of(highest.data())))
}

/// The open epoch as of `checkpoint`: its own epoch, plus one if it closed it.
fn open_epoch_of(checkpoint: &CheckpointSummary) -> EpochId {
    if checkpoint.is_last_checkpoint_of_epoch() {
        checkpoint.epoch + 1
    } else {
        checkpoint.epoch
    }
}

// Load a CheckpointData struct, including events for any transaction that
// emitted them (the epoch-change tx's events feed the EPOCH_INFO proof bundle);
// a pruned events table surfaces as a `Missing` error.
fn assemble_sparse_checkpoint_data(
    authority_store: &AuthorityStore,
    summary: VerifiedCheckpoint,
    contents: CheckpointContents,
) -> Result<CheckpointData, StorageError> {
    use iota_types::{
        effects::TransactionEffectsAPI, full_checkpoint_content::CheckpointTransaction,
    };

    let transaction_digests = contents
        .iter()
        .map(|execution_digests| execution_digests.transaction)
        .collect::<Vec<_>>();
    let transactions = authority_store
        .multi_get_transaction_blocks(&transaction_digests)?
        .into_iter()
        .map(|maybe_transaction| {
            maybe_transaction.ok_or_else(|| StorageError::missing("missing transaction"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let effects = authority_store
        .multi_get_executed_effects(&transaction_digests)?
        .into_iter()
        .map(|maybe_effects| maybe_effects.ok_or_else(|| StorageError::missing("missing effects")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut full_transactions = Vec::with_capacity(transactions.len());
    for (tx, fx) in transactions.into_iter().zip(effects) {
        let input_objects =
            iota_types::storage::get_transaction_input_objects(authority_store, &fx)?;
        let output_objects =
            iota_types::storage::get_transaction_output_objects(authority_store, &fx)?;

        // Load events for any emitting tx. Only the last (epoch-change) tx's
        // events are consumed downstream, but loading all keeps the assembled
        // `CheckpointData` self-consistent and costs little. A pruned events
        // table surfaces as a `Missing` error.
        let events = if fx.events_digest().is_some() {
            Some(
                authority_store
                    .get_events(fx.transaction_digest())
                    .map_err(|e| StorageError::custom(format!("loading events: {e}")))?
                    .ok_or_else(|| {
                        StorageError::missing("missing events for an emitting transaction")
                    })?,
            )
        } else {
            None
        };

        let full_transaction = CheckpointTransaction {
            transaction: tx.into(),
            effects: fx,
            events,
            input_objects,
            output_objects,
        };

        full_transactions.push(full_transaction);
    }

    let checkpoint_data = CheckpointData {
        checkpoint_summary: summary.into(),
        checkpoint_contents: contents,
        transactions: full_transactions,
    };

    Ok(checkpoint_data)
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::GasCostSummary;
    use iota_types::{
        crypto::AuthorityStrongQuorumSignInfo,
        digests::TransactionDigest,
        effects::{TransactionEffects, TransactionEffectsExtForTesting, TransactionEvents},
        iota_system_state::IotaSystemState,
        message_envelope::Envelope,
        messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSummary, EndOfEpochData},
    };
    use typed_store::rocks::{MetricConf, ReadWriteOptions, open_cf_opts};

    use super::*;

    /// A minimal certified summary for `epoch` at `sequence_number` (no
    /// end-of-epoch data, placeholder signature).
    fn certified_summary(epoch: EpochId, sequence_number: u64) -> CertifiedCheckpointSummary {
        certified_summary_with(epoch, sequence_number, None)
    }

    fn certified_summary_with(
        epoch: EpochId,
        sequence_number: u64,
        end_of_epoch_data: Option<EndOfEpochData>,
    ) -> CertifiedCheckpointSummary {
        let summary = CheckpointSummary {
            epoch,
            sequence_number,
            network_total_transactions: 0,
            content_digest: Default::default(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: GasCostSummary::default(),
            end_of_epoch_data,
            timestamp_ms: 0,
            version_specific_data: Vec::new(),
            checkpoint_commitments: Vec::new(),
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        Envelope::new_from_data_and_sig(summary, sig)
    }

    /// An executed (non-boundary) checkpoint for seeding a test
    /// `CheckpointStore`.
    fn executed_checkpoint(epoch: EpochId, sequence_number: u64) -> VerifiedCheckpoint {
        VerifiedCheckpoint::new_unchecked(certified_summary(epoch, sequence_number))
    }

    /// An executed close-of-epoch checkpoint — the state a snapshot restore
    /// leaves as the highest executed checkpoint.
    fn closing_checkpoint(epoch: EpochId, sequence_number: u64) -> VerifiedCheckpoint {
        VerifiedCheckpoint::new_unchecked(certified_summary_with(
            epoch,
            sequence_number,
            Some(EndOfEpochData {
                next_epoch_committee: Vec::new(),
                next_epoch_protocol_version: 1.into(),
                epoch_commitments: Vec::new(),
                epoch_supply_change: 0,
            }),
        ))
    }

    /// A finalized `EpochInfoV2` row (its `epoch_close_proof` is `Some`) — the
    /// only shape `reconcile` counts toward the `EpochIndexed` watermark.
    fn complete_epoch_info(epoch: EpochId) -> EpochInfoV2 {
        EpochInfoV2 {
            epoch,
            start_checkpoint: 0,
            start_timestamp_ms: 0,
            system_state: IotaSystemState::for_testing(epoch, 1),
            epoch_close_proof: Some(EpochInfoV1Entry {
                last_checkpoint_summary: certified_summary(epoch, 0),
                last_checkpoint_contents: CheckpointContents::new_with_digests_only_for_tests(
                    std::iter::empty(),
                ),
                end_of_epoch_tx_effects: TransactionEffects::new_empty_v1_for_testing(
                    TransactionDigest::ZERO,
                ),
                end_of_epoch_tx_events: TransactionEvents::default(),
                next_epoch_start_system_state_objects: Vec::new(),
            }),
        }
    }

    /// `insert_epoch_info` reconciles the watermark to the contiguous-prefix
    /// maximum: rows above a gap (even from genesis) don't advance it, and an
    /// insert that fills the gap advances it across the whole now-contiguous
    /// prefix.
    #[tokio::test]
    async fn insert_epoch_info_round_trips_and_advances_watermark() {
        let tmp_dir = iota_common::tempdir();
        let tables = IndexStoreTables::open(tmp_dir.path().to_path_buf());

        // A first insert that doesn't start at genesis leaves the watermark
        // absent: the contiguous-from-0 prefix is still empty.
        tables
            .insert_epoch_info(vec![complete_epoch_info(5)])
            .unwrap();
        assert!(tables.get_epoch_info(5).unwrap().is_some());
        assert_eq!(tables.highest_indexed_epoch().unwrap(), None);

        tables
            .insert_epoch_info(vec![
                complete_epoch_info(0),
                complete_epoch_info(1),
                complete_epoch_info(2),
            ])
            .unwrap();

        for epoch in 0..=2 {
            assert!(
                tables.get_epoch_info(epoch).unwrap().is_some(),
                "epoch {epoch} row must be present after insert"
            );
        }
        assert_eq!(tables.highest_indexed_epoch().unwrap(), Some(2));

        // A row at epoch 4 leaves a gap at epoch 3, so the watermark stays at 2.
        tables
            .insert_epoch_info(vec![complete_epoch_info(4)])
            .unwrap();
        assert!(tables.get_epoch_info(4).unwrap().is_some());
        assert_eq!(tables.highest_indexed_epoch().unwrap(), Some(2));

        // Filling the gap at epoch 3 makes [0, 5] contiguous, so the watermark
        // jumps across every stranded row to 5.
        tables
            .insert_epoch_info(vec![complete_epoch_info(3)])
            .unwrap();
        assert_eq!(tables.highest_indexed_epoch().unwrap(), Some(5));
    }

    /// `reconcile` is monotonic: it never lowers `EpochIndexed`. A seed that
    /// covers only a short prefix must not clobber a watermark the live
    /// indexer already advanced further, which would break the
    /// `[0, watermark]` fully-populated invariant.
    #[tokio::test]
    async fn reconcile_epoch_indexed_watermark_never_regresses() {
        let tmp_dir = iota_common::tempdir();
        let tables = IndexStoreTables::open(tmp_dir.path().to_path_buf());

        // Seed a contiguous prefix [0, 2] -> watermark 2.
        tables
            .insert_epoch_info(vec![
                complete_epoch_info(0),
                complete_epoch_info(1),
                complete_epoch_info(2),
            ])
            .unwrap();
        assert_eq!(tables.highest_indexed_epoch().unwrap(), Some(2));

        // Simulate a concurrent live advance to a higher epoch.
        tables
            .watermark
            .insert(&Watermark::EpochIndexed, &5)
            .unwrap();

        // A reconcile that only sees the [0, 2] prefix must NOT lower it.
        tables.reconcile_epoch_indexed_watermark().unwrap();
        assert_eq!(
            tables.highest_indexed_epoch().unwrap(),
            Some(5),
            "reconcile must not regress a higher watermark"
        );
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

        let owner = IotaAddress::from_u16(42);
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

    /// `current_epoch_start_checkpoint` returns `None` (skip — don't error)
    /// when a pruned node has neither the previous epoch's `epochs_v2` row
    /// nor an `epoch_last_checkpoint_map` entry exists. That `None` is what
    /// keeps `initialize_current_epoch_info` from panicking `init`. It
    /// returns `Some` for genesis, from the never-pruned map entry alone (no
    /// summary needed), and from the previous epoch's row (preferred).
    #[tokio::test]
    async fn current_epoch_start_checkpoint_skips_when_history_unavailable() {
        let tmp_dir = iota_common::tempdir();
        let tables = IndexStoreTables::open(tmp_dir.path().to_path_buf());

        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));

        // Genesis always starts at checkpoint 0; no history needed.
        assert_eq!(
            tables
                .current_epoch_start_checkpoint(&checkpoint_store, 0)
                .unwrap(),
            Some(0)
        );

        // Epoch 5 with no previous-epoch row and no map entry -> `None`
        // (skip), not an error.
        assert_eq!(
            tables
                .current_epoch_start_checkpoint(&checkpoint_store, 5)
                .unwrap(),
            None
        );

        // The never-pruned map entry alone resolves the start, even with every
        // checkpoint summary absent: epoch 4 ended at 41 -> start is 42.
        checkpoint_store
            .insert_epoch_last_checkpoint(4, &executed_checkpoint(4, 41))
            .unwrap();
        assert_eq!(
            tables
                .current_epoch_start_checkpoint(&checkpoint_store, 5)
                .unwrap(),
            Some(42)
        );

        // The previous epoch's row takes precedence over the map.
        // `complete_epoch_info(4)` has `end_checkpoint == Some(0)` -> start 1.
        tables
            .epochs_v2
            .insert(&4, &complete_epoch_info(4))
            .unwrap();
        assert_eq!(
            tables
                .current_epoch_start_checkpoint(&checkpoint_store, 5)
                .unwrap(),
            Some(1)
        );
    }

    /// A snapshot restore leaves the previous epoch's closing checkpoint as
    /// the highest executed one; the open epoch — the one whose row
    /// `initialize_current_epoch_info` must seed — is the next one. Keying
    /// the seed off the checkpoint's own epoch instead would leave the open
    /// epoch's row permanently missing (no later writer exists for it) and
    /// wedge the `EpochIndexed` watermark below it.
    #[tokio::test]
    async fn first_open_epoch_steps_past_a_closing_checkpoint() {
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));

        // Nothing executed yet.
        assert_eq!(first_open_epoch(&checkpoint_store).unwrap(), None);

        // Mid-epoch checkpoint: its own epoch is still open.
        let mid = executed_checkpoint(3, 10);
        checkpoint_store.insert_verified_checkpoint(&mid).unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&mid)
            .unwrap();
        assert_eq!(first_open_epoch(&checkpoint_store).unwrap(), Some(3));

        // Close-of-epoch checkpoint: epoch 3 is closed, epoch 4 is open.
        let closing = closing_checkpoint(3, 11);
        checkpoint_store
            .insert_verified_checkpoint(&closing)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&closing)
            .unwrap();
        assert_eq!(first_open_epoch(&checkpoint_store).unwrap(), Some(4));
    }

    /// `epochs_v2_gap` flags a contiguous prefix that falls short of the last
    /// executed closed epoch — and nothing else: no closed epoch yet and a
    /// backfill seeded past local execution both count as complete.
    #[tokio::test]
    async fn epochs_v2_gap_flags_short_prefix_not_overshoot() {
        let tmp_dir = iota_common::tempdir();
        let grpc = GrpcIndexesStore::new_without_init(tmp_dir.path().to_path_buf());
        let cp_dir = iota_common::tempdir();
        let checkpoint_store = CheckpointStore::new(&cp_dir.path().join("checkpoints"));

        // Nothing executed yet -> nothing to guard.
        assert_eq!(grpc.epochs_v2_gap(&checkpoint_store).unwrap(), None);

        // Genesis epoch still open -> no closed epoch -> no gap.
        let genesis = executed_checkpoint(0, 0);
        checkpoint_store
            .insert_verified_checkpoint(&genesis)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&genesis)
            .unwrap();
        assert_eq!(grpc.epochs_v2_gap(&checkpoint_store).unwrap(), None);

        // Executed into epoch 2 (synthetic jump — the store doesn't validate
        // epoch continuity) -> closed epochs [0, 1]; an empty index and a
        // prefix short of epoch 1 are both gaps.
        let in_epoch_2 = executed_checkpoint(2, 1);
        checkpoint_store
            .insert_verified_checkpoint(&in_epoch_2)
            .unwrap();
        checkpoint_store
            .update_highest_executed_checkpoint(&in_epoch_2)
            .unwrap();
        assert_eq!(
            grpc.epochs_v2_gap(&checkpoint_store).unwrap(),
            Some((None, 1))
        );
        grpc.insert_epoch_info(vec![complete_epoch_info(0)])
            .unwrap();
        assert_eq!(
            grpc.epochs_v2_gap(&checkpoint_store).unwrap(),
            Some((Some(0), 1))
        );

        // Complete through the last closed epoch -> no gap.
        grpc.insert_epoch_info(vec![complete_epoch_info(1)])
            .unwrap();
        assert_eq!(grpc.epochs_v2_gap(&checkpoint_store).unwrap(), None);

        // A backfill seeded beyond local execution (remote snapshot ahead of a
        // catching-up node) is a superset, not a gap.
        grpc.insert_epoch_info(vec![complete_epoch_info(2), complete_epoch_info(3)])
            .unwrap();
        assert_eq!(grpc.epochs_v2_gap(&checkpoint_store).unwrap(), None);
    }

    /// On open under the `#[deprecated_db_map]` schema, an existing `epochs`
    /// column family must be dropped from disk without being migrated, and
    /// stay absent on subsequent opens. Mirrors the macro contract verified
    /// by `deprecate_test` in `typed-store/tests/macro_tests.rs`.
    #[tokio::test]
    async fn deprecated_epochs_cf_is_dropped_without_migration() {
        let tmp_dir = iota_common::tempdir();
        let dbdir = tmp_dir.path().to_path_buf();

        // Open RocksDB with the old `epochs` CF on disk (mimicking a
        // pre-`epochs_v2` store) and write one row to it.
        {
            let opt_cfs: Vec<(&str, typed_store::rocksdb::Options)> =
                vec![("epochs", typed_store::rocks::default_db_options().options)];
            let db = open_cf_opts(&dbdir, None, MetricConf::default(), &opt_cfs)
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

        // Open via the current schema: the `epochs` CF must be dropped and
        // nothing carried over into `epochs_v2`.
        let tables = IndexStoreTables::open(dbdir.clone());
        assert!(tables.epochs_v2.is_empty());
        drop(tables);

        let listed = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
        assert!(
            !listed.contains(&"epochs".to_string()),
            "epochs CF should have been dropped; saw: {listed:?}"
        );

        // Reopening must not panic and must stay empty.
        let tables = IndexStoreTables::open(dbdir);
        assert!(tables.epochs_v2.is_empty());
    }

    /// `try_advance_epoch_indexed_watermark` must advance `EpochIndexed`
    /// only when `prev_epoch` extends the contiguous prefix by exactly
    /// one. This guards the snapshot writer's `[0, watermark]` "every row
    /// fully populated" invariant against pre-bootstrap gaps.
    #[tokio::test]
    async fn try_advance_epoch_indexed_watermark_is_gap_aware() {
        let tmp_dir = iota_common::tempdir();
        let tables = IndexStoreTables::open(tmp_dir.path().to_path_buf());

        let advance = |epoch| {
            let mut batch = tables.watermark.batch();
            tables
                .try_advance_epoch_indexed_watermark(epoch, &mut batch)
                .unwrap();
            batch.write().unwrap();
            tables.watermark.get(&Watermark::EpochIndexed).unwrap()
        };

        // From absent: prev_epoch=5 must NOT advance (bootstrap mid-history).
        assert_eq!(
            advance(5),
            None,
            "absent watermark + non-zero prev_epoch must stay absent"
        );

        // From absent: prev_epoch=0 advances to 0 (genesis close).
        assert_eq!(advance(0), Some(0), "absent watermark + 0 advances to 0");

        // From 0: prev_epoch=2 must NOT advance (gap at 1).
        assert_eq!(
            advance(2),
            Some(0),
            "non-contiguous advance must be a no-op"
        );

        // From 0: prev_epoch=1 advances to 1.
        assert_eq!(advance(1), Some(1), "contiguous advance succeeds");

        // From 1: prev_epoch=1 again is a no-op (already covered).
        assert_eq!(advance(1), Some(1), "re-advance to same value is a no-op");

        // From 1: prev_epoch=2 advances to 2.
        assert_eq!(advance(2), Some(2), "next contiguous epoch advances");
    }
}
