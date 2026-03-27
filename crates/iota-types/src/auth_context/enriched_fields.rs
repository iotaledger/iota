// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Enriched versions of the `AuthContext` types that carry additional metadata
//! (type names, `is_entry` markers, return types) compared to the base types.
use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{ObjectDigest, ObjectID, SequenceNumber},
    error::{ExecutionError, ExecutionErrorKind},
    object::Object,
    transaction::{Argument, Command, ObjectArg, ProgrammableMoveCall},
    type_input::TypeName,
};

// ---------------------------------------------------------------------------
// Module / struct name constants
// ---------------------------------------------------------------------------

pub const ENRICHED_CALL_ARG_MODULE_NAME: &IdentStr = ident_str!("enriched_call_arg");
pub const ENRICHED_CALL_ARG_STRUCT_NAME: &IdentStr = ident_str!("EnrichedCallArg");
pub const ENRICHED_COMMAND_MODULE_NAME: &IdentStr = ident_str!("enriched_command");
pub const ENRICHED_COMMAND_STRUCT_NAME: &IdentStr = ident_str!("EnrichedCommand");

// ---------------------------------------------------------------------------
// Object argument helpers
// ---------------------------------------------------------------------------

/// Enriched representation of an immutable-or-owned object argument.
///
/// Extends the plain `(ObjectID, SequenceNumber, ObjectDigest)` tuple with
/// mutability information and the fully-qualified type name of the object.
///
/// `mutable` is `true` when the object is borrowed as `&mut T` in at least one
/// command of the PTB.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct MoveImmOrOwnedObjectArg {
    pub id: ObjectID,
    pub version: SequenceNumber,
    pub digest: ObjectDigest,
    pub mutable: bool,
    pub type_name: TypeName,
}

/// Enriched representation of a shared object argument.
///
/// Adds the current object digest (looked up from state) and the
/// fully-qualified type name on top of the plain shared-object fields.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct MoveSharedObjectArg {
    pub id: ObjectID,
    pub initial_shared_version: SequenceNumber,
    pub mutable: bool,
    pub digest: ObjectDigest,
    pub type_name: TypeName,
}

// ---------------------------------------------------------------------------
// MoveEnrichedCallArg
// ---------------------------------------------------------------------------

/// Enriched counterpart of CallArg.
/// Adds type names for pure values and objects, mutability information for
/// immutable/owned objects, and the current digest for shared objects.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum MoveEnrichedCallArg {
    /// A pure (non-object) value together with its Move type name.
    Pure { value: Vec<u8>, type_name: TypeName },
    /// An immutable or owned object argument.
    ImmOrOwnedObject(MoveImmOrOwnedObjectArg),
    /// A shared object argument.
    SharedObject(MoveSharedObjectArg),
    /// An object that is being received in this transaction.
    Receiving(MoveImmOrOwnedObjectArg),
}

impl MoveEnrichedCallArg {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: ENRICHED_CALL_ARG_MODULE_NAME.to_owned(),
            name: ENRICHED_CALL_ARG_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// MoveEnrichedProgrammableMoveCall
// ---------------------------------------------------------------------------

/// Enriched counterpart of [`crate::auth_context::MoveProgrammableMoveCall`].
///
/// Adds `is_entry` and the list of return-type names on top of the base call
/// data.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct MoveEnrichedProgrammableMoveCall {
    /// The package containing the module and function.
    pub package: ObjectID,
    /// The specific module in the package containing the function.
    pub module: String,
    /// The function to be called.
    pub function: String,
    /// Whether this function is marked as an `entry` function.
    pub is_entry: bool,
    /// The type arguments to the function (resolved to canonical names).
    pub type_arguments: Vec<TypeName>,
    /// The arguments to the function.
    pub arguments: Vec<Argument>,
    /// The return-type names of the function.
    pub returns: Vec<TypeName>,
}

// ---------------------------------------------------------------------------
// MoveEnrichedCommand
// ---------------------------------------------------------------------------

/// Enriched counterpart of [`crate::auth_context::MoveCommand`].
///
/// Only `MoveCall` differs from its base variant – all other commands are
/// identical to `MoveCommand`.
///
/// **BCS variant indices** (must match Move
/// `enriched_command::MoveEnrichedCommand` and the existing `MoveCommand` /
/// `Command` ordering):
/// * 0 – `MoveCall`
/// * 1 – `TransferObjects`
/// * 2 – `SplitCoins`
/// * 3 – `MergeCoins`
/// * 4 – `Publish`
/// * 5 – `MakeMoveVec`
/// * 6 – `Upgrade`
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub enum MoveEnrichedCommand {
    MoveCall(Box<MoveEnrichedProgrammableMoveCall>),
    TransferObjects(Vec<Argument>, Argument),
    SplitCoins(Argument, Vec<Argument>),
    MergeCoins(Argument, Vec<Argument>),
    Publish(Vec<Vec<u8>>, Vec<ObjectID>),
    MakeMoveVec(Option<TypeName>, Vec<Argument>),
    Upgrade(Vec<Vec<u8>>, Vec<ObjectID>, ObjectID, Argument),
}

