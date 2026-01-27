// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This module provides the move-side replication of a
// Programmable Transaction Block (PTB). It is useful
// for reading the content of a transaction during the
// execution of an authentication.
module iota::ptb;

use iota::ptb_call_arg::CallArg;
use iota::ptb_command::Command;

// === Structs and Enums ===

// Replicates a whole programmable transaction in Move.
// It contains the input arguments and the commands to execute.
public struct ProgrammableTransaction has copy, drop {
    inputs: vector<CallArg>,
    commands: vector<Command>,
}
// === Public functions ===

// === ProgrammableTransaction ===

public fun inputs(tx: &ProgrammableTransaction): &vector<CallArg> {
    &tx.inputs
}

public fun commands(tx: &ProgrammableTransaction): &vector<Command> {
    &tx.commands
}

// === Public(package) functions ===

public(package) fun new_programmable_transaction_for_testing(
    inputs: vector<CallArg>,
    commands: vector<Command>,
): ProgrammableTransaction {
    ProgrammableTransaction { inputs, commands }
}
