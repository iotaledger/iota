// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectDigest, ObjectID, SequenceNumber},
    transaction::{Argument, CallArg, Command, ObjectArg},
    type_input::TypeName,
};

// ---------------------------------------------------------------------------
// Module / struct name constants
// ---------------------------------------------------------------------------

pub const CALL_ARG_MODULE_NAME: &IdentStr = ident_str!("ptb_call_arg");
pub const CALL_ARG_STRUCT_NAME: &IdentStr = ident_str!("CallArg");
pub const OBJECT_ARG_STRUCT_NAME: &IdentStr = ident_str!("ObjectArg");
pub const OBJECT_REF_STRUCT_NAME: &IdentStr = ident_str!("ObjectRef");

pub const COMMAND_MODULE_NAME: &IdentStr = ident_str!("ptb_command");
pub const COMMAND_STRUCT_NAME: &IdentStr = ident_str!("Command");
pub const ARGUMENT_STRUCT_NAME: &IdentStr = ident_str!("Argument");
pub const PROGRAMMABLE_MOVE_CALL_STRUCT_NAME: &IdentStr = ident_str!("ProgrammableMoveCall");
pub const TRANSFER_OBJECTS_DATA_STRUCT_NAME: &IdentStr = ident_str!("TransferObjectsData");
pub const SPLIT_COINS_DATA_STRUCT_NAME: &IdentStr = ident_str!("SplitCoinsData");
pub const MERGE_COINS_DATA_STRUCT_NAME: &IdentStr = ident_str!("MergeCoinsData");
pub const PUBLISH_DATA_STRUCT_NAME: &IdentStr = ident_str!("PublishData");
pub const MAKE_MOVE_VEC_DATA_STRUCT_NAME: &IdentStr = ident_str!("MakeMoveVecData");
pub const UPGRADE_DATA_STRUCT_NAME: &IdentStr = ident_str!("UpgradeData");

// ---------------------------------------------------------------------------
// MoveProgrammableMoveCall
// ---------------------------------------------------------------------------

/// Mirrors [`crate::transaction::ProgrammableMoveCall`] for use in
/// [`MoveCommand`], substituting [`TypeName`] for
/// [`crate::type_input::TypeInput`] so that the type can derive
/// [`Serialize`]/[`Deserialize`] without a custom implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveProgrammableMoveCall {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
    pub type_arguments: Vec<TypeName>,
    pub arguments: Vec<Argument>,
}

// ---------------------------------------------------------------------------
// MoveCommand
// ---------------------------------------------------------------------------

/// Mirrors [`crate::transaction::Command`], substituting [`TypeName`] for
/// [`crate::type_input::TypeInput`] in `MoveCall` and `MakeMoveVec` so that
/// the type matches the BCS layout expected by the Move-side
/// `ptb_command::Command`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveCommand {
    MoveCall(Box<MoveProgrammableMoveCall>),
    TransferObjects(Vec<Argument>, Argument),
    SplitCoins(Argument, Vec<Argument>),
    MergeCoins(Argument, Vec<Argument>),
    Publish(Vec<Vec<u8>>, Vec<ObjectID>),
    MakeMoveVec(Option<TypeName>, Vec<Argument>),
    Upgrade(Vec<Vec<u8>>, Vec<ObjectID>, ObjectID, Argument),
}

impl From<&Command> for MoveCommand {
    fn from(cmd: &Command) -> Self {
        match cmd {
            Command::MoveCall(m) => MoveCommand::MoveCall(Box::new(MoveProgrammableMoveCall {
                package: m.package,
                module: m.module.clone(),
                function: m.function.clone(),
                type_arguments: m.type_arguments.iter().map(TypeName::from).collect(),
                arguments: m.arguments.clone(),
            })),
            Command::TransferObjects(objects, recipient) => {
                MoveCommand::TransferObjects(objects.clone(), *recipient)
            }
            Command::SplitCoins(coin, amounts) => MoveCommand::SplitCoins(*coin, amounts.clone()),
            Command::MergeCoins(target_coin, source_coins) => {
                MoveCommand::MergeCoins(*target_coin, source_coins.clone())
            }
            Command::Publish(modules, dependencies) => {
                MoveCommand::Publish(modules.clone(), dependencies.clone())
            }
            Command::MakeMoveVec(type_arg, elements) => {
                MoveCommand::MakeMoveVec(type_arg.as_ref().map(TypeName::from), elements.clone())
            }
            Command::Upgrade(modules, dependencies, package, upgrade_ticket) => {
                MoveCommand::Upgrade(
                    modules.clone(),
                    dependencies.clone(),
                    *package,
                    *upgrade_ticket,
                )
            }
        }
    }
}

