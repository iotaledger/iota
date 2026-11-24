// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use iota_indexer::{models::display::StoredDisplay, schema::display};
use iota_json_rpc_types::{MAX_DISPLAY_OUTPUT_SIZE, render_display_template};
use iota_sdk_types::TypeTag;
use move_core_types::annotated_value::MoveStruct;

use crate::{
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
};

pub(crate) struct Display {
    pub stored: StoredDisplay,
}

/// The set of named templates defined on-chain for the type of this object,
/// to be handled off-chain. The server substitutes data from the object
/// into these templates to generate a display string per template.
#[derive(Debug, SimpleObject)]
pub(crate) struct DisplayEntry {
    /// The identifier for a particular template string of the Display object.
    pub key: String,
    /// The template string for the key with placeholder values substituted.
    pub value: Option<String>,
    /// An error string describing why the template could not be rendered.
    pub error: Option<String>,
}

impl Display {
    /// Query for a `Display` object by the type that it is displaying
    pub(crate) async fn query(db: &Db, type_: TypeTag) -> Result<Option<Display>, Error> {
        let stored: Option<StoredDisplay> = db
            .execute(move |conn| {
                conn.first(move || {
                    use display::dsl;
                    dsl::display.filter(
                        dsl::object_type.eq(type_.to_canonical_string(/* with_prefix */ true)),
                    )
                })
                .optional()
            })
            .await?;

        Ok(stored.map(|stored| Display { stored }))
    }

    /// Render the fields defined by this `Display` from the contents of
    /// `struct_`.
    pub(crate) fn render(&self, struct_: &MoveStruct) -> Result<Vec<DisplayEntry>, Error> {
        let display_fields = self
            .stored
            .to_display_fields()
            .map_err(|e| Error::Internal(e.to_string()))?;

        let mut rendered = vec![];
        let mut output_budget = MAX_DISPLAY_OUTPUT_SIZE;
        for entry in display_fields.contents {
            rendered.push(
                match render_display_template(&entry.value, struct_, &mut output_budget) {
                    Ok(v) => DisplayEntry::create_value(entry.key, v),
                    Err(e) => DisplayEntry::create_error(entry.key, e.to_string()),
                },
            );
        }

        Ok(rendered)
    }
}

impl DisplayEntry {
    pub(crate) fn create_value(key: String, value: String) -> Self {
        Self {
            key,
            value: Some(value),
            error: None,
        }
    }

    pub(crate) fn create_error(key: String, error: String) -> Self {
        Self {
            key,
            value: None,
            error: Some(error),
        }
    }
}
