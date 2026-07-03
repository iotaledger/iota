// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Identifier, ObjectData, ObjectId, Owner, StructTag, TypeTag};
use serde::{Deserialize, Serialize};

use crate::{
    account_abstraction::account::AuthenticatorFunctionRefV1Key,
    base_types::{ObjectRef, TransactionDigest},
    dynamic_field::{self, Field},
    error::{IotaError, UserInputError, UserInputResult},
    execution::DynamicallyLoadedObjectMetadata,
    object::Object,
};

pub const AUTHENTICATOR_FUNCTION_MODULE_NAME: Identifier =
    Identifier::from_static("authenticator_function");
pub const AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME: Identifier =
    Identifier::from_static("AuthenticatorFunctionRefV1");

/// An enum representing different versions of AuthenticatorFunctionRef. This is
/// used to represent the reference to an authenticator function in Move.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum AuthenticatorFunctionRef {
    V1(AuthenticatorFunctionRefV1),
}

impl From<AuthenticatorFunctionRef> for Option<AuthenticatorFunctionRefV1> {
    fn from(authenticator_function_ref: AuthenticatorFunctionRef) -> Self {
        match authenticator_function_ref {
            AuthenticatorFunctionRef::V1(v1) => Some(v1),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefV1 {
    pub package: ObjectId,
    pub module: String,
    pub function: String,
}

impl AuthenticatorFunctionRefV1 {
    pub fn type_(type_param: StructTag) -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            AUTHENTICATOR_FUNCTION_MODULE_NAME,
            AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME,
            vec![TypeTag::Struct(Box::new(type_param))],
        )
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AuthenticatorFunctionRefV1 object: {err}"),
        })
    }

    pub fn is_authenticator_function_ref_v1(tag: &StructTag) -> bool {
        tag.address() == Address::FRAMEWORK
            && tag.module() == &AUTHENTICATOR_FUNCTION_MODULE_NAME
            && tag.name() == &AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME
    }
}

impl TryFrom<Object> for AuthenticatorFunctionRefV1 {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            ObjectData::Struct(o) => {
                if AuthenticatorFunctionRefV1::is_authenticator_function_ref_v1(o.struct_tag()) {
                    return AuthenticatorFunctionRefV1::from_bcs_bytes(o.contents());
                }
            }
            ObjectData::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a AuthenticatorFunctionRefV1: {object:?}"),
        })
    }
}

/// A struct used to hold AuthenticatorFunctionRef and
/// DynamicallyLoadedObjectMetadata together, in order to pass this information
/// to the execution side.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefForExecution {
    pub authenticator_function_ref: AuthenticatorFunctionRef,
    pub loaded_object_id: ObjectId,
    pub loaded_object_metadata: DynamicallyLoadedObjectMetadata,
}

impl AuthenticatorFunctionRefForExecution {
    pub fn new_v1(
        authenticator_function_ref: AuthenticatorFunctionRefV1,
        loaded_object_ref: ObjectRef,
        owner: Owner,
        storage_rebate: u64,
        previous_transaction: TransactionDigest,
    ) -> Self {
        Self {
            authenticator_function_ref: AuthenticatorFunctionRef::V1(authenticator_function_ref),
            loaded_object_id: loaded_object_ref.object_id,
            loaded_object_metadata: DynamicallyLoadedObjectMetadata {
                version: loaded_object_ref.version,
                digest: loaded_object_ref.digest,
                owner,
                storage_rebate,
                previous_transaction,
            },
        }
    }
}

/// Derive the id of the dynamic field on the account object that holds its
/// [`AuthenticatorFunctionRefV1`].
pub fn derive_authenticator_function_ref_field_id(
    account_object_id: ObjectId,
) -> UserInputResult<ObjectId> {
    dynamic_field::derive_dynamic_field_id(
        account_object_id,
        &AuthenticatorFunctionRefV1Key::tag().into(),
        &AuthenticatorFunctionRefV1Key::default().to_bcs_bytes(),
    )
    .map_err(|_| UserInputError::UnableToGetMoveAuthenticatorId { account_object_id })
}

/// Decode a loaded authenticator dynamic-field object (see
/// [`derive_authenticator_function_ref_field_id`]) into an
/// [`AuthenticatorFunctionRefForExecution`].
pub fn authenticator_function_ref_from_field_object(
    account_object_id: ObjectId,
    field_obj: &Object,
) -> UserInputResult<AuthenticatorFunctionRefForExecution> {
    // A dynamic field is never a package object, so a non-struct here means the
    // object at the derived id is not the authenticator field.
    let field_move_object = field_obj
        .data
        .as_opt_struct()
        .ok_or(UserInputError::InvalidAuthenticatorFunctionRefField { account_object_id })?;

    let field: Field<AuthenticatorFunctionRefV1Key, AuthenticatorFunctionRefV1> = field_move_object
        .to_rust()
        .map_err(|_| UserInputError::InvalidAuthenticatorFunctionRefField { account_object_id })?;

    Ok(AuthenticatorFunctionRefForExecution::new_v1(
        field.value,
        field_obj.object_ref(),
        field_obj.owner,
        field_obj.storage_rebate,
        field_obj.previous_transaction,
    ))
}

/// Extracts the sender's and sponsor's [`AuthenticatorFunctionRef`] by calling
/// `find_ref` for `sender` and, when the gas owner differs, for `gas_owner`.
pub fn extract_auth_fun_refs(
    sender: Address,
    gas_owner: Address,
    find_ref: impl Fn(Address) -> Option<AuthenticatorFunctionRef>,
) -> (
    Option<AuthenticatorFunctionRef>,
    Option<AuthenticatorFunctionRef>,
) {
    (
        find_ref(sender),
        if gas_owner != sender {
            find_ref(gas_owner)
        } else {
            None
        },
    )
}

#[cfg(test)]
#[path = "../unit_tests/authenticator_function_tests.rs"]
mod authenticator_function_tests;
