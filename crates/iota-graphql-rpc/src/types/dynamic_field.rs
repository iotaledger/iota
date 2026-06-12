// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{
    connection::{Connection, CursorType, Edge},
    *,
};
use iota_indexer::types::OwnerType;
use iota_types::{
    dynamic_field::{
        DynamicFieldInfo, DynamicFieldType, derive_dynamic_field_id,
        visitor::{Field, FieldVisitor},
    },
    iota_sdk_types_conversions::type_tag_core_to_sdk,
};

use crate::{
    backward_view::{consistent, dynamic_fields},
    data::{Db, QueryExecutor, package_resolver::PackageResolver},
    error::Error,
    filter,
    raw_query::RawQuery,
    types::{
        available_range::AvailableRange,
        base64::Base64,
        cursor::{Page, Target},
        iota_address::IotaAddress,
        move_object::MoveObject,
        move_value::MoveValue,
        object::{self, ActiveObject, Object, StoredBackwardObject},
        type_filter::ExactTypeFilter,
    },
};

pub(crate) struct DynamicField {
    pub super_: MoveObject,
}

#[derive(Union)]
pub(crate) enum DynamicFieldValue {
    MoveObject(Box<MoveObject>), // DynamicObject
    MoveValue(MoveValue),        // DynamicField
}

#[derive(InputObject)] // used as input object
pub(crate) struct DynamicFieldName {
    /// The string type of the DynamicField's 'name' field.
    /// A string representation of a Move primitive like 'u64', or a struct type
    /// like '0x2::kiosk::Listing'
    pub type_: ExactTypeFilter,
    /// The Base64 encoded bcs serialization of the DynamicField's 'name' field.
    pub bcs: Base64,
}

/// Dynamic fields are heterogeneous fields that can be added or removed at
/// runtime, and can have arbitrary user-assigned names. There are two sub-types
/// of dynamic fields:
///
/// 1) Dynamic Fields can store any value that has the `store` ability, however
///    an object stored in this kind of field will be considered wrapped and
///    will not be accessible directly via its ID by external tools (explorers,
///    wallets, etc) accessing storage.
/// 2) Dynamic Object Fields values must be IOTA objects (have the `key` and
///    `store` abilities, and id: UID as the first field), but will still be
///    directly accessible off-chain via their object ID after being attached.
#[Object]
impl DynamicField {
    /// The string type, data, and serialized value of the DynamicField's 'name'
    /// field. This field is used to uniquely identify a child of the parent
    /// object.
    async fn name(&self, ctx: &Context<'_>) -> Result<Option<MoveValue>> {
        let resolver: &PackageResolver = ctx.data_unchecked();

        let type_ = self.super_.native.type_tag();
        let layout = resolver.type_layout(type_.clone()).await.map_err(|e| {
            Error::Internal(format!(
                "Error fetching layout for type {}: {e}",
                type_.to_canonical_string(/* with_prefix */ true)
            ))
        })?;

        let Field {
            name_layout,
            name_bytes,
            ..
        } = FieldVisitor::deserialize(self.super_.native.contents(), &layout)
            .map_err(|e| Error::Internal(e.to_string()))
            .extend()?;

        Ok(Some(MoveValue::new(
            type_tag_core_to_sdk(&name_layout.into()),
            Base64::from(name_bytes.to_owned()),
        )))
    }

    /// The returned dynamic field is an object if its return type is
    /// `MoveObject`, in which case it is also accessible off-chain via its
    /// address. Its contents will be from the latest version that is at
    /// most equal to its parent object's version.
    async fn value(&self, ctx: &Context<'_>) -> Result<Option<DynamicFieldValue>> {
        let resolver: &PackageResolver = ctx.data_unchecked();

        let type_ = self.super_.native.type_tag();
        let layout = resolver.type_layout(type_.clone()).await.map_err(|e| {
            Error::Internal(format!(
                "Error fetching layout for type {}: {e}",
                type_.to_canonical_string(/* with_prefix */ true)
            ))
        })?;

        let Field {
            kind,
            value_layout,
            value_bytes,
            ..
        } = FieldVisitor::deserialize(self.super_.native.contents(), &layout)
            .map_err(|e| Error::Internal(e.to_string()))
            .extend()?;

        if kind == DynamicFieldType::DynamicObject {
            let df_object_id: IotaAddress = bcs::from_bytes(value_bytes)
                .map_err(|e| Error::Internal(format!("Failed to deserialize object ID: {e}")))
                .extend()?;

            let obj = MoveObject::query(
                ctx,
                df_object_id,
                Object::under_parent(self.root_version(), self.super_.super_.checkpoint_viewed_at),
            )
            .await
            .extend()?;

            Ok(obj.map(|obj| DynamicFieldValue::MoveObject(Box::new(obj))))
        } else {
            Ok(Some(DynamicFieldValue::MoveValue(MoveValue::new(
                type_tag_core_to_sdk(&value_layout.into()),
                Base64::from(value_bytes.to_owned()),
            ))))
        }
    }
}

