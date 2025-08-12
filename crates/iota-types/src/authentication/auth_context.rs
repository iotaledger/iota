// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    authentication::auth_digest::AuthenticationDigest,
    transaction::{CallArg, Command},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    pub auth_digest: AuthenticationDigest,
    /// The authentication input objects or primitive values
    pub tx_inputs: Vec<CallArg>,
    /// The authentication commands to be executed sequentially.
    pub tx_commands: Vec<Command>,
}

impl AuthContext {
    pub fn new(
        auth_digest: AuthenticationDigest,
        tx_inputs: Vec<CallArg>,
        tx_commands: Vec<Command>,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs,
            tx_commands,
        }
    }
}
