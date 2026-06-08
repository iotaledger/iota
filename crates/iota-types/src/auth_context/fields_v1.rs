// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Argument, Command, ObjectId, TypeTag};
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectRef, SequenceNumber},
    iota_serde::TypeName,
    transaction::CallArg,
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

/// Mirrors [`iota_sdk_types::MoveCall`] for use in
/// [`MoveCommand`], substituting [`TypeTag`] for a string in the type arguments
/// so that the type matches the BCS layout expected by the Move-side
/// `ptb_command::ProgrammableMoveCall`.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveProgrammableMoveCall {
    pub package: ObjectId,
    pub module: String,
    pub function: String,
    #[serde_as(as = "Vec<TypeName>")]
    pub type_arguments: Vec<TypeTag>,
    pub arguments: Vec<Argument>,
}

// ---------------------------------------------------------------------------
// MoveCommand
// ---------------------------------------------------------------------------

/// Mirrors [`iota_sdk_types::Command`], substituting [`TypeTag`] for
/// a string in `MoveCall` and `MakeMoveVec` so that
/// the type matches the BCS layout expected by the Move-side
/// `ptb_command::Command`.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveCommand {
    MoveCall(Box<MoveProgrammableMoveCall>),
    TransferObjects(Vec<Argument>, Argument),
    SplitCoins(Argument, Vec<Argument>),
    MergeCoins(Argument, Vec<Argument>),
    Publish(Vec<Vec<u8>>, Vec<ObjectId>),
    MakeMoveVec(
        #[serde_as(as = "Option<TypeName>")] Option<TypeTag>,
        Vec<Argument>,
    ),
    Upgrade(Vec<Vec<u8>>, Vec<ObjectId>, ObjectId, Argument),
}

