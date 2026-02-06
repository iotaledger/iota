// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::ptb_call_arg::CallArg;
use iota::ptb_command::Command;

// === Structs ===

public struct AuthContext has drop {
    /// The digest of the MoveAuthenticator
    auth_digest: vector<u8>,
    /// The transaction input objects or primitive values
    tx_inputs: vector<CallArg>,
    /// The transaction commands to be executed sequentially.
    tx_commands: vector<Command>,
}

// === Public functions ===

public fun digest(ctx: &AuthContext): &vector<u8> {
    &ctx.auth_digest
}

public fun tx_inputs(ctx: &AuthContext): &vector<CallArg> {
    &ctx.tx_inputs
}

public fun tx_commands(ctx: &AuthContext): &vector<Command> {
    &ctx.tx_commands
}

// === Test-only functions ===

#[test_only]
public fun new_with_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
): AuthContext {
    AuthContext {
        auth_digest,
        tx_inputs,
        tx_commands,
    }
}
