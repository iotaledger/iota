use move_core_types::{account_address::AccountAddress, ident_str, language_storage::StructTag};

use crate::{base_types::ObjectID, transaction::CallArg};

/// Temporary created structures.
/// This part will be removed once the real types are implemented.
pub struct MoveAuthenticator {
    pub inputs: Vec<CallArg>,
}

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
}