impl From<&Command> for MoveCommand {
    fn from(cmd: &Command) -> Self {
        match cmd {
            Command::MoveCall(cmd) => MoveCommand::MoveCall(Box::new(MoveProgrammableMoveCall {
                package: cmd.package,
                module: cmd.module.to_string(),
                function: cmd.function.to_string(),
                type_arguments: cmd.type_arguments.clone(),
                arguments: cmd.arguments.clone(),
            })),
            Command::TransferObjects(cmd) => {
                MoveCommand::TransferObjects(cmd.objects.clone(), cmd.address)
            }
            Command::SplitCoins(cmd) => MoveCommand::SplitCoins(cmd.coin, cmd.amounts.clone()),
            Command::MergeCoins(cmd) => {
                MoveCommand::MergeCoins(cmd.coin, cmd.coins_to_merge.clone())
            }
            Command::Publish(cmd) => {
                MoveCommand::Publish(cmd.modules.clone(), cmd.dependencies.clone())
            }
            Command::MakeMoveVector(cmd) => {
                MoveCommand::MakeMoveVec(cmd.type_.clone(), cmd.elements.clone())
            }
            Command::Upgrade(cmd) => MoveCommand::Upgrade(
                cmd.modules.clone(),
                cmd.dependencies.clone(),
                cmd.package,
                cmd.ticket,
            ),
            _ => unimplemented!("a new Command enum variant was added and needs to be handled"),
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

/// Mirrors `ObjectArg`, matching the BCS layout expected
/// by the Move-side `ptb_call_arg::ObjectArg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveObjectArg {
    ImmOrOwnedObject(ObjectRef),
    SharedObject {
        id: ObjectId,
        initial_shared_version: SequenceNumber,
        mutable: bool,
    },
    Receiving(ObjectRef),
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
            CallArg::ImmutableOrOwned(obj_arg) => {
                MoveCallArg::Object(MoveObjectArg::ImmOrOwnedObject(*obj_arg))
            }
            CallArg::Shared(obj_arg) => MoveCallArg::Object(MoveObjectArg::SharedObject {
                id: obj_arg.object_id,
                initial_shared_version: obj_arg.initial_shared_version,
                mutable: obj_arg.mutable,
            }),
            CallArg::Receiving(obj_arg) => MoveCallArg::Object(MoveObjectArg::Receiving(*obj_arg)),
            _ => unimplemented!("a new CallArg enum variant was added and needs to be handled"),
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
    use std::str::FromStr;

    use iota_sdk_types::{Identifier, ObjectReference, StructTag, TypeTag};

    use super::*;
    use crate::{
        base_types::{IotaAddress, ObjectDigest, SequenceNumber},
        transaction::{CallArg, SharedObjectRef},
    };

    // ── helpers ─────────────────────────────────────────────────────────────

    fn obj_id() -> ObjectId {
        ObjectId::from_prefixed_short_hex("0x0000000000000000000000000000000000000001").unwrap()
    }

    fn obj_ref() -> ObjectReference {
        ObjectReference {
            object_id: obj_id(),
            version: SequenceNumber::from(1),
            digest: ObjectDigest::new([1u8; 32]),
        }
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
        let converted = MoveCallArg::from(&CallArg::ImmutableOrOwned(obj_ref()));
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

    // ── BCS compatibility: MoveCallArg ↔ CallArg ─────────────────────

    #[test]
    fn call_arg_bcs_compatible_imm_or_owned() {
        let tx_arg = CallArg::ImmutableOrOwned(obj_ref());
        let ctx_arg = MoveCallArg::from(&tx_arg);
        assert_eq!(
            bcs::to_bytes(&tx_arg).unwrap(),
            bcs::to_bytes(&ctx_arg).unwrap()
        );
    }

    #[test]
    fn call_arg_bcs_compatible_shared() {
        let tx_arg = CallArg::Shared(SharedObjectRef::new(
            obj_id(),
            SequenceNumber::from(5),
            true,
        ));
        let ctx_arg = MoveCallArg::from(&tx_arg);
        assert_eq!(
            bcs::to_bytes(&tx_arg).unwrap(),
            bcs::to_bytes(&ctx_arg).unwrap()
        );
    }

    #[test]
    fn call_arg_bcs_compatible_receiving() {
        let tx_arg = CallArg::Receiving(obj_ref());
        let ctx_arg = MoveCallArg::from(&tx_arg);
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
            type_arguments: vec![TypeTag::U64],
            arguments: vec![Argument::Gas, Argument::Input(0)],
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
        let cmd = MoveCommand::SplitCoins(Argument::Gas, vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn command_merge_coins_round_trip() {
        let cmd =
            MoveCommand::MergeCoins(Argument::Gas, vec![Argument::Input(0), Argument::Input(1)]);
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
            Some(TypeTag::from_str("0x2::coin::Coin<u64>").unwrap()),
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

    /// Primitive TypeTag variants (Bool, U8, …) must be converted to their
    /// canonical string representation as TypeTag.
    #[test]
    fn command_from_move_call_primitive_type_tag() {
        let cases = [
            (TypeTag::Bool, "bool"),
            (TypeTag::U8, "u8"),
            (TypeTag::U64, "u64"),
            (TypeTag::U128, "u128"),
            (TypeTag::U16, "u16"),
            (TypeTag::U32, "u32"),
            (TypeTag::U256, "u256"),
            (TypeTag::Address, "address"),
        ];
        for (type_tag, expected_name) in cases {
            let cmd = Command::new_move_call(
                obj_id(),
                Identifier::new_unchecked("m"),
                Identifier::new_unchecked("f"),
                vec![type_tag],
                vec![],
            );
            let MoveCommand::MoveCall(call) = MoveCommand::from(&cmd) else {
                panic!("expected MoveCall");
            };
            assert_eq!(
                call.type_arguments,
                vec![TypeTag::from_str(expected_name).unwrap()],
                "failed for {expected_name}"
            );
        }
    }

    /// Struct TypeTag must be converted to its canonical qualified name.
    #[test]
    fn command_from_move_call_struct_type_tag() {
        let expected = TypeTag::Struct(Box::new(StructTag::new(
            IotaAddress::FRAMEWORK,
            "coin",
            "Coin",
            vec![TypeTag::U64],
        )));

        let cmd = Command::new_move_call(
            obj_id(),
            Identifier::new_unchecked("m"),
            Identifier::new_unchecked("f"),
            vec![expected.clone()],
            vec![],
        );
        let MoveCommand::MoveCall(call) = MoveCommand::from(&cmd) else {
            panic!("expected MoveCall");
        };
        assert_eq!(call.type_arguments, vec![expected]);
    }

    #[test]
    fn command_from_make_move_vec_type_tag_becomes_type_name() {
        let expected = TypeTag::Bool;
        let cmd = Command::new_make_move_vector(Some(expected.clone()), vec![Argument::Input(0)]);
        let MoveCommand::MakeMoveVec(name, _) = MoveCommand::from(&cmd) else {
            panic!("expected MakeMoveVec");
        };
        assert_eq!(name, Some(expected));
    }

    #[test]
    fn command_from_make_move_vec_none_type() {
        let cmd = Command::new_make_move_vector(None, vec![]);
        let MoveCommand::MakeMoveVec(name, elements) = MoveCommand::from(&cmd) else {
            panic!("expected MakeMoveVec");
        };
        assert!(name.is_none());
        assert!(elements.is_empty());
    }

    #[test]
    fn command_from_command() {
        let cmd = Command::new_move_call(
            obj_id(),
            Identifier::new_unchecked("m"),
            Identifier::new_unchecked("f"),
            vec![TypeTag::U8],
            vec![],
        );
        let converted = MoveCommand::from(&cmd);
        assert!(matches!(converted, MoveCommand::MoveCall(_)));
    }
}
