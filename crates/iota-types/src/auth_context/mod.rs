// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod fields_v1;

pub use fields_v1::*;
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::StructTag,
};
use serde::Serialize;

use crate::{
    IOTA_FRAMEWORK_ADDRESS, digests::MoveAuthenticatorDigest, transaction::ProgrammableTransaction,
};

pub const AUTH_CONTEXT_MODULE_NAME: &IdentStr = ident_str!("auth_context");
pub const AUTH_CONTEXT_STRUCT_NAME: &IdentStr = ident_str!("AuthContext");

/// `AuthContext` provides a lightweight execution context used during the
/// authentication phase of a transaction.
///
/// It allows authenticator functions to:
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
/// public fun authenticate(account: &Account, signature: &vector<u8>, auth_ctx: &AuthContext, , ctx: &TxContext) {
///     assert!(ed25519::ed25519_verify(signature, &account.pub_key, ctx.digest()), EEd25519VerificationFailed);
///     
///     assert!(is_authorized(&extract_function_key(&auth_ctx)), EUnauthorized);
///     ...
/// }
/// ```
// Conceptually similar to `TxContext`, but designed specifically for use in the authentication
// flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    auth_digest: MoveAuthenticatorDigest,
    /// The authentication input objects or primitive values
    tx_inputs: Vec<MoveCallArg>,
    /// The authentication commands to be executed sequentially.
    tx_commands: Vec<MoveCommand>,
}

impl AuthContext {
    pub fn new_from_components(
        auth_digest: MoveAuthenticatorDigest,
        ptb: &ProgrammableTransaction,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs: ptb.inputs.iter().map(MoveCallArg::from).collect(),
            tx_commands: ptb.commands.iter().map(MoveCommand::from).collect(),
        }
    }

    pub fn new_for_testing() -> Self {
        Self {
            auth_digest: MoveAuthenticatorDigest::default(),
            tx_inputs: Vec::new(),
            tx_commands: Vec::new(),
        }
    }

    pub fn digest(&self) -> &MoveAuthenticatorDigest {
        &self.auth_digest
    }

    pub fn tx_inputs(&self) -> &Vec<MoveCallArg> {
        &self.tx_inputs
    }

    pub fn tx_commands(&self) -> &Vec<MoveCommand> {
        &self.tx_commands
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn to_move_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&MoveAuthContext::default()).unwrap()
    }

    /// Returns whether the type signature is &mut AuthContext, &AuthContext, or
    /// none of the above.
    pub fn kind(module: &CompiledModule, token: &SignatureToken) -> AuthContextKind {
        use SignatureToken as S;

        let (kind, token) = match token {
            S::MutableReference(token) => (AuthContextKind::Mutable, token),
            S::Reference(token) => (AuthContextKind::Immutable, token),
            _ => return AuthContextKind::None,
        };

        let S::Datatype(idx) = &**token else {
            return AuthContextKind::None;
        };

        let (module_addr, module_name, struct_name) = resolve_struct(module, *idx);

        if is_auth_context(module_addr, module_name, struct_name) {
            kind
        } else {
            AuthContextKind::None
        }
    }

    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: AUTH_CONTEXT_MODULE_NAME.to_owned(),
            name: AUTH_CONTEXT_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }

    /// Replaces the contents of the `AuthContext` with new values. This is
    /// intended for use within a Move test function, as the `AuthContext`
    /// should be immutable during normal use.
    pub fn replace(
        &mut self,
        auth_digest: MoveAuthenticatorDigest,
        tx_inputs: Vec<MoveCallArg>,
        tx_commands: Vec<MoveCommand>,
    ) {
        self.auth_digest = auth_digest;
        self.tx_inputs = tx_inputs;
        self.tx_commands = tx_commands;
    }
}

/// A Move-side `AuthContext` representation.
/// It is supposed to be used with empty fields since the Move `AuthContext`
/// struct is managed by the native functions.
#[derive(Default, Serialize)]
pub struct MoveAuthContext {
    auth_digest: MoveAuthenticatorDigest,
    tx_inputs: Vec<MoveCallArg>,
    tx_commands: Vec<MoveCommand>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AuthContextKind {
    // Not AuthContext
    None,
    // &mut AuthContext
    Mutable,
    // &AuthContext
    Immutable,
}

pub fn is_auth_context(
    module_addr: &AccountAddress,
    module_name: &IdentStr,
    struct_name: &IdentStr,
) -> bool {
    module_addr == &IOTA_FRAMEWORK_ADDRESS
        && module_name == AUTH_CONTEXT_MODULE_NAME
        && struct_name == AUTH_CONTEXT_STRUCT_NAME
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        base_types::ObjectID,
        transaction::{Argument, CallArg, Command, ProgrammableMoveCall, ProgrammableTransaction},
        type_input::{TypeInput, TypeName},
    };

    #[test]
    fn auth_context_new_from_components() {
        let ptb = ProgrammableTransaction {
            inputs: vec![CallArg::Pure(vec![0xab])],
            commands: vec![Command::MoveCall(Box::new(ProgrammableMoveCall {
                package: ObjectID::from_hex_literal("0x0000000000000000000000000000000000000001")
                    .unwrap(),
                module: "mod".to_string(),
                function: "fun".to_string(),
                type_arguments: vec![TypeInput::U8],
                arguments: vec![Argument::GasCoin],
            }))],
        };

        let ctx = AuthContext::new_from_components(MoveAuthenticatorDigest::default(), &ptb);

        assert_eq!(ctx.tx_inputs().len(), 1);
        assert_eq!(ctx.tx_commands().len(), 1);

        assert!(matches!(ctx.tx_inputs()[0], MoveCallArg::Pure(_)));

        // Commands must have TypeName substituted for TypeInput.
        let MoveCommand::MoveCall(call) = &ctx.tx_commands()[0] else {
            panic!("expected MoveCall");
        };
        assert_eq!(
            call.type_arguments,
            vec![TypeName {
                name: "u8".to_string()
            }]
        );
    }

    #[test]
    fn auth_context_to_bcs_bytes_is_deterministic() {
        let ctx = AuthContext::new_for_testing();
        assert_eq!(ctx.to_bcs_bytes(), ctx.to_bcs_bytes());
    }

    #[test]
    fn auth_context_to_bcs_bytes_reflects_content() {
        let mut ctx = AuthContext::new_for_testing();
        let empty_bytes = ctx.to_bcs_bytes();

        ctx.replace(
            MoveAuthenticatorDigest::default(),
            vec![MoveCallArg::Pure(vec![1])],
            vec![],
        );
        let non_empty_bytes = ctx.to_bcs_bytes();

        assert_ne!(empty_bytes, non_empty_bytes);
    }
}
