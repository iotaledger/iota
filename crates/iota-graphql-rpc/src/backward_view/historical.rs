// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Historical view queries: exact id+version lookups without consistency
//! filtering. Combines all objects from `checkpointed_objects` with all
//! past versions from `objects_backward_history`.

use super::{NOT_YET_CREATED, OBJECT_COLUMNS, merge_and_deduplicate};
use crate::{
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a historical view by merging all objects from `checkpointed_objects`
/// with all past versions from `objects_backward_history`, without consistency
/// filtering. Used for exact id+version lookups (`objectKeys`).
pub(crate) fn query(page: &Page<Cursor>, filter_fn: impl Fn(RawQuery) -> RawQuery) -> RawQuery {
    merge_and_deduplicate(
        checkpointed_objects(page, &filter_fn),
        historical_objects(page, &filter_fn),
    )
}

/// Returns all objects from `checkpointed_objects` (including tombstones)
/// that satisfy the provided filter.
fn checkpointed_objects(
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
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

/// Returns all objects from `objects_backward_history` that satisfy the
/// provided filter, excluding `NOT_YET_CREATED` entries.
fn historical_objects(page: &Page<Cursor>, filter_fn: &impl Fn(RawQuery) -> RawQuery) -> RawQuery {
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
