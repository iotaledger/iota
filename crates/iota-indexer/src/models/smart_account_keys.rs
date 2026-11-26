// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;

use diesel::prelude::*;
use iota_sdk_move_types::iota_framework::dynamic_field::Field;
use iota_sdk_types::{Address, Identifier, MoveStruct, Owner, StructTag};
use iota_types::{
    account_abstraction::builtin_authenticator_functions::PublicKeyFieldName,
    move_public_key::MovePublicKey, object::Object,
};

use crate::{
    errors::IndexerError, ingestion::common::prepare::RemovedObject,
    models::objects::StoredDeletedObject, schema::smart_account_keys,
};

/// A public key dynamic field `Field<PublicKeyFieldName, PublicKey>` that is
/// attached to a `SmartAccount` object, with the hash of the public key it
/// contains.
#[derive(Queryable, Insertable, Debug, Identifiable, Clone, PartialEq, Eq)]
#[diesel(table_name = smart_account_keys, primary_key(object_id))]
pub struct StoredSmartAccountKey {
    /// The id of the dynamic field that contains a key.
    pub object_id: Vec<u8>,
    /// Object version of the dynamic field.
    pub object_version: i64,
    /// The id of `SmartAccount` the field is attached to.
    pub account_id: Vec<u8>,
    /// Hash of public key that is stored in the dynamic field:
    /// blake2b256(scheme flag || raw public key bytes).
    pub key_id: Vec<u8>,
}

impl StoredSmartAccountKey {
    /// Returns the rows for the public key dynamic fields in `objects` that
    /// are attached to a `SmartAccount` object.
    ///
    /// Attaching, replacing or removing a dynamic field mutates the object it
    /// is attached to, so the parent `SmartAccount` objects are expected to be present in the same
    /// `objects` batch. Public keys attached to objects other than `SmartAccount` are
    /// filtered out.
    pub fn from_objects<'a>(
        objects: impl IntoIterator<Item = &'a Object>,
    ) -> Result<Vec<Self>, IndexerError> {
        let key_field_tag = public_key_field_tag();
        let smart_account_tag = smart_account_tag();
        let mut smart_accounts = HashSet::new();
        let mut key_fields = Vec::new();
        for object in objects {
            let Some(tag) = struct_tag(object) else {
                continue;
            };
            if *tag == smart_account_tag {
                smart_accounts.insert(object.id());
            } else if *tag == key_field_tag {
                key_fields.push(object);
            }
        }
        key_fields
            .into_iter()
            .filter(|field| {
                matches!(field.owner(), Owner::Object(account_id) if smart_accounts.contains(account_id))
            })
            .map(Self::from_key_field)
            .collect()
    }

    /// Returns the rows to delete from `smart_account_keys` based on all `removed` objects in a
    /// batch.
    ///
    /// Checking if parent is a `SmartAccount` is skipped, since deleting a nonexistent row is a
    /// noop.
    pub(crate) fn deletions<'a>(
        removed: impl IntoIterator<Item = &'a RemovedObject>,
    ) -> Vec<StoredDeletedObject> {
        let key_field_tag = public_key_field_tag();
        removed
            .into_iter()
            .filter(|object| object.struct_tag() == Some(&key_field_tag))
            .map(StoredDeletedObject::from)
            .collect()
    }

    /// Returns the row for `field`, a public key dynamic field
    /// `Field<PublicKeyFieldName, PublicKey>` that is known to be attached to a
    /// `SmartAccount` object.
    ///
    /// # Errors
    /// Fails if the field is not attached to an object or its contents do not
    /// decode as a public key.
    pub fn from_key_field(field: &Object) -> Result<Self, IndexerError> {
        let Owner::Object(account_id) = field.owner() else {
            return Err(IndexerError::DynamicField(format!(
                "public key dynamic field {} is not attached to an object, its owner is {:?}",
                field.id(),
                field.owner()
            )));
        };
        let field_contents: Field<PublicKeyFieldName, MovePublicKey> =
            bcs::from_bytes(field.data.as_struct().contents()).map_err(|e| {
                IndexerError::DynamicField(format!(
                    "public key dynamic field {} does not decode: {e}",
                    field.id()
                ))
            })?;
        Ok(Self {
            object_id: field.id().as_bytes().to_vec(),
            object_version: field.version().as_u64() as i64,
            account_id: account_id.as_bytes().to_vec(),
            key_id: field_contents.value.key_id().to_vec(),
        })
    }
}

fn struct_tag(object: &Object) -> Option<&StructTag> {
    object.data.as_opt_struct().map(MoveStruct::struct_tag)
}

/// Returns the struct tag of `Field<PublicKeyFieldName, PublicKey>`, the
/// public key dynamic field of a `SmartAccount`.
pub fn public_key_field_tag() -> StructTag {
    StructTag::new_dynamic_field(PublicKeyFieldName::tag(), MovePublicKey::tag())
}

/// Returns the struct tag of `0x2::smart_account::SmartAccount`.
pub fn smart_account_tag() -> StructTag {
    StructTag::new(
        Address::FRAMEWORK,
        Identifier::from_static("smart_account"),
        Identifier::from_static("SmartAccount"),
        Vec::new(),
    )
}
