// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod fields_v1;

pub use fields_v1::*;
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{
    account_address::AccountAddress,
    ident_str,
    identifier::IdentStr,
    language_storage::StructTag,
    runtime_value::{MoveStructLayout, MoveTypeLayout},
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    auth_digest: MoveAuthenticatorDigest,
    /// The authentication input objects or primitive values
    tx_inputs: Vec<AuthContextCallArg>,
    /// The authentication commands to be executed sequentially.
    tx_commands: Vec<AuthContextCommand>,
}

impl AuthContext {
    pub fn new_from_components(
        auth_digest: MoveAuthenticatorDigest,
        ptb: &ProgrammableTransaction,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs: ptb.inputs.iter().map(AuthContextCallArg::from).collect(),
            tx_commands: ptb.commands.iter().map(AuthContextCommand::from).collect(),
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

    pub fn tx_inputs(&self) -> &Vec<AuthContextCallArg> {
        &self.tx_inputs
    }

    pub fn tx_commands(&self) -> &Vec<AuthContextCommand> {
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

    pub fn layout_with_custom_field(custom_field: MoveTypeLayout) -> MoveTypeLayout {
        MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![custom_field]))))
    }

    // Move test only API
    //
    pub fn replace(
        &mut self,
        auth_digest: MoveAuthenticatorDigest,
        tx_inputs: Vec<AuthContextCallArg>,
        tx_commands: Vec<AuthContextCommand>,
    ) {
        self.auth_digest = auth_digest;
        self.tx_inputs = tx_inputs;
        self.tx_commands = tx_commands;
    }
}

#[derive(Default, Serialize)]
pub struct MoveAuthContext {
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
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

        assert!(matches!(ctx.tx_inputs()[0], AuthContextCallArg::Pure(_)));

        // Commands must have TypeName substituted for TypeInput.
        let AuthContextCommand::MoveCall(call) = &ctx.tx_commands()[0] else {
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
            vec![AuthContextCallArg::Pure(vec![1])],
            vec![],
        );
        let non_empty_bytes = ctx.to_bcs_bytes();

        assert_ne!(empty_bytes, non_empty_bytes);
    }
}
