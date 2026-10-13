// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Fills the RPC index store's live-state tables from a stream of live
//! objects: during a rebuild from the local live object set
//! (`LiveObjectSetIndexer`) and during a formal-snapshot restore
//! ([`RpcIndexesRestorer`]). Both use one indexer per partition of the stream,
//! and both write the rows to SST files that are ingested once every
//! partition is done.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use iota_sdk_types::Owner;
use iota_types::{
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    storage::{
        DynamicFieldKey, PackageVersionInfo, PackageVersionKey, error::Error as StorageError,
    },
};
use parking_lot::Mutex;
use serde::Serialize;
use typed_store::{
    TypedStoreError,
    database::wait_for_database_close,
    rocks::{SstFileBuffer, bulk_ingestion_options},
    traits::Map,
};

use super::{
    RpcIndexesStore,
    schema::{
        CURRENT_DB_VERSION, CoinIndexInfo, CoinIndexKey, IndexGroup, IndexStoreTables,
        MetadataInfo, OwnerIndexInfo, OwnerIndexKey, is_dynamic_field, merge_coin_into,
        try_create_coin_index_info, try_create_package_version_info,
        try_create_regulated_coin_info,
    },
};

/// The coin metadata of the objects seen so far. A coin type's metadata,
/// treasury and regulated metadata objects may land in different partitions,
/// so the rows are written once every partition has been indexed.
type CoinMetadata = Mutex<HashMap<CoinIndexKey, CoinIndexInfo>>;

/// Indexes the live object set into the tables of `groups`, one indexer per
/// scan partition. [`Self::finish`] ingests the rows once every partition is
/// done.
pub(super) struct LiveObjectSetIndexer<'a> {
    tables: &'a IndexStoreTables,
    groups: &'a BTreeSet<IndexGroup>,
    coin_metadata: CoinMetadata,
    sst_files: SstFiles,
}

impl<'a> LiveObjectSetIndexer<'a> {
    pub(super) fn new(
        tables: &'a IndexStoreTables,
        groups: &'a BTreeSet<IndexGroup>,
        sst_files: SstFiles,
    ) -> Self {
        Self {
            tables,
            groups,
            coin_metadata: Mutex::new(HashMap::new()),
            sst_files,
        }
    }

    /// Ingests the partitions' SST files and writes the coin metadata
    /// gathered across them.
    pub(super) fn finish(self) -> Result<(), StorageError> {
        self.sst_files.ingest(self.tables)?;
        write_coin_metadata(self.tables, self.coin_metadata)?;
        Ok(())
    }
}

impl crate::par_index_live_object_set::ParMakeLiveObjectIndexer for LiveObjectSetIndexer<'_> {
    type ObjectIndexer<'a>
        = RpcIndexesPartitionIndexer<'a>
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        RpcIndexesPartitionIndexer::new(self.groups, &self.coin_metadata, &self.sst_files)
    }
}

/// Where a rebuild or a restore keeps its SST files inside the index
/// database's directory until it ingests them. One that stops before that
/// leaves them for the next, which clears the whole directory.
pub(super) const BULK_INGESTION_SST_DIR: &str = "bulk_ingestion";

/// The rows the partitions of a bulk ingestion index, written to SST files
/// and moved into the tables by [`Self::ingest`].
///
/// A partition takes a set of buffers from a shared pool, fills it, and hands
/// it back when it is done, for the next partition to keep filling. A buffer
/// is written to a file only once it is full, so the number of files follows
/// the amount of data rather than the number of partitions; and there are no
/// more buffers than partitions indexed at once.
pub(super) struct SstFiles {
    dir: PathBuf,
    /// The bulk ingestion options the tables are open with.
    options: typed_store::rocksdb::Options,
    /// How much memory one set of buffers may take before it is written out.
    buffer_limit: usize,
    next_file: AtomicUsize,
    /// The buffers no partition is filling right now.
    idle: Mutex<Vec<SstBuffers>>,
    owner: Mutex<Vec<PathBuf>>,
    dynamic_field: Mutex<Vec<PathBuf>>,
    package_version: Mutex<Vec<PathBuf>>,
}

