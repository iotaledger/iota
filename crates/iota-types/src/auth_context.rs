// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    digests::MoveAuthenticatorDigest,
    transaction::{CallArg, Command, ProgrammableTransaction},
};

/// `AuthContext` provides a lightweight execution context used during the
/// authentication phase of a transaction.
///
/// It allows authenticator functions to:
/// - Identify the transaction sender
/// - Inspect the programmable transaction block (PTB) inputs and commands
/// - Perform function-level permission checks
/// - Support OTP, time-locked auth, or regulatory rule enforcement
///
/// This struct is **immutable** during the auth phase and must not allow
/// mutation of state or access to storage beyond what is declared.
///
/// It is guaranteed to be available to all smart accounts implementing a
/// custom authenticator function.
///
/// Typical use:
/// ```move
/// public fun authenticate(tx_hash: vector<u8>, input: &MyAuthInput, ctx: &AuthContext) {
///     assert!(ed25519::ed25519_verify(&input.sig, &input.pk, &tx_hash), 0);
///     assert!(verify_digest(ctx.digest()), 1);
///     ...
/// }
/// ```
// Conceptually similar to `TxContext`, but designed specifically for use in the authentication
// flow.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    auth_digest: MoveAuthenticatorDigest,
    /// The authentication input objects or primitive values
    tx_inputs: Vec<CallArg>,
    /// The authentication commands to be executed sequentially.
    tx_commands: Vec<Command>,
}

impl AuthContext {
    pub fn new_from_components(
        auth_digest: MoveAuthenticatorDigest,
        ptb: &ProgrammableTransaction,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs: ptb.inputs.clone(),
            tx_commands: ptb.commands.clone(),
        }
    }

    pub fn digest(&self) -> &MoveAuthenticatorDigest {
        &self.auth_digest
    }

    pub fn tx_inputs(&self) -> &Vec<CallArg> {
        &self.tx_inputs
    }

    pub fn tx_commands(&self) -> &Vec<Command> {
        &self.tx_commands
    }

    pub fn to_vec(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}
