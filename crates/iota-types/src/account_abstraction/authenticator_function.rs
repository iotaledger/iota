// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectID, ObjectRef, TransactionDigest},
    error::IotaError,
    execution::DynamicallyLoadedObjectMetadata,
    object::{Data, Object, Owner},
};

pub const AUTHENTICATOR_FUNCTION_MODULE_NAME: &IdentStr = ident_str!("authenticator_function");
pub const AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME: &IdentStr =
    ident_str!("AuthenticatorFunctionRefV1");

/// An enum representing different versions of AuthenticatorFunctionRef. This is
/// used to represent the reference to an authenticator function in Move.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum AuthenticatorFunctionRef {
    V1(AuthenticatorFunctionRefV1),
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefV1 {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
}

impl AuthenticatorFunctionRefV1 {
    pub fn type_(type_param: StructTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: AUTHENTICATOR_FUNCTION_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME.to_owned(),
            type_params: vec![TypeTag::Struct(Box::new(type_param))],
        }
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AuthenticatorFunctionRefV1 object: {err}"),
        })
    }

    pub fn is_authenticator_function_ref_v1(tag: &StructTag) -> bool {
        tag.address == IOTA_FRAMEWORK_ADDRESS
            && tag.module.as_ident_str() == AUTHENTICATOR_FUNCTION_MODULE_NAME
            && tag.name.as_ident_str() == AUTHENTICATOR_FUNCTION_REF_V1_STRUCT_NAME
    }
}

impl TryFrom<Object> for AuthenticatorFunctionRefV1 {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_authenticator_function_ref_v1() {
                    return AuthenticatorFunctionRefV1::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
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
    pub loaded_object_id: ObjectID,
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
            loaded_object_id: loaded_object_ref.0,
            loaded_object_metadata: DynamicallyLoadedObjectMetadata {
                version: loaded_object_ref.1,
                digest: loaded_object_ref.2,
                owner,
                storage_rebate,
                previous_transaction,
            },
        }
    }
}
