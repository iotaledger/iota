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
