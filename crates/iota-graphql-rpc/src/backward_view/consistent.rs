// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Consistent view queries: reconstruct object state at a specific checkpoint
//! by combining unchanged objects from `checkpointed_objects` with previous
//! versions from `objects_backward_history`.

use crate::{
    backward_view::{ACTIVE, OBJECT_COLUMNS, merge_and_deduplicate},
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a consistent view at the given checkpoint by merging objects from
/// `checkpointed_objects` that haven't changed since the target checkpoint
/// with previous versions of objects that were superseded after it.
pub(crate) fn query(
    checkpoint_viewed_at: u64,
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpoint_viewed_at = checkpoint_viewed_at as i64;
    merge_and_deduplicate(vec![
        consistent_checkpointed_objects(checkpoint_viewed_at, page, &filter_fn),
        consistent_historical_objects(checkpoint_viewed_at, page, &filter_fn),
    ])
}

/// Returns active objects from `checkpointed_objects` that were consistent
/// also at the given checkpoint.
///
/// Uses a LEFT JOIN against `objects_backward_history` to exclude objects
/// that have any entry with `superseded_at_checkpoint > checkpoint_viewed_at`.
fn consistent_checkpointed_objects(
    checkpoint_viewed_at: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter!(
        filter_fn(query!(format!(
            "SELECT {OBJECT_COLUMNS} FROM checkpointed_objects"
        ))),
        format!("object_status = {ACTIVE}")
    );

    let changed_subquery = query!(format!(
        "SELECT DISTINCT object_id FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {checkpoint_viewed_at}"
    ));
    let mut source = query!(
        r#"SELECT candidates.* FROM ({}) candidates
           LEFT JOIN ({}) changed ON candidates.object_id = changed.object_id"#,
        checkpointed_filtered,
        changed_subquery
    );
    source = filter!(source, "changed.object_id IS NULL");
    page.apply::<StoredBackwardObject>(source)
}

/// Returns active objects from `objects_backward_history` that were consistent
/// at the given checkpoint.
///
/// Picks the earliest superseded version (`MIN(object_version)`) per object,
/// which represents the state just before the first change after the target
/// checkpoint. Keeps only `Active` entries: when that earliest version is a
/// tombstone (or `NotYetCreated`), the object had no live state at the target
/// checkpoint and drops out of the join.
fn consistent_historical_objects(
    checkpoint_viewed_at: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {OBJECT_COLUMNS} FROM objects_backward_history"
    )));

    let history_window = filter!(
        history_filtered,
        format!("superseded_at_checkpoint > {checkpoint_viewed_at} AND object_status = {ACTIVE}")
    );

    let oldest_subquery = query!(format!(
        "SELECT object_id, MIN(object_version) AS min_version \
         FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {checkpoint_viewed_at} \
         GROUP BY object_id"
    ));
    let source = query!(
        r#"SELECT candidates.* FROM ({}) candidates
           JOIN ({}) oldest ON candidates.object_id = oldest.object_id
               AND candidates.object_version = oldest.min_version"#,
        history_window,
        oldest_subquery
    );
    page.apply::<StoredBackwardObject>(source)
}
