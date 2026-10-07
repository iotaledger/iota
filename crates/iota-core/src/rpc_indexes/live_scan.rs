// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Fills the RPC index store's live-state tables from a stream of live
//! objects: during a rebuild from the local live object set
//! ([`LiveObjectSetIndexer`]) and during a formal-snapshot restore
//! ([`RpcIndexesRestorer`]). Both use one indexer per partition of the stream.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

use iota_sdk_types::Owner;
use iota_types::{
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    storage::{DynamicFieldKey, error::Error as StorageError},
};
use parking_lot::Mutex;
use typed_store::{
    TypedStoreError,
    database::wait_for_database_close,
    rocks::{DBBatch, bulk_ingestion_options, bulk_ingestion_write_options},
    traits::Map,
};

use super::{
    RpcIndexesStore,
    schema::{
        CURRENT_DB_VERSION, CoinIndexInfo, CoinIndexKey, IndexGroup, IndexStoreTables,
        MetadataInfo, OwnerIndexKey, is_dynamic_field, merge_coin_into, try_create_coin_index_info,
        try_create_package_version_info, try_create_regulated_coin_info,
    },
};

/// The coin metadata of the objects seen so far. A coin type's metadata,
/// treasury and regulated metadata objects may land in different partitions,
/// so the rows are written once every partition has been indexed.
type CoinMetadata = Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>;

/// Indexes the live object set into the tables of `groups`, one indexer per
/// scan partition. [`Self::finish`] must run once after every partition.
pub(super) struct LiveObjectSetIndexer<'a> {
    tables: &'a IndexStoreTables,
    groups: &'a BTreeSet<IndexGroup>,
    coin_metadata: CoinMetadata,
    batch_size_limit: usize,
}

impl<'a> LiveObjectSetIndexer<'a> {
    pub(super) fn new(
        tables: &'a IndexStoreTables,
        groups: &'a BTreeSet<IndexGroup>,
        batch_size_limit: usize,
    ) -> Self {
        Self {
            tables,
            groups,
            coin_metadata: Mutex::new(HashMap::new()),
            batch_size_limit,
        }
    }

    /// Writes the coin metadata gathered across the partitions.
    pub(super) fn finish(self) -> Result<(), TypedStoreError> {
        write_coin_metadata(self.tables, self.coin_metadata)
    }
}

impl crate::par_index_live_object_set::ParMakeLiveObjectIndexer for LiveObjectSetIndexer<'_> {
    type ObjectIndexer<'a>
        = RpcIndexesPartitionIndexer<'a>
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        RpcIndexesPartitionIndexer::new(
            self.tables,
            self.groups,
            &self.coin_metadata,
            self.batch_size_limit,
        )
    }
}

/// Indexes one partition, writing its rows out in batches of bounded size.
pub struct RpcIndexesPartitionIndexer<'a> {
    tables: &'a IndexStoreTables,
    groups: &'a BTreeSet<IndexGroup>,
    coin_metadata: &'a CoinMetadata,
    batch: DBBatch,
    batch_size_limit: usize,
}

impl<'a> RpcIndexesPartitionIndexer<'a> {
    fn new(
        tables: &'a IndexStoreTables,
        groups: &'a BTreeSet<IndexGroup>,
        coin_metadata: &'a CoinMetadata,
        batch_size_limit: usize,
    ) -> Self {
        Self {
            tables,
            groups,
            coin_metadata,
            batch: tables.owner.batch(),
            batch_size_limit,
        }
    }

    /// Stages `object`'s rows in every table the store's groups maintain.
    pub fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                if let Some((key, info)) = OwnerIndexKey::for_object(owner, object) {
                    self.batch.insert_batch(&self.tables.owner, [(key, info)])?;
                }
            }
            Owner::Object(parent) => {
                if is_dynamic_field(object) {
                    self.batch.insert_batch(
                        &self.tables.dynamic_field,
                        [(DynamicFieldKey::new(parent, object.id()), ())],
                    )?;
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }

        if self.groups.contains(&IndexGroup::Grpc) {
            if let Some((key, info)) = try_create_coin_index_info(object) {
                merge_coin_into(&mut self.coin_metadata.lock(), key, info);
            }
            if let Some((key, object_id)) = try_create_regulated_coin_info(object) {
                merge_coin_into(
                    &mut self.coin_metadata.lock(),
                    key,
                    CoinIndexInfo {
                        regulated_coin_metadata_object_id: Some(object_id),
                        ..Default::default()
                    },
                );
            }
            if let Some((key, info)) = try_create_package_version_info(object) {
                self.batch
                    .insert_batch(&self.tables.package_version, [(key, info)])?;
            }
        }

        if self.batch.size_in_bytes() >= self.batch_size_limit {
            std::mem::replace(&mut self.batch, self.tables.owner.batch())
                .write_opt(&bulk_ingestion_write_options())?;
        }

        Ok(())
    }

    /// Writes this partition's remaining rows.
    pub fn finish(self) -> Result<(), StorageError> {
        self.batch.write_opt(&bulk_ingestion_write_options())?;
        Ok(())
    }
}