impl MoveEnrichedCommand {
    pub fn type_() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: ENRICHED_COMMAND_MODULE_NAME.to_owned(),
            name: ENRICHED_COMMAND_STRUCT_NAME.to_owned(),
            type_params: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Enrichment helpers
// ---------------------------------------------------------------------------

/// Enriches an [`ObjectArg`] into an [`MoveEnrichedCallArg`].
///
/// The caller must guarantee that `object` exists (i.e. it was already
/// retrieved from the authenticator input objects by `obj_arg.id()`).
/// `mutable` reflects whether the input is passed as `&mut T` in any
/// `MoveCall` command of the PTB.
///
/// Returns an error only when the object is not a Move object (e.g. a package)
/// and therefore has no struct tag — which is an invariant violation.
pub fn enrich_object_arg(
    obj_arg: &ObjectArg,
    object: &Object,
    mutable: bool,
) -> Result<MoveEnrichedCallArg, ExecutionError> {
    let struct_tag = object.struct_tag().ok_or_else(|| {
        ExecutionError::new_with_source(
            ExecutionErrorKind::VMInvariantViolation,
            format!(
                "Object {} is not a Move object and has no struct tag",
                object.id()
            ),
        )
    })?;

    let type_name = TypeName::from(&TypeTag::from(struct_tag));

    match obj_arg {
        ObjectArg::ImmOrOwnedObject((id, version, digest)) => Ok(
            MoveEnrichedCallArg::ImmOrOwnedObject(MoveImmOrOwnedObjectArg {
                id: *id,
                version: *version,
                digest: *digest,
                mutable,
                type_name,
            }),
        ),
        ObjectArg::SharedObject {
            id,
            initial_shared_version,
            mutable: shared_mutable,
        } => Ok(MoveEnrichedCallArg::SharedObject(MoveSharedObjectArg {
            id: *id,
            initial_shared_version: *initial_shared_version,
            mutable: *shared_mutable,
            digest: object.digest(),
            type_name,
        })),
        ObjectArg::Receiving((id, version, digest)) => {
            Ok(MoveEnrichedCallArg::Receiving(MoveImmOrOwnedObjectArg {
                id: *id,
                version: *version,
                digest: *digest,
                mutable: false,
                type_name,
            }))
        }
    }
}

/// Returns the pure-input arguments of a built-in PTB command together with
/// their expected Move type, derived from protocol constants.
///
/// The Move VM has no knowledge of built-in commands, so types must be
/// inferred from the PTB protocol:
///   - `TransferObjects` recipient     → `address`
///   - `SplitCoins` amounts            → `u64`
///   - `MakeMoveVec(Some<T>, _)`       → `T` (per element)
///   - `MakeMoveVec(None, _)`          → `None` per element (type unknown)
///   - everything else                 → no pure inputs
pub fn pure_param_types_for_cmd(cmd: &Command) -> Vec<(&Argument, Option<TypeTag>)> {
    match cmd {
        Command::TransferObjects(_, recipient) => vec![(recipient, Some(TypeTag::Address))],
        Command::SplitCoins(_, amounts) => {
            amounts.iter().map(|a| (a, Some(TypeTag::U64))).collect()
        }
        Command::MakeMoveVec(ty_input_opt, args) => {
            let tag = ty_input_opt.as_ref().and_then(|ty| ty.as_type_tag().ok());
            args.iter().map(|a| (a, tag.clone())).collect()
        }
        _ => vec![],
    }
}

/// Converts a non-[`Command::MoveCall`] command into a
/// [`MoveEnrichedCommand`].
///
/// Call [`enrich_move_call_command`] for [`Command::MoveCall`] variants, which
/// require VM-resolved `is_entry` and return-type metadata.
pub fn enrich_non_move_call_command(cmd: &Command) -> MoveEnrichedCommand {
    match cmd {
        Command::MoveCall(_) => {
            unreachable!("use enrich_move_call_command for MoveCall variants")
        }
        Command::TransferObjects(objects, recipient) => {
            MoveEnrichedCommand::TransferObjects(objects.clone(), *recipient)
        }
        Command::SplitCoins(coin, amounts) => {
            MoveEnrichedCommand::SplitCoins(*coin, amounts.clone())
        }
        Command::MergeCoins(target, sources) => {
            MoveEnrichedCommand::MergeCoins(*target, sources.clone())
        }
        Command::Publish(modules, deps) => {
            MoveEnrichedCommand::Publish(modules.clone(), deps.clone())
        }
        Command::MakeMoveVec(type_arg, elements) => MoveEnrichedCommand::MakeMoveVec(
            type_arg.as_ref().map(TypeName::from),
            elements.clone(),
        ),
        Command::Upgrade(modules, deps, package, ticket) => {
            MoveEnrichedCommand::Upgrade(modules.clone(), deps.clone(), *package, *ticket)
        }
    }
}

/// Converts a [`ProgrammableMoveCall`] into a [`MoveEnrichedCommand`].
///
/// `is_entry` and `returns` are VM-resolved metadata that must be provided by
/// the caller — this function never falls back to defaults.
pub fn enrich_move_call_command(
    call: &ProgrammableMoveCall,
    is_entry: bool,
    returns: Vec<TypeName>,
) -> MoveEnrichedCommand {
    MoveEnrichedCommand::MoveCall(Box::new(MoveEnrichedProgrammableMoveCall {
        package: call.package,
        module: call.module.clone(),
        function: call.function.clone(),
        is_entry,
        type_arguments: call.type_arguments.iter().map(TypeName::from).collect(),
        arguments: call.arguments.clone(),
        returns,
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        base_types::{ObjectDigest, ObjectID, SequenceNumber},
        transaction::Argument,
        type_input::TypeName,
    };

    fn obj_id() -> ObjectID {
        ObjectID::from_hex_literal("0x0000000000000000000000000000000000000001").unwrap()
    }

    fn type_name(s: &str) -> TypeName {
        TypeName {
            name: s.to_string(),
        }
    }

    fn obj_digest() -> ObjectDigest {
        ObjectDigest::new([1u8; 32])
    }

    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let bytes = bcs::to_bytes(value).unwrap();
        bcs::from_bytes(&bytes).unwrap()
    }

    // ── MoveImmOrOwnedObjectArg ──────────────────────────────────────────────

    #[test]
    fn imm_or_owned_object_arg_round_trip() {
        let arg = MoveImmOrOwnedObjectArg {
            id: obj_id(),
            version: SequenceNumber::from(3),
            digest: obj_digest(),
            mutable: true,
            type_name: type_name("0x2::coin::Coin<u64>"),
        };
        assert_eq!(round_trip(&arg), arg);
    }

    // ── MoveSharedObjectArg ──────────────────────────────────────────────────

    #[test]
    fn shared_object_arg_round_trip() {
        let arg = MoveSharedObjectArg {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(1),
            mutable: false,
            digest: obj_digest(),
            type_name: type_name("0x2::balance::Balance<u64>"),
        };
        assert_eq!(round_trip(&arg), arg);
    }

    // ── MoveEnrichedCallArg ──────────────────────────────────────────────────

    #[test]
    fn enriched_call_arg_pure_round_trip() {
        let arg = MoveEnrichedCallArg::Pure {
            value: vec![1, 2, 3],
            type_name: type_name("u64"),
        };
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn enriched_call_arg_imm_or_owned_round_trip() {
        let arg = MoveEnrichedCallArg::ImmOrOwnedObject(MoveImmOrOwnedObjectArg {
            id: obj_id(),
            version: SequenceNumber::from(1),
            digest: obj_digest(),
            mutable: false,
            type_name: type_name("0x2::object::UID"),
        });
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn enriched_call_arg_shared_round_trip() {
        let arg = MoveEnrichedCallArg::SharedObject(MoveSharedObjectArg {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(1),
            mutable: true,
            digest: obj_digest(),
            type_name: type_name("0x2::clock::Clock"),
        });
        assert_eq!(round_trip(&arg), arg);
    }

    #[test]
    fn enriched_call_arg_receiving_round_trip() {
        let arg = MoveEnrichedCallArg::Receiving(MoveImmOrOwnedObjectArg {
            id: obj_id(),
            version: SequenceNumber::from(5),
            digest: obj_digest(),
            mutable: false,
            type_name: type_name("0x2::coin::Coin<u64>"),
        });
        assert_eq!(round_trip(&arg), arg);
    }

    /// BCS variant indices: Pure=0, ImmOrOwnedObject=1, SharedObject=2,
    /// Receiving=3. Verify by checking the first byte of serialized form.
    #[test]
    fn enriched_call_arg_variant_indices() {
        let pure = MoveEnrichedCallArg::Pure {
            value: vec![],
            type_name: type_name("u8"),
        };
        let imm = MoveEnrichedCallArg::ImmOrOwnedObject(MoveImmOrOwnedObjectArg {
            id: obj_id(),
            version: SequenceNumber::from(1),
            digest: obj_digest(),
            mutable: false,
            type_name: type_name("u8"),
        });
        let shared = MoveEnrichedCallArg::SharedObject(MoveSharedObjectArg {
            id: obj_id(),
            initial_shared_version: SequenceNumber::from(1),
            mutable: false,
            digest: obj_digest(),
            type_name: type_name("u8"),
        });
        let receiving = MoveEnrichedCallArg::Receiving(MoveImmOrOwnedObjectArg {
            id: obj_id(),
            version: SequenceNumber::from(1),
            digest: obj_digest(),
            mutable: false,
            type_name: type_name("u8"),
        });

        assert_eq!(bcs::to_bytes(&pure).unwrap()[0], 0);
        assert_eq!(bcs::to_bytes(&imm).unwrap()[0], 1);
        assert_eq!(bcs::to_bytes(&shared).unwrap()[0], 2);
        assert_eq!(bcs::to_bytes(&receiving).unwrap()[0], 3);
    }

    // ── MoveEnrichedProgrammableMoveCall ─────────────────────────────────────

    #[test]
    fn enriched_programmable_move_call_round_trip() {
        let call = MoveEnrichedProgrammableMoveCall {
            package: obj_id(),
            module: "my_module".to_string(),
            function: "my_func".to_string(),
            is_entry: true,
            type_arguments: vec![type_name("u64")],
            arguments: vec![Argument::GasCoin, Argument::Input(0)],
            returns: vec![type_name("0x2::coin::Coin<u64>")],
        };
        assert_eq!(round_trip(&call), call);
    }

    // ── MoveEnrichedCommand ──────────────────────────────────────────────────

    #[test]
    fn enriched_command_move_call_round_trip() {
        let cmd = MoveEnrichedCommand::MoveCall(Box::new(MoveEnrichedProgrammableMoveCall {
            package: obj_id(),
            module: "m".to_string(),
            function: "f".to_string(),
            is_entry: false,
            type_arguments: vec![],
            arguments: vec![Argument::Input(0)],
            returns: vec![],
        }));
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_transfer_objects_round_trip() {
        let cmd =
            MoveEnrichedCommand::TransferObjects(vec![Argument::Input(0)], Argument::Input(1));
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_split_coins_round_trip() {
        let cmd = MoveEnrichedCommand::SplitCoins(Argument::GasCoin, vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_merge_coins_round_trip() {
        let cmd = MoveEnrichedCommand::MergeCoins(Argument::GasCoin, vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_publish_round_trip() {
        let cmd = MoveEnrichedCommand::Publish(vec![vec![1, 2, 3]], vec![obj_id()]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_make_move_vec_round_trip() {
        let cmd =
            MoveEnrichedCommand::MakeMoveVec(Some(type_name("u64")), vec![Argument::Input(0)]);
        assert_eq!(round_trip(&cmd), cmd);
    }

    #[test]
    fn enriched_command_upgrade_round_trip() {
        let cmd = MoveEnrichedCommand::Upgrade(
            vec![vec![0xde, 0xad]],
            vec![obj_id()],
            obj_id(),
            Argument::Result(0),
        );
        assert_eq!(round_trip(&cmd), cmd);
    }

    /// BCS variant indices must match Move `MoveEnrichedCommand` variant order.
    #[test]
    fn enriched_command_variant_indices() {
        let move_call = MoveEnrichedCommand::MoveCall(Box::new(MoveEnrichedProgrammableMoveCall {
            package: obj_id(),
            module: "m".to_string(),
            function: "f".to_string(),
            is_entry: false,
            type_arguments: vec![],
            arguments: vec![],
            returns: vec![],
        }));
        let transfer = MoveEnrichedCommand::TransferObjects(vec![], Argument::GasCoin);
        let split = MoveEnrichedCommand::SplitCoins(Argument::GasCoin, vec![]);
        let merge = MoveEnrichedCommand::MergeCoins(Argument::GasCoin, vec![]);
        let publish = MoveEnrichedCommand::Publish(vec![], vec![]);
        let make_vec = MoveEnrichedCommand::MakeMoveVec(None, vec![]);
        let upgrade = MoveEnrichedCommand::Upgrade(vec![], vec![], obj_id(), Argument::GasCoin);

        assert_eq!(bcs::to_bytes(&move_call).unwrap()[0], 0);
        assert_eq!(bcs::to_bytes(&transfer).unwrap()[0], 1);
        assert_eq!(bcs::to_bytes(&split).unwrap()[0], 2);
        assert_eq!(bcs::to_bytes(&merge).unwrap()[0], 3);
        assert_eq!(bcs::to_bytes(&publish).unwrap()[0], 4);
        assert_eq!(bcs::to_bytes(&make_vec).unwrap()[0], 5);
        assert_eq!(bcs::to_bytes(&upgrade).unwrap()[0], 6);
    }
}