impl SstFiles {
    /// For a bulk ingestion into the database at `db_path`, with as many
    /// partitions at once as the live object set scan runs.
    pub(super) fn new(db_path: &Path) -> Result<Self, StorageError> {
        Self::with_buffer_limit(
            db_path,
            bulk_ingestion_options().sst_buffer_limit(crate::par_index_live_object_set::TASKS),
        )
    }

    pub(super) fn with_buffer_limit(
        db_path: &Path,
        buffer_limit: usize,
    ) -> Result<Self, StorageError> {
        let dir = db_path.join(BULK_INGESTION_SST_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| {
            StorageError::custom(format!(
                "unable to create the SST file directory {dir:?}: {e}"
            ))
        })?;
        Ok(Self {
            dir,
            options: bulk_ingestion_options().column_family_options.options,
            buffer_limit,
            next_file: AtomicUsize::new(0),
            idle: Mutex::new(Vec::new()),
            owner: Mutex::new(Vec::new()),
            dynamic_field: Mutex::new(Vec::new()),
            package_version: Mutex::new(Vec::new()),
        })
    }

    fn take_buffers(&self) -> SstBuffers {
        self.idle.lock().pop().unwrap_or_default()
    }

    fn return_buffers(&self, buffers: SstBuffers) {
        self.idle.lock().push(buffers);
    }

    /// Writes `buffer` to a new file of `table`'s list, if it holds any rows.
    fn write<K: Serialize, V: Serialize>(
        &self,
        buffer: &mut SstFileBuffer<K, V>,
        table: &Mutex<Vec<PathBuf>>,
    ) -> Result<(), TypedStoreError> {
        if buffer.is_empty() {
            return Ok(());
        }
        let path = self.dir.join(format!(
            "{}.sst",
            self.next_file.fetch_add(1, Ordering::Relaxed)
        ));
        buffer.write_file(&path, &self.options)?;
        table.lock().push(path);
        Ok(())
    }

    /// Writes the rows still buffered, moves every file into its table and
    /// removes the directory. Every partition indexer must have been dropped.
    fn ingest(self, tables: &IndexStoreTables) -> Result<(), StorageError> {
        let idle = std::mem::take(&mut *self.idle.lock());
        let files = &self;
        // One thread per set, as after a rebuild every scan task leaves one.
        std::thread::scope(|scope| {
            idle.into_iter()
                .map(|mut buffers| scope.spawn(move || buffers.write(files)))
                .collect::<Vec<_>>()
                .into_iter()
                .try_for_each(|writer| {
                    writer.join().unwrap_or_else(|_| {
                        Err(TypedStoreError::RocksDB(
                            "an SST file writer thread panicked".to_string(),
                        ))
                    })
                })
        })?;
        tables.owner.ingest_sst_files(&self.owner.into_inner())?;
        tables
            .dynamic_field
            .ingest_sst_files(&self.dynamic_field.into_inner())?;
        tables
            .package_version
            .ingest_sst_files(&self.package_version.into_inner())?;
        std::fs::remove_dir_all(&self.dir).map_err(|e| {
            StorageError::custom(format!(
                "unable to remove the SST file directory {:?}: {e}",
                self.dir
            ))
        })
    }
}

/// One buffer per table.
#[derive(Default)]
struct SstBuffers {
    owner: SstFileBuffer<OwnerIndexKey, OwnerIndexInfo>,
    dynamic_field: SstFileBuffer<DynamicFieldKey, ()>,
    package_version: SstFileBuffer<PackageVersionKey, PackageVersionInfo>,
}

impl SstBuffers {
    fn size_in_bytes(&self) -> usize {
        self.owner.size_in_bytes()
            + self.dynamic_field.size_in_bytes()
            + self.package_version.size_in_bytes()
    }