impl MoveCommand {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: COMMAND_MODULE_NAME.to_owned(),
            name: COMMAND_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// MoveCallArg
// ---------------------------------------------------------------------------

/// Mirrors [`crate::transaction::ObjectArg`], matching the BCS layout expected
/// by the Move-side `ptb_call_arg::ObjectArg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveObjectArg {
    ImmOrOwnedObject((ObjectID, SequenceNumber, ObjectDigest)),
    SharedObject {
        id: ObjectID,
        initial_shared_version: SequenceNumber,
        mutable: bool,
    },
    Receiving((ObjectID, SequenceNumber, ObjectDigest)),
}

impl From<&ObjectArg> for MoveObjectArg {
    fn from(obj: &ObjectArg) -> Self {
        match obj {
            ObjectArg::ImmOrOwnedObject(r) => MoveObjectArg::ImmOrOwnedObject(*r),
            ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable,
            } => MoveObjectArg::SharedObject {
                id: *id,
                initial_shared_version: *initial_shared_version,
                mutable: *mutable,
            },
            ObjectArg::Receiving(r) => MoveObjectArg::Receiving(*r),
        }
    }
}

impl MoveObjectArg {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: CALL_ARG_MODULE_NAME.to_owned(),
            name: OBJECT_ARG_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }
}

/// Mirrors [`crate::transaction::CallArg`], matching the BCS layout expected
/// by the Move-side `ptb_call_arg::CallArg`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveCallArg {
    Pure(Vec<u8>),
    Object(MoveObjectArg),
}

impl From<&CallArg> for MoveCallArg {
    fn from(arg: &CallArg) -> Self {
        match arg {
            CallArg::Pure(bytes) => MoveCallArg::Pure(bytes.clone()),
            CallArg::Object(obj_arg) => MoveCallArg::Object(MoveObjectArg::from(obj_arg)),
        }
    }
}

impl MoveCallArg {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: CALL_ARG_MODULE_NAME.to_owned(),
            name: CALL_ARG_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use move_core_types::account_address::AccountAddress;

