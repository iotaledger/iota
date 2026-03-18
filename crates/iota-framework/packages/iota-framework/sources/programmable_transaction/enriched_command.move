// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Enriched variants of PTB command types exposed via `auth_context`.
//
// BCS layout MUST match the Rust counterparts in
// `iota_types::auth_context::enriched_types` field-for-field and
// variant-for-variant.
module iota::enriched_command;

use std::type_name::TypeName;
use iota::ptb_command::{
    Argument,
    TransferObjectsData,
    SplitCoinsData,
    MergeCoinsData,
    PublishData,
    MakeMoveVecData,
    UpgradeData,
};

// === Structs ===

/// Enriched counterpart of `ptb_command::ProgrammableMoveCall`.
///
/// Adds `is_entry` and `returns` (return-type names) on top of the base data.
///
/// Field order MUST match the Rust `EnrichedProgrammableMoveCall` struct
/// to preserve BCS compatibility:
///   package, module_name, function, is_entry, type_arguments, arguments, returns
public struct EnrichedProgrammableMoveCall has copy, drop {
    package: ID,
    module_name: std::ascii::String,
    function: std::ascii::String,
    is_entry: bool,
    type_arguments: vector<TypeName>,
    arguments: vector<Argument>,
    returns: vector<TypeName>,
}

// === Enums ===

/// Enriched counterpart of `ptb_command::Command`.
///
/// The only difference is that `MoveCall` wraps `EnrichedProgrammableMoveCall`
/// instead of `ProgrammableMoveCall`.  All other variants share the same data
/// structs as the base `Command` enum and therefore have the same BCS layout.
///
/// Variant order MUST match the Rust `EnrichedCommand` enum:
///   0 – MoveCall
///   1 – TransferObjects
///   2 – SplitCoins
///   3 – MergeCoins
///   4 – Publish
///   5 – MakeMoveVec
///   6 – Upgrade
public enum EnrichedCommand has copy, drop {
    MoveCall(EnrichedProgrammableMoveCall),
    TransferObjects(TransferObjectsData),
    SplitCoins(SplitCoinsData),
    MergeCoins(MergeCoinsData),
    Publish(PublishData),
    MakeMoveVec(MakeMoveVecData),
    Upgrade(UpgradeData),
}

// === Public functions ===

// -- EnrichedCommand --

public fun is_move_call(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::MoveCall(_) => true,
        _ => false,
    }
}

public fun is_transfer_objects(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::TransferObjects(_) => true,
        _ => false,
    }
}

public fun is_split_coins(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::SplitCoins(_) => true,
        _ => false,
    }
}

public fun is_merge_coins(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::MergeCoins(_) => true,
        _ => false,
    }
}

public fun is_publish(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::Publish(_) => true,
        _ => false,
    }
}

public fun is_make_move_vec(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::MakeMoveVec(_) => true,
        _ => false,
    }
}

public fun is_upgrade(cmd: &EnrichedCommand): bool {
    match (cmd) {
        EnrichedCommand::Upgrade(_) => true,
        _ => false,
    }
}

// -- EnrichedProgrammableMoveCall accessors --

public fun move_call_package(call: &EnrichedProgrammableMoveCall): &ID {
    &call.package
}

public fun move_call_module(call: &EnrichedProgrammableMoveCall): &std::ascii::String {
    &call.module_name
}

public fun move_call_function(call: &EnrichedProgrammableMoveCall): &std::ascii::String {
    &call.function
}

public fun move_call_is_entry(call: &EnrichedProgrammableMoveCall): bool {
    call.is_entry
}

public fun move_call_type_arguments(call: &EnrichedProgrammableMoveCall): &vector<TypeName> {
    &call.type_arguments
}

public fun move_call_arguments(call: &EnrichedProgrammableMoveCall): &vector<Argument> {
    &call.arguments
}

public fun move_call_returns(call: &EnrichedProgrammableMoveCall): &vector<TypeName> {
    &call.returns
}

// === Test-only functions ===

#[test_only]
public fun new_enriched_move_call_for_testing(
    package: ID,
    module_name: std::ascii::String,
    function: std::ascii::String,
    is_entry: bool,
    type_arguments: vector<TypeName>,
    arguments: vector<Argument>,
    returns: vector<TypeName>,
): EnrichedCommand {
    EnrichedCommand::MoveCall(EnrichedProgrammableMoveCall {
        package,
        module_name,
        function,
        is_entry,
        type_arguments,
        arguments,
        returns,
    })
}
