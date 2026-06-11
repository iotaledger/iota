// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Historical view queries: exact id+version lookups without consistency
//! filtering. Combines active objects from `checkpointed_objects` with active
//! past versions from `objects_backward_history`. Wrapped or deleted
//! tombstones are excluded, so such versions resolve as non-existent.

use crate::{
    backward_view::{
        CHECKPOINTED_ACTIVE, HISTORY_ACTIVE, HistoricalFilter, OBJECT_COLUMNS,
        merge_and_deduplicate,
    },
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a historical view query for a `HistoricalFilter` by merging active
/// objects from `checkpointed_objects` with active prior versions from
/// `objects_backward_history`.
pub(crate) fn query(page: &Page<Cursor>, filter: &HistoricalFilter) -> RawQuery {
    let filter_fn = |q| filter.apply(q);

    merge_and_deduplicate(vec![
        checkpointed_objects(page, &filter_fn),
        historical_objects(page, &filter_fn),
    ])
}

/// Returns active objects from `checkpointed_objects` that satisfy the
/// provided filter.
fn checkpointed_objects(
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter!(
        filter_fn(query!(format!(
            "SELECT {OBJECT_COLUMNS} FROM checkpointed_objects"
        ))),
        format!("object_status = {CHECKPOINTED_ACTIVE}")
    );
    let source = query!(
        "SELECT candidates.* FROM ({}) candidates",
        checkpointed_filtered
    );
    page.apply::<StoredBackwardObject>(source)
}

/// Returns active objects from `objects_backward_history` that satisfy the
/// provided filter.
fn historical_objects(page: &Page<Cursor>, filter_fn: &impl Fn(RawQuery) -> RawQuery) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {OBJECT_COLUMNS} FROM objects_backward_history"
    )));
    let history_window = filter!(
        history_filtered,
        format!("object_status = {HISTORY_ACTIVE}")
    );
    let source = query!("SELECT candidates.* FROM ({}) candidates", history_window);
    page.apply::<StoredBackwardObject>(source)
}
