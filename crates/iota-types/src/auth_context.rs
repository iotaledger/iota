// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::ObjectID,
    digests::MoveAuthenticatorDigest,
    transaction::{Argument, CallArg, Command, ObjectArg, ProgrammableTransaction},
    type_input::TypeName,
};

pub const AUTH_CONTEXT_MODULE_NAME: &IdentStr = ident_str!("auth_context");
pub const AUTH_CONTEXT_STRUCT_NAME: &IdentStr = ident_str!("AuthContext");

/// Mirrors [`crate::transaction::ProgrammableMoveCall`] for use in
/// [`AuthContextCommand`], substituting [`TypeName`] for
/// [`crate::type_input::TypeInput`] so that the type can derive
/// [`Serialize`]/[`Deserialize`] without a custom implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContextMoveCall {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
    pub type_arguments: Vec<TypeName>,
    pub arguments: Vec<Argument>,
}

/// Mirrors [`crate::transaction::Command`], substituting [`TypeName`] for
/// [`crate::type_input::TypeInput`] in `MoveCall` and `MakeMoveVec` so that
/// the type matches the BCS layout expected by the Move-side
/// `ptb_command::Command`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthContextCommand {
    MoveCall(Box<AuthContextMoveCall>),
    TransferObjects(Vec<Argument>, Argument),
    SplitCoins(Argument, Vec<Argument>),
    MergeCoins(Argument, Vec<Argument>),
    Publish(Vec<Vec<u8>>, Vec<ObjectID>),
    MakeMoveVec(Option<TypeName>, Vec<Argument>),
    Upgrade(Vec<Vec<u8>>, Vec<ObjectID>, ObjectID, Argument),
}

impl From<&Command> for AuthContextCommand {
    fn from(cmd: &Command) -> Self {
        match cmd {
            Command::MoveCall(m) => AuthContextCommand::MoveCall(Box::new(AuthContextMoveCall {
                package: m.package,
                module: m.module.clone(),
                function: m.function.clone(),
                type_arguments: m.type_arguments.iter().map(TypeName::from).collect(),
                arguments: m.arguments.clone(),
            })),
            Command::TransferObjects(objects, recipient) => {
                AuthContextCommand::TransferObjects(objects.clone(), *recipient)
            }
            Command::SplitCoins(coin, amounts) => {
                AuthContextCommand::SplitCoins(*coin, amounts.clone())
            }
            Command::MergeCoins(target_coin, source_coins) => {
                AuthContextCommand::MergeCoins(*target_coin, source_coins.clone())
            }
            Command::Publish(modules, dependencies) => {
                AuthContextCommand::Publish(modules.clone(), dependencies.clone())
            }
            Command::MakeMoveVec(type_arg, elements) => AuthContextCommand::MakeMoveVec(
                type_arg.as_ref().map(TypeName::from),
                elements.clone(),
            ),
            Command::Upgrade(modules, dependencies, package, upgrade_ticket) => {
                AuthContextCommand::Upgrade(
                    modules.clone(),
                    dependencies.clone(),
                    *package,
                    *upgrade_ticket,
                )
            }
        }
    }
}

/// Mirrors [`crate::transaction::CallArg`], matching the BCS layout expected
/// by the Move-side `ptb_call_arg::CallArg`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthContextCallArg {
    Pure(Vec<u8>),
    Object(ObjectArg),
}

