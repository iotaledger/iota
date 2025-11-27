// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::programmable_transaction;

use std::ascii::String;
use std::type_name::TypeName;

// === Constants ===

const EInvalidEnumVariant: u64 = 0;
const EInvalidArgumentType: u64 = 1;

// === Structs ===

// These structs are the move-side replication of iota-types's rust transaction
// They are used in the MoveAuthenticator and the AuthContext
// The main reason we need this is because AuthContext requires a way to read PTBs on the move level

// Replicates MoveCall(Box<ProgrammableMoveCall>), used in the Command enum
// It represents a call to a Move function in a programmable transaction
// CLI e.g: iota client ptb --move-call iota::tx_context::sender
public struct ProgrammableMoveCall has copy, drop {
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<TypeName>,
    arguments: vector<Argument>,
}

// Replicates TransferObjects(Vec<Argument>, Argument)
public struct ProgrammableTransaction has copy, drop {
    inputs: vector<CallArg>,
    commands: vector<Command>,
}

// Replicates ObjectRef
public struct ObjectRef has copy, drop {
    object_id: ID,
    sequence_number: u64,
    object_digest: vector<u8>,
}

// === Enums ===

public enum CallArg has copy, drop {
    PureData(vector<u8>),
    ObjectData(ObjectArg),
}

public enum Command has copy, drop {
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

// === Public getters ===

public fun package_id(call: &ProgrammableMoveCall): &ID {
    &call.package
}

public fun module_name(call: &ProgrammableMoveCall): &String {
    &call.module_name
}

public fun function_name(call: &ProgrammableMoveCall): &String {
    &call.function
}

public fun type_arguments(call: &ProgrammableMoveCall): &vector<TypeName> {
    &call.type_arguments
}

public fun arguments(call: &ProgrammableMoveCall): &vector<Argument> {
    &call.arguments
}

public fun object_id(obj_ref: &ObjectRef): &ID {
    &obj_ref.object_id
}

public fun sequence_number(obj_ref: &ObjectRef): u64 {
    obj_ref.sequence_number
}

public fun object_digest(obj_ref: &ObjectRef): &vector<u8> {
    &obj_ref.object_digest
}

public fun inputs(tx: &ProgrammableTransaction): &vector<CallArg> {
    &tx.inputs
}

public fun commands(tx: &ProgrammableTransaction): &vector<Command> {
    &tx.commands
}

public fun pure_data(arg: &CallArg): &vector<u8> {
    match (arg) {
        CallArg::PureData(data) => data,
        _ => abort EInvalidEnumVariant,
    }
}

public fun object_data(arg: &CallArg): &ObjectArg {
    match (arg) {
        CallArg::ObjectData(obj) => obj,
        _ => abort EInvalidEnumVariant,
    }
}

//Returns the integer representation of a command variant
public fun command_to_int(command: &Command): u8 {
    match (command) {
        Command::MoveCall(_) => 0,
        Command::TransferObjects(_, _) => 1,
        Command::SplitCoins(_, _) => 2,
        Command::MergeCoins(_, _) => 3,
        Command::Publish(_, _) => 4,
        Command::MakeMoveVec(_, _) => 5,
        Command::Upgrade(_, _, _, _) => 6,
    }
}

public fun move_call_data(command: &Command): &ProgrammableMoveCall {
    match (command) {
        Command::MoveCall(call) => call,
        _ => abort EInvalidArgumentType,
    }
}

public fun transfer_objects_data(command: &Command): (&vector<Argument>, &Argument) {
    match (command) {
        Command::TransferObjects(objects, recipient) => (objects, recipient),
        _ => abort EInvalidArgumentType,
    }
}

public fun split_coins_data(command: &Command): (&Argument, &vector<Argument>) {
    match (command) {
        Command::SplitCoins(coin, amounts) => (coin, amounts),
        _ => abort EInvalidArgumentType,
    }
}

public fun merge_coins_data(command: &Command): (&Argument, &vector<Argument>) {
    match (command) {
        Command::MergeCoins(target_coin, source_coins) => (target_coin, source_coins),
        _ => abort EInvalidArgumentType,
    }
}

public fun publish_data(command: &Command): (&vector<vector<u8>>, &vector<ID>) {
    match (command) {
        Command::Publish(modules, dependencies) => (modules, dependencies),
        _ => abort EInvalidArgumentType,
    }
}

public fun make_move_vec_data(command: &Command): (&Option<TypeName>, &vector<Argument>) {
    match (command) {
        Command::MakeMoveVec(type_arg, args) => (type_arg, args),
        _ => abort EInvalidArgumentType,
    }
}

public fun upgrade_data(command: &Command): (&vector<vector<u8>>, &vector<ID>, &ID, &Argument) {
    match (command) {
        Command::Upgrade(modules, dependencies, package_id, argument) => (
            modules,
            dependencies,
            package_id,
            argument,
        ),
        _ => abort EInvalidArgumentType,
    }
}

public fun argument_input(arg: &Argument): u16 {
    match (arg) {
        Argument::Input(input) => *input,
        _ => abort EInvalidEnumVariant,
    }
}

public fun argument_result(arg: &Argument): u16 {
    match (arg) {
        Argument::Result(result) => *result,
        _ => abort EInvalidEnumVariant,
    }
}

public fun argument_nested_result(arg: &Argument): (u16, u16) {
    match (arg) {
        Argument::NestedResult(outer_index, inner_index) => (*outer_index, *inner_index),
        _ => abort EInvalidEnumVariant,
    }
}

public fun object_ref(obj_arg: &ObjectArg): &ObjectRef {
    match (obj_arg) {
        ObjectArg::ImmOrOwnedObject(obj_ref) => obj_ref,
        ObjectArg::ReceivingObject(obj_ref) => obj_ref,
        _ => abort EInvalidArgumentType,
    }
}

public fun shared_object_data(obj_arg: &ObjectArg): (ID, u64, bool) {
    match (obj_arg) {
        ObjectArg::SharedObject { id, initial_shared_version, mutable } => (
            *id,
            *initial_shared_version,
            *mutable,
        ),
        _ => abort EInvalidArgumentType,
    }
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

// === Test-only functions ===

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

#[test_only]
public fun new_programmable_transaction(
    inputs: vector<CallArg>,
    commands: vector<Command>,
): ProgrammableTransaction {
    ProgrammableTransaction { inputs, commands }
}

#[test_only]
public fun new_move_call(call: ProgrammableMoveCall): Command {
    Command::MoveCall(call)
}

#[test_only]
public fun new_transfer_objects(objects: vector<Argument>, recipient: Argument): Command {
    Command::TransferObjects(objects, recipient)
}

#[test_only]
public fun new_split_coins(coin: Argument, amounts: vector<Argument>): Command {
    Command::SplitCoins(coin, amounts)
}

#[test_only]
public fun new_merge_coins(target_coin: Argument, source_coins: vector<Argument>): Command {
    Command::MergeCoins(target_coin, source_coins)
}

#[test_only]
public fun new_publish(modules: vector<vector<u8>>, dependencies: vector<ID>): Command {
    Command::Publish(modules, dependencies)
}

#[test_only]
public fun new_pure(data: vector<u8>): CallArg {
    CallArg::PureData(data)
}

#[test_only]
public fun new_object(obj: ObjectArg): CallArg {
    CallArg::ObjectData(obj)
}
