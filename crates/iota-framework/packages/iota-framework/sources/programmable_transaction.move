// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::programmable_transaction;

use std::ascii::String;
use std::type_name::TypeName;
// === Constants ===
const EInvalidEnumVariant: u64 = 0;
const EInvalidArgumentType: u64 = 1;

// === Enums ===

public enum CallArg has copy, drop {
    PureData(vector<u8>),
    ObjectData(ObjectArg),
}

public enum CommandArg has copy, drop {
    MoveCall(ProgrammableMoveCall),
    TransferObjects(vector<Argument>, Argument),
    SplitCoins(Argument, vector<Argument>),  
    MergeCoins(Argument, vector<Argument>),  
    Publish(vector<vector<u8>>, vector<ID>),  
    MakeMoveVec(Option<TypeName>, vector<Argument>),  
    Upgrade(vector<vector<u8>>, vector<ID>, ID, Argument),  
}

public enum Argument has copy, drop {
    GasCoin,
    Input(u16),
    Result(u16),
    NestedResult(u16, u16),
}

public enum ObjectArg has copy, drop {
    ImmOrOwnedObject(ObjectRef),
    SharedObject {
        id: ID,
        initial_shared_version: u64,
        mutable: bool,
    },
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
    inputs: vector<CallArg>,  
    commands: vector<CommandArg>,  
}

// --- Command Data ---



public struct ObjectRef has copy, drop {
    object_id: ID,
    sequence_number: u64,
    object_digest: vector<u8>,
}


// === CallArg Helpers ===

#[test_only]
public fun new_pure(data: vector<u8>): CallArg {
        CallArg::PureData(data)
    
}

#[test_only]
public fun new_object(obj: ObjectArg): CallArg {
    CallArg::ObjectData(obj)
    
}

public fun is_pure_data(arg: &CallArg): bool {
    match (arg) {
        CallArg::PureData(_) => true,
        _ => false,
    }
}

public fun is_object_data(arg: &CallArg): bool {
    match (arg) {
        CallArg::ObjectData(_) => true,
        _ => false,
    }
}

public fun get_pure_data(arg: &CallArg): &vector<u8> {
    match (arg) {
        CallArg::PureData(data) => data,
        _ => abort EInvalidEnumVariant,
    }
}

public fun get_object_data(arg: &CallArg): &ObjectArg {
    match (arg) {
        CallArg::ObjectData(obj) => obj,
        _ => abort EInvalidEnumVariant,
    }
}

// === Command Helpers ===

#[test_only]
public fun new_move_call(call: ProgrammableMoveCall): CommandArg {
        CommandArg::MoveCall(call)
}

#[test_only]
public fun new_transfer_objects(objects: vector<Argument>, recipient: Argument): CommandArg {
    CommandArg::TransferObjects(objects, recipient)
}

#[test_only]
public fun new_split_coins(coin: Argument, amounts: vector<Argument>): CommandArg {
    CommandArg::SplitCoins(coin, amounts)
}

#[test_only]
public fun new_merge_coins(target_coin: Argument, source_coins: vector<Argument>): CommandArg {
    CommandArg::MergeCoins(target_coin, source_coins)
}

#[test_only]
public fun new_publish(modules: vector<vector<u8>>, dependencies: vector<ID>): CommandArg {
    CommandArg::Publish(modules, dependencies)
}

// === Command Getter ===

public fun get_command_variant_name(command: &CommandArg): u8 {
    match (command) {
        CommandArg::MoveCall(_) => 0,
        CommandArg::TransferObjects(_, _) => 1,
        CommandArg::SplitCoins(_, _) => 2,
        CommandArg::MergeCoins(_, _) => 3,
        CommandArg::Publish(_, _) => 4,
        CommandArg::MakeMoveVec(_, _) => 5,
        CommandArg::Upgrade(_, _, _, _) => 6,
    }
}

// === CommandArg Getters ===

public fun get_move_call_data(command: &CommandArg): &ProgrammableMoveCall {
    match (command) {
        CommandArg::MoveCall(call) => call,
        _ => abort EInvalidArgumentType,
    }
}

