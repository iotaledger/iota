use crate::{
    base_types::{ObjectID, ObjectRef},
    transaction::CallArg,
};

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

impl MoveAuthenticator {
    pub fn receiving_objects(&self) -> Vec<ObjectRef> {
        self.inputs
            .iter()
            .flat_map(|arg| arg.receiving_objects())
            .collect()
    }
}
