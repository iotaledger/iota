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

use iota_sdk_types::{StructTag, TypeTag};
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber},
    committee::EpochId,
    digests::TransactionDigest,
    error::IotaResult,
    full_checkpoint_content::CheckpointData,
    iota_system_state::IotaSystemStateTrait,
    messages_checkpoint::{CheckpointContents, CheckpointSequenceNumber, VerifiedCheckpoint},
    move_package::MovePackageExt,
    object::{Object, Owner},
    storage::{
        AccountOwnedObjectInfo, DynamicFieldKey, EpochInfo, EpochInfoV2, OwnedObjectCursor,
        OwnedObjectIteratorItem, PackageVersionInfo, PackageVersionIteratorItem, PackageVersionKey,
        TransactionInfo, error::Error as StorageError,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use typed_store::{
    DBMapUtils, TypedStoreError,
    rocks::{DBMap, MetricConf, ReadWriteOptions},
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
    /// Highest epoch whose `epochs_v2` row is fully populated. Value is
    /// an `EpochId`, not a checkpoint sequence number. Advanced atomically
    /// with the close-of-epoch upsert in `index_epoch`. The snapshot V2 writer
    /// reads this before publishing; a value `< snapshot_epoch` (or
    /// absent) means the writer cannot vouch for `[0, snapshot_epoch]`
    /// and must refuse the snapshot.
    EpochIndexed,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoinIndexKey {
    coin_type: StructTag,
}

/// Coin index value with regulated coin metadata.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinIndexInfo {
    pub coin_metadata_object_id: Option<ObjectID>,
    pub treasury_object_id: Option<ObjectID>,
    pub regulated_coin_metadata_object_id: Option<ObjectID>,
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
    pub object_id: ObjectID,
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
            object_id: ObjectID::ZERO,
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
        object_id: ObjectID::MAX,
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

    /// Deprecated: superseded by `epochs_v2`.
    #[allow(dead_code)]
    #[deprecated_db_map(migration = "migrate_epochs_to_v2")]
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
    /// a particular ObjectID. Only the key is stored; field metadata is loaded
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

    /// Phase 2 of `init`: rebuild the live-state indexes by scanning the
    /// current live object set in parallel. Must re-run on any drift to keep
    /// them consistent.
    fn index_live_object_set(&self, authority_store: &AuthorityStore) -> Result<(), StorageError> {
        let coin_index = Mutex::new(HashMap::new());

        let make_live_object_indexer = GrpcParLiveObjectSetIndexer {
            tables: self,
            coin_index: &coin_index,
        };

        crate::par_index_live_object_set::par_index_live_object_set(
            authority_store,
            &make_live_object_indexer,
        )?;

        self.coin.multi_insert(coin_index.into_inner().unwrap())?;
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
        //   - Epoch boundaries additionally need object data, so they can only be
        //     locally indexed when `epoch_history_reaches_genesis` (both stores reach
        //     back to genesis). Otherwise a partial replay would leave gaps in
        //     `epochs_v2` with no way to fill them from the local store, and we defer
        //     to `backfill_epochs`, which will pull the missing rows from a formal
        //     snapshot.
        let tx_range =
            self.transaction_index_range(checkpoint_store, highest_executed_checkpoint)?;
        let epoch_history_reaches_genesis =
            self.epoch_history_reaches_genesis(authority_store, checkpoint_store)?;

        // `tx_range` is `None` only when no checkpoints have ever been
        // executed on this node — and in that state there are no epoch
        // transitions to observe either, so skipping the entire block
        // (both phase-1 indexing and any `backfill_epochs` call) is
        // correct.
        if let Some(range) = tx_range {
            self.index_historical_checkpoints(
                authority_store,
                checkpoint_store,
                range,
                epoch_history_reaches_genesis,
            )?;
            if !epoch_history_reaches_genesis {
                self.backfill_epochs()?;
            }
        }
        self.initialize_current_epoch_info(authority_store, checkpoint_store)?;

        // Phase 2 — live-state indexes from the current live object set.
        self.index_live_object_set(authority_store)?;

        // `Watermark::Indexed` and `meta` are written last so a crash
        // before this point leaves a recoverably-inconsistent on-disk
        // state that the next `new` call wipes and re-inits.
        self.watermark.insert(
            &Watermark::Indexed,
            &highest_executed_checkpoint.unwrap_or(0),
        )?;
        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )?;

        info!("Finished initializing gRPC indexes");

        Ok(())
    }

    /// Stub for archive-backed epoch backfill. Invoked from `init` when
    /// `epoch_history_reaches_genesis` returns false, so a local replay
    /// would leave gaps in `epochs_v2`. The real implementation
    /// will pull the missing rows from a historical archive — for now
    /// it's a no-op and the node simply runs without populated
    /// `epochs_v2` rows until the backfill PR lands. The snapshot V2
    /// writer's `EpochIndexed` precondition will keep it from publishing
    /// in the meantime.
    ///
    /// TODO: until this is implemented, `initialize_current_epoch_info`
    /// may fail on pruned-bootstrap nodes whose available checkpoint
    /// history doesn't reach the previous epoch's close-of-epoch
    /// checkpoint (its fallback `scan_for_epoch_start_checkpoint` walks
    /// backwards looking for that checkpoint and returns an error if
    /// pruning has advanced past it). The contract for the real backfill
    /// is to populate `epochs_v2[0..=current_epoch - 1]` before
    /// `initialize_current_epoch_info` runs, so the current-epoch seed
    /// can derive `start_checkpoint` from `epochs_v2[current_epoch - 1]`
    /// without falling back to the scan.
    fn backfill_epochs(&self) -> Result<(), StorageError> {
        info!("backfill_epochs stub invoked - no-op until archive-backed backfill lands");
        Ok(())
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
                summary.end_of_epoch_data.is_some() || checkpoint_sequence_number == 0;

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

        // Finalize `prev_epoch`'s row with the close-of-epoch fields and
        // advance `Watermark::EpochIndexed`. Genesis has no previous epoch
        // to finalize; the close-of-epoch write for epoch 0 fires when
        // epoch 1 is seeded, so `EpochIndexed` stays absent until then.
        if new_epoch_id > 0 {
            let prev_epoch = new_epoch_id - 1;
            // In safe mode the AdvanceEpoch tx is replaced by
            // `advance_epoch_safe_mode`, which mutates `0x5` but emits no
            // events. `Some(default())` says "finalized, with no events" —
            // distinct from the not-yet-finalized `None` lifecycle state.
            let end_of_epoch_tx_events = Some(end_of_epoch_events.unwrap_or_default());
            let last_checkpoint_summary = checkpoint.checkpoint_summary.clone();
            // If no row exists for `prev_epoch`, this node didn't see its
            // start (e.g. bootstrapped mid-epoch). Skip the upsert; the
            // row stays absent and the watermark stays behind, so the
            // snapshot writer correctly refuses to publish until an
            // external backfill fills the gap.
            if let Some(mut previous_epoch) = self.epochs_v2.get(&prev_epoch)? {
                previous_epoch.end_timestamp_ms = Some(epoch_info.start_timestamp_ms);
                previous_epoch.end_checkpoint = Some(epoch_info.start_checkpoint - 1);
                previous_epoch.last_checkpoint_summary = Some(last_checkpoint_summary);
                previous_epoch.end_of_epoch_tx_events = end_of_epoch_tx_events;
                batch.insert_batch(&self.epochs_v2, [(prev_epoch, previous_epoch)])?;
                self.try_advance_epoch_indexed_watermark(prev_epoch, batch)?;
            }
        }

        // seed the new epoch's row.
        let new_info = EpochInfoV2 {
            epoch: epoch_info.epoch,
            protocol_version: epoch_info.protocol_version,
            start_timestamp_ms: epoch_info.start_timestamp_ms,
            end_timestamp_ms: epoch_info.end_timestamp_ms,
            start_checkpoint: epoch_info.start_checkpoint,
            end_checkpoint: epoch_info.end_checkpoint,
            reference_gas_price: epoch_info.reference_gas_price,
            system_state: epoch_info.system_state,
            last_checkpoint_summary: None,
            end_of_epoch_tx_events: None,
        };
        batch.insert_batch(&self.epochs_v2, [(new_epoch_id, new_info)])?;

        Ok(())
    }

    /// Read `Watermark::EpochIndexed`: the highest epoch whose
    /// `epochs_v2` row has its end-of-epoch fields populated
    /// (`last_checkpoint_summary` and `end_of_epoch_tx_events`).
    /// `None` if no epoch has been fully indexed yet.
    fn highest_indexed_epoch(&self) -> Result<Option<EpochId>, TypedStoreError> {
        self.watermark.get(&Watermark::EpochIndexed)
    }

    // After attempting to reindex past epochs, ensure that the current epoch is at
    // least partially initialized
    fn initialize_current_epoch_info(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        let Some(checkpoint) = checkpoint_store.get_highest_executed_checkpoint()? else {
            return Ok(());
        };

        if self.epochs_v2.get(&checkpoint.epoch)?.is_some() {
            // no need to initialize if it already exists
            return Ok(());
        }

        let system_state = iota_types::iota_system_state::get_iota_system_state(authority_store)
            .map_err(|e| StorageError::custom(format!("Failed to find system state: {e}")))?;

        // Determine the start checkpoint of the current epoch
        let start_checkpoint = if checkpoint.epoch != 0 {
            let previous_epoch = checkpoint.epoch - 1;

            // Find the last checkpoint of the previous epoch
            if let Some(previous_epoch_info) = self.epochs_v2.get(&previous_epoch)? {
                if let Some(end_checkpoint) = previous_epoch_info.end_checkpoint {
                    end_checkpoint + 1
                } else {
                    // Fall back to scanning checkpoints if the end_checkpoint is None
                    self.scan_for_epoch_start_checkpoint(
                        checkpoint_store,
                        checkpoint.sequence_number,
                        previous_epoch,
                    )?
                }
            } else {
                // Fall back to scanning checkpoints if the previous epoch info is missing
                self.scan_for_epoch_start_checkpoint(
                    checkpoint_store,
                    checkpoint.sequence_number,
                    previous_epoch,
                )?
            }
        } else {
            // First epoch starts at checkpoint 0
            0
        };

        let epoch_info = EpochInfoV2 {
            epoch: checkpoint.epoch,
            protocol_version: system_state.protocol_version(),
            start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
            end_timestamp_ms: None,
            start_checkpoint,
            end_checkpoint: None,
            reference_gas_price: system_state.reference_gas_price(),
            system_state,
            last_checkpoint_summary: None,
            end_of_epoch_tx_events: None,
        };

        self.epochs_v2.insert(&epoch_info.epoch, &epoch_info)?;

        Ok(())
    }

    fn scan_for_epoch_start_checkpoint(
        &self,
        checkpoint_store: &CheckpointStore,
        current_checkpoint_seq_number: u64,
        previous_epoch: EpochId,
    ) -> Result<u64, StorageError> {
        // Scan from current checkpoint backwards to 0 to find the start of this epoch.
        let mut last_checkpoint_seq_number_of_prev_epoch = None;
        for checkpoint_seq_number in (0..=current_checkpoint_seq_number).rev() {
            let Some(chkpt) = checkpoint_store
                .get_checkpoint_by_sequence_number(checkpoint_seq_number)
                .ok()
                .flatten()
            else {
                // continue if there is a gap in the checkpoints
                continue;
            };

            if chkpt.epoch < previous_epoch {
                // we must stop searching if we are past the previous epoch
                break;
            }

            if chkpt.epoch == previous_epoch && chkpt.end_of_epoch_data.is_some() {
                // We found the checkpoint with end of epoch data for the previous epoch
                last_checkpoint_seq_number_of_prev_epoch = Some(chkpt.sequence_number);
                break;
            }
        }

        let last_checkpoint_seq_number_of_prev_epoch = last_checkpoint_seq_number_of_prev_epoch
            .ok_or(StorageError::custom(format!(
                "Failed to get the last checkpoint of the previous epoch {previous_epoch}",
            )))?;

        Ok(last_checkpoint_seq_number_of_prev_epoch + 1)
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
        let mut regulated_coin_keys: Vec<(CoinIndexKey, ObjectID)> = Vec::new();
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
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> Result<impl Iterator<Item = Result<DynamicFieldKey, TypedStoreError>> + '_, TypedStoreError>
    {
        let lower_bound = DynamicFieldKey::new(parent, cursor.unwrap_or(ObjectID::ZERO));
        let upper_bound = DynamicFieldKey::new(parent, ObjectID::MAX);
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
        original_package_id: ObjectID,
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
                    IndexStoreTables::open(path)
                };

                tables
                    .init(&authority_store, checkpoint_store)
                    .expect("unable to initialize gRPC index");
                tables
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
        parent: ObjectID,
        cursor: Option<ObjectID>,
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
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> Result<impl Iterator<Item = PackageVersionIteratorItem> + '_, TypedStoreError> {
        self.tables
            .package_versions_iter(original_package_id, cursor)
    }

    pub fn highest_indexed_epoch(&self) -> Result<Option<EpochId>, TypedStoreError> {
        self.tables.highest_indexed_epoch()
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
        parent: ObjectID,
        cursor: Option<ObjectID>,
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
        original_package_id: ObjectID,
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
fn try_create_regulated_coin_info(object: &Object) -> Option<(CoinIndexKey, ObjectID)> {
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

struct GrpcParLiveObjectSetIndexer<'a> {
    tables: &'a IndexStoreTables,
    coin_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
}

struct GrpcLiveObjectIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: typed_store::rocks::DBBatch,
    coin_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
}

impl<'a> ParMakeLiveObjectIndexer for GrpcParLiveObjectSetIndexer<'a> {
    type ObjectIndexer = GrpcLiveObjectIndexer<'a>;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer {
        GrpcLiveObjectIndexer {
            tables: self.tables,
            batch: self.tables.owner.batch(),
            coin_index: self.coin_index,
        }
    }
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

// Load a CheckpointData struct without event data
fn assemble_sparse_checkpoint_data(
    authority_store: &AuthorityStore,
    summary: VerifiedCheckpoint,
    contents: CheckpointContents,
) -> Result<CheckpointData, StorageError> {
    use iota_types::full_checkpoint_content::CheckpointTransaction;

    let transaction_digests = contents
        .iter()
        .map(|execution_digests| execution_digests.transaction)
        .collect::<Vec<_>>();
    let transactions = authority_store
        .multi_get_transaction_blocks(&transaction_digests)?
        .into_iter()
        .map(|maybe_transaction| {
            maybe_transaction.ok_or_else(|| StorageError::custom("missing transaction"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let effects = authority_store
        .multi_get_executed_effects(&transaction_digests)?
        .into_iter()
        .map(|maybe_effects| maybe_effects.ok_or_else(|| StorageError::custom("missing effects")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut full_transactions = Vec::with_capacity(transactions.len());
    for (tx, fx) in transactions.into_iter().zip(effects) {
        let input_objects =
            iota_types::storage::get_transaction_input_objects(authority_store, &fx)?;
        let output_objects =
            iota_types::storage::get_transaction_output_objects(authority_store, &fx)?;

        let full_transaction = CheckpointTransaction {
            transaction: tx.into(),
            effects: fx,
            events: None,
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

fn migrate_epochs_to_v2(db: &Arc<typed_store::database::Database>) -> Result<(), TypedStoreError> {
    let old = DBMap::<EpochId, EpochInfo>::reopen(
        db,
        Some("epochs"),
        &ReadWriteOptions::default(),
        true, // is_deprecated
    )?;
    let new = DBMap::<EpochId, EpochInfoV2>::reopen(
        db,
        Some("epochs_v2"),
        &ReadWriteOptions::default(),
        false,
    )?;

    // TODO: Add correct batching, and backfill the new fields from historical
    // archive
    let mut batch = new.batch();
    for entry in old.safe_iter() {
        let (epoch_id, old_info) = entry?;
        let new_info = EpochInfoV2 {
            epoch: old_info.epoch,
            protocol_version: old_info.protocol_version,
            start_timestamp_ms: old_info.start_timestamp_ms,
            end_timestamp_ms: old_info.end_timestamp_ms,
            start_checkpoint: old_info.start_checkpoint,
            end_checkpoint: old_info.end_checkpoint,
            reference_gas_price: old_info.reference_gas_price,
            system_state: old_info.system_state,
            last_checkpoint_summary: None,
            end_of_epoch_tx_events: None,
        };
        batch.insert_batch(&new, std::iter::once((epoch_id, new_info)))?;
    }
    batch.write()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_types::iota_system_state::IotaSystemState;
    use typed_store::rocks::{MetricConf, open_cf_opts};

    use super::*;

    /// On first open under the `#[deprecated_db_map(migration = ...)]`
    /// schema, an existing `epochs` column family must be:
    ///   1. drained into `epochs_v2` (with `None` for the end-of-epoch fields
    ///      that the old schema did not carry),
    ///   2. dropped from disk, and
    ///   3. left absent on subsequent opens so the migration is not re-run.
    ///
    /// Mirrors the macro contract verified by `migration_test` in
    /// `typed-store/tests/macro_tests.rs`.
    #[tokio::test]
    async fn migrate_epochs_to_v2_backfills_then_drops_old_cf() {
        let tmp_dir = iota_common::tempdir();
        let dbdir = tmp_dir.path().to_path_buf();

        // Step 1 — open RocksDB with both column families (mimicking the
        // pre-migration schema on disk) and write one row to `epochs`.
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
        {
            let opt_cfs: Vec<(&str, typed_store::rocksdb::Options)> = vec![
                ("epochs", typed_store::rocks::default_db_options().options),
                (
                    "epochs_v2",
                    typed_store::rocks::default_db_options().options,
                ),
            ];
            let db = open_cf_opts(&dbdir, None, MetricConf::default(), &opt_cfs)
                .expect("open DB with both CFs");
            let epochs = DBMap::<EpochId, EpochInfo>::reopen(
                &db,
                Some("epochs"),
                &ReadWriteOptions::default(),
                false,
            )
            .unwrap();
            epochs.insert(&old_info.epoch, &old_info).unwrap();
        }

        // Step 2 — open via the new schema. The macro must invoke
        // `migrate_epochs_to_v2` and then drop the `epochs` CF.
        let tables = IndexStoreTables::open_tables_read_write(
            dbdir.clone(),
            MetricConf::default(),
            None,
            None,
        );
        let migrated = tables
            .epochs_v2
            .get(&old_info.epoch)
            .unwrap()
            .expect("migrated row must be present in epochs_v2");
        assert_eq!(migrated.epoch, old_info.epoch);
        assert_eq!(migrated.protocol_version, old_info.protocol_version);
        assert_eq!(migrated.start_timestamp_ms, old_info.start_timestamp_ms);
        assert_eq!(migrated.end_timestamp_ms, old_info.end_timestamp_ms);
        assert_eq!(migrated.start_checkpoint, old_info.start_checkpoint);
        assert_eq!(migrated.end_checkpoint, old_info.end_checkpoint);
        assert_eq!(migrated.reference_gas_price, old_info.reference_gas_price);
        // Migrated rows carry dummy `None` for the end-of-epoch fields.
        assert!(migrated.last_checkpoint_summary.is_none());
        assert!(migrated.end_of_epoch_tx_events.is_none());
        drop(tables);

        // The `epochs` CF must have been dropped from disk.
        let listed = typed_store::rocks::list_tables(dbdir.clone()).unwrap();
        assert!(
            !listed.contains(&"epochs".to_string()),
            "epochs CF should have been dropped after migration; saw: {listed:?}"
        );

        // Step 3 — reopen. Migration must be idempotent: no panic, data
        // still present in `epochs_v2`.
        let tables2 =
            IndexStoreTables::open_tables_read_write(dbdir, MetricConf::default(), None, None);
        assert!(
            tables2.epochs_v2.get(&old_info.epoch).unwrap().is_some(),
            "migrated data must survive reopen"
        );
    }

    /// `try_advance_epoch_indexed_watermark` must advance `EpochIndexed`
    /// only when `prev_epoch` extends the contiguous prefix by exactly
    /// one. This guards the snapshot writer's `[0, watermark]` "every row
    /// fully populated" invariant against pre-bootstrap gaps and replaces
    /// the post-loop reconciliation pass that used to live in `init`.
    #[tokio::test]
    async fn try_advance_epoch_indexed_watermark_is_gap_aware() {
        let tmp_dir = iota_common::tempdir();
        let tables = IndexStoreTables::open_tables_read_write(
            tmp_dir.path().to_path_buf(),
            MetricConf::default(),
            None,
            None,
        );

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
