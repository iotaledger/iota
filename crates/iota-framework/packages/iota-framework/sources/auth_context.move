// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::ptb_call_arg::CallArg;
use iota::ptb_command::Command;

// === Structs ===

public struct AuthContext has drop {}

// === Public functions ===

public fun digest(_: &AuthContext): vector<u8> {
    native_digest()
}

public fun tx_commands(_: &AuthContext): vector<Command> {
    native_tx_commands()
}

public fun tx_inputs(_: &AuthContext): vector<CallArg> {
    native_tx_inputs()
}

// === Native functions ===

native fun native_digest(): vector<u8>;

native fun native_tx_commands<C>(): vector<C>;

native fun native_tx_inputs<I>(): vector<I>;

// === Test-only functions ===

#[test_only]
public fun new_with_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
): AuthContext {
    native_replace(auth_digest, tx_inputs, tx_commands);
    AuthContext {}
}

#[test_only]
native fun native_replace<I, C>(
    auth_digest: vector<u8>,
    tx_inputs: vector<I>,
    tx_commands: vector<C>,
);
