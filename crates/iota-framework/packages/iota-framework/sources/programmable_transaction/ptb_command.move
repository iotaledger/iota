// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This module provides the move-side replication of a
// Programmable Transaction Block (PTB). It is useful
// for reading the content of a transaction during the
// execution of an authentication.
module iota::ptb_command;

use std::ascii::String;
use std::option::{some, none};
use std::type_name::TypeName;

// === Structs and Enums ===

// Replicates Command enum used in PTBs.
// It represents the different commands that can be executed in a PTB.
public enum Command has copy, drop {
    MoveCall(ProgrammableMoveCall),
    TransferObjects(TransferObjectsData),
    SplitCoins(SplitCoinsData),
    MergeCoins(MergeCoinsData),
    Publish(PublishData),
    MakeMoveVec(MakeMoveVecData),
    Upgrade(UpgradeData),
}

// Replicates Argument enum used in PTBs.
// It represents different types of arguments that can be passed to commands in a PTB,
// i.e., a pointer to a gas coin, an input object, a result from a previous command.
public enum Argument has copy, drop {
    GasCoin,
    Input(u16),
    Result(u16),
    NestedResult(u16, u16),
}

// Replicates ProgrammableMoveCall used in PTBs.
// It represents a call to a Move function in a PTB. It includes the package ID,
// module name, function name for the call, and the type arguments and arguments
//to pass to the function.
public struct ProgrammableMoveCall has copy, drop {
    package: ID,
    module_name: String,
    function: String,
    type_arguments: vector<TypeName>,
    arguments: vector<Argument>,
}

// Replicates TransferObjects command data structure.
// It includes the list of objects to transfer and the recipient argument.
public struct TransferObjectsData has copy, drop {
    objects: vector<Argument>,
    recipient: Argument,
}

// Replicates SplitCoins command data structure.
// It includes the coin to split and the list of amounts.
public struct SplitCoinsData has copy, drop {
    coin: Argument,
    amounts: vector<Argument>,
}

// Replicates MergeCoins command data structure.
// It includes the target coin and the list of source coins.
public struct MergeCoinsData has copy, drop {
    target_coin: Argument,
    source_coins: vector<Argument>,
}

// Replicates Publish command data structure.
// It includes the list of bytes of modules to publish and their dependencies.
public struct PublishData has copy, drop {
    modules: vector<vector<u8>>,
    dependencies: vector<ID>,
}

// Replicates MakeMoveVec command data structure.
// It includes an optional TypeName for the elements and the list of elements.
public struct MakeMoveVecData has copy, drop {
    type_arg: Option<TypeName>,
    elements: vector<Argument>,
}

// Replicates Upgrade command data structure.
// It includes the list of bytes of modules to upgrade, their dependencies,
// the object ID of the package being upgraded, and the upgrade ticket argument.
public struct UpgradeData has copy, drop {
    modules: vector<vector<u8>>,
    dependencies: vector<ID>,
    package: ID,
    upgrade_ticket: Argument,
}

// === Public functions ===

// === Command ===

public fun is_move_call(command: &Command): bool {
    match (command) {
        Command::MoveCall(_) => true,
        _ => false,
    }
}

public fun is_transfer_objects(command: &Command): bool {
    match (command) {
        Command::TransferObjects(_) => true,
        _ => false,
    }
}

public fun is_split_coins(command: &Command): bool {
    match (command) {
        Command::SplitCoins(_) => true,
        _ => false,
    }
}

public fun is_merge_coins(command: &Command): bool {
    match (command) {
        Command::MergeCoins(_) => true,
        _ => false,
    }
}

public fun is_publish(command: &Command): bool {
    match (command) {
        Command::Publish(_) => true,
        _ => false,
    }
}

public fun is_make_move_vec(command: &Command): bool {
    match (command) {
        Command::MakeMoveVec(_) => true,
        _ => false,
    }
}

public fun is_upgrade(command: &Command): bool {
    match (command) {
        Command::Upgrade(_) => true,
        _ => false,
    }
}

public fun as_move_call(command: &Command): Option<ProgrammableMoveCall> {
    match (command) {
        Command::MoveCall(call) => some(*call),
        _ => none(),
    }
}

public fun as_transfer_objects(command: &Command): Option<TransferObjectsData> {
    match (command) {
        Command::TransferObjects(data) => some(*data),
        _ => none(),
    }
}

public fun as_split_coins(command: &Command): Option<SplitCoinsData> {
    match (command) {
        Command::SplitCoins(data) => some(*data),
        _ => none(),
    }
}

public fun as_merge_coins(command: &Command): Option<MergeCoinsData> {
    match (command) {
        Command::MergeCoins(data) => some(*data),
        _ => none(),
    }
}

public fun as_publish(command: &Command): Option<PublishData> {
    match (command) {
        Command::Publish(data) => some(*data),
        _ => none(),
    }
}

public fun as_make_move_vec(command: &Command): Option<MakeMoveVecData> {
    match (command) {
        Command::MakeMoveVec(data) => some(*data),
        _ => none(),
    }
}

public fun as_upgrade(command: &Command): Option<UpgradeData> {
    match (command) {
        Command::Upgrade(data) => some(*data),
        _ => none(),
    }
}

// === Argument ===

public fun is_gas_coin(arg: &Argument): bool {
    match (arg) {
        Argument::GasCoin => true,
        _ => false,
    }
}

public fun is_input(arg: &Argument): bool {
    match (arg) {
        Argument::Input(_) => true,
        _ => false,
    }
}

public fun is_result(arg: &Argument): bool {
    match (arg) {
        Argument::Result(_) => true,
        _ => false,
    }
}