impl crate::par_index_live_object_set::LiveObjectIndexer for RpcIndexesPartitionIndexer<'_> {
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        RpcIndexesPartitionIndexer::index_object(self, object)
    }

    fn finish(self) -> Result<(), StorageError> {
        RpcIndexesPartitionIndexer::finish(self)
    }
}

/// Writes the coin metadata gathered across a stream's partitions. Must run
/// before the markers are written, so the flush that makes the build durable
/// covers these rows.
fn write_coin_metadata(
    tables: &IndexStoreTables,
    coin_metadata: CoinMetadata,
) -> Result<(), TypedStoreError> {
    tables.coin.multi_insert(coin_metadata.into_inner())
}

/// The RPC index tables opened for a formal-snapshot restore.
///
/// Feed the live objects, in any order, through [`Self::partition_indexer`],
/// then call [`Self::finalize`] so a node opens the store in place instead of
/// rebuilding it.
pub struct RpcIndexesRestorer {
    tables: IndexStoreTables,
    groups: BTreeSet<IndexGroup>,
    coin_metadata: CoinMetadata,
    batch_size_limit: usize,
}

impl RpcIndexesRestorer {
    /// Opens the store for bulk ingestion and records this schema version and
    /// `groups`. The watermark is written only by [`Self::finalize`], so a
    /// node opening the store of an interrupted restore rebuilds it.
    pub fn open(path: PathBuf, groups: BTreeSet<IndexGroup>) -> Result<Self, TypedStoreError> {
        let tables = IndexStoreTables::open_for_bulk_ingestion(path);
        tables.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
                groups: groups.clone(),
            },
        )?;
        Ok(Self {
            tables,
            groups,
            coin_metadata: Mutex::new(HashMap::new()),
            batch_size_limit: bulk_ingestion_options().batch_size_limit,
        })
    }

    /// Returns an indexer for one partition of the snapshot's live objects.
    pub fn partition_indexer(&self) -> RpcIndexesPartitionIndexer<'_> {
        RpcIndexesPartitionIndexer::new(
            &self.tables,
            &self.groups,
            &self.coin_metadata,
            self.batch_size_limit,
        )
    }

    /// Writes the markers at `restore_checkpoint`, the restore's highest
    /// executed checkpoint, flushes the bulk writes and closes the database,
    /// so the caller can move its directory once this returns.
    ///
    /// Callers must have restored the complete live object set first, through
    /// [`Self::partition_indexer`].
    pub async fn finalize(
        self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), StorageError> {
        let Self {
            tables,
            coin_metadata,
            ..
        } = self;
        write_coin_metadata(&tables, coin_metadata)?;
        tables.adopt_bulk_ingestion(Some(restore_checkpoint))?;

        let weak_db = Arc::downgrade(&tables.meta.db);
        drop(tables);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the RPC index database after the restore",
            ));
        }
        Ok(())
    }

    /// Reopens the finalized store the way a node does and reads back the
    /// markers and the live state, so a database the node would wipe and
    /// rebuild — or one that carries no restored objects — fails the restore
    /// instead. `live_object_count` is the number of objects the restore
    /// wrote. `historic_db_path` is the root of the store's history
    /// buckets, as for [`RpcIndexesStore::new`].
    ///
    /// Whichever groups the restore built, the `owner` table is the one every
    /// one of them fills, so it is what proves the object stream landed: the
    /// gRPC group's coin metadata and package versions depend on the snapshot
    /// carrying such objects at all.
    pub async fn verify_restored(
        path: &Path,
        historic_db_path: Option<&Path>,
        restore_checkpoint: CheckpointSequenceNumber,
        live_object_count: u64,
    ) -> Result<(), StorageError> {
        let reopened = RpcIndexesStore::open_index_db(
            path,
            &crate::epoch_buckets::historic_root(path, historic_db_path),
        )
        .map_err(|e| {
            StorageError::custom(format!(
                "unable to reopen the restored RPC index database: {e}"
            ))
        })?;
        let metadata = reopened.tables.meta.get(&())?.ok_or_else(|| {
            StorageError::custom("the restored RPC index database has no metadata")
        })?;
        if metadata.version != CURRENT_DB_VERSION {
            return Err(StorageError::custom(format!(
                "restored RPC index database version mismatch: expected {}, found {}",
                CURRENT_DB_VERSION, metadata.version
            )));
        }
        if metadata.groups.is_empty() {
            return Err(StorageError::custom(
                "the restored RPC index database records no API groups",
            ));
        }
        let watermark = reopened.tables.watermark.get(&())?;
        if watermark != Some(restore_checkpoint) {
            return Err(StorageError::custom(format!(
                "the restored RPC index is watermarked at {watermark:?}, expected \
                 {restore_checkpoint}"
            )));
        }
        // Not `is_empty`, which reads an unreadable table as non-empty.
        let owner_is_empty = reopened
            .tables
            .owner
            .safe_iter()
            .next()
            .transpose()?
            .is_none();
        if live_object_count > 0 && owner_is_empty {
            return Err(StorageError::custom(format!(
                "the restored RPC index has an empty owner index after {live_object_count} live \
                 objects"
            )));
        }

        let weak_db = Arc::downgrade(&reopened.tables.meta.db);
        drop(reopened);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the RPC index database after verifying the restore",
            ));
        }
        Ok(())
    }
}
