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
    account_abstraction::builtin_authenticator_functions::PreloadedBuiltinAuthenticatorData,
    base_types::ObjectID,
    error::IotaError,
    execution::DynamicallyLoadedObjectMetadata,
    object::{Data, Object},
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

/// A struct used to hold authenticator information required by the signing
/// validation path.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefForSigning {
    pub authenticator_function_ref: AuthenticatorFunctionRef,
    pub builtin_authenticator_data: Option<PreloadedBuiltinAuthenticatorData>,
}

/// A struct used to hold authenticator information required by the execution
/// validation path.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefForExecution {
    pub authenticator_function_ref: AuthenticatorFunctionRef,
    pub builtin_authenticator_data: Option<PreloadedBuiltinAuthenticatorData>,
    pub loaded_objects: Vec<(ObjectID, DynamicallyLoadedObjectMetadata)>,
}

impl AuthenticatorFunctionRefForExecution {
    pub fn new_v1(
        authenticator_function_ref: AuthenticatorFunctionRefV1,
        builtin_authenticator_data: Option<PreloadedBuiltinAuthenticatorData>,
        loaded_objects: Vec<(ObjectID, DynamicallyLoadedObjectMetadata)>,
    ) -> Self {
        Self {
            authenticator_function_ref: AuthenticatorFunctionRef::V1(authenticator_function_ref),
            builtin_authenticator_data,
            loaded_objects,
        }
    }
}

impl From<AuthenticatorFunctionRefForExecution> for AuthenticatorFunctionRefForSigning {
    fn from(value: AuthenticatorFunctionRefForExecution) -> Self {
        AuthenticatorFunctionRefForSigning {
            authenticator_function_ref: value.authenticator_function_ref,
            builtin_authenticator_data: value.builtin_authenticator_data,
        }
    }
}
