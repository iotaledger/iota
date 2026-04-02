// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    hash::Hasher,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use iota_types::{
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber},
    committee::EpochId,
    digests::TransactionDigest,
    dynamic_field::visitor as DFV,
    error::IotaResult,
    full_checkpoint_content::CheckpointData,
    iota_system_state::IotaSystemStateTrait,
    layout_resolver::LayoutResolver,
    messages_checkpoint::{CheckpointContents, CheckpointSequenceNumber},
    object::{Object, Owner},
    storage::{
        BackingPackageStore, DynamicFieldIndexInfo, DynamicFieldKey, EpochInfo,
        OwnedObjectV2Cursor, PackageVersionInfo, PackageVersionIteratorItem, PackageVersionKey,
        TransactionInfo, error::Error as StorageError,
    },
};
use move_core_types::language_storage::StructTag;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use typed_store::{
    DBMapUtils, TypedStoreError,
    rocks::{DBMap, MetricConf},
    traits::Map,
};

use crate::{
    authority::{AuthorityStore, authority_per_epoch_store::AuthorityPerEpochStore},
    checkpoints::CheckpointStore,
    par_index_live_object_set::{LiveObjectIndexer, ParMakeLiveObjectIndexer},
};

/// Bump this when changing the serialization format of an existing table.
/// A version mismatch triggers a full re-index via
/// `needs_to_do_initialization`.
///
/// NOTE: Adding a *new* table does NOT require a version bump.  New tables
/// start empty and are populated by a background backfill task tracked via
/// dedicated `Watermark` variants (`PackageVersionBackfilled`,
/// `CoinV2Backfilled`, `OwnerV2Backfilled`).  While the backfill runs, affected
/// endpoints return `Code::Unavailable` with a `RetryInfo` hint.
const CURRENT_DB_VERSION: u64 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct MetadataInfo {
    /// Version of the Database
    version: u64,
}