impl DynamicField {
    /// Fetch a single dynamic field entry from the `db`, on `parent` object,
    /// with field name `name`, and kind `kind` (dynamic field or dynamic
    /// object field). The dynamic field is bound by the `parent_version` if
    /// provided - the fetched field will be the latest version at or before
    /// the provided version. If `parent_version` is not provided, the latest
    /// version of the field is returned as bounded by the
    /// `checkpoint_viewed_at` parameter.
    pub(crate) async fn query(
        ctx: &Context<'_>,
        parent: IotaAddress,
        parent_version: Option<u64>,
        name: DynamicFieldName,
        kind: DynamicFieldType,
        checkpoint_viewed_at: u64,
    ) -> Result<Option<DynamicField>, Error> {
        let type_ = match kind {
            DynamicFieldType::DynamicField => name.type_.0,
            DynamicFieldType::DynamicObject => {
                DynamicFieldInfo::dynamic_object_field_wrapper(name.type_.0).into()
            }
        };

        let field_id = derive_dynamic_field_id(parent, &type_, &name.bcs.0)
            .map_err(|e| Error::Internal(format!("Failed to derive dynamic field id: {e}")))?;

        let super_ = MoveObject::query(
            ctx,
            IotaAddress::from(field_id),
            if let Some(parent_version) = parent_version {
                Object::under_parent(parent_version, checkpoint_viewed_at)
            } else {
                Object::latest_at(checkpoint_viewed_at)
            },
        )
        .await?;

        super_.map(Self::try_from).transpose()
    }

    /// Query the `db` for a `page` of dynamic fields attached to object with ID
    /// `parent`. The returned dynamic fields are bound by the
    /// `parent_version` if provided - each field will be the latest version
    /// at or before the provided version. If `parent_version` is not provided,
    /// the latest version of each field is returned as bounded by the
    /// `checkpoint_viewed-at` parameter.`
    pub(crate) async fn paginate(
        db: &Db,
        page: Page<object::Cursor>,
        parent: IotaAddress,
        parent_version: Option<u64>,
        checkpoint_viewed_at: u64,
    ) -> Result<Connection<String, DynamicField>, Error> {
        // If cursors are provided, defer to the `checkpoint_viewed_at` in the cursor if
        // they are consistent. Otherwise, use the value from the parameter, or
        // set to None. This is so that paginated queries are consistent with
        // the previous query that created the cursor.
        let cursor_viewed_at = page.validate_cursor_consistency()?;
        let checkpoint_viewed_at = cursor_viewed_at.unwrap_or(checkpoint_viewed_at);

        let max_available_range = db.max_available_range;

        let Some((prev, next, results)) = db
            .execute_repeatable(move |conn| {
                if !AvailableRange::is_checkpoint_in_backward_history_range(
                    conn,
                    checkpoint_viewed_at,
                    max_available_range,
                )? {
                    return Ok::<_, diesel::result::Error>(None);
                };

                let query = match parent_version {
                    Some(pv) => dynamic_fields::query(parent, pv, &page),
                    None => {
                        consistent::query(checkpoint_viewed_at, &page, |q| apply_filter(q, parent))
                    }
                };

                Ok(Some(page.paginate_raw_query::<StoredBackwardObject>(
                    conn,
                    checkpoint_viewed_at,
                    query,
                )?))
            })
            .await?
        else {
            return Err(Error::Client(
                "Requested data is outside the available range".to_string(),
            ));
        };

        let mut conn: Connection<String, DynamicField> = Connection::new(prev, next);

        for stored in results {
            // To maintain consistency, the returned cursor should have the same upper-bound
            // as the checkpoint found on the cursor.
            let cursor = stored.cursor(checkpoint_viewed_at).encode_cursor();
            let stored_history = stored.into_stored_history(checkpoint_viewed_at);
            let active_object = ActiveObject::try_from(stored_history)?;
            let object =
                Object::from_active_object(active_object, checkpoint_viewed_at, parent_version);

            let move_ = MoveObject::try_from(&object).map_err(|_| {
                Error::Internal(format!(
                    "Failed to deserialize as Move object: {}",
                    object.address
                ))
            })?;

            let dynamic_field = DynamicField::try_from(move_)?;
            conn.edges.push(Edge::new(cursor, dynamic_field));
        }

        Ok(conn)
    }

    pub(crate) fn root_version(&self) -> u64 {
        self.super_.root_version()
    }
}

impl TryFrom<MoveObject> for DynamicField {
    type Error = Error;

    fn try_from(stored: MoveObject) -> Result<Self, Error> {
        let super_ = &stored.super_;

        let native = super_.native_impl();

        let Some(object) = native.data.as_struct_opt() else {
            return Err(Error::Internal("DynamicField is not an object".to_string()));
        };

        if !DynamicFieldInfo::is_dynamic_field(object.struct_tag()) {
            return Err(Error::Internal("Wrong type for DynamicField".to_string()));
        }

        Ok(DynamicField { super_: stored })
    }
}

fn apply_filter(query: RawQuery, parent: IotaAddress) -> RawQuery {
    filter!(
        query,
        format!(
            "owner_id = '\\x{}'::bytea AND owner_type = {} AND df_kind IS NOT NULL",
            hex::encode(parent.into_vec()),
            OwnerType::Object as i16
        )
    )
}
