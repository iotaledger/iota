// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//! Logic and abstractions for supporting different databases that the snapshot
//! can restore.

use std::{future::Future, num::NonZeroUsize, sync::Arc};

use anyhow::{Context, Result};
use bytes::Bytes;
use iota_core::{
    authority::{
        AuthorityStore,
        authority_store_tables::{AuthorityPerpetualTables, LiveObject},
    },
    checkpoints::CheckpointStore,
    rpc_indexes::live_scan::RpcIndexesRestorer,
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
    ///
    /// While this future runs, the downloads are runtime tasks that the
    /// reader does not poll. The reader itself takes a finished download and
    /// starts its insert only while fewer than `max_concurrent_inserts`
    /// inserts run. An implementation that blocks its thread stops the
    /// downloads that this thread would run: all of them on a current-thread
    /// runtime, and the download started just before the insert when the read
    /// runs on a worker of a multi-thread runtime. Such an implementation
    /// should move its work to the blocking pool.
    fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> impl Future<Output = Result<()>> + Send;

    /// How many partitions the reader may insert at the same time. Each
    /// insert in progress holds one downloaded partition in memory.
    ///
    /// The reader polls all inserts from one task, so the inserts overlap only
    /// if `insert_partition` does not block the thread that polls it. An
    /// implementation that works inline gets one insert at a time, whatever it
    /// returns here.
    fn max_concurrent_inserts(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }
}

impl Restore for AuthorityPerpetualTables {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> Result<()> {
        let live_objects = LiveObjectIter::new(&file_metadata, bytes)?;
        bulk_insert_partition(self, live_objects, expected_checksum, &file_metadata)
    }
}

/// Inserts the objects of a partition. A checksum mismatch, which means that
/// downloaded data failed verification, and any other failure of the bulk
/// insert (a failed database write, for example) are errors rather than
/// panics. After a checksum mismatch nothing of the partition is written to
/// the perpetual tables.
fn bulk_insert_partition(
    perpetual_tables: &AuthorityPerpetualTables,
    live_objects: impl Iterator<Item = LiveObject>,
    expected_checksum: &[u8; SHA3_BYTES],
    file_metadata: &FileMetadata,
) -> Result<()> {
    AuthorityStore::bulk_insert_live_objects(perpetual_tables, live_objects, expected_checksum)
        .map_err(anyhow::Error::from)
        .with_context(|| {
            format!(
                "failed to insert the live objects of bucket {} partition {}",
                file_metadata.bucket_num, file_metadata.part_num
            )
        })
}

/// Restore target that, given an [`RpcIndexesRestorer`], also indexes each
/// partition's objects as they stream into the perpetual tables.
///
/// After the read finishes, the caller must still call
/// [`RpcIndexesRestorer::finalize`].
///
/// Dropping the future of [`Restore::insert_partition`], or of the read that
/// polls it, does not stop an insert that was started, also when it still waits
/// for a blocking thread. The insert finishes on the blocking pool and holds
/// its `Arc`s until then.
///
/// After an error the restorer must not be finalized: the partition indexer
/// hands its buffers back to the shared pool when it is dropped, so the index
/// rows of the failed partition stay staged.
pub struct RestoreWithIndexes {
    perpetual_tables: Arc<AuthorityPerpetualTables>,
    rpc_indexes_restorer: Option<Arc<RpcIndexesRestorer>>,
}

impl RestoreWithIndexes {
    pub fn new(
        perpetual_tables: Arc<AuthorityPerpetualTables>,
        rpc_indexes_restorer: Option<Arc<RpcIndexesRestorer>>,
    ) -> Self {
        Self {
            perpetual_tables,
            rpc_indexes_restorer,
        }
    }
}

impl Restore for RestoreWithIndexes {
    /// Inserts the partition on the blocking pool.
    ///
    /// # Errors
    ///
    /// Returns an error if the partition cannot be decoded, if the objects do
    /// not match `expected_checksum`, if they cannot be written or indexed, or
    /// if the insert was cancelled.
    ///
    /// # Panics
    ///
    /// Panics if there is no Tokio runtime. A panic of the insert is re-raised.
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> Result<()> {
        let perpetual_tables = self.perpetual_tables.clone();
        let rpc_indexes_restorer = self.rpc_indexes_restorer.clone();
        let expected_checksum = *expected_checksum;
        match tokio::task::spawn_blocking(move || {
            insert_partition_blocking(
                &perpetual_tables,
                rpc_indexes_restorer.as_deref(),
                file_metadata,
                bytes,
                &expected_checksum,
            )
        })
        .await
        {
            Ok(result) => result,
            Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
            Err(e) => Err(anyhow::Error::new(e).context("inserting a partition was cancelled")),
        }
    }
}

fn insert_partition_blocking(
    perpetual_tables: &AuthorityPerpetualTables,
    rpc_indexes_restorer: Option<&RpcIndexesRestorer>,
    file_metadata: FileMetadata,
    bytes: Bytes,
    expected_checksum: &[u8; SHA3_BYTES],
) -> Result<()> {
    let mut partition_indexer = rpc_indexes_restorer.map(|restorer| restorer.partition_indexer());
    // An index error must not cut the stream short: `bulk_insert_live_objects`
    // has to consume every object to verify the partition's checksum.
    let mut index_error: Option<StorageError> = None;
    let live_objects = LiveObjectIter::new(&file_metadata, bytes)?.inspect(|live_object| {
        if index_error.is_some() {
            return;
        }
        if let Some(indexer) = &mut partition_indexer {
            if let Err(e) = indexer.index_object(&live_object.object) {
                index_error = Some(e);
            }
        }
    });
    bulk_insert_partition(
        perpetual_tables,
        live_objects,
        expected_checksum,
        &file_metadata,
    )?;
    if let Some(e) = index_error {
        return Err(e.into());
    }
    if let Some(indexer) = partition_indexer {
        indexer.finish()?;
    }
    Ok(())
}

/// A database that can persist a snapshot's `EPOCH_INFO` rows.
///
/// Deliberately separate from [`Restore`]: the two cover different snapshot
/// payloads with different targets (live objects vs. epoch metadata), and a
/// database implements only the traits for the data it hosts — the node's
/// perpetual tables take live objects, the CheckpointStore takes epoch info,
/// and an external indexer bootstrapping from a snapshot takes both. Keeping
/// them separate lets each call site require exactly the capability it uses,
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

impl RestoreEpochInfo for CheckpointStore {
    async fn restore_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<()> {
        // Pure optimization: skip rows below the watermark. `insert_epoch_info`
        // is idempotent.
        let highest_indexed = self
            .highest_indexed_epoch()
            .map_err(|e| anyhow::anyhow!("failed to read the epoch_info watermark: {e}"))?;
        let rows: Vec<EpochInfoV2> = rows
            .into_iter()
            .filter(|row| highest_indexed.is_none_or(|highest| row.epoch > highest))
            .collect();
        self.insert_epoch_info(rows)
            .map_err(|e| anyhow::anyhow!("failed to seed epoch_info from snapshot: {e}"))
    }
}