/// Checkpoint watermark type
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Watermark {
    Indexed,
    Pruned,
    /// Written once the `package_version` table backfill has completed.
    PackageVersionBackfilled,
    /// Written once the `coin_v2` table backfill has completed.
    CoinV2Backfilled,
    /// Written once the `owner_v2` table backfill has completed.
    OwnerV2Backfilled,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct OwnerIndexKey {
    pub owner: IotaAddress,
    pub object_id: ObjectID,
}

impl OwnerIndexKey {
    fn new(owner: IotaAddress, object_id: ObjectID) -> Self {
        Self { owner, object_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerIndexInfo {
    // object_id of the object is a part of the Key
    pub version: SequenceNumber,
    pub type_: MoveObjectType,
}

impl OwnerIndexInfo {
    pub fn new(object: &Object) -> Self {
        Self {
            version: object.version(),
            type_: object.type_().expect("packages cannot be owned").to_owned(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CoinIndexKey {
    coin_type: StructTag,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinIndexInfo {
    pub coin_metadata_object_id: Option<ObjectID>,
    pub treasury_object_id: Option<ObjectID>,
}

impl CoinIndexInfo {
    fn merge(&mut self, other: Self) {
        self.coin_metadata_object_id = self
            .coin_metadata_object_id
            .or(other.coin_metadata_object_id);
        self.treasury_object_id = self.treasury_object_id.or(other.treasury_object_id);
    }
}

/// Extended coin index value that absorbs `regulated_coin` into a single table.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinIndexInfoV2 {
    pub coin_metadata_object_id: Option<ObjectID>,
    pub treasury_object_id: Option<ObjectID>,
    pub regulated_coin_metadata_object_id: Option<ObjectID>,
}

impl From<CoinIndexInfo> for CoinIndexInfoV2 {
    fn from(info: CoinIndexInfo) -> Self {
        Self {
            coin_metadata_object_id: info.coin_metadata_object_id,
            treasury_object_id: info.treasury_object_id,
            regulated_coin_metadata_object_id: None,
        }
    }
}

impl From<CoinIndexInfoV2> for iota_types::storage::CoinInfoV2 {
    fn from(info: CoinIndexInfoV2) -> Self {
        Self {
            coin_metadata_object_id: info.coin_metadata_object_id,
            treasury_object_id: info.treasury_object_id,
            regulated_coin_metadata_object_id: info.regulated_coin_metadata_object_id,
        }
    }
}

impl CoinIndexInfoV2 {
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

/// Insert-or-merge a `CoinIndexInfoV2` into an in-memory HashMap.
fn merge_coin_into_v2(
    index: &mut HashMap<CoinIndexKey, CoinIndexInfoV2>,
    key: CoinIndexKey,
    v2: CoinIndexInfoV2,
) {
    use std::collections::hash_map::Entry;
    match index.entry(key) {
        Entry::Occupied(mut o) => o.get_mut().merge(v2),
        Entry::Vacant(v) => {
            v.insert(v2);
        }
    }
}

/// Read-modify-write a `CoinIndexInfoV2` entry in the `coin_v2` DB table.
///
/// Reads the current value (if any), applies `mutate`, and stages the result
/// into `batch`.  Used for incremental indexing where the full value is built
/// across multiple objects (e.g. `CoinMetadata` + `RegulatedCoinMetadata`).
fn read_merge_write_coin_v2(
    table: &DBMap<CoinIndexKey, CoinIndexInfoV2>,
    batch: &mut typed_store::rocks::DBBatch,
    key: CoinIndexKey,
    mutate: impl FnOnce(&mut CoinIndexInfoV2),
) -> Result<(), StorageError> {
    let mut v2 = table.get(&key).ok().flatten().unwrap_or_default();
    mutate(&mut v2);
    batch.insert_batch(table, [(key, v2)])?;
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
pub struct OwnerIndexKeyV2 {
    pub owner: IotaAddress,
    pub object_type_identifier: u64,
    pub object_type_params: u64,
    pub inverted_balance: Option<u64>,
    pub object_id: ObjectID,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnerIndexInfoV2 {
    pub object_type: StructTag,
    pub version: SequenceNumber,
}

/// Type filter for `owner_v2_iter`.
///
/// - `None` — all objects for the owner.
/// - `BaseType` — all objects whose `address::module::name` matches (e.g. all
///   `Coin<*>`). Post-filters hash collisions via `tag`.
/// - `ExactType` — only objects of the exact `StructTag` (e.g. `Coin<IOTA>`).
///   Post-filters hash collisions via `tag`.
#[derive(Clone)]
pub enum OwnerV2TypeFilter {
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

impl OwnerV2TypeFilter {
    /// Construct an `OwnerV2TypeFilter` from an optional `StructTag` filter.
    ///
    /// If `None`, returns `OwnerV2TypeFilter::None`.  If `Some(tag)` with no
    /// type params, returns `OwnerV2TypeFilter::BaseType`.  If `Some(tag)`
    /// with type params, returns `OwnerV2TypeFilter::ExactType`.
    pub fn from_struct_tag(tag: Option<&StructTag>) -> Self {
        if let Some(tag) = tag {
            if tag.type_params.is_empty() {
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
    hasher.write(tag.address.as_ref());
    hasher.write(tag.module.as_bytes());
    hasher.write(tag.name.as_bytes());
    hasher.finish()
}

fn hash_type_params(tag: &StructTag) -> u64 {
    let mut hasher = twox_hash::XxHash64::with_seed(1);
    let bytes = bcs::to_bytes(&tag.type_params).expect("type_params serialization cannot fail");
    hasher.write(&bytes);
    hasher.finish()
}

/// Compute inclusive lower and upper `OwnerIndexKeyV2` bounds for a
/// `safe_iter_with_bounds` range scan, narrowed by `type_filter`.
///
/// When `cursor` is `Some`, the lower bound is set to the cursor's exact
/// position (inclusive) so that RocksDB can seek directly.
fn owner_v2_bounds(
    owner: IotaAddress,
    cursor: Option<&OwnedObjectV2Cursor>,
    filter: &OwnerV2TypeFilter,
) -> (OwnerIndexKeyV2, OwnerIndexKeyV2) {
    let lower_bound = if let Some(c) = cursor {
        // Resume from the exact cursor position in the v2 key space.
        OwnerIndexKeyV2 {
            owner,
            object_type_identifier: c.object_type_identifier,
            object_type_params: c.object_type_params,
            inverted_balance: c.inverted_balance,
            object_id: c.object_id,
        }
    } else {
        let (lower_id, _, lower_params, _) = match filter {
            OwnerV2TypeFilter::None => (0, u64::MAX, 0, u64::MAX),
            OwnerV2TypeFilter::BaseType { id_hash, .. } => (*id_hash, *id_hash, 0, u64::MAX),
            OwnerV2TypeFilter::ExactType {
                id_hash,
                params_hash,
                ..
            } => (*id_hash, *id_hash, *params_hash, *params_hash),
        };
        OwnerIndexKeyV2 {
            owner,
            object_type_identifier: lower_id,
            object_type_params: lower_params,
            inverted_balance: None,
            object_id: ObjectID::ZERO,
        }
    };

    let (_, upper_bound_id, _, upper_bound_params) = match filter {
        OwnerV2TypeFilter::None => (0, u64::MAX, 0, u64::MAX),
        OwnerV2TypeFilter::BaseType { id_hash, .. } => (*id_hash, *id_hash, 0, u64::MAX),
        OwnerV2TypeFilter::ExactType {
            id_hash,
            params_hash,
            ..
        } => (*id_hash, *id_hash, *params_hash, *params_hash),
    };

    let upper_bound = OwnerIndexKeyV2 {
        owner,
        object_type_identifier: upper_bound_id,
        object_type_params: upper_bound_params,
        inverted_balance: Some(u64::MAX),
        object_id: ObjectID::MAX,
    };

    (lower_bound, upper_bound)
}

/// Build an `OwnerIndexKeyV2` for an address-owned object.
fn make_owner_v2_key(
    owner: IotaAddress,
    object: &Object,
) -> Option<(OwnerIndexKeyV2, OwnerIndexInfoV2)> {
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

    let key = OwnerIndexKeyV2 {
        owner,
        object_type_identifier: id_hash,
        object_type_params: params_hash,
        inverted_balance,
        object_id: object.id(),
    };
    let info = OwnerIndexInfoV2 {
        object_type: struct_tag,
        version: object.version(),
    };
    Some((key, info))
}

/// RocksDB tables for the RestIndexStore
///
/// Anytime a new table is added, or and existing one has it's schema changed,
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

    /// An index of extra metadata for Epochs.
    ///
    /// Only contains entries for epochs which have yet to be pruned from the
    /// main database.
    // TODO: https://github.com/iotaledger/iota/issues/10957
    epochs: DBMap<EpochId, EpochInfo>,

    /// An index of extra metadata for Transactions.
    ///
    /// Only contains entries for transactions which have yet to be pruned from
    /// the main database.
    transactions: DBMap<TransactionDigest, TransactionInfo>,

    /// An index of object ownership.
    ///
    /// Allows an efficient iterator to list all objects currently owned by a
    /// specific user account.
    // REST-API only
    // TODO: Remove once REST-API server is deprecated — gRPC uses owner_v2.
    owner: DBMap<OwnerIndexKey, OwnerIndexInfo>,

    /// An index of object ownership.
    ///
    /// Uses fixed-size u64 hash keys for correct RocksDB byte-order iteration.
    /// Allows an efficient iterator to list all objects currently owned by a
    /// specific user account, optionally filtered by type.
    ///
    /// Full `StructTag` stored in value for collision filtering & API
    /// responses. Bounded by the live object set (one entry per
    /// address-owned object).
    // gRPC-server only
    owner_v2: DBMap<OwnerIndexKeyV2, OwnerIndexInfoV2>,

    /// An index of dynamic fields (children objects).
    ///
    /// Allows an efficient iterator to list all of the dynamic fields owned by
    /// a particular ObjectID.
    // REST-API and gRPC
    // TODO: Replace DynamicFieldIndexInfo with () once the REST-API server is
    // deprecated — gRPC only needs the key.
    dynamic_field: DBMap<DynamicFieldKey, DynamicFieldIndexInfo>,

    /// An index of Coin Types
    ///
    /// Allows looking up information related to published Coins, like the
    /// ObjectID of its corresponding CoinMetadata.
    // REST-API only
    coin: DBMap<CoinIndexKey, CoinIndexInfo>,

    /// Same key as `coin`, extended value with regulated coin metadata.
    /// Bounded by the live object set (one entry per coin type).
    // gRPC-server only
    coin_v2: DBMap<CoinIndexKey, CoinIndexInfoV2>,

    /// An index of Package versions.
    ///
    /// Maps original package ID and version to the storage ID of that version.
    /// Allows efficient listing of all versions of a package, including
    /// upgraded user packages that have different storage IDs.
    /// Bounded by the live object set (one entry per package version).
    /// gRPC-server only
    package_version: DBMap<PackageVersionKey, PackageVersionInfo>,
    // NOTE: Authors and Reviewers before adding any new tables ensure that they are either:
    // - bounded in size by the live object set
    // - are prune-able and have corresponding logic in the `prune` function
}

impl IndexStoreTables {
    fn open<P: Into<PathBuf>>(path: P) -> Self {
        IndexStoreTables::open_tables_read_write(
            path.into(),
            MetricConf::new("rest-index"),
            None,
            None,
        )
    }

    fn needs_to_do_initialization(&self, checkpoint_store: &CheckpointStore) -> bool {
        (match self.meta.get(&()) {
            Ok(Some(metadata)) => metadata.version != CURRENT_DB_VERSION,
            Ok(None) => true,
            Err(_) => true,
        }) || self.is_indexed_watermark_out_of_date(checkpoint_store)
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

    #[tracing::instrument(skip_all)]
    fn init(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        epoch_store: &AuthorityPerEpochStore,
        package_store: &Arc<dyn BackingPackageStore + Send + Sync>,
    ) -> Result<(), StorageError> {
        info!("Initializing REST indexes");

        let highest_executed_checkpoint =
            checkpoint_store.get_highest_executed_checkpoint_seq_number()?;
        let lowest_available_checkpoint = checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()?
            .map(|c| c.saturating_add(1))
            .unwrap_or(0);
        let lowest_available_checkpoint_objects = authority_store
            .perpetual_tables
            .get_highest_pruned_checkpoint()?
            .map(|c| c.saturating_add(1))
            .unwrap_or(0);

        // Doing backfill requires processing objects so we have to restrict our
        // backfill range to the range of checkpoints that we have objects for.
        let lowest_available_checkpoint =
            lowest_available_checkpoint.max(lowest_available_checkpoint_objects);

        let checkpoint_range = highest_executed_checkpoint.map(|highest_executed_checkpoint| {
            lowest_available_checkpoint..=highest_executed_checkpoint
        });

        if let Some(checkpoint_range) = checkpoint_range {
            self.index_existing_transactions(authority_store, checkpoint_store, checkpoint_range)?;
        }

        self.initialize_current_epoch(authority_store, checkpoint_store)?;

        let coin_index = Mutex::new(HashMap::new());
        let coin_v2_index = Mutex::new(HashMap::new());

        let make_live_object_indexer = RestParLiveObjectSetIndexer {
            tables: self,
            coin_index: &coin_index,
            coin_v2_index: &coin_v2_index,
            epoch_store,
            package_store,
        };

        crate::par_index_live_object_set::par_index_live_object_set(
            authority_store,
            &make_live_object_indexer,
        )?;

        self.coin.multi_insert(coin_index.into_inner().unwrap())?;
        self.coin_v2
            .multi_insert(coin_v2_index.into_inner().unwrap())?;

        self.watermark.insert(
            &Watermark::Indexed,
            &highest_executed_checkpoint.unwrap_or(0),
        )?;

        // Mark the new backfill-only tables as complete: a full init populates
        // them via par_index_live_object_set, so no background backfill needed.
        self.watermark
            .insert(&Watermark::PackageVersionBackfilled, &0u64)?;
        self.watermark.insert(&Watermark::CoinV2Backfilled, &0u64)?;
        self.watermark
            .insert(&Watermark::OwnerV2Backfilled, &0u64)?;

        self.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )?;

        info!("Finished initializing REST indexes");

        Ok(())
    }

    #[tracing::instrument(skip(self, authority_store, checkpoint_store))]
    fn index_existing_transactions(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
        checkpoint_range: std::ops::RangeInclusive<u64>,
    ) -> Result<(), StorageError> {
        info!(
            "Indexing {} checkpoints in range {checkpoint_range:?}",
            checkpoint_range.size_hint().0
        );
        let start_time = Instant::now();

        checkpoint_range.into_par_iter().try_for_each(|seq| {
            let checkpoint_data =
                sparse_checkpoint_data_for_backfill(authority_store, checkpoint_store, seq)?;

            let mut batch = self.transactions.batch();

            self.index_epoch(&checkpoint_data, &mut batch)?;
            self.index_transactions(&checkpoint_data, &mut batch)?;

            batch.write().map_err(StorageError::from)
        })?;

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
        let mut batch = self.transactions.batch();

        let transactions_to_prune = checkpoint_contents_to_prune
            .iter()
            .flat_map(|contents| contents.iter().map(|digests| digests.transaction));

        batch.delete_batch(&self.transactions, transactions_to_prune)?;
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
        resolver: &mut dyn LayoutResolver,
    ) -> Result<typed_store::rocks::DBBatch, StorageError> {
        debug!(
            checkpoint = checkpoint.checkpoint_summary.sequence_number,
            "indexing checkpoint"
        );

        let mut batch = self.transactions.batch();

        self.index_epoch(checkpoint, &mut batch)?;
        self.index_transactions(checkpoint, &mut batch)?;
        self.index_objects(checkpoint, resolver, &mut batch)?;

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
        let Some(epoch_info) = checkpoint.epoch_info()? else {
            return Ok(());
        };

        // We need to handle closing the previous epoch by updating the entry for it, if
        // it exists.
        if epoch_info.epoch > 0 {
            let prev_epoch = epoch_info.epoch - 1;

            if let Some(mut previous_epoch) = self.epochs.get(&prev_epoch)? {
                previous_epoch.end_timestamp_ms = Some(epoch_info.start_timestamp_ms);
                previous_epoch.end_checkpoint = Some(epoch_info.start_checkpoint - 1);
                batch.insert_batch(&self.epochs, [(prev_epoch, previous_epoch)])?;
            }
        }

        // Insert the current epoch info
        batch.insert_batch(&self.epochs, [(epoch_info.epoch, epoch_info)])?;

        Ok(())
    }

    // After attempting to reindex past epochs, ensure that the current epoch is at
    // least partially initialized
    fn initialize_current_epoch(
        &mut self,
        authority_store: &AuthorityStore,
        checkpoint_store: &CheckpointStore,
    ) -> Result<(), StorageError> {
        let Some(checkpoint) = checkpoint_store.get_highest_executed_checkpoint()? else {
            return Ok(());
        };

        if self.epochs.get(&checkpoint.epoch)?.is_some() {
            // no need to initialize if it already exists
            return Ok(());
        }

        let system_state = iota_types::iota_system_state::get_iota_system_state(authority_store)
            .map_err(|e| StorageError::custom(format!("Failed to find system state: {e}")))?;

        // Determine the start checkpoint of the current epoch
        let start_checkpoint = if checkpoint.epoch != 0 {
            let previous_epoch = checkpoint.epoch - 1;

            // Find the last checkpoint of the previous epoch
            if let Some(previous_epoch_info) = self.epochs.get(&previous_epoch)? {
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

        let epoch_info = EpochInfo {
            epoch: checkpoint.epoch,
            protocol_version: system_state.protocol_version(),
            start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
            end_timestamp_ms: None,
            start_checkpoint,
            end_checkpoint: None,
            reference_gas_price: system_state.reference_gas_price(),
            system_state,
        };

        self.epochs.insert(&epoch_info.epoch, &epoch_info)?;

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
        for seq in (0..=current_checkpoint_seq_number).rev() {
            let Some(chkpt) = checkpoint_store
                .get_checkpoint_by_sequence_number(seq)
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
        checkpoint: &CheckpointData,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        for tx in &checkpoint.transactions {
            let info = TransactionInfo::new(
                &tx.input_objects,
                &tx.output_objects,
                checkpoint.checkpoint_summary.sequence_number,
            );

            let digest = tx.transaction.digest();
            batch.insert_batch(&self.transactions, [(digest, info)])?;
        }

        Ok(())
    }

    fn index_objects(
        &self,
        checkpoint: &CheckpointData,
        resolver: &mut dyn LayoutResolver,
        batch: &mut typed_store::rocks::DBBatch,
    ) -> Result<(), StorageError> {
        let mut coin_index: HashMap<CoinIndexKey, CoinIndexInfo> = HashMap::new();
        let mut coin_v2_index: HashMap<CoinIndexKey, CoinIndexInfoV2> = HashMap::new();

        for tx in &checkpoint.transactions {
            // determine changes from removed objects
            for removed_object in tx.removed_objects_pre_version() {
                match removed_object.owner() {
                    Owner::AddressOwner(address) => {
                        let owner_key = OwnerIndexKey::new(*address, removed_object.id());
                        batch.delete_batch(&self.owner, [owner_key])?;

                        // owner_v2: delete old entry
                        if let Some((v2_key, _)) = make_owner_v2_key(*address, removed_object) {
                            batch.delete_batch(&self.owner_v2, [v2_key])?;
                        }
                    }
                    Owner::ObjectOwner(object_id) => {
                        batch.delete_batch(
                            &self.dynamic_field,
                            [DynamicFieldKey::new(*object_id, removed_object.id())],
                        )?;
                    }
                    Owner::Shared { .. } | Owner::Immutable => {}
                }
            }

            // determine changes from changed objects
            for (object, old_object) in tx.changed_objects() {
                if let Some(old_object) = old_object {
                    match old_object.owner() {
                        Owner::AddressOwner(address) => {
                            let owner_key = OwnerIndexKey::new(*address, old_object.id());
                            batch.delete_batch(&self.owner, [owner_key])?;

                            // owner_v2: delete old entry
                            if let Some((v2_key, _)) = make_owner_v2_key(*address, old_object) {
                                batch.delete_batch(&self.owner_v2, [v2_key])?;
                            }
                        }

                        Owner::ObjectOwner(object_id) => {
                            if old_object.owner() != object.owner() {
                                batch.delete_batch(
                                    &self.dynamic_field,
                                    [DynamicFieldKey::new(*object_id, old_object.id())],
                                )?;
                            }
                        }

                        Owner::Shared { .. } | Owner::Immutable => {}
                    }
                }

                match object.owner() {
                    Owner::AddressOwner(owner) => {
                        let owner_key = OwnerIndexKey::new(*owner, object.id());
                        let owner_info = OwnerIndexInfo::new(object);
                        batch.insert_batch(&self.owner, [(owner_key, owner_info)])?;

                        // owner_v2 index
                        if let Some((v2_key, v2_info)) = make_owner_v2_key(*owner, object) {
                            batch.insert_batch(&self.owner_v2, [(v2_key, v2_info)])?;
                        }
                    }
                    Owner::ObjectOwner(parent) => {
                        if let Some(field_info) = try_create_dynamic_field_info(object, resolver)? {
                            let field_key = DynamicFieldKey::new(*parent, object.id());

                            batch.insert_batch(&self.dynamic_field, [(field_key, field_info)])?;
                        }
                    }
                    Owner::Shared { .. } | Owner::Immutable => {}
                }
            }

            // coin indexing
            //
            // coin indexing relies on the fact that CoinMetadata and TreasuryCap are
            // created in the same transaction so we don't need to worry about
            // overriding any older value that may exist in the database
            // (because there necessarily cannot be).
            for (key, value) in tx.created_objects().flat_map(try_create_coin_index_info) {
                use std::collections::hash_map::Entry;

                merge_coin_into_v2(
                    &mut coin_v2_index,
                    key.clone(),
                    CoinIndexInfoV2::from(value.clone()),
                );

                match coin_index.entry(key) {
                    Entry::Occupied(mut o) => o.get_mut().merge(value),
                    Entry::Vacant(v) => {
                        v.insert(value);
                    }
                }
            }
        }

        batch.insert_batch(&self.coin, coin_index)?;
        batch.insert_batch(&self.coin_v2, coin_v2_index)?;

        // package version + regulated coin → coin_v2 indexing
        // Both use created_objects(): packages and RegulatedCoinMetadata objects are
        // always created, never mutated in-place, so changed_objects() would only add
        // noise from unrelated object mutations.
        let mut package_version_index: Vec<(PackageVersionKey, PackageVersionInfo)> = Vec::new();
        let mut regulated_coin_v2_keys: Vec<(CoinIndexKey, ObjectID)> = Vec::new();
        for tx in &checkpoint.transactions {
            for object in tx.created_objects() {
                if let Some((key, info)) = try_create_package_version_info(object) {
                    package_version_index.push((key, info));
                }
                if let Some((key, object_id)) = try_create_regulated_coin_info(object) {
                    regulated_coin_v2_keys.push((key, object_id));
                }
            }
        }
        batch.insert_batch(&self.package_version, package_version_index)?;
        // Merge regulated coin entries into coin_v2.
        // These are rare (at most one per regulated coin type per checkpoint),
        // so read-modify-write is acceptable.
        for (key, object_id) in regulated_coin_v2_keys {
            read_merge_write_coin_v2(&self.coin_v2, batch, key, |v2| {
                v2.regulated_coin_metadata_object_id = Some(object_id);
            })?;
        }

        Ok(())
    }

    // only used in "grpc-server"
    fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfo>, TypedStoreError> {
        self.epochs.get(&epoch)
    }

    // used in both "grpc-server" and "rest-api"
    fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        self.transactions.get(digest)
    }

    // only used in "rest-api"
    fn owner_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKey, OwnerIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        let lower_bound = OwnerIndexKey::new(owner, cursor.unwrap_or(ObjectID::ZERO));
        let upper_bound = OwnerIndexKey::new(owner, ObjectID::MAX);
        Ok(self
            .owner
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound)))
    }

    // only used in "grpc-server"
    fn owner_v2_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        type_filter: OwnerV2TypeFilter,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKeyV2, OwnerIndexInfoV2), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        let (lower_bound, upper_bound) = owner_v2_bounds(owner, cursor, &type_filter);
        Ok(self
            .owner_v2
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound))
            .filter(move |result| match result {
                // Post-filter out hash collisions based on the full `StructTag` stored in the
                // value.
                Ok((_, info)) => match &type_filter {
                    OwnerV2TypeFilter::None => true,
                    OwnerV2TypeFilter::BaseType { tag, .. } => {
                        info.object_type.address == tag.address
                            && info.object_type.module == tag.module
                            && info.object_type.name == tag.name
                    }
                    OwnerV2TypeFilter::ExactType { tag, .. } => info.object_type == *tag,
                },
                // Don't filter out DB errors — let them pass through to the caller.
                Err(_) => true,
            }))
    }

    // used in both "grpc-server" and "rest-api"
    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> Result<
        impl Iterator<Item = Result<(DynamicFieldKey, DynamicFieldIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        let lower_bound = DynamicFieldKey::new(parent, cursor.unwrap_or(ObjectID::ZERO));
        let upper_bound = DynamicFieldKey::new(parent, ObjectID::MAX);
        let iter = self
            .dynamic_field
            .safe_iter_with_bounds(Some(lower_bound), Some(upper_bound));
        Ok(iter)
    }

    // only used in "rest-api"
    fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfo>, TypedStoreError> {
        let key = CoinIndexKey {
            coin_type: coin_type.to_owned(),
        };
        self.coin.get(&key)
    }

    // only used in "grpc-server"
    fn get_coin_v2_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfoV2>, TypedStoreError> {
        let key = CoinIndexKey {
            coin_type: coin_type.to_owned(),
        };
        self.coin_v2.get(&key)
    }

    // only used in "grpc-server"
    // Note: bounds are inclusive (same as `owner_iter` / `dynamic_field_iter`).
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

pub struct RestIndexStore {
    tables: Arc<IndexStoreTables>,
    pending_updates: Mutex<BTreeMap<u64, typed_store::rocks::DBBatch>>,
    /// Set to `true` once the `package_version` table backfill completes.
    package_version_ready: Arc<AtomicBool>,
    /// Set to `true` once the `coin_v2` table backfill completes.
    coin_v2_ready: Arc<AtomicBool>,
    /// Set to `true` once the `owner_v2` table backfill completes.
    owner_v2_ready: Arc<AtomicBool>,
}

impl RestIndexStore {
    pub async fn new(
        path: PathBuf,
        authority_store: Arc<AuthorityStore>,
        checkpoint_store: &CheckpointStore,
        epoch_store: &AuthorityPerEpochStore,
        package_store: &Arc<dyn BackingPackageStore + Send + Sync>,
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
                        .expect("unable to destroy old rpc-index db");
                    IndexStoreTables::open(path)
                };

                tables
                    .init(
                        &authority_store,
                        checkpoint_store,
                        epoch_store,
                        package_store,
                    )
                    .expect("unable to initialize rest index from live object set");
                tables
            } else {
                tables
            }
        };

        let tables = Arc::new(tables);

        // Check whether the backfill-only tables have been populated.  After a
        // full `init()` the watermarks are written, so nodes that just ran init
        // won't spawn any background tasks.  Upgrading nodes that already have
        // DB version 1 but never ran the new init will have the watermarks
        // absent and will spawn background backfills.
        let pkg_done = tables
            .watermark
            .get(&Watermark::PackageVersionBackfilled)
            .ok()
            .flatten()
            .is_some();
        let coin_v2_done = tables
            .watermark
            .get(&Watermark::CoinV2Backfilled)
            .ok()
            .flatten()
            .is_some();
        let owner_v2_done = tables
            .watermark
            .get(&Watermark::OwnerV2Backfilled)
            .ok()
            .flatten()
            .is_some();

        let package_version_ready = Arc::new(AtomicBool::new(pkg_done));
        let coin_v2_ready = Arc::new(AtomicBool::new(coin_v2_done));
        let owner_v2_ready = Arc::new(AtomicBool::new(owner_v2_done));

        if !pkg_done || !coin_v2_done || !owner_v2_done {
            let tables_clone = Arc::clone(&tables);
            let auth_clone = Arc::clone(&authority_store);
            let pkg_flag = Arc::clone(&package_version_ready);
            let coin_v2_flag = Arc::clone(&coin_v2_ready);
            let owner_v2_flag = Arc::clone(&owner_v2_ready);
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(move || {
                    backfill_new_tables(
                        &tables_clone,
                        &auth_clone,
                        &[
                            BackfillTask {
                                needed: !pkg_done,
                                done_flag: &pkg_flag,
                                watermark: Watermark::PackageVersionBackfilled,
                            },
                            BackfillTask {
                                needed: !coin_v2_done,
                                done_flag: &coin_v2_flag,
                                watermark: Watermark::CoinV2Backfilled,
                            },
                            BackfillTask {
                                needed: !owner_v2_done,
                                done_flag: &owner_v2_flag,
                                watermark: Watermark::OwnerV2Backfilled,
                            },
                        ],
                    );
                })
                .await
                {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::error!("background backfill task panicked: {e}");
                    }
                }
            });
        }

        Self {
            tables,
            pending_updates: Default::default(),
            package_version_ready,
            coin_v2_ready,
            owner_v2_ready,
        }
    }

    pub fn new_without_init(path: PathBuf) -> Self {
        let tables = Arc::new(IndexStoreTables::open(path));

        Self {
            tables,
            pending_updates: Default::default(),
            // new_without_init is used in tests / tooling — mark all tables
            // as ready so callers don't get spurious "backfill in progress"
            // errors.
            package_version_ready: Arc::new(AtomicBool::new(true)),
            coin_v2_ready: Arc::new(AtomicBool::new(true)),
            owner_v2_ready: Arc::new(AtomicBool::new(true)),
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
    pub fn index_checkpoint(&self, checkpoint: &CheckpointData, resolver: &mut dyn LayoutResolver) {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        let batch = self
            .tables
            .index_checkpoint(checkpoint, resolver)
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

        Ok(batch.write()?)
    }

    // only used in "grpc-server"
    pub fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfo>, TypedStoreError> {
        self.tables.get_epoch_info(epoch)
    }

    // used in both "grpc-server" and "rest-api"
    pub fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionInfo>, TypedStoreError> {
        self.tables.get_transaction_info(digest)
    }

    // only used in "rest-api"
    pub fn owner_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKey, OwnerIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        self.tables.owner_iter(owner, cursor)
    }

    // only used in "grpc-server"
    pub fn owner_v2_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        type_filter: OwnerV2TypeFilter,
    ) -> Result<
        impl Iterator<Item = Result<(OwnerIndexKeyV2, OwnerIndexInfoV2), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        self.tables.owner_v2_iter(owner, cursor, type_filter)
    }

    // used in both "grpc-server" and "rest-api"
    pub fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> Result<
        impl Iterator<Item = Result<(DynamicFieldKey, DynamicFieldIndexInfo), TypedStoreError>> + '_,
        TypedStoreError,
    > {
        self.tables.dynamic_field_iter(parent, cursor)
    }

    // used in both "grpc-server" and "rest-api"
    pub fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfo>, TypedStoreError> {
        self.tables.get_coin_info(coin_type)
    }

    // only used in "grpc-server"
    pub fn get_coin_v2_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<CoinIndexInfoV2>, TypedStoreError> {
        self.tables.get_coin_v2_info(coin_type)
    }

    // only used in "grpc-server"
    pub fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> Result<impl Iterator<Item = PackageVersionIteratorItem> + '_, TypedStoreError> {
        self.tables
            .package_versions_iter(original_package_id, cursor)
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    pub fn is_coin_v2_index_ready(&self) -> bool {
        self.coin_v2_ready.load(Ordering::Acquire)
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    pub fn is_owner_v2_index_ready(&self) -> bool {
        self.owner_v2_ready.load(Ordering::Acquire)
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    pub fn is_package_version_index_ready(&self) -> bool {
        self.package_version_ready.load(Ordering::Acquire)
    }
}

fn try_create_dynamic_field_info(
    object: &Object,
    resolver: &mut dyn LayoutResolver,
) -> Result<Option<DynamicFieldIndexInfo>, StorageError> {
    // Skip if not a move object
    let Some(move_object) = object.data.try_as_move() else {
        return Ok(None);
    };

    // Skip any objects that aren't of type `Field<Name, Value>`
    //
    // All dynamic fields are of type:
    //   - Field<Name, Value> for dynamic fields
    //   - Field<Wrapper<Name, ID>> for dynamic field objects where the ID is the id
    //     of the pointed
    //   to object
    //
    if !move_object.type_().is_dynamic_field() {
        return Ok(None);
    }

    let layout = resolver
        .get_annotated_layout(&move_object.type_().clone().into())
        .map_err(StorageError::custom)?
        .into_layout();

    let field = DFV::FieldVisitor::deserialize(move_object.contents(), &layout)
        .map_err(StorageError::custom)?;

    let value_metadata = field.value_metadata().map_err(StorageError::custom)?;

    Ok(Some(DynamicFieldIndexInfo {
        name_type: field.name_layout.into(),
        name_value: field.name_bytes.to_owned(),
        dynamic_field_type: field.kind,
        dynamic_object_id: if let DFV::ValueMetadata::DynamicObjectField(id) = value_metadata {
            Some(id)
        } else {
            None
        },
    }))
}

fn try_create_coin_index_info(object: &Object) -> Option<(CoinIndexKey, CoinIndexInfo)> {
    use iota_types::coin::{CoinMetadata, TreasuryCap};

    let object_type = object.type_()?.other()?;

    if let Some(coin_type) = CoinMetadata::is_coin_metadata_with_coin_type(object_type).cloned() {
        return Some((
            CoinIndexKey { coin_type },
            CoinIndexInfo {
                coin_metadata_object_id: Some(object.id()),
                treasury_object_id: None,
            },
        ));
    }

    if let Some(coin_type) = TreasuryCap::is_treasury_with_coin_type(object_type).cloned() {
        return Some((
            CoinIndexKey { coin_type },
            CoinIndexInfo {
                coin_metadata_object_id: None,
                treasury_object_id: Some(object.id()),
            },
        ));
    }

    None
}

/// Returns `(CoinIndexKey, regulated_coin_metadata_object_id)` if `object` is
/// a `RegulatedCoinMetadata<T>`.  Used to populate the `coin_v2` table.
fn try_create_regulated_coin_info(object: &Object) -> Option<(CoinIndexKey, ObjectID)> {
    use move_core_types::language_storage::TypeTag;

    let move_object_type = object.type_()?;
    if !move_object_type.is_regulated_coin_metadata() {
        return None;
    }
    let object_type = move_object_type.other()?;
    // RegulatedCoinMetadata<T> has one type parameter: the coin type
    let coin_type = match object_type.type_params.first()? {
        TypeTag::Struct(s) => *s.clone(),
        _ => return None,
    };
    Some((CoinIndexKey { coin_type }, object.id()))
}

fn try_create_package_version_info(
    object: &Object,
) -> Option<(PackageVersionKey, PackageVersionInfo)> {
    let package = object.data.try_as_package()?;
    Some((
        PackageVersionKey {
            original_package_id: package.original_package_id(),
            version: object.version().value(),
        },
        PackageVersionInfo {
            storage_id: object.id(),
        },
    ))
}

struct RestParLiveObjectSetIndexer<'a> {
    tables: &'a IndexStoreTables,
    coin_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
    coin_v2_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfoV2>>,
    epoch_store: &'a AuthorityPerEpochStore,
    package_store: &'a Arc<dyn BackingPackageStore + Send + Sync>,
}

struct RestLiveObjectIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: typed_store::rocks::DBBatch,
    coin_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>,
    coin_v2_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfoV2>>,
    resolver: Box<dyn LayoutResolver + 'a>,
}

impl<'a> ParMakeLiveObjectIndexer for RestParLiveObjectSetIndexer<'a> {
    type ObjectIndexer = RestLiveObjectIndexer<'a>;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer {
        RestLiveObjectIndexer {
            tables: self.tables,
            batch: self.tables.owner.batch(),
            coin_index: self.coin_index,
            coin_v2_index: self.coin_v2_index,
            resolver: self
                .epoch_store
                .executor()
                .type_layout_resolver(Box::new(self.package_store)),
        }
    }
}

impl LiveObjectIndexer for RestLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        match object.owner {
            // Owner Index (legacy REST + new owner_v2 for gRPC)
            Owner::AddressOwner(owner) => {
                let owner_key = OwnerIndexKey::new(owner, object.id());
                let owner_info = OwnerIndexInfo::new(&object);
                self.batch
                    .insert_batch(&self.tables.owner, [(owner_key, owner_info)])?;

                // owner_v2 index
                if let Some((v2_key, v2_info)) = make_owner_v2_key(owner, &object) {
                    self.batch
                        .insert_batch(&self.tables.owner_v2, [(v2_key, v2_info)])?;
                }
            }

            // Dynamic Field Index
            Owner::ObjectOwner(parent) => {
                if let Some(field_info) =
                    try_create_dynamic_field_info(&object, self.resolver.as_mut())?
                {
                    let field_key = DynamicFieldKey::new(parent, object.id());

                    self.batch
                        .insert_batch(&self.tables.dynamic_field, [(field_key, field_info)])?;
                }
            }

            Owner::Shared { .. } | Owner::Immutable => {}
        }

        // Look for CoinMetadata<T> and TreasuryCap<T> objects
        if let Some((key, value)) = try_create_coin_index_info(&object) {
            use std::collections::hash_map::Entry;

            merge_coin_into_v2(
                &mut self.coin_v2_index.lock().unwrap(),
                key.clone(),
                CoinIndexInfoV2::from(value.clone()),
            );

            match self.coin_index.lock().unwrap().entry(key) {
                Entry::Occupied(mut o) => o.get_mut().merge(value),
                Entry::Vacant(v) => {
                    v.insert(value);
                }
            }
        }

        // Package version index
        if let Some((key, info)) = try_create_package_version_info(&object) {
            self.batch
                .insert_batch(&self.tables.package_version, [(key, info)])?;
        }

        // Regulated coin index (coin_v2 only)
        if let Some((key, object_id)) = try_create_regulated_coin_info(&object) {
            merge_coin_into_v2(
                &mut self.coin_v2_index.lock().unwrap(),
                key,
                CoinIndexInfoV2 {
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
// Background backfill infrastructure
//
// When a new index table is added without bumping CURRENT_DB_VERSION, existing
// nodes will have an empty table.  The functions below scan the live object set
// in the background and populate the table, then write a Watermark entry so the
// backfill is not repeated on the next restart.
// ---------------------------------------------------------------------------

/// Combined backfill indexer that populates `package_version`, `coin_v2`,
/// and `owner_v2` tables in a single pass over the live object set.
///
/// `coin_v2` entries are accumulated in a shared `Mutex<HashMap>` (like the
/// full-init path) to avoid lost-update races when parallel workers encounter
/// `CoinMetadata` and `TreasuryCap` for the same coin type in different
/// ObjectID ranges.
struct BackfillIndexer<'a> {
    tables: &'a IndexStoreTables,
    coin_v2_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfoV2>>,
    backfill_package_version: bool,
    backfill_coin_v2: bool,
    backfill_owner_v2: bool,
}

struct BackfillBatchIndexer<'a> {
    tables: &'a IndexStoreTables,
    batch: typed_store::rocks::DBBatch,
    coin_v2_index: &'a Mutex<HashMap<CoinIndexKey, CoinIndexInfoV2>>,
    backfill_package_version: bool,
    backfill_coin_v2: bool,
    backfill_owner_v2: bool,
}

impl<'a> ParMakeLiveObjectIndexer for BackfillIndexer<'a> {
    type ObjectIndexer = BackfillBatchIndexer<'a>;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer {
        BackfillBatchIndexer {
            batch: self.tables.package_version.batch(),
            tables: self.tables,
            coin_v2_index: self.coin_v2_index,
            backfill_package_version: self.backfill_package_version,
            backfill_coin_v2: self.backfill_coin_v2,
            backfill_owner_v2: self.backfill_owner_v2,
        }
    }
}

impl LiveObjectIndexer for BackfillBatchIndexer<'_> {
    fn index_object(&mut self, object: Object) -> Result<(), StorageError> {
        if self.backfill_package_version {
            if let Some((key, info)) = try_create_package_version_info(&object) {
                self.batch
                    .insert_batch(&self.tables.package_version, [(key, info)])?;
            }
        }
        if self.backfill_coin_v2 {
            if let Some((key, value)) = try_create_coin_index_info(&object) {
                merge_coin_into_v2(
                    &mut self.coin_v2_index.lock().unwrap(),
                    key,
                    CoinIndexInfoV2::from(value),
                );
            }
            if let Some((key, object_id)) = try_create_regulated_coin_info(&object) {
                merge_coin_into_v2(
                    &mut self.coin_v2_index.lock().unwrap(),
                    key,
                    CoinIndexInfoV2 {
                        regulated_coin_metadata_object_id: Some(object_id),
                        ..Default::default()
                    },
                );
            }
        }
        if self.backfill_owner_v2 {
            if let Owner::AddressOwner(owner) = object.owner {
                if let Some((key, info)) = make_owner_v2_key(owner, &object) {
                    self.batch
                        .insert_batch(&self.tables.owner_v2, [(key, info)])?;
                }
            }
        }
        // If the batch size grows to greater that 128MB then write out to the DB so
        // that the data we need to hold in memory doesn't grown unbounded.
        if self.batch.size_in_bytes() >= 1 << 27 {
            std::mem::replace(&mut self.batch, self.tables.package_version.batch()).write()?;
        }
        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        self.batch.write()?;
        Ok(())
    }
}

/// Describes a single backfill-only table that needs populating.
struct BackfillTask<'a> {
    needed: bool,
    done_flag: &'a AtomicBool,
    watermark: Watermark,
}

