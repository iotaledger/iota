// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::programmable_transaction;

use std::ascii::String;
use std::type_name::TypeName;
// === Constants ===
const EBadTxHashLength: u64 = 0;

// === Enums ===

public enum CallArgType has copy, drop {
    PureData(vector<u8>),
    ObjectData(ObjectArgType),
}

public enum CommandArgType has copy, drop {
    MoveCall(ProgrammableMoveCall),
    TransferObjects(vector<Argument>, Argument),
    SplitCoins(SplitCoinsData),
    MergeCoins(MergeCoinsData),
    Publish(PublishData),
    MakeMoveVec(MakeMoveVecData),
    Upgrade(UpgradeData),
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

public struct ProgrammableMoveCall has copy, drop {
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<TypeName>,
    arguments: vector<Argument>,
}

public struct ProgrammableTransaction has copy, drop {  
    inputs: vector<CallArgType>,  
    commands: vector<CommandArgType>,  
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
    type_input: Option<TypeName>,
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
    sequence_number: u64,
    object_digest: vector<u8>,
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
public fun new_pure(data: vector<u8>): CallArgType {
        CallArgType::PureData(data)
    
}

#[test_only]
public fun new_object(obj: ObjectArgType): CallArgType {
    CallArgType::ObjectData(obj)
    
}

public fun is_pure_data(arg: &CallArgType): bool {
    match (arg) {
        CallArgType::PureData(_) => true,
        _ => false,
    }
}

public fun is_object_data(arg: &CallArgType): bool {
    match (arg) {
        CallArgType::ObjectData(_) => true,
        _ => false,
    }
}

public fun get_pure_data(arg: &CallArgType): &vector<u8> {
    match (arg) {
        CallArgType::PureData(data) => data,
        _ => abort EBadTxHashLength,
    }
}

public fun get_object_data(arg: &CallArgType): &ObjectArgType {
    match (arg) {
        CallArgType::ObjectData(obj) => obj,
        _ => abort EBadTxHashLength,
    }
}

// === Command Helpers ===

#[test_only]
public fun new_move_call(call: ProgrammableMoveCall): CommandArgType {
        CommandArgType::MoveCall(call)
}

#[test_only]
public fun new_transfer_objects(data: TransferObjectsData): CommandArgType {
        CommandArgType::TransferObjects(data.objects, data.recipient)
}

#[test_only]
public fun new_split_coins(data: SplitCoinsData): CommandArgType {
         CommandArgType::SplitCoins(data)
}

#[test_only]
public fun new_merge_coins(data: MergeCoinsData): CommandArgType {

    CommandArgType::MergeCoins(data)
    
}

#[test_only]
public fun new_publish_data(data: PublishData): CommandArgType {
    
    CommandArgType::Publish(data)
    
}

#[test_only]
public fun new_upgrade_data(data: UpgradeData): CommandArgType {
    
    CommandArgType::Upgrade(data)
}

// === Command Getters ===

public fun get_command_type(command: &CommandArgType): CommandArgType {
    *command
}

// === ProgrammableMoveCall Getters ===

#[test_only]
public fun new_programmable_move_call(
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<TypeName>,
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

public fun get_type_arguments(call: &ProgrammableMoveCall): vector<TypeName> {
    call.type_arguments
}

public fun get_arguments(call: &ProgrammableMoveCall): vector<Argument> {
    call.arguments
}

// === Argument Helpers ===

#[test_only]
public fun new_argument(arg_type: ArgumentType): Argument {
    Argument {
        argument_type: arg_type
    }
}

public fun get_argument_type(arg: &Argument): ArgumentType {
    arg.argument_type
}
