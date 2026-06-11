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
    grpc_indexes::GrpcIndexesStore,
};
use iota_storage::SHA3_BYTES;
use iota_types::storage::EpochInfoV2;

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

/// A consumer that can persist a snapshot's `EPOCH_INFO` rows.
///
/// Separate from [`Restore`]: epoch rows target a different store and are one
/// synchronous batch rather than streamed partitions. Implementations must be
/// idempotent; skipping already-covered rows is an optimization, not a
/// correctness requirement.
pub trait SeedEpochInfo {
    fn seed_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<()>;
}

impl SeedEpochInfo for GrpcIndexesStore {
    fn seed_epoch_info(&self, rows: Vec<EpochInfoV2>) -> Result<()> {
        // Pure optimization: skip epochs the `EpochIndexed` watermark already
        // covers; `insert_epoch_info` is idempotent either way.
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