impl From<&CallArg> for AuthContextCallArg {
    fn from(arg: &CallArg) -> Self {
        match arg {
            CallArg::Pure(bytes) => AuthContextCallArg::Pure(bytes.clone()),
            CallArg::Object(obj_arg) => AuthContextCallArg::Object(*obj_arg),
        }
    }
}

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
    use move_core_types::account_address::AccountAddress;

    use super::*;
    use crate::{
        base_types::{ObjectDigest, ObjectID, SequenceNumber},
        transaction::{
            Argument, CallArg, Command, ObjectArg, ProgrammableMoveCall, ProgrammableTransaction,
        },
        type_input::{StructInput, TypeInput},
    };

    // ── helpers ─────────────────────────────────────────────────────────────

    fn obj_id() -> ObjectID {
        ObjectID::from_hex_literal("0x0000000000000000000000000000000000000001").unwrap()
    }

    fn obj_ref() -> (ObjectID, SequenceNumber, ObjectDigest) {
        (
            obj_id(),
            SequenceNumber::from(1),
            ObjectDigest::new([1u8; 32]),
        )
    }

    /// BCS round-trip helper.
    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let bytes = bcs::to_bytes(value).unwrap();
        bcs::from_bytes(&bytes).unwrap()
    }

    // ── AuthContextCallArg ───────────────────────────────────────────────────

    #[test]
    fn call_arg_pure_round_trip() {
        let arg = AuthContextCallArg::Pure(vec![1, 2, 3]);
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_imm_or_owned_round_trip() {
        let arg = AuthContextCallArg::Object(ObjectArg::ImmOrOwnedObject(obj_ref()));
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_shared_object_round_trip() {
        let arg = AuthContextCallArg::Object(ObjectArg::SharedObject {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(5),
            mutable: true,
        });
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_receiving_round_trip() {
        let arg = AuthContextCallArg::Object(ObjectArg::Receiving(obj_ref()));
        assert_eq!(round_trip(&arg), arg);
    }

    // ── From<&CallArg> for AuthContextCallArg ────────────────────────────────

    #[test]
    fn call_arg_from_pure() {
        let data = vec![10, 20, 30];
        let converted = AuthContextCallArg::from(&CallArg::Pure(data.clone()));
        assert_eq!(converted, AuthContextCallArg::Pure(data));
    }

    #[test]
    fn call_arg_from_object() {
        let obj_arg = ObjectArg::ImmOrOwnedObject(obj_ref());
        let converted = AuthContextCallArg::from(&CallArg::Object(obj_arg));
        assert_eq!(converted, AuthContextCallArg::Object(obj_arg));
    }

    #[test]
    fn call_arg_from_call_arg() {
        let call_arg = CallArg::Pure(vec![99]);
        let converted = AuthContextCallArg::from(&call_arg);
        assert!(matches!(converted, AuthContextCallArg::Pure(_)));
    }

    // ── AuthContextCommand round-trips ────────────────────────────────────────

    fn sample_move_call() -> AuthContextCommand {
        AuthContextCommand::MoveCall(Box::new(AuthContextMoveCall {
            package: obj_id(),
            module: "my_module".to_string(),
            function: "my_func".to_string(),
            type_arguments: vec![TypeName {
                name: "u64".to_string(),
            }],
            arguments: vec![Argument::GasCoin, Argument::Input(0)],
        }))
    }

    #[test]
    fn command_move_call_round_trip() {
        assert_eq!(round_trip(&sample_move_call()), sample_move_call());
    }

    #[test]
    fn command_transfer_objects_round_trip() {
        let cmd = AuthContextCommand::TransferObjects(
            vec![Argument::Input(0), Argument::Result(1)],
            Argument::Input(2),
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_split_coins_round_trip() {
        let cmd = AuthContextCommand::SplitCoins(Argument::GasCoin, vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_merge_coins_round_trip() {
        let cmd = AuthContextCommand::MergeCoins(
            Argument::GasCoin,
            vec![Argument::Input(0), Argument::Input(1)],
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_publish_round_trip() {
        let cmd = AuthContextCommand::Publish(vec![vec![1, 2, 3]], vec![obj_id()]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_make_move_vec_with_type_round_trip() {
        let cmd = AuthContextCommand::MakeMoveVec(
            Some(TypeName {
                name: "0x2::coin::Coin<u64>".to_string(),
            }),
            vec![Argument::Input(0)],
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_make_move_vec_no_type_round_trip() {
        let cmd = AuthContextCommand::MakeMoveVec(None, vec![Argument::Result(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_upgrade_round_trip() {
        let cmd = AuthContextCommand::Upgrade(
            vec![vec![0xde, 0xad]],
            vec![obj_id()],
            obj_id(),
            Argument::Result(0),
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    // ── From<&Command> for AuthContextCommand ────────────────────────────────

    /// Primitive TypeInput variants (Bool, U8, …) must be converted to their
    /// canonical string representation as TypeName.
    #[test]
    fn command_from_move_call_primitive_type_input() {
        let cases = [
            (TypeInput::Bool, "bool"),
            (TypeInput::U8, "u8"),
            (TypeInput::U64, "u64"),
            (TypeInput::U128, "u128"),
            (TypeInput::U16, "u16"),
            (TypeInput::U32, "u32"),
            (TypeInput::U256, "u256"),
            (TypeInput::Address, "address"),
        ];
        for (type_input, expected_name) in cases {
            let cmd = Command::MoveCall(Box::new(ProgrammableMoveCall {
                package: obj_id(),
                module: "m".to_string(),
                function: "f".to_string(),
                type_arguments: vec![type_input],
                arguments: vec![],
            }));
            let AuthContextCommand::MoveCall(call) = AuthContextCommand::from(&cmd) else {
                panic!("expected MoveCall");
            };
            assert_eq!(
                call.type_arguments,
                vec![TypeName {
                    name: expected_name.to_string()
                }],
                "failed for {expected_name}"
            );
        }
    }

    /// Struct TypeInput must be converted to its canonical qualified name.
    #[test]
    fn command_from_move_call_struct_type_input() {
        let type_input = TypeInput::Struct(Box::new(StructInput {
            address: AccountAddress::from_hex_literal("0x2").unwrap(),
            module: "coin".to_string(),
            name: "Coin".to_string(),
            type_params: vec![TypeInput::U64],
        }));
        let expected = TypeName::from(&type_input);

        let cmd = Command::MoveCall(Box::new(ProgrammableMoveCall {
            package: obj_id(),
            module: "m".to_string(),
            function: "f".to_string(),
            type_arguments: vec![type_input],
            arguments: vec![],
        }));
        let AuthContextCommand::MoveCall(call) = AuthContextCommand::from(&cmd) else {
            panic!("expected MoveCall");
        };
        assert_eq!(call.type_arguments, vec![expected]);
    }

    #[test]
    fn command_from_make_move_vec_type_input_becomes_type_name() {
        let type_input = TypeInput::Bool;
        let expected = TypeName::from(&type_input);
        let cmd = Command::MakeMoveVec(Some(type_input), vec![Argument::Input(0)]);
        let AuthContextCommand::MakeMoveVec(name, _) = AuthContextCommand::from(&cmd) else {
            panic!("expected MakeMoveVec");
        };
        assert_eq!(name, Some(expected));
    }

    #[test]
    fn command_from_make_move_vec_none_type() {
        let cmd = Command::MakeMoveVec(None, vec![]);
        let AuthContextCommand::MakeMoveVec(name, elements) = AuthContextCommand::from(&cmd) else {
            panic!("expected MakeMoveVec");
        };
        assert!(name.is_none());
        assert!(elements.is_empty());
    }

    #[test]
    fn command_from_command() {
        let cmd = Command::MoveCall(Box::new(ProgrammableMoveCall {
            package: obj_id(),
            module: "m".to_string(),
            function: "f".to_string(),
            type_arguments: vec![TypeInput::U8],
            arguments: vec![],
        }));
        let converted = AuthContextCommand::from(&cmd);
        assert!(matches!(converted, AuthContextCommand::MoveCall(_)));
    }

    // ── AuthContext struct ───────────────────────────────────────────────────

    #[test]
    fn auth_context_new_from_components() {
        let ptb = ProgrammableTransaction {
            inputs: vec![CallArg::Pure(vec![0xab])],
            commands: vec![Command::MoveCall(Box::new(ProgrammableMoveCall {
                package: obj_id(),
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
