// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod enriched_fields;
mod fields_v1;

pub use enriched_fields::*;
pub use fields_v1::*;
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{
    account_address::AccountAddress, ident_str, identifier::IdentStr, language_storage::StructTag,
};
use serde::{Deserialize, Serialize};

/// Downgrades an [`MoveEnrichedCallArg`] to a plain [`MoveCallArg`], dropping
/// the enriched metadata (type name, mutability).
fn downgrade_call_arg(arg: &MoveEnrichedCallArg) -> MoveCallArg {
    match arg {
        MoveEnrichedCallArg::Pure { value, .. } => MoveCallArg::Pure(value.clone()),
        MoveEnrichedCallArg::ImmOrOwnedObject(obj) => MoveCallArg::Object(
            MoveObjectArg::ImmOrOwnedObject((obj.id, obj.version, obj.digest)),
        ),
        MoveEnrichedCallArg::SharedObject(obj) => {
            MoveCallArg::Object(MoveObjectArg::SharedObject {
                id: obj.id,
                initial_shared_version: obj.initial_shared_version,
                mutable: obj.mutable,
            })
        }
        MoveEnrichedCallArg::Receiving(obj) => {
            MoveCallArg::Object(MoveObjectArg::Receiving((obj.id, obj.version, obj.digest)))
        }
    }
}

/// Downgrades an [`MoveEnrichedCommand`] to a plain [`MoveCommand`], dropping
/// the enriched metadata (`is_entry`, `returns`).
fn downgrade_command(cmd: &MoveEnrichedCommand) -> MoveCommand {
    match cmd {
        MoveEnrichedCommand::MoveCall(call) => {
            MoveCommand::MoveCall(Box::new(MoveProgrammableMoveCall {
                package: call.package,
                module: call.module.clone(),
                function: call.function.clone(),
                type_arguments: call.type_arguments.clone(),
                arguments: call.arguments.clone(),
            }))
        }
        MoveEnrichedCommand::TransferObjects(objects, recipient) => {
            MoveCommand::TransferObjects(objects.clone(), *recipient)
        }
        MoveEnrichedCommand::SplitCoins(coin, amounts) => {
            MoveCommand::SplitCoins(*coin, amounts.clone())
        }
        MoveEnrichedCommand::MergeCoins(target, sources) => {
            MoveCommand::MergeCoins(*target, sources.clone())
        }
        MoveEnrichedCommand::Publish(modules, deps) => {
            MoveCommand::Publish(modules.clone(), deps.clone())
        }
        MoveEnrichedCommand::MakeMoveVec(type_arg, elements) => {
            MoveCommand::MakeMoveVec(type_arg.clone(), elements.clone())
        }
        MoveEnrichedCommand::Upgrade(modules, deps, package, ticket) => {
            MoveCommand::Upgrade(modules.clone(), deps.clone(), *package, *ticket)
        }
    }
}

use crate::{IOTA_FRAMEWORK_ADDRESS, digests::MoveAuthenticatorDigest};

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    /// The digest of the MoveAuthenticator
    auth_digest: MoveAuthenticatorDigest,
    /// The enriched transaction input objects or primitive values.
    ///
    /// Stored in enriched form so that native functions can serve pre-built
    /// enriched data directly, and downgrade to plain `CallArg` on demand.
    tx_inputs: Vec<MoveEnrichedCallArg>,
    /// The enriched transaction commands.
    ///
    /// Same rationale as `tx_inputs`.
    tx_commands: Vec<MoveEnrichedCommand>,
}

impl AuthContext {
    /// Construct an `AuthContext` from pre-built enriched inputs and commands.
    ///
    /// Callers (e.g. the execution engine) are responsible for building the
    /// enriched data, which may include object-type resolution via the backing
    /// store and function-metadata resolution via the VM.
    pub fn new_with_enriched(
        auth_digest: MoveAuthenticatorDigest,
        tx_inputs: Vec<MoveEnrichedCallArg>,
        tx_commands: Vec<MoveEnrichedCommand>,
    ) -> Self {
        Self {
            auth_digest,
            tx_inputs,
            tx_commands,
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

    /// Returns the plain (non-enriched) transaction inputs, downgraded from
    /// the stored enriched representation. Use [`enriched_tx_inputs`] when
    /// the enriched metadata (type name, mutability) is needed.
    pub fn tx_inputs(&self) -> Vec<MoveCallArg> {
        self.tx_inputs.iter().map(downgrade_call_arg).collect()
    }

    /// Returns the enriched transaction inputs with full type and mutability
    /// metadata.
    pub fn enriched_tx_inputs(&self) -> &Vec<MoveEnrichedCallArg> {
        &self.tx_inputs
    }

    /// Returns the plain (non-enriched) transaction commands, downgraded from
    /// the stored enriched representation. Use [`enriched_tx_commands`] when
    /// the enriched metadata (`is_entry`, `returns`) is needed.
    pub fn tx_commands(&self) -> Vec<MoveCommand> {
        self.tx_commands.iter().map(downgrade_command).collect()
    }

    /// Returns the enriched transaction commands with full function metadata.
    pub fn enriched_tx_commands(&self) -> &Vec<MoveEnrichedCommand> {
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
    /// Replaces the contents of the `AuthContext` with new enriched values.
    /// This is intended for use within a Move test function, as the
    /// `AuthContext` should be immutable during normal use.
    pub fn replace(
        &mut self,
        auth_digest: MoveAuthenticatorDigest,
        tx_inputs: Vec<MoveEnrichedCallArg>,
        tx_commands: Vec<MoveEnrichedCommand>,
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
    use crate::type_input::TypeName;

    #[test]
    fn auth_context_new_from_components() {
        let inputs = vec![MoveEnrichedCallArg::Pure {
            value: vec![0xab],
            type_name: TypeName {
                name: String::new(),
            },
        }];
        let commands = vec![MoveEnrichedCommand::MoveCall(Box::new(
            MoveEnrichedProgrammableMoveCall {
                package: crate::base_types::ObjectID::from_hex_literal(
                    "0x0000000000000000000000000000000000000001",
                )
                .unwrap(),
                module: "mod".to_string(),
                function: "fun".to_string(),
                is_entry: false,
                type_arguments: vec![TypeName {
                    name: "u8".to_string(),
                }],
                arguments: vec![],
                returns: vec![],
            },
        ))];

        let ctx =
            AuthContext::new_with_enriched(MoveAuthenticatorDigest::default(), inputs, commands);

        assert_eq!(ctx.enriched_tx_inputs().len(), 1);
        assert_eq!(ctx.enriched_tx_commands().len(), 1);
        assert!(matches!(
            ctx.enriched_tx_inputs()[0],
            MoveEnrichedCallArg::Pure { .. }
        ));
        assert!(matches!(
            ctx.enriched_tx_commands()[0],
            MoveEnrichedCommand::MoveCall(_)
        ));
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
            vec![MoveEnrichedCallArg::Pure {
                value: vec![1],
                type_name: TypeName {
                    name: String::new(),
                },
            }],
            vec![],
        );
        let non_empty_bytes = ctx.to_bcs_bytes();

        assert_ne!(empty_bytes, non_empty_bytes);
    }
}
