// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Historical view queries: exact id+version lookups without consistency
//! filtering. Combines all objects from `checkpointed_objects` with all
//! past versions from `objects_backward_history`.

use crate::{
    backward_view::{
        BACKWARD_HISTORY_WATERMARK_ENTITY, CHECKPOINTED_COLUMNS, HISTORY_COLUMNS, HistoricalFilter,
        NOT_YET_CREATED, NativeObjectStatus, merge_and_deduplicate,
    },
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a historical view query for a `HistoricalFilter`. Internally
/// branches on whether type/owner are also constrained: keys-only filters
/// include the `objects_version` source so real tombstone versions are
/// reachable, while filters with type/owner skip it (tombstones can't
/// match).
pub(crate) fn query(
    checkpoint_viewed_at: u64,
    page: &Page<Cursor>,
    filter: &HistoricalFilter,
) -> RawQuery {
    let filter_fn = |q| filter.apply(q);

    let mut sources = vec![
        checkpointed_objects(page, &filter_fn),
        historical_objects(page, &filter_fn),
    ];
    if !filter.has_type_or_owner() {
        sources.push(tombstones_from_objects_version(
            checkpoint_viewed_at,
            page,
            &filter_fn,
        ));
    }
    merge_and_deduplicate(sources)
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
/// Why surviving rows are necessarily `WrappedOrDeleted`: every version in
/// `objects_version` falls into exactly one of three cases.
///   1. Currently-latest state of the object (active row or tombstone) — lives
///      in `checkpointed_objects` and is filtered out by the first `NOT
///      EXISTS`.
///   2. Prior active state superseded by a later transaction — recorded in
///      `objects_backward_history` at its actual `object_version` with status
///      `ACTIVE`, and filtered out by the second `NOT EXISTS`.
///   3. Prior wrap/delete tombstone that was later overwritten in
///      `checkpointed_objects` (typically by an unwrap). The wrap state is
///      recorded in `objects_backward_history`, but at `lamport - 1` of the
///      unwrapping transaction — which generally differs from the real
///      tombstone version. The real version in `objects_version` therefore
///      survives both `NOT EXISTS` filters. By construction this is the only
///      surviving case, hence the static `WrappedOrDeleted` tag.
///
/// Only used for keys-only queries where the filter contains only
/// `(object_id, object_version)` pairs — the `NOT EXISTS` subqueries hit
/// primary keys so the cost is proportional to the number of requested keys.
///
/// Filters out versions outside the `[backward-history watermark,
/// checkpoint_viewed_at]` window to avoid false tombstones from pruned ranges
/// and concurrent in-flight batch writes.
fn tombstones_from_objects_version(
    checkpoint_viewed_at: u64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let wrapped_or_deleted = NativeObjectStatus::WrappedOrDeleted as i16;
    let checkpoint_viewed_at = checkpoint_viewed_at as i64;

    // Apply the keys filter directly to the bare table select so its disjunctive
    // `(object_id = ... AND object_version = ...)` clauses become the leading
    // WHERE — this lets the planner use the `(object_id, object_version)`
    // primary key for the lookup instead of scanning the whole cp window. The
    // cp bounds and the two `NOT EXISTS` checks are then AND'd on top.
    let base = query!(format!(
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
         FROM objects_version ov"
    ));

    let with_keys = filter_fn(base);

    let with_bounds = filter!(
        with_keys,
        format!(
            "cp_sequence_number >= COALESCE(\
                 (SELECT min_available_cp FROM watermarks \
                  WHERE entity = '{BACKWARD_HISTORY_WATERMARK_ENTITY}'), 0) \
             AND cp_sequence_number <= {checkpoint_viewed_at}"
        )
    );

    let inner = filter!(
        with_bounds,
        "NOT EXISTS (\
             SELECT 1 FROM checkpointed_objects co \
             WHERE co.object_id = ov.object_id \
               AND co.object_version = ov.object_version) \
         AND NOT EXISTS (\
             SELECT 1 FROM objects_backward_history bh \
             WHERE bh.object_id = ov.object_id \
               AND bh.object_version = ov.object_version)"
    );

    let source = query!("SELECT candidates.* FROM ({}) candidates", inner);
    page.apply::<StoredBackwardObject>(source)
}