/// Run a single pass over the live object set, populating whichever of the
/// backfill-only tables still need populating.
fn backfill_new_tables(
    tables: &IndexStoreTables,
    authority_store: &AuthorityStore,
    tasks: &[BackfillTask<'_>],
) {
    let (mut backfill_package_version, mut backfill_coin_v2, mut backfill_owner_v2) =
        (false, false, false);
    for task in tasks {
        if !task.needed {
            continue;
        }
        match task.watermark {
            Watermark::PackageVersionBackfilled => backfill_package_version = true,
            Watermark::CoinV2Backfilled => backfill_coin_v2 = true,
            Watermark::OwnerV2Backfilled => backfill_owner_v2 = true,
            _ => {}
        }
    }

    info!(
        "Starting background backfill (package_version={backfill_package_version}, \
         coin_v2={backfill_coin_v2}, owner_v2={backfill_owner_v2})"
    );

    let coin_v2_index = Mutex::new(HashMap::new());

    let indexer = BackfillIndexer {
        tables,
        coin_v2_index: &coin_v2_index,
        backfill_package_version,
        backfill_coin_v2,
        backfill_owner_v2,
    };

    match crate::par_index_live_object_set::par_index_live_object_set(authority_store, &indexer) {
        Ok(()) => {
            // Flush coin_v2 entries accumulated in memory to the DB.
            //
            // Use per-key read-merge-write instead of `multi_insert` to
            // avoid clobbering concurrent incremental writes.  While the
            // backfill was scanning the live object set, the incremental
            // checkpoint indexer may have written
            // `regulated_coin_metadata_object_id` (or other fields) for
            // the same coin type. A plain `multi_insert` would overwrite
            // those with the backfill's snapshot (which lacks the new
            // data).  Merging preserves whichever fields are already
            // present in the DB.
            //
            // Each key is read-merged-written individually so that the
            // TOCTOU window is per-key (microseconds) rather than across
            // the entire flush.
            if backfill_coin_v2 {
                for (key, backfill_value) in coin_v2_index.into_inner().unwrap() {
                    let mut existing = tables.coin_v2.get(&key).ok().flatten().unwrap_or_default();
                    existing.merge(backfill_value);
                    if let Err(e) = tables.coin_v2.insert(&key, &existing) {
                        tracing::error!("Failed to flush coin_v2 entry: {e}");
                        return;
                    }
                }
            }

            for task in tasks {
                if !task.needed {
                    continue;
                }
                if let Err(e) = tables.watermark.insert(&task.watermark, &0u64) {
                    tracing::error!("Failed to write {:?} watermark: {e}", task.watermark);
                    return;
                }
                task.done_flag.store(true, Ordering::Release);
                info!("{:?} backfill complete", task.watermark);
            }
        }
        Err(e) => tracing::error!("background backfill failed: {e}"),
    }
}

// ---------------------------------------------------------------------------

// TODO figure out a way to dedup this logic. Today we'd need to do quite a bit
// of refactoring to make it possible.
//
// Load a CheckpointData struct without event data
fn sparse_checkpoint_data_for_backfill(
    authority_store: &AuthorityStore,
    checkpoint_store: &CheckpointStore,
    checkpoint: u64,
) -> Result<CheckpointData, StorageError> {
    use iota_types::full_checkpoint_content::CheckpointTransaction;

    let summary = checkpoint_store
        .get_checkpoint_by_sequence_number(checkpoint)?
        .ok_or_else(|| StorageError::missing(format!("missing checkpoint {checkpoint}")))?;
    let contents = checkpoint_store
        .get_checkpoint_contents(&summary.content_digest)?
        .ok_or_else(|| StorageError::missing(format!("missing checkpoint {checkpoint}")))?;

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