public fun is_nested_result(arg: &Argument): bool {
    match (arg) {
        Argument::NestedResult(_, _) => true,
        _ => false,
    }
}

public fun input_index(arg: &Argument): Option<u16> {
    match (arg) {
        Argument::Input(index) => some(*index),
        _ => none(),
    }
}

public fun result_command_index(arg: &Argument): Option<u16> {
    match (arg) {
        Argument::Result(command_index) => some(*command_index),
        _ => none(),
    }
}

public fun nested_result_command_index(arg: &Argument): Option<u16> {
    match (arg) {
        Argument::NestedResult(command_index, _) => some(*command_index),
        _ => none(),
    }
}

public fun nested_result_inner_index(arg: &Argument): Option<u16> {
    match (arg) {
        Argument::NestedResult(_, inner_index) => some(*inner_index),
        _ => none(),
    }
}

// === ProgrammableMoveCall ===

public fun package(call: &ProgrammableMoveCall): &ID {
    &call.package
}

public fun module_name(call: &ProgrammableMoveCall): &String {
    &call.module_name
}

public fun function(call: &ProgrammableMoveCall): &String {
    &call.function
}

public fun type_arguments(call: &ProgrammableMoveCall): &vector<TypeName> {
    &call.type_arguments
}

public fun arguments(call: &ProgrammableMoveCall): &vector<Argument> {
    &call.arguments
}

// === TransferObjectsData ===

public fun objects(data: &TransferObjectsData): &vector<Argument> {
    &data.objects
}

public fun recipient(data: &TransferObjectsData): &Argument {
    &data.recipient
}

// === SplitCoinsData ===

public fun coin(data: &SplitCoinsData): &Argument {
    &data.coin
}

public fun amounts(data: &SplitCoinsData): &vector<Argument> {
    &data.amounts
}

// === MergeCoinsData ===

public fun target_coin(data: &MergeCoinsData): &Argument {
    &data.target_coin
}

public fun source_coins(data: &MergeCoinsData): &vector<Argument> {
    &data.source_coins
}

// === PublishData ===

public fun modules(data: &PublishData): &vector<vector<u8>> {
    &data.modules
}

public fun dependencies(data: &PublishData): &vector<ID> {
    &data.dependencies
}

// === MakeMoveVecData ===

public fun type_arg(data: &MakeMoveVecData): &Option<TypeName> {
    &data.type_arg
}

public fun elements(data: &MakeMoveVecData): &vector<Argument> {
    &data.elements
}

// === UpgradeData ===

public fun upgrade_modules(data: &UpgradeData): &vector<vector<u8>> {
    &data.modules
}

public fun upgrade_dependencies(data: &UpgradeData): &vector<ID> {
    &data.dependencies
}

public fun upgrade_package(data: &UpgradeData): &ID {
    &data.package
}

public fun upgrade_ticket(data: &UpgradeData): &Argument {
    &data.upgrade_ticket
}

// === Test-only functions ===

#[test_only]
public fun new_move_call_command_for_testing(call: ProgrammableMoveCall): Command {
    Command::MoveCall(call)
}

#[test_only]
public fun new_transfer_objects_command_for_testing(data: TransferObjectsData): Command {
    Command::TransferObjects(data)
}

#[test_only]
public fun new_split_coins_command_for_testing(data: SplitCoinsData): Command {
    Command::SplitCoins(data)
}

#[test_only]
public fun new_merge_coins_command_for_testing(data: MergeCoinsData): Command {
    Command::MergeCoins(data)
}

#[test_only]
public fun new_publish_command_for_testing(data: PublishData): Command {
    Command::Publish(data)
}

#[test_only]
public fun new_make_move_vec_command_for_testing(data: MakeMoveVecData): Command {
    Command::MakeMoveVec(data)
}

#[test_only]
public fun new_upgrade_command_for_testing(data: UpgradeData): Command {
    Command::Upgrade(data)
}

#[test_only]
public fun new_gas_coin_argument_for_testing(): Argument {
    Argument::GasCoin
}

#[test_only]
public fun new_input_argument_for_testing(index: u16): Argument {
    Argument::Input(index)
}

#[test_only]
public fun new_result_argument_for_testing(index: u16): Argument {
    Argument::Result(index)
}

#[test_only]
public fun new_nested_result_argument_for_testing(outer_index: u16, inner_index: u16): Argument {
    Argument::NestedResult(outer_index, inner_index)
}

#[test_only]
public fun new_programmable_move_call_for_testing(
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
public fun new_transfer_objects_for_testing(
    objects: vector<Argument>,
    recipient: Argument,
): TransferObjectsData {
    TransferObjectsData { objects, recipient }
}

#[test_only]
public fun new_split_coins_for_testing(coin: Argument, amounts: vector<Argument>): SplitCoinsData {
    SplitCoinsData { coin, amounts }
}

#[test_only]
public fun new_merge_coins_for_testing(
    target_coin: Argument,
    source_coins: vector<Argument>,
): MergeCoinsData {
    MergeCoinsData { target_coin, source_coins }
}

#[test_only]
public fun new_publish_for_testing(
    modules: vector<vector<u8>>,
    dependencies: vector<ID>,
): PublishData {
    PublishData { modules, dependencies }
}

#[test_only]
public fun new_make_move_vec_for_testing(
    type_arg: Option<TypeName>,
    elements: vector<Argument>,
): MakeMoveVecData {
    MakeMoveVecData { type_arg, elements }
}

#[test_only]
public fun new_upgrade_for_testing(
    modules: vector<vector<u8>>,
    dependencies: vector<ID>,
    package: ID,
    upgrade_ticket: Argument,
): UpgradeData {
    UpgradeData { modules, dependencies, package, upgrade_ticket }
}
