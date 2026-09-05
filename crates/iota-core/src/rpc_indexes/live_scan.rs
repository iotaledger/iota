// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::*;

impl ParMakeLiveObjectIndexer for JsonRpcLiveObjectSetIndexer<'_> {
    type ObjectIndexer<'a>
        = JsonRpcLiveObjectIndexer<'a>
    where
        Self: 'a;

    fn make_live_object_indexer(&self) -> Self::ObjectIndexer<'_> {
        JsonRpcLiveObjectIndexer {
            tables: self.tables,
            batch: self.tables.owner_index.batch(),
            batch_size_limit: self.batch_size_limit,
        }
    }
}

impl LiveObjectIndexer for JsonRpcLiveObjectIndexer<'_> {
    fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        match object.owner {
            Owner::Address(owner) => {
                self.batch.insert_batch(
                    &self.tables.owner_index,
                    [((owner, object.id()), ObjectInfo::from_object(object))],
                )?;
                if let Some(coin_info) = CoinInfo::from_object(object) {
                    let coin_type = object
                        .opt_coin_type()
                        .expect("coin object must have a coin type")
                        .to_string();
                    self.batch.insert_batch(
                        &self.tables.coin_index,
                        [((owner, coin_type, object.id()), coin_info)],
                    )?;
                }
            }
            Owner::Object(parent) => {
                if is_dynamic_field(object) {
                    self.batch.insert_batch(
                        &self.tables.dynamic_field_index,
                        [((parent, object.id()), ())],
                    )?;
                }
            }
            Owner::Shared(_) | Owner::Immutable => {}
            _ => unimplemented!("a new Owner enum variant was added and needs to be handled"),
        }

        // If the batch size grows beyond the limit then write out to the DB so
        // that the data we need to hold in memory doesn't grow unbounded.
        if self.batch.size_in_bytes() >= self.batch_size_limit {
            std::mem::replace(&mut self.batch, self.tables.owner_index.batch())
                .write_opt(&bulk_ingestion_write_options())?;
        }

        Ok(())
    }

    fn finish(self) -> Result<(), StorageError> {
        self.batch.write_opt(&bulk_ingestion_write_options())?;
        Ok(())
    }
}

impl JsonRpcPartitionIndexer<'_> {
    pub fn index_object(&mut self, object: &Object) -> Result<(), StorageError> {
        self.0.index_object(object)
    }

    /// Writes the partition's remaining batch.
    pub fn finish(self) -> Result<(), StorageError> {
        self.0.finish()
    }
}

/// Builds the live-state indexes (owner, coin, dynamic field) from a parallel
/// scan of the live object set during `init`.
pub(crate) struct JsonRpcLiveObjectSetIndexer<'a> {
    pub(crate) tables: &'a IndexStoreTables,
    pub(crate) batch_size_limit: usize,
}

/// One worker's indexer within a [`JsonRpcLiveObjectSetIndexer`] run, and the
/// per-partition indexer of a formal-snapshot restore.
pub(crate) struct JsonRpcLiveObjectIndexer<'a> {
    pub(crate) tables: &'a IndexStoreTables,
    pub(crate) batch: DBBatch,
    pub(crate) batch_size_limit: usize,
}

impl JsonRpcIndexRestorer {
    /// Opens the store with bulk-ingestion options and stamps it with this
    /// schema version. `meta` is written now and `watermark` only in
    /// [`Self::finalize`], so a node opening a store from a restore that
    /// crashed in between wipes and rebuilds it.
    pub fn open(path: PathBuf) -> Result<Self, TypedStoreError> {
        let tables = IndexStoreTables::open_for_bulk_ingestion(path, RESTORE_CONCURRENT_STORES);
        tables.meta.insert(
            &(),
            &MetadataInfo {
                version: CURRENT_DB_VERSION,
            },
        )?;
        Ok(Self {
            tables,
            batch_size_limit: bulk_ingestion_options_split_between(RESTORE_CONCURRENT_STORES)
                .batch_size_limit,
        })
    }

    /// Seeds the markers so a node opens the store in place, flushes the
    /// WAL-less bulk writes, and closes the database. `restore_checkpoint`
    /// is the restore's highest executed checkpoint; no history below it
    /// exists locally, so there is nothing for the background replay to
    /// backfill.
    ///
    /// Callers must have restored the complete live object set first,
    /// through [`Self::partition_indexer`].
    pub async fn finalize(
        self,
        restore_checkpoint: CheckpointSequenceNumber,
    ) -> Result<(), StorageError> {
        let Self { tables, .. } = self;
        tables.adopt_bulk_ingestion(Some(restore_checkpoint))?;

        // Release every RocksDB handle before returning, so the caller can
        // move the database directory.
        let weak_db = Arc::downgrade(&tables.meta.db);
        drop(tables);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the JSON-RPC index database after the restore",
            ));
        }
        Ok(())
    }

    /// Reopens the finalized store the way a node does and reads back the
    /// markers and the live state, so a database the node would wipe and
    /// rebuild — or one that carries no restored objects — fails the restore
    /// instead. `live_object_count` is the number of objects the restore
    /// wrote.
    pub async fn verify_restored(
        path: &Path,
        restore_checkpoint: CheckpointSequenceNumber,
        live_object_count: u64,
    ) -> Result<(), StorageError> {
        let reopened = IndexStore::open_index_db(path).map_err(|e| {
            StorageError::custom(format!(
                "unable to reopen the restored JSON-RPC index database: {e}"
            ))
        })?;
        let stored_version = reopened.tables.meta.get(&())?.ok_or_else(|| {
            StorageError::custom("the restored JSON-RPC index database has no metadata")
        })?;
        if stored_version.version != CURRENT_DB_VERSION {
            return Err(StorageError::custom(format!(
                "restored JSON-RPC index database version mismatch: expected {}, found {}",
                CURRENT_DB_VERSION, stored_version.version
            )));
        }
        let watermark = reopened.tables.watermark.get(&())?;
        if watermark != Some(restore_checkpoint) {
            return Err(StorageError::custom(format!(
                "the restored JSON-RPC index is watermarked at {watermark:?}, expected \
                 {restore_checkpoint}"
            )));
        }
        // The version and the watermark are written by the finalize itself;
        // only the live state proves the object stream landed. `is_empty`
        // has no error channel and reads an unreadable index as non-empty,
        // so the scan is run here and its failure fails the restore.
        let owner_index_is_empty = reopened
            .tables
            .owner_index
            .safe_iter()
            .next()
            .transpose()?
            .is_none();
        if live_object_count > 0 && owner_index_is_empty {
            return Err(StorageError::custom(format!(
                "the restored JSON-RPC index has an empty owner index after {live_object_count} \
                 live objects"
            )));
        }

        let weak_db = Arc::downgrade(&reopened.tables.meta.db);
        drop(reopened);
        if !wait_for_database_close(weak_db).await {
            return Err(StorageError::custom(
                "unable to close the JSON-RPC index database after verifying the restore",
            ));
        }
        Ok(())
    }
}
