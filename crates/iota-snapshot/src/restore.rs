// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//! Logic and abstractions for supporting different databases that the snapshot
//! can restore.

use std::future::Future;

use anyhow::Result;
use bytes::Bytes;
use iota_core::{
    authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables},
    grpc_indexes::{GrpcIndexesStore, GrpcLiveObjectRestorer},
};
use iota_storage::SHA3_BYTES;
use iota_types::storage::{EpochInfoV2, error::Error as StorageError};

use crate::{FileMetadata, reader::LiveObjectIter};

/// A trait for databases that can be restored from a formal snapshot.
pub trait Restore {
    /// Inserts a partition of live objects.
    ///
    /// The checksum of the partition can be computed as the SHA3 hash of all
    /// objects' digests included. Then it can be verified against the given
    /// expected value.
    fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> impl Future<Output = Result<()>> + Send;
}

impl Restore for AuthorityPerpetualTables {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> Result<()> {
        let live_objects = LiveObjectIter::new(&file_metadata, bytes)?;
        AuthorityStore::bulk_insert_live_objects(self, live_objects, expected_checksum)
            .expect("Failed to insert live objects");
        Ok(())
    }
}

/// Restore target that builds the gRPC index store alongside the live-object
/// restore: each partition's objects are teed into the gRPC live-object
/// indexer while they stream into the perpetual tables, so the gRPC store is
/// complete without a second pass over the restored state.
///
/// After the read finishes, the caller must still call
/// [`GrpcLiveObjectRestorer::finish`] (cross-partition coin aggregation) and
/// `GrpcIndexesStore::finalize_restore`.
pub struct RestoreWithGrpcIndexes<'a> {
    perpetual_tables: &'a AuthorityPerpetualTables,
    grpc_restorer: &'a GrpcLiveObjectRestorer<'a>,
}

impl<'a> RestoreWithGrpcIndexes<'a> {
    pub fn new(
        perpetual_tables: &'a AuthorityPerpetualTables,
        grpc_restorer: &'a GrpcLiveObjectRestorer<'a>,
    ) -> Self {
        Self {
            perpetual_tables,
            grpc_restorer,
        }
    }
}

impl Restore for RestoreWithGrpcIndexes<'_> {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> Result<()> {
        let mut partition_indexer = self.grpc_restorer.begin_partition();
        // Defer index errors so the decode stream is driven to completion by
        // `bulk_insert_live_objects` either way.
        let mut index_error: Option<StorageError> = None;
        let live_objects = LiveObjectIter::new(&file_metadata, bytes)?.inspect(|live_object| {
            if index_error.is_none() {
                if let Err(e) = partition_indexer.index_object(live_object.object.clone()) {
                    index_error = Some(e);
                }
            }
        });
        AuthorityStore::bulk_insert_live_objects(
            self.perpetual_tables,
            live_objects,
            expected_checksum,
        )
        .expect("Failed to insert live objects");
        if let Some(e) = index_error {
            return Err(e.into());
        }
        partition_indexer.finish()?;
        Ok(())
    }
}

/// A database that can persist a snapshot's `EPOCH_INFO` rows.
///
/// Deliberately separate from [`Restore`]: the two cover different snapshot
/// payloads with different targets (live objects vs. epoch metadata), and a
/// database implements only the traits for the data it hosts — the node's
/// perpetual tables take live objects, the gRPC index takes epoch info, and an
/// external indexer bootstrapping from a snapshot takes both. Keeping them
/// separate lets each call site require exactly the capability it uses,
/// instead of forcing no-op stubs that would let an incomplete restore pass
/// silently.
///
/// The insert must be idempotent: a caller may re-restore rows that are
/// already present (e.g. on a retried bootstrap), and implementations must
/// tolerate that. Rows are passed as one `Vec` because the on-disk
/// `EPOCH_INFO` file is a single BCS blob the reader fully materializes
/// anyway; a streaming signature only makes sense once the format itself is
/// chunked.
pub trait RestoreEpochInfo {
    fn restore_epoch_info(&self, rows: Vec<EpochInfoV2>)
    -> impl Future<Output = Result<()>> + Send;
}

impl RestoreEpochInfo for GrpcIndexesStore {
    async fn restore_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<()> {
        // Skip epochs already covered by the `EpochIndexed` watermark to avoid
        // re-writing held rows. `insert_epoch_info` is itself idempotent, so
        // this filter is a pure optimization.
        let highest_indexed = self
            .highest_indexed_epoch()
            .map_err(|e| anyhow::anyhow!("failed to read the epochs_v2 watermark: {e}"))?;
        let rows: Vec<EpochInfoV2> = rows
            .into_iter()
            .filter(|row| highest_indexed.is_none_or(|highest| row.epoch > highest))
            .collect();
        self.insert_epoch_info(rows)
            .map_err(|e| anyhow::anyhow!("failed to seed epochs_v2 from snapshot: {e}"))
    }
}
