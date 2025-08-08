// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use std::ascii::String;

// === Constants ===
const EBadTxHashLength: u64 = 0;

// === Enums ===

public enum CallArgType has copy, drop {
    PureData(vector<u8>),
    ObjectData(ObjectArgType),
}

public enum CommandArgType has copy, drop {
    MoveCall(ProgrammableMoveCall),
    TransferObjects(TransferObjectsData),
    SplitCoins(SplitCoinsData),
    MergeCoins(MergeCoinsData),
    Publish(PublishData),
    MakeMoveVec(MakeMoveVecData),
    Upgrade(UpgradeData),
}

public enum TypeInput has copy, drop {
    BoolType(bool),
    U8Type(bool),
    U16Type(bool),
    U32Type(bool),
    U64Type(bool),
    U128Type(bool),
    U256Type(bool),
    AddressType(bool),
    SignerType(bool),
    // TODO: VectorType(Box<TypeInput>)
    // TODO: StructType(StructInput)
}

public enum ArgumentType has copy, drop {
    GasCoin,
    Input(u16),
    Result(u16),
    NestedResult(u16, u16),
}

public enum ObjectArgType has copy, drop {
    ImmOrOwnedObject(ObjectRef),
    SharedObject(SharedObjectData),
    ReceivingObject(ObjectRef),
}

// === Structs ===

public struct CallArg has copy, drop {
    call_type: CallArgType,
}

public struct Command has copy, drop {
    command_type: CommandArgType,
}

public struct ProgrammableMoveCall has copy, drop {
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<std::type_name::TypeName>,
    arguments: vector<Argument>,
}

// --- Command Data ---

public struct TransferObjectsData has copy, drop {
    objects: vector<Argument>,
    recipient: Argument,
}

public struct SplitCoinsData has copy, drop {
    coin: Argument,
    amounts: vector<Argument>,
}

public struct MergeCoinsData has copy, drop {
    target_coin: Argument,
    coins_to_merge: vector<Argument>,
}

public struct PublishData has copy, drop {
    package_bytes: vector<vector<u8>>,
    dependencies: vector<ID>,
}

public struct MakeMoveVecData has copy, drop {
    type_input: Option<TypeInput>,
    arguments: vector<Argument>,
}

public struct UpgradeData has copy, drop {
    modules: vector<vector<u8>>,
    dependencies: vector<ID>,
    package_id: ID,
    upgrade_ticket: Argument,
}

public struct Argument has copy, drop {
    argument_type: ArgumentType,
}

// --- Helper Structs ---

public struct NestedResultData has copy, drop {
    command_index: u16,
    result_index: u16,
}

public struct SharedObjectData has copy, drop {
    id: ID,
    initial_shared_version: SequenceNumber,
    mutable: bool,
}

public struct ObjectRef has copy, drop {
    object_id: ID,
    sequence_number: SequenceNumber,
    object_digest: ObjectDigest,
}

public struct ObjectDigest has copy, drop {
    digest: Digest,
}

public struct SequenceNumber has copy, drop, store {
    value: u64,
}

public struct Digest has copy, drop, store {
    bytes: vector<u8>, // Should always be 32 bytes
}

// === CallArg Helpers ===

#[test_only]
public fun new_pure(data: vector<u8>): CallArg {
    CallArg {
        call_type: CallArgType::PureData(data),
    }
}

#[test_only]
public fun new_object(obj: ObjectArgType): CallArg {
    CallArg {
        call_type: CallArgType::ObjectData(obj),
    }
}

public fun is_pure_data(arg: &CallArg): bool {
    match (&arg.call_type) {
        CallArgType::PureData(_) => true,
        _ => false,
    }
}

public fun is_object_data(arg: &CallArg): bool {
    match (&arg.call_type) {
        CallArgType::ObjectData(_) => true,
        _ => false,
    }
}

public fun get_pure_data(arg: &CallArg): &vector<u8> {
    match (&arg.call_type) {
        CallArgType::PureData(data) => data,
        _ => abort EBadTxHashLength,
    }
}

public fun get_object_data(arg: &CallArg): &ObjectArgType {
    match (&arg.call_type) {
        CallArgType::ObjectData(obj) => obj,
        _ => abort EBadTxHashLength,
    }
}

// === Command Helpers ===

#[test_only]
public fun new_move_call(call: ProgrammableMoveCall): Command {
    Command {
        command_type: CommandArgType::MoveCall(call),
    }
}

#[test_only]
public fun new_transfer_objects(data: TransferObjectsData): Command {
    Command {
        command_type: CommandArgType::TransferObjects(data),
    }
}

#[test_only]
public fun new_split_coins(data: SplitCoinsData): Command {
    Command {
        command_type: CommandArgType::SplitCoins(data),
    }
}

#[test_only]
public fun new_merge_coins(data: MergeCoinsData): Command {
    Command {
        command_type: CommandArgType::MergeCoins(data),
    }
}

#[test_only]
public fun new_publish_data(data: PublishData): Command {
    Command {
        command_type: CommandArgType::PublishData(data),
    }
}

#[test_only]
public fun new_upgrade_data(data: UpgradeData): Command {
    Command {
        command_type: CommandArgType::UpgradeData(data),
    }
}

// === Command Getters ===

public fun get_command_type(command: &Command): CommandArgType {
    command.command_type
}

// === ProgrammableMoveCall Getters ===

#[test_only]
public fun new_programmable_move_call(
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<std::type_name::TypeName>,
    arguments: vector<Argument>,
): ProgrammableMoveCall {
    ProgrammableMoveCall {
        package,
        module_name,
        function,
        type_arguments,
        arguments,
    }
}

public fun get_package_id(call: &ProgrammableMoveCall): ID {
    call.package
}

public fun get_module_name(call: &ProgrammableMoveCall): String {
    call.module_name
}

public fun get_function_name(call: &ProgrammableMoveCall): String {
    call.function
}

public fun get_type_arguments(call: &ProgrammableMoveCall): vector<std::type_name::TypeName> {
    call.type_arguments
}

public fun get_arguments(call: &ProgrammableMoveCall): vector<Argument> {
    call.arguments
}

// === Argument Helpers ===

#[test_only]
public fun new_argument(arg_type: ArgumentType): Argument {
    Argument {
        argument_type: arg_type,
    }
}

public fun get_argument_type(arg: &Argument): ArgumentType {
    arg.argument_type
}

// ==== test-only functions ====
// TODO
