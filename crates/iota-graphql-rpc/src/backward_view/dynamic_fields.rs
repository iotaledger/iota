// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Version-pinned dynamic-field queries: consistent view at a tx-sequence
//! boundary corresponding to the queried `parent_version`.
//!
//! The caller resolves `parent_version → root_version_tx_seq` (the largest
//! tx_sequence_number at which the parent was still at the requested version)
//! and dispatches to [`query`]. The implementation mirrors
//! [`super::consistent::query`] but pivots on
//! `superseded_at_tx_sequence_number` instead of `superseded_at_checkpoint` —
//! a checkpoint can contain multiple versions of the same object, and the
//! cp-axis cannot order them.

use super::{CHECKPOINTED_COLUMNS, HISTORY_COLUMNS, NOT_YET_CREATED, merge_and_deduplicate};
use crate::{
    filter, query,
    raw_query::RawQuery,
    types::{
        cursor::Page,
        object::{Cursor, StoredBackwardObject},
    },
};

/// Builds a "consistent view at root_version_tx_seq" query for dynamic fields.
/// `root_version_tx_seq` is the tx_sequence_number after which the parent
/// object was at the requested version (translated by the caller).
pub(crate) fn query(
    root_version_tx_seq: u64,
    page: &Page<Cursor>,
    filter_fn: impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let root_version_tx_seq = root_version_tx_seq as i64;
    merge_and_deduplicate(vec![
        consistent_checkpointed_objects(root_version_tx_seq, page, &filter_fn),
        consistent_historical_objects(root_version_tx_seq, page, &filter_fn),
    ])
}

/// Source A: rows from `checkpointed_objects` whose current state was already
/// in effect at `root_version_tx_seq` — i.e. no entry exists in
/// `objects_backward_history` for that object with
/// `superseded_at_tx_sequence_number > root_version_tx_seq`.
fn consistent_checkpointed_objects(
    root_version_tx_seq: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let checkpointed_filtered = filter_fn(query!(format!(
        "SELECT {CHECKPOINTED_COLUMNS} FROM checkpointed_objects"
    )));

    let changed_subquery = query!(format!(
        "SELECT DISTINCT object_id FROM objects_backward_history \
         WHERE superseded_at_tx_sequence_number > {root_version_tx_seq}"
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

/// Source B: rows from `objects_backward_history` giving each object's state
/// at `root_version_tx_seq` — the earliest version superseded after that
/// tx_seq (`MIN(object_version)`). Excludes `NotYetCreated` so objects that
/// didn't exist at `root_version_tx_seq` aren't returned. Mirrors
/// [`super::consistent::consistent_historical_objects`] on the tx_seq axis.
fn consistent_historical_objects(
    root_version_tx_seq: i64,
    page: &Page<Cursor>,
    filter_fn: &impl Fn(RawQuery) -> RawQuery,
) -> RawQuery {
    let history_filtered = filter_fn(query!(format!(
        "SELECT {HISTORY_COLUMNS} FROM objects_backward_history"
    )));

    let history_window = filter!(
        history_filtered,
        format!(
            "superseded_at_tx_sequence_number > {root_version_tx_seq} AND object_status != {NOT_YET_CREATED}"
        )
    );

    let oldest_subquery = query!(format!(
        "SELECT object_id, MIN(object_version) AS min_version \
         FROM objects_backward_history \
         WHERE superseded_at_tx_sequence_number > {root_version_tx_seq} \
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
