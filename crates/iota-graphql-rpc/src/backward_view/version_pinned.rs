// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Version-pinned consistent view: the state of each candidate object at the
//! moment its parent reached the requested `parent_version`.
//!
//! For each candidate, the target version is the largest version in
//! `objects_version` whose value is `<= parent_version`. The state row at
//! `(object_id, target_version)` is then read from `checkpointed_objects` (if
//! it's the current state) or `objects_backward_history` (if it's a prior
//! state). Non-Active rows (tombstones, NYC markers, synth
//! WrappedOrDeleted) carry NULL `owner_id`/`df_kind`/etc., so any caller
//! filter constraining those columns drops the candidate when target
//! version lands on such a row.
//!
//! Mirrors the **earliest / produced-at** semantics of the original
//! forward-diff `df.object_version <= parent_version` filter.

use crate::{
    backward_view::{CHECKPOINTED_COLUMNS, HISTORY_COLUMNS, merge_and_deduplicate},
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a version-pinned consistent view at `parent_version`.
pub(crate) fn query(
    parent_version: u64,
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let parent_version = parent_version as i64;
    merge_and_deduplicate(vec![
        version_pinned_checkpointed_objects(parent_version, page, &filter_fn),
        version_pinned_historical_objects(parent_version, page, &filter_fn),
    ])
}

/// Source A: rows in `checkpointed_objects` whose current `object_version`
/// equals the largest `objects_version` entry `<= parent_version` for that
/// `object_id`. Excludes rows that are also present in
/// `objects_backward_history` at the same `(object_id, object_version)` —
/// during the brief race window between backward-history and
/// checkpointed-objects writes the prior state may already be in
/// `objects_backward_history`, in which case Source B is authoritative.
fn version_pinned_checkpointed_objects(
    parent_version: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {CHECKPOINTED_COLUMNS} FROM checkpointed_objects"
    )));

    let with_target = filter!(
        checkpointed_filtered,
        format!(
            "object_version = (\
                 SELECT MAX(object_version) FROM objects_version ov \
                 WHERE ov.object_id = checkpointed_objects.object_id \
                   AND ov.object_version <= {parent_version})"
        )
    );

    let no_overlap = filter!(
        with_target,
        "NOT EXISTS (\
             SELECT 1 FROM objects_backward_history bh \
             WHERE bh.object_id = checkpointed_objects.object_id \
               AND bh.object_version = checkpointed_objects.object_version)"
    );

    let source = query!("SELECT candidates.* FROM ({}) candidates", no_overlap);
    page.apply::<StoredBackwardObject>(source)
}

/// Source B: rows in `objects_backward_history` whose `object_version` equals
/// the largest `objects_version` entry `<= parent_version` for that
/// `object_id`. This row carries the prior-state data of the object as it
/// was at `parent_version`.
fn version_pinned_historical_objects(
    parent_version: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {HISTORY_COLUMNS} FROM objects_backward_history"
    )));

    let with_target = filter!(
        history_filtered,
        format!(
            "object_version = (\
                 SELECT MAX(object_version) FROM objects_version ov \
                 WHERE ov.object_id = objects_backward_history.object_id \
                   AND ov.object_version <= {parent_version})"
        )
    );

    let source = query!("SELECT candidates.* FROM ({}) candidates", with_target);
    page.apply::<StoredBackwardObject>(source)
}
