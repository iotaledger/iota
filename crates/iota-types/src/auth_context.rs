// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    digests::MoveAuthenticationDigest,
    transaction::{CallArg, Command, ProgrammableTransaction},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    pub auth_digest: MoveAuthenticationDigest,
    /// The authentication input objects or primitive values
    pub tx_inputs: Vec<CallArg>,
    /// The authentication commands to be executed sequentially.
    pub tx_commands: Vec<Command>,
}

impl AuthContext {
    pub fn new_from_components(
        auth_digest: MoveAuthenticationDigest,
        ptb: ProgrammableTransaction,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs: ptb.inputs,
            tx_commands: ptb.commands,
        }
    }
}
