// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Historical view queries: exact id+version lookups without consistency
//! filtering. Combines all objects from `checkpointed_objects` with all
//! past versions from `objects_backward_history`.

use super::{
    BACKWARD_HISTORY_WATERMARK_ENTITY, CHECKPOINTED_COLUMNS, HISTORY_COLUMNS, NOT_YET_CREATED,
    NativeObjectStatus, merge_and_deduplicate, merge_and_deduplicate_three,
};
use crate::{
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a historical view with additional filters (type, owner, etc.).
/// Since tombstones have NULL data fields, they can never match these
/// filters, so the `objects_version` fallback is skipped.
pub(crate) fn query_with_filter(
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    merge_and_deduplicate(
        checkpointed_objects(page, &filter_fn),
        historical_objects(page, &filter_fn),
    )
}

/// Builds a historical view with only key filters (no type/owner).
/// Includes synthetic tombstones from `objects_version` for versions not
/// found in the other sources, since tombstones could match.
pub(crate) fn query_keys_only(
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    merge_and_deduplicate_three(
        checkpointed_objects(page, &filter_fn),
        historical_objects(page, &filter_fn),
        tombstones_from_objects_version(page, &filter_fn),
    )
}

/// Returns all objects from `checkpointed_objects` (including tombstones)
/// that satisfy the provided filter.
fn checkpointed_objects(
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {CHECKPOINTED_COLUMNS} FROM checkpointed_objects"
    )));
    let source = query!(
        "SELECT candidates.* FROM ({}) candidates",
        checkpointed_filtered
    );
    page.apply::<StoredBackwardObject>(source)
}

/// Returns all objects from `objects_backward_history` that satisfy the
/// provided filter, excluding `NOT_YET_CREATED` entries.
fn historical_objects(page: &Page<Cursor>, filter_fn: &impl Fn(RawQuery) -> RawQuery) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {HISTORY_COLUMNS} FROM objects_backward_history"
    )));
    let history_window = filter!(
        history_filtered,
        format!("object_status != {NOT_YET_CREATED}")
    );
    let source = query!("SELECT candidates.* FROM ({}) candidates", history_window);
    page.apply::<StoredBackwardObject>(source)
}

/// Returns synthetic `WrappedOrDeleted` tombstone rows from `objects_version`
/// for versions that exist there but are NOT present in `checkpointed_objects`
/// or `objects_backward_history`. This allows `objectKeys` lookups to find
/// objects by their real tombstone version.
///
/// Only used for keys-only queries where the filter contains only
/// `(object_id, object_version)` pairs — the `NOT EXISTS` subqueries hit
/// primary keys so the cost is proportional to the number of requested keys.
///
/// Filters out versions below the backward history watermark to avoid
/// returning false tombstones for pruned ranges.
fn tombstones_from_objects_version(
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let wrapped_or_deleted = NativeObjectStatus::WrappedOrDeleted as i16;

    // Inner query: select from objects_version, excluding versions that already
    // exist in checkpointed_objects or objects_backward_history, and filtering
    // out pruned ranges via the backward history watermark.
    let inner = query!(format!(
        "SELECT object_id, object_version, \
         {wrapped_or_deleted}::smallint AS object_status, \
         NULL::bytea AS object_digest, \
         NULL::smallint AS owner_type, \
         NULL::bytea AS owner_id, \
         NULL::text AS object_type, \
         NULL::bytea AS object_type_package, \
         NULL::text AS object_type_module, \
         NULL::text AS object_type_name, \
         NULL::bytea AS serialized_object, \
         NULL::text AS coin_type, \
         NULL::bigint AS coin_balance, \
         NULL::smallint AS df_kind, \
         FALSE AS from_backward_history \
         FROM objects_version ov \
         WHERE cp_sequence_number >= COALESCE(\
             (SELECT min_available_cp FROM watermarks \
              WHERE entity = '{BACKWARD_HISTORY_WATERMARK_ENTITY}'), 0) \
         AND NOT EXISTS (\
             SELECT 1 FROM checkpointed_objects co \
             WHERE co.object_id = ov.object_id \
               AND co.object_version = ov.object_version) \
         AND NOT EXISTS (\
             SELECT 1 FROM objects_backward_history bh \
             WHERE bh.object_id = ov.object_id \
               AND bh.object_version = ov.object_version)"
    ));

    // Wrap in a subquery so filter_fn can apply objectKeys WHERE clause cleanly.
    let version_filtered = filter_fn(query!("SELECT * FROM ({}) ov_filtered", inner));

    let source = query!("SELECT candidates.* FROM ({}) candidates", version_filtered);
    page.apply::<StoredBackwardObject>(source)
}
