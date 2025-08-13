use move_core_types::{account_address::AccountAddress, ident_str, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    base_types::ObjectID,
    error::IotaError,
    object::{Data, Object},
    transaction::CallArg,
};

/// Temporary created structures.
/// This part will be removed once the real types are implemented.

pub const AUTHENTICATOR_DF_NAME: &str = "IOTA_AUTHENTICATION";

pub struct MoveAuthenticator {
    pub inputs: Vec<CallArg>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorInfo {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
}

impl AuthenticatorInfo {
    pub fn tag() -> StructTag {
        StructTag {
            address: AccountAddress::ZERO,
            module: ident_str!("account").to_owned(),
            name: ident_str!("AuthenticatorInfoV1").to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize TreasuryCap object: {err}"),
        })
    }
}

impl TryFrom<Object> for AuthenticatorInfo {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_treasury_cap() {
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
