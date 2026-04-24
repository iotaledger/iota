// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Backward diff query builders for reconstructing object state from
//! `checkpointed_objects` and `objects_backward_history`.

pub(crate) mod consistent;
pub(crate) mod historical;

use iota_indexer::models::objects::BackwardHistoryObjectStatus;

use crate::{query, raw_query::RawQuery};

/// Status value for objects that did not exist yet. These entries are excluded
/// from backward diff results.
pub(super) const NOT_YET_CREATED: i16 = BackwardHistoryObjectStatus::NotYetCreated as i16;

/// Watermark entity name for `objects_backward_history`. Must match the
/// `CommitterTables::ObjectsBackwardHistory` strum serialization in
/// `iota-indexer`.
pub(crate) const BACKWARD_HISTORY_WATERMARK_ENTITY: &str = "objects_backward_history";

/// Column list shared by both `checkpointed_objects` and
/// `objects_backward_history` projections into `StoredBackwardObject` layout.
pub(super) const OBJECT_COLUMNS: &str = "\
    object_id, object_version, object_status, \
    object_digest, owner_type, owner_id, object_type, object_type_package, object_type_module, \
    object_type_name, serialized_object, coin_type, coin_balance, df_kind";

/// Merges two sources with UNION ALL and picks the most recent version per
/// `object_id` using `DISTINCT ON`.
///
/// The result is wrapped so cursor pagination can reference
/// `candidates.object_id`.
pub(super) fn merge_and_deduplicate(source_a: RawQuery, source_b: RawQuery) -> RawQuery {
    let combined = query!(
        r#"SELECT DISTINCT ON (object_id) * FROM (({}) UNION ALL ({})) candidates"#,
        source_a,
        source_b
    )
    .order_by("object_id")
    .order_by("object_version DESC");

    query!("SELECT * FROM ({}) candidates", combined)
}