public fun get_transfer_objects_data(command: &CommandArg): (&vector<Argument>, &Argument) {
    match (command) {
        CommandArg::TransferObjects(objects, recipient) => (objects, recipient),
        _ => abort EInvalidArgumentType,
    }
}

public fun get_split_coins_data(command: &CommandArg): (&Argument, &vector<Argument>) {
    match (command) {
        CommandArg::SplitCoins(coin, amounts) => (coin, amounts),
        _ => abort EInvalidArgumentType,
    }
}

public fun get_merge_coins_data(command: &CommandArg): (&Argument, &vector<Argument>) {
    match (command) {
        CommandArg::MergeCoins(target_coin, source_coins) => (target_coin, source_coins),
        _ => abort EInvalidArgumentType,
    }
}

public fun get_publish_data(command: &CommandArg): (&vector<vector<u8>>, &vector<ID>) {
    match (command) {
        CommandArg::Publish(modules, dependencies) => (modules, dependencies),
        _ => abort EInvalidArgumentType,
    }
}

public fun get_make_move_vec_data(command: &CommandArg): (&Option<TypeName>, &vector<Argument>) {
    match (command) {
        CommandArg::MakeMoveVec(type_arg, args) => (type_arg, args),
        _ => abort EInvalidArgumentType,
    }
}

public fun get_upgrade_data(command: &CommandArg): (&vector<vector<u8>>, &vector<ID>, &ID, &Argument) {
    match (command) {
        CommandArg::Upgrade(modules, dependencies, package_id, argument) => (modules, dependencies, package_id, argument),
        _ => abort EInvalidArgumentType,
    }
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

// === Argument Constructors & Helpers ===

#[test_only]
public fun new_gas_coin_argument(): Argument {
    Argument::GasCoin
}

#[test_only]
public fun new_input_argument(index: u16): Argument {
    Argument::Input(index)
}

#[test_only]
public fun new_result_argument(index: u16): Argument {
    Argument::Result(index)
}

#[test_only]
public fun new_nested_result_argument(outer_index: u16, inner_index: u16): Argument {
    Argument::NestedResult(outer_index, inner_index)
}

public fun get_argument_input(arg: &Argument): u16 {
    match (arg) {
        Argument::Input(input) => *input,
        _ => abort EInvalidEnumVariant,
    }
}

public fun get_argument_result(arg: &Argument): u16 {
    match (arg) {
        Argument::Result(result) => *result,
        _ => abort EInvalidEnumVariant,
    }
}

public fun get_argument_nested_result(arg: &Argument): (u16, u16) {
    match (arg) {
        Argument::NestedResult(outer_index, inner_index) => (*outer_index, *inner_index),
        _ => abort EInvalidEnumVariant,
    }
}

// === ObjectArg Getters ===

public fun get_object_ref(obj_arg: &ObjectArg): &ObjectRef {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => obj_ref,
        ObjectArg::ReceivingObject(obj_ref) => obj_ref,
        _ => abort EInvalidArgumentType,
    }
}

public fun get_shared_object_data(obj_arg: &ObjectArg): (ID, u64, bool) {
    match (obj_arg) {
        ObjectArg::SharedObject { id, initial_shared_version, mutable } => 
            (*id, *initial_shared_version, *mutable),
        _ => abort EInvalidArgumentType,
    }
}

// === ObjectRef Getters ===

public fun get_object_id(obj_ref: &ObjectRef): ID {
    obj_ref.object_id
}

public fun get_sequence_number(obj_ref: &ObjectRef): u64 {
    obj_ref.sequence_number
}

public fun get_object_digest(obj_ref: &ObjectRef): &vector<u8> {
    &obj_ref.object_digest
}

// === ProgrammableTransaction Getters ===

public fun get_inputs(tx: &ProgrammableTransaction): &vector<CallArg> {
    &tx.inputs
}

public fun get_commands(tx: &ProgrammableTransaction): &vector<CommandArg> {
    &tx.commands
}


