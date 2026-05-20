// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//! Logic and abstractions for supporting different databases that the snapshot
//! can restore.

use std::future::Future;

use anyhow::Result;
use bytes::Bytes;
use iota_core::authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables};
use iota_storage::SHA3_BYTES;

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
