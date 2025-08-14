use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{MoveObjectType, ObjectID, ObjectRef},
    error::IotaError,
    object::{Data, Object},
    transaction::{CallArg, InputObjectKind},
};

/// Temporary created structures.
/// This part will be removed once the real types are implemented.

pub const AUTHENTICATOR_DF_NAME: &str = "IOTA_AUTHENTICATION";

pub const AUTHENTICATOR_INFO_MODULE_NAME: &IdentStr = ident_str!("account");
pub const AUTHENTICATOR_INFO_STRUCT_NAME: &IdentStr = ident_str!("AuthenticatorInfo");

pub struct MoveAuthenticator {
    pub inputs: Vec<CallArg>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorInfo {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
}

impl MoveAuthenticator {
    pub fn input_objects(&self) -> Vec<InputObjectKind> {
        self.inputs
            .iter()
            .flat_map(|arg| arg.input_objects())
            .collect::<Vec<_>>()
    }

    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.inputs
            .iter()
            .flat_map(|arg| arg.receiving_objects())
            .collect()
    }
}

impl AuthenticatorInfo {
    pub fn tag() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: AUTHENTICATOR_INFO_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_INFO_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AuthenticatorInfo object: {err}"),
        })
    }

    // TODO: Needs to be moved to MoveObjectType.
    pub fn is_authenticator_info(other: &MoveObjectType) -> bool {
        other.address() == IOTA_FRAMEWORK_ADDRESS
            && other.module() == AUTHENTICATOR_INFO_MODULE_NAME
            && other.name() == AUTHENTICATOR_INFO_STRUCT_NAME
    }
}

impl TryFrom<Object> for AuthenticatorInfo {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if AuthenticatorInfo::is_authenticator_info(o.type_()) {
                    return AuthenticatorInfo::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a AuthenticatorInfo: {object:?}"),
        })
    }
}
