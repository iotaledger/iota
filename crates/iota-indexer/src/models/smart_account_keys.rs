// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_sdk_move_types::iota_framework::dynamic_field::Field;
use iota_sdk_types::Owner;
use iota_types::{
    account_abstraction::builtin_authenticator_functions::PublicKeyFieldName,
    move_public_key::MovePublicKey, object::Object,
};

use crate::{errors::IndexerError, schema::smart_account_keys};

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