    use super::*;
    use crate::{
        base_types::{ObjectDigest, ObjectID, SequenceNumber},
        transaction::{Argument, CallArg, Command, ObjectArg, ProgrammableMoveCall},
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

    // ── MoveCallArg ───────────────────────────────────────────────────

    #[test]
    fn call_arg_pure_round_trip() {
        let arg = MoveCallArg::Pure(vec![1, 2, 3]);
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_imm_or_owned_round_trip() {
        let arg = MoveCallArg::Object(MoveObjectArg::ImmOrOwnedObject(obj_ref()));
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_shared_object_round_trip() {
        let arg = MoveCallArg::Object(MoveObjectArg::SharedObject {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(5),
            mutable: true,
        });
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn call_arg_receiving_round_trip() {
        let arg = MoveCallArg::Object(MoveObjectArg::Receiving(obj_ref()));
        assert_eq!(round_trip(&arg), arg);
    }

    // ── From<&CallArg> for MoveCallArg ────────────────────────────────

    #[test]
    fn call_arg_from_pure() {
        let data = vec![10, 20, 30];
        let converted = MoveCallArg::from(&CallArg::Pure(data.clone()));
        assert_eq!(converted, MoveCallArg::Pure(data));
    }

    #[test]
    fn call_arg_from_object() {
        let obj_arg = ObjectArg::ImmOrOwnedObject(obj_ref());
        let converted = MoveCallArg::from(&CallArg::Object(obj_arg));
        assert_eq!(
            converted,
            MoveCallArg::Object(MoveObjectArg::ImmOrOwnedObject(obj_ref()))
        );
    }

    #[test]
    fn call_arg_from_call_arg() {
        let call_arg = CallArg::Pure(vec![99]);
        let converted = MoveCallArg::from(&call_arg);
        assert!(matches!(converted, MoveCallArg::Pure(_)));
    }

    // ── BCS compatibility: MoveObjectArg ↔ ObjectArg ─────────────────

    #[test]
    fn object_arg_bcs_compatible_imm_or_owned() {
        let tx_arg = ObjectArg::ImmOrOwnedObject(obj_ref());
        let ctx_arg = MoveObjectArg::from(&tx_arg);
        assert_eq!(
            bcs::to_bytes(&tx_arg).unwrap(),
            bcs::to_bytes(&ctx_arg).unwrap()
        );
    }

    #[test]
    fn object_arg_bcs_compatible_shared() {
        let tx_arg = ObjectArg::SharedObject {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(5),
            mutable: true,
        };
        let ctx_arg = MoveObjectArg::from(&tx_arg);
        assert_eq!(
            bcs::to_bytes(&tx_arg).unwrap(),
            bcs::to_bytes(&ctx_arg).unwrap()
        );
    }

    #[test]
    fn object_arg_bcs_compatible_receiving() {
        let tx_arg = ObjectArg::Receiving(obj_ref());
        let ctx_arg = MoveObjectArg::from(&tx_arg);
        assert_eq!(
            bcs::to_bytes(&tx_arg).unwrap(),
            bcs::to_bytes(&ctx_arg).unwrap()
        );
    }

    // ── MoveCommand round-trips ────────────────────────────────────────

    fn sample_move_call() -> MoveCommand {
        MoveCommand::MoveCall(Box::new(MoveProgrammableMoveCall {
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
        let cmd = MoveCommand::TransferObjects(
            vec![Argument::Input(0), Argument::Result(1)],
            Argument::Input(2),
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_split_coins_round_trip() {
        let cmd = MoveCommand::SplitCoins(Argument::GasCoin, vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_merge_coins_round_trip() {
        let cmd = MoveCommand::MergeCoins(
            Argument::GasCoin,
            vec![Argument::Input(0), Argument::Input(1)],
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_publish_round_trip() {
        let cmd = MoveCommand::Publish(vec![vec![1, 2, 3]], vec![obj_id()]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_make_move_vec_with_type_round_trip() {
        let cmd = MoveCommand::MakeMoveVec(
            Some(TypeName {
                name: "0x2::coin::Coin<u64>".to_string(),
            }),
            vec![Argument::Input(0)],
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_make_move_vec_no_type_round_trip() {
        let cmd = MoveCommand::MakeMoveVec(None, vec![Argument::Result(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_upgrade_round_trip() {
        let cmd = MoveCommand::Upgrade(
            vec![vec![0xde, 0xad]],
            vec![obj_id()],
            obj_id(),
            Argument::Result(0),
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    // ── From<&Command> for MoveCommand ────────────────────────────────

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
            let MoveCommand::MoveCall(call) = MoveCommand::from(&cmd) else {
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
        let MoveCommand::MoveCall(call) = MoveCommand::from(&cmd) else {
            panic!("expected MoveCall");
        };
        assert_eq!(call.type_arguments, vec![expected]);
    }

    #[test]
    fn command_from_make_move_vec_type_input_becomes_type_name() {
        let type_input = TypeInput::Bool;
        let expected = TypeName::from(&type_input);
        let cmd = Command::MakeMoveVec(Some(type_input), vec![Argument::Input(0)]);
        let MoveCommand::MakeMoveVec(name, _) = MoveCommand::from(&cmd) else {
            panic!("expected MakeMoveVec");
        };
        assert_eq!(name, Some(expected));
    }

    #[test]
    fn command_from_make_move_vec_none_type() {
        let cmd = Command::MakeMoveVec(None, vec![]);
        let MoveCommand::MakeMoveVec(name, elements) = MoveCommand::from(&cmd) else {
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
        let converted = MoveCommand::from(&cmd);
        assert!(matches!(converted, MoveCommand::MoveCall(_)));
    }
}
