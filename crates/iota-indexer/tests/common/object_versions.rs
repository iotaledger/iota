// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared query helpers for `objects_version` tests.

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::IndexerError, models::obj_indices::StoredObjectVersion, schema::objects_version,
    store::PgIndexerStore,
};

/// Looks up all object-version entries for a checkpoint, ordered by
/// (object_id, object_version).
pub fn find_object_versions_at_checkpoint(
    store: &PgIndexerStore,
    checkpoint: i64,
) -> Result<Vec<StoredObjectVersion>, IndexerError> {
    iota_indexer::read_only_blocking!(&store.blocking_cp(), |conn| {
        objects_version::table
            .filter(objects_version::cp_sequence_number.eq(checkpoint))
            .order((
                objects_version::object_id.asc(),
                objects_version::object_version.asc(),
            ))
            .load::<StoredObjectVersion>(conn)
    })
}
