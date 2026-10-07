// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared query helpers for `objects_backward_history` tests.

use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};
use iota_indexer::{
    errors::IndexerError,
    models::objects::StoredBackwardHistoryObject,
    read_only_blocking,
    schema::objects_backward_history,
    store::{PgIndexerStore, diesel_macro::spawn_blocking_task},
};

/// Looks up a backward history entry by object_id and superseded_at_checkpoint.
pub async fn find_backward_entry(
    store: &PgIndexerStore,
    object_id: &[u8],
    checkpoint: i64,
) -> Result<Option<StoredBackwardHistoryObject>, IndexerError> {
    let blocking_cp = store.blocking_cp();
    let object_id = object_id.to_vec();
    spawn_blocking_task(move || {
        read_only_blocking!(&blocking_cp, |conn| {
            objects_backward_history::table
                .filter(objects_backward_history::object_id.eq(object_id))
                .filter(objects_backward_history::superseded_at_checkpoint.eq(checkpoint))
                .select(StoredBackwardHistoryObject::as_select())
                .first::<StoredBackwardHistoryObject>(conn)
                .optional()
        })
    })
    .await
    .expect("failed to join Tokio blocking task")
}

/// Looks up all backward history entries for an object_id at a given
/// checkpoint, ordered by object_version.
pub async fn find_all_entries_at_checkpoint(
    store: &PgIndexerStore,
    object_id: &[u8],
    checkpoint: i64,
) -> Result<Vec<StoredBackwardHistoryObject>, IndexerError> {
    let blocking_cp = store.blocking_cp();
    let object_id = object_id.to_vec();
    spawn_blocking_task(move || {
        read_only_blocking!(&blocking_cp, |conn| {
            objects_backward_history::table
                .filter(objects_backward_history::object_id.eq(object_id))
                .filter(objects_backward_history::superseded_at_checkpoint.eq(checkpoint))
                .order(objects_backward_history::object_version.asc())
                .select(StoredBackwardHistoryObject::as_select())
                .load::<StoredBackwardHistoryObject>(conn)
        })
    })
    .await
    .expect("failed to join Tokio blocking task")
}

/// Looks up all backward history entries for an object_id, ordered by
/// superseded_at_checkpoint.
pub async fn find_all_entries_for_object(
    store: &PgIndexerStore,
    object_id: &[u8],
) -> Result<Vec<StoredBackwardHistoryObject>, IndexerError> {
    let blocking_cp = store.blocking_cp();
    let object_id = object_id.to_vec();
    spawn_blocking_task(move || {
        read_only_blocking!(&blocking_cp, |conn| {
            objects_backward_history::table
                .filter(objects_backward_history::object_id.eq(object_id))
                .order(objects_backward_history::superseded_at_checkpoint.asc())
                .select(StoredBackwardHistoryObject::as_select())
                .load::<StoredBackwardHistoryObject>(conn)
        })
    })
    .await
    .expect("failed to join Tokio blocking task")
}
