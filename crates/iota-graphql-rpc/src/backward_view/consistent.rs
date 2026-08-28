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
/// Uses a NOT EXISTS subquery against `objects_backward_history` to exclude
/// objects that have any entry with
/// `superseded_at_checkpoint > checkpoint_viewed_at`.
///
/// # Implementation notes
///
/// NOT EXISTS lets Postgres answer "did this object change?" row by row,
/// with one index lookup each. A LEFT JOIN on a `SELECT DISTINCT` subquery
/// takes that option away: the full list of changed objects must always be
/// built first, and in the worst plans it is then also scanned for every
/// row.
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

    let mut source = query!(
        "SELECT candidates.* FROM ({}) candidates",
        checkpointed_filtered
    );
    source = filter!(
        source,
        format!(
            "NOT EXISTS (\
                 SELECT 1 FROM objects_backward_history changed \
                 WHERE changed.object_id = candidates.object_id \
                   AND changed.superseded_at_checkpoint > {checkpoint_viewed_at})"
        )
    );
    page.apply::<StoredBackwardObject>(source)
}

/// Returns active objects from `objects_backward_history` that were consistent
/// at the given checkpoint.
///
/// Picks the earliest superseded version per object, which represents the state
/// just before the first change after the target checkpoint. Keeps only
/// `Active` entries: when that earliest version is a tombstone (or
/// `NotYetCreated`), the object had no live state at the target checkpoint and
/// drops out.
///
/// # Implementation notes
///
/// The candidate object_ids matching the filter are deduplicated first, so the
/// per-object live-version lookup runs once per object and only matching
/// objects are visited. The lookup finds the newest version already superseded
/// at the checkpoint by scanning `object_version DESC` and returns the next
/// one, so it reads only the object's recent versions. `MIN(object_version)
/// WHERE object_id = ... AND superseded_at_checkpoint > cp` would instead scan
/// up from the oldest version through everything superseded by the checkpoint -
/// slower the more history an object has.
fn consistent_historical_objects(
    checkpoint_viewed_at: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    // Distinct object_ids that changed after the checkpoint and match the filter.
    // The filtered columns are indexed on objects_backward_history, so this scans
    // only matching objects - except an unfiltered listing, which scans the window.
    let candidate_ids = filter!(
        filter_fn(query!(
            "SELECT DISTINCT object_id FROM objects_backward_history"
        )),
        format!("superseded_at_checkpoint > {checkpoint_viewed_at}")
    );
    let (candidate_ids_sql, binds) = candidate_ids.finish();

    // Find the highest version already superseded by the checkpoint and take the
    // next one - that is the version that was live at the checkpoint (or the
    // object's first version, if no version was superseded earlier).
    let live_version = format!(
        "SELECT live.object_version FROM objects_backward_history live \
         WHERE live.object_id = candidate_ids.object_id \
           AND live.object_version > COALESCE(( \
                 SELECT superseded.object_version \
                 FROM objects_backward_history superseded \
                 WHERE superseded.object_id = candidate_ids.object_id \
                   AND superseded.superseded_at_checkpoint <= {checkpoint_viewed_at} \
                 ORDER BY superseded.object_version DESC \
                 LIMIT 1), -1) \
         ORDER BY live.object_version ASC \
         LIMIT 1"
    );

    let live_rows = RawQuery::new(
        format!(
            "SELECT {OBJECT_COLUMNS} FROM ( \
                 SELECT objects_backward_history.* \
                 FROM ({candidate_ids_sql}) candidate_ids \
                 JOIN objects_backward_history \
                     ON objects_backward_history.object_id = candidate_ids.object_id \
                 WHERE objects_backward_history.object_version = ({live_version}) \
             ) AS objects_backward_history"
        ),
        binds,
    );

    // An object is in candidate_ids if any of its versions superseded after the
    // checkpoint matched the filter, but the version live at the checkpoint might
    // not. Re-apply the filter and require Active to drop those.
    let history_window = filter!(filter_fn(live_rows), format!("object_status = {ACTIVE}"));

    let source = query!("SELECT candidates.* FROM ({}) candidates", history_window);
    page.apply::<StoredBackwardObject>(source)
}
