// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::programmable_transaction::{CallArg, CommandArg};

// === Structs ===

public struct AuthContext has drop {
    /// The digest of the MoveAuthenticator
    auth_digest: vector<u8>,
    /// The transaction input objects or primitive values
    tx_inputs: vector<CallArg>, //TODO: remove for 2nd iteration phase
    /// The transaction commands to be executed sequentially.
    tx_commands: vector<CommandArg>, //TODO: remove for 2nd iteration phase
}

// === Public functions ===

public fun digest(ctx: &AuthContext): &vector<u8> {
    &ctx.auth_digest
}

public fun tx_inputs(ctx: &AuthContext): &vector<CallArg> {
    &ctx.tx_inputs
}

public fun tx_commands(ctx: &AuthContext): &vector<CommandArg> {
    &ctx.tx_commands
}

//TODO: implement this for the 2nd iteration phase

// public fun tx_inputs(_ctx: &AuthContext): vector<CallArg> {
//     native_tx_inputs()
// }

// public fun tx_commands(_ctx: &AuthContext): vector<CommandArg> {
//     native_tx_commands()
// }

// native fun native_tx_inputs(): vector<CallArg>;

// native fun native_tx_commands(): vector<CommandArg>;

// === Test-only functions ===

#[test_only]
public fun new_with_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<CommandArg>,
): AuthContext {
    AuthContext {
        auth_digest,
        tx_inputs,
        tx_commands,
    }
}