    fn write(&mut self, files: &SstFiles) -> Result<(), TypedStoreError> {
        files.write(&mut self.owner, &files.owner)?;
        files.write(&mut self.dynamic_field, &files.dynamic_field)?;
        files.write(&mut self.package_version, &files.package_version)
    }
}

/// Indexes one partition into a set of buffers taken from `SstFiles`, and
/// hands them back when dropped.
pub struct RpcIndexesPartitionIndexer<'a> {
    groups: &'a BTreeSet<IndexGroup>,
    coin_metadata: &'a CoinMetadata,
    files: &'a SstFiles,
    buffers: SstBuffers,
}

impl<'a> RpcIndexesPartitionIndexer<'a> {
    fn new(
        groups: &'a BTreeSet<IndexGroup>,
        coin_metadata: &'a CoinMetadata,
        files: &'a SstFiles,
    ) -> Self {
        Self {
            groups,
            coin_metadata,
            files,
            buffers: files.take_buffers(),
        }
    }

    /// Stages `object`'s rows in every table the store's groups maintain.
    pub fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                if let Some((key, info)) = OwnerIndexKey::for_object(owner, object) {
                    self.buffers.owner.insert(&key, &info)?;
                }
            }
            Owner::Object(parent) => {
                if is_dynamic_field(object) {
                    self.buffers
                        .dynamic_field
                        .insert(&DynamicFieldKey::new(parent, object.id()), &())?;
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
                self.buffers.package_version.insert(&key, &info)?;
            }
        }

        if self.buffers.size_in_bytes() >= self.files.buffer_limit {
            self.buffers.write(self.files)?;
        }
        Ok(())
    }

    /// Ends this partition, handing its buffers back for the next one.
    pub fn finish(self) -> Result<(), StorageError> {
        drop(self);
        Ok(())
    }
}

/// Hands the buffers back even when the partition fails, so the rows of the
/// partitions indexed into them earlier are not lost.
impl Drop for RpcIndexesPartitionIndexer<'_> {
    fn drop(&mut self) {
        self.files.return_buffers(std::mem::take(&mut self.buffers));
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
    sst_files: SstFiles,
}

impl RpcIndexesRestorer {
    /// Opens the store for bulk ingestion and records this schema version and
    /// `groups`. The watermark is written only by [`Self::finalize`], so a
    /// node opening the store of an interrupted restore rebuilds it.
    pub fn open(path: PathBuf, groups: BTreeSet<IndexGroup>) -> Result<Self, StorageError> {
        // After the open, which clears a database directory it finds in use.
        let tables = IndexStoreTables::open_for_bulk_ingestion(path.clone());
        let sst_files = SstFiles::new(&path)?;
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
            sst_files,
        })
    }

    /// Returns an indexer for one partition of the snapshot's live objects.
    pub fn partition_indexer(&self) -> RpcIndexesPartitionIndexer<'_> {
        RpcIndexesPartitionIndexer::new(&self.groups, &self.coin_metadata, &self.sst_files)
    }

    /// Ingests the restored rows, writes the markers at `restore_checkpoint`,
    /// the restore's highest executed checkpoint, flushes the bulk writes and
    /// closes the database, so the caller can move its directory once this
    /// returns.
    ///
    /// Callers must have restored the complete live object set first, through
    /// [`Self::partition_indexer`], and dropped every partition indexer.
    pub async fn finalize(
        self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), StorageError> {
        let Self {
            tables,
            coin_metadata,
            sst_files,
            ..
        } = self;
        sst_files.ingest(&tables)?;
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

    /// Reopens the finalized store as a node does and checks its markers, and
    /// that the `owner` table is non-empty if `live_object_count` objects were
    /// restored, so a store the node would rebuild fails the restore instead.
    /// `historic_db_path` is as for [`RpcIndexesStore::new`].
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
