// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared query helpers for `objects_backward_history` tests.

use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};
use iota_indexer::{
    errors::IndexerError, models::objects::StoredBackwardHistoryObject,
    schema::objects_backward_history, store::PgIndexerStore,
};

/// Looks up a backward history entry by object_id and superseded_at_checkpoint.
pub fn find_backward_entry(
    store: &PgIndexerStore,
    object_id: &[u8],
    checkpoint: i64,
) -> Result<Option<StoredBackwardHistoryObject>, IndexerError> {
    iota_indexer::read_only_blocking!(&store.blocking_cp(), |conn| {
        objects_backward_history::table
            .filter(objects_backward_history::object_id.eq(object_id))
            .filter(objects_backward_history::superseded_at_checkpoint.eq(checkpoint))
            .select(StoredBackwardHistoryObject::as_select())
            .first::<StoredBackwardHistoryObject>(conn)
            .optional()
    })
}

/// Looks up all backward history entries for an object_id at a given
/// checkpoint, ordered by object_version.
pub fn find_all_entries_at_checkpoint(
    store: &PgIndexerStore,
    object_id: &[u8],
    checkpoint: i64,
) -> Result<Vec<StoredBackwardHistoryObject>, IndexerError> {
    iota_indexer::read_only_blocking!(&store.blocking_cp(), |conn| {
        objects_backward_history::table
            .filter(objects_backward_history::object_id.eq(object_id))
            .filter(objects_backward_history::superseded_at_checkpoint.eq(checkpoint))
            .order(objects_backward_history::object_version.asc())
            .select(StoredBackwardHistoryObject::as_select())
            .load::<StoredBackwardHistoryObject>(conn)
    })
}

/// Looks up all backward history entries for an object_id, ordered by
/// superseded_at_checkpoint.
pub fn find_all_entries_for_object(
    store: &PgIndexerStore,
    object_id: &[u8],
) -> Result<Vec<StoredBackwardHistoryObject>, IndexerError> {
    iota_indexer::read_only_blocking!(&store.blocking_cp(), |conn| {
        objects_backward_history::table
            .filter(objects_backward_history::object_id.eq(object_id))
            .order(objects_backward_history::superseded_at_checkpoint.asc())
            .select(StoredBackwardHistoryObject::as_select())
            .load::<StoredBackwardHistoryObject>(conn)
    })
}
