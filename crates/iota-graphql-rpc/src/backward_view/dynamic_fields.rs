// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Consistent view of the dynamic fields of a parent at the moment the
//! parent reached the requested `parent_version`.
//!
//! For each candidate DF, the target version is the largest version in
//! `objects_version` whose value is `<= parent_version`. The state row at
//! `(object_id, target_version)` is then read from `checkpointed_objects` (if
//! it's the current state) or `objects_backward_history` (if it's a prior
//! state). When the target version is a tombstone, `NotYetCreated` marker,
//! or synth `WrappedOrDeleted` row, `owner_id`/`df_kind`/etc. are NULL — the
//! `owner_id`/`df_kind` filter applied internally drops it.
//!
//! Returned semantics: for each DF, the earliest state it had while the
//! parent was at `parent_version`. Note that there can be multiple distinct DF
//! states observable at the same `parent_version`, we return the first one.

use iota_indexer::types::OwnerType;

use crate::{
    backward_view::{CHECKPOINTED_COLUMNS, HISTORY_COLUMNS, merge_and_deduplicate},
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        iota_address::IotaAddress,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a consistent view of the dynamic fields owned by `parent` at
/// `parent_version`.
///
/// The DF-shape filter
/// (`owner_id = parent AND owner_type = Object AND df_kind IS NOT NULL`) is
/// applied internally — it both narrows the candidate set and excludes
/// non-Active rows (whose `owner_id`/`df_kind` are NULL).
pub(crate) fn query(parent: IotaAddress, parent_version: u64, page: &Page<Cursor>) -> RawQuery {
    let parent_version = parent_version as i64;
    let parent_filter = parent_dynamic_field_filter(parent);
    merge_and_deduplicate(vec![
        dynamic_fields_from_checkpointed_objects(parent_version, page, &parent_filter),
        dynamic_fields_from_historical_objects(parent_version, page, &parent_filter),
    ])
}

/// Returns a filter that constrains a row to be a dynamic field of `parent`.
///
/// Applied to both Source A and Source B; excludes non-Active rows by virtue
/// of their NULL `owner_id`/`df_kind` columns.
fn parent_dynamic_field_filter(parent: IotaAddress) -> impl Fn(RawQuery) -> RawQuery {
    move |q| {
        filter!(
            q,
            format!(
                "owner_id = '\\x{}'::bytea AND owner_type = {} AND df_kind IS NOT NULL",
                hex::encode(parent.into_vec()),
                OwnerType::Object as i16
            )
        )
    }
}

/// Source A — DFs whose current state in `checkpointed_objects` is at or
/// before `parent_version`.
///
/// Picks rows where `co.object_version <= parent_version`. Since
/// `checkpointed_objects` tracks each object's latest version, this version
/// is also `MAX(objects_version.object_version)` for the id, so the simple
/// `<=` check is equivalent to "the largest known version of this object
/// is at or before `parent_version`" — no `objects_version` lookup is
/// needed here.
///
/// # Constraints
///
/// Excludes rows that also appear in `objects_backward_history` at the same
/// `(object_id, object_version)`, keeping Source A and Source B disjoint at
/// the row level. This is needed because a `(id, version)` pair can
/// briefly exist in both tables during the race window between
/// `objects_backward_history` and `checkpointed_objects` writes.
///
/// # Implementation notes
///
/// The merge step uses `DISTINCT ON` to deduplicate *within a page* only;
/// source-level disjointness is required to keep the result correct across
/// page boundaries.
fn dynamic_fields_from_checkpointed_objects(
    parent_version: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {CHECKPOINTED_COLUMNS} FROM checkpointed_objects"
    )));

    let with_target = filter!(
        checkpointed_filtered,
        format!("object_version <= {parent_version}")
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

/// Source B — DFs whose prior state in `objects_backward_history` is the
/// one current at `parent_version`.
///
/// For each `object_id`, picks the row in `objects_backward_history` whose
/// `object_version` equals `target_v`, where `target_v` is the largest
/// `objects_version.object_version` entry `<= parent_version`. The chosen
/// row carries the prior-state data of the DF as it was while the parent
/// was at `parent_version`.
///
/// # Implementation notes
///
/// `objects_version` is consulted as the version-timeline authority (it
/// records every real version, including tombstone versions). When
/// `target_v` lands on a tombstone version, no Active state row exists at
/// that key — the candidate naturally drops out of the result.
fn dynamic_fields_from_historical_objects(
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
