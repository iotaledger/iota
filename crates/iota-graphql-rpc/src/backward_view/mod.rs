// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Backward diff query builders for reconstructing object state from
//! `checkpointed_objects` and `objects_backward_history`.

pub(crate) mod consistent;
pub(crate) mod historical;

use iota_indexer::{
    models::objects::BackwardHistoryObjectStatus, types::ObjectStatus as NativeObjectStatus,
};

use crate::{query, raw_query::RawQuery, types::object::ObjectFilter};

/// An `ObjectFilter` validated for use against the historical view.
///
/// The historical view is keyed on `(object_id, object_version)` lookups
/// against `checkpointed_objects`, `objects_backward_history`, and (for
/// keys-only queries) `objects_version`, so it only makes sense when the
/// filter pins down specific keys. Constructible only via
/// `TryFrom<ObjectFilter>`, which requires `object_keys` to be set.
#[derive(Debug)]
pub(crate) struct HistoricalFilter(ObjectFilter);

#[derive(thiserror::Error, Debug)]
pub(crate) enum HistoricalFilterError {
    #[error(
        "ObjectFilter is missing object_keys; the historical view requires explicit (object_id, object_version) keys"
    )]
    MissingKeys,
}

impl TryFrom<ObjectFilter> for HistoricalFilter {
    type Error = HistoricalFilterError;

    fn try_from(filter: ObjectFilter) -> Result<Self, Self::Error> {
        if filter.object_keys.is_some() {
            Ok(Self(filter))
        } else {
            Err(HistoricalFilterError::MissingKeys)
        }
    }
}

impl HistoricalFilter {
    /// Whether the filter additionally constrains `type_` or `owner`.
    pub(crate) fn has_type_or_owner(&self) -> bool {
        self.0.type_.is_some() || self.0.owner.is_some()
    }

    pub(crate) fn apply(&self, query: RawQuery) -> RawQuery {
        self.0.apply(query)
    }
}

/// Status value for objects that did not exist yet. These entries are excluded
/// from backward diff results.
pub(super) const NOT_YET_CREATED: i16 = BackwardHistoryObjectStatus::NotYetCreated as i16;

/// Watermark entity name for `objects_backward_history`. Must match the
/// `CommitterTables::ObjectsBackwardHistory` strum serialization in
/// `iota-indexer`.
pub(crate) const BACKWARD_HISTORY_WATERMARK_ENTITY: &str = "objects_backward_history";

/// Column list for `checkpointed_objects` rows, tagged with
/// `from_backward_history = FALSE`.
pub(super) const CHECKPOINTED_COLUMNS: &str = "\
    object_id, object_version, object_status, \
    object_digest, owner_type, owner_id, object_type, object_type_package, object_type_module, \
    object_type_name, serialized_object, coin_type, coin_balance, df_kind, \
    FALSE AS from_backward_history";

/// Column list for `objects_backward_history` rows, tagged with
/// `from_backward_history = TRUE`.
pub(super) const HISTORY_COLUMNS: &str = "\
    object_id, object_version, object_status, \
    object_digest, owner_type, owner_id, object_type, object_type_package, object_type_module, \
    object_type_name, serialized_object, coin_type, coin_balance, df_kind, \
    TRUE AS from_backward_history";

/// Merges any non-empty set of sources with `UNION ALL` and picks the most
/// recent version per `object_id` using `DISTINCT ON`.
///
/// The result is wrapped so cursor pagination can reference
/// `candidates.object_id`.
pub(super) fn merge_and_deduplicate(sources: Vec<RawQuery>) -> RawQuery {
    assert!(
        !sources.is_empty(),
        "merge_and_deduplicate requires at least one source"
    );

    let mut binds: Vec<String> = Vec::new();
    let union_terms: Vec<String> = sources
        .into_iter()
        .map(|source| {
            let (sql, source_binds) = source.finish();
            binds.extend(source_binds);
            format!("({sql})")
        })
        .collect();

    let select = format!(
        r#"SELECT DISTINCT ON (object_id) * FROM ({}) candidates"#,
        union_terms.join(" UNION ALL ")
    );

    let combined = RawQuery::new(select, binds)
        .order_by("object_id")
        .order_by("object_version DESC");

    query!("SELECT * FROM ({}) candidates", combined)
}
