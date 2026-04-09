// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::models::objects::BackwardHistoryObjectStatus;

use crate::{
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

#[derive(Copy, Clone)]
pub(crate) enum BackwardView {
    /// Exact lookup by id+version, no consistency filtering.
    Historical,
    /// End-of-checkpoint state. Reconstructs object state at the given
    /// checkpoint using `superseded_at_checkpoint > checkpoint_viewed_at`
    /// and `MIN(version)`.
    Consistent { checkpoint_viewed_at: u64 },
}

/// Status value for objects that did not exist yet. These entries are excluded
/// from backward diff results.
const NOT_YET_CREATED: i16 = BackwardHistoryObjectStatus::NotYetCreated as i16;

/// Column list shared by both `checkpointed_objects` and
/// `objects_backward_history` projections into `StoredBackwardObject` layout.
const OBJECT_COLUMNS: &str = "\
    object_id, object_version, object_status, \
    object_digest, owner_type, owner_id, object_type, object_type_package, object_type_module, \
    object_type_name, serialized_object, coin_type, coin_balance, df_kind";

/// Builds a backward diff query for objects.
///
/// Combines `checkpointed_objects` (current state, including tombstones for
/// wrapped/deleted objects) with `objects_backward_history` (previous versions)
/// to produce a consistent or historical view depending on the `BackwardView`
/// variant.
pub(crate) fn build_backward_objects_query(
    view: BackwardView,
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    match view {
        BackwardView::Consistent {
            checkpoint_viewed_at,
        } => build_consistent_query(checkpoint_viewed_at as i64, page, filter_fn),
        BackwardView::Historical => build_historical_query(page, filter_fn),
    }
}

/// Builds a consistent view by merging non-superseded live objects with
/// previous versions of objects that changed after the target checkpoint.
fn build_consistent_query(
    cv: i64,
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    merge_and_deduplicate(
        non_superseded_live_objects(cv, page, &filter_fn),
        superseded_past_versions(cv, page, &filter_fn),
    )
}

/// Builds a historical view by merging all live objects with all past
/// versions, without consistency filtering.
fn build_historical_query(
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    merge_and_deduplicate(
        all_live_objects(page, &filter_fn),
        all_past_versions(page, &filter_fn),
    )
}

/// Returns objects from `checkpointed_objects` (including tombstones) that
/// were not superseded after the given checkpoint.
///
/// Uses a LEFT JOIN against `objects_backward_history` to exclude objects
/// that have any entry with `superseded_at_checkpoint > cv`.
fn non_superseded_live_objects(
    cv: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {} FROM checkpointed_objects",
        OBJECT_COLUMNS
    )));

    let changed_subquery = query!(format!(
        "SELECT DISTINCT object_id FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {cv}"
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

/// Returns previous versions of objects that were superseded after the given
/// checkpoint.
///
/// Picks the earliest superseded version (`MIN(object_version)`) per object,
/// which represents the state just before the first change after the target
/// checkpoint. Excludes `NOT_YET_CREATED` entries (objects that didn't exist
/// at the target checkpoint).
fn superseded_past_versions(
    cv: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {OBJECT_COLUMNS} FROM objects_backward_history"
    )));

    let history_window = filter!(
        history_filtered,
        format!(
            "superseded_at_checkpoint > {} AND object_status != {NOT_YET_CREATED}",
            cv
        )
    );

    let oldest_subquery = query!(format!(
        "SELECT object_id, MIN(object_version) AS min_version \
         FROM objects_backward_history \
         WHERE superseded_at_checkpoint > {cv} \
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

/// Returns all objects from `checkpointed_objects` (including tombstones)
/// without consistency filtering.
fn all_live_objects(page: &Page<Cursor>, filter_fn: &impl Fn(RawQuery) -> RawQuery) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {} FROM checkpointed_objects",
        OBJECT_COLUMNS
    )));
    let source = query!(
        "SELECT candidates.* FROM ({}) candidates",
        checkpointed_filtered
    );
    page.apply::<StoredBackwardObject>(source)
}

/// Returns all past versions from `objects_backward_history`, excluding
/// `NOT_YET_CREATED` entries.
fn all_past_versions(page: &Page<Cursor>, filter_fn: &impl Fn(RawQuery) -> RawQuery) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {OBJECT_COLUMNS} FROM objects_backward_history"
    )));
    let history_window = filter!(
        history_filtered,
        format!("object_status != {NOT_YET_CREATED}")
    );
    let source = query!("SELECT candidates.* FROM ({}) candidates", history_window);
    page.apply::<StoredBackwardObject>(source)
}

/// Merges two sources with UNION ALL and picks the most recent version per
/// `object_id` using `DISTINCT ON`.
///
/// The result is wrapped so cursor pagination can reference
/// `candidates.object_id`.
fn merge_and_deduplicate(source_a: RawQuery, source_b: RawQuery) -> RawQuery {
    let combined = query!(
        r#"SELECT DISTINCT ON (object_id) * FROM (({}) UNION ALL ({})) candidates"#,
        source_a,
        source_b
    )
    .order_by("object_id")
    .order_by("object_version DESC");

    query!("SELECT * FROM ({}) candidates", combined)
}
