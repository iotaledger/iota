// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::ptb_call_arg::CallArg;
use iota::ptb_command::Command;

// === Errors ===

#[test_only]
#[error(code = 0)]
const EBadAuthDigestLength: vector<u8> =
    b"Expected an auth digest of length 32, but found a different length.";

// === Constants ===

#[test_only]
/// Number of bytes in an auth digest.
const AUTH_DIGEST_LENGTH: u64 = 32;

// === Structs ===

public struct AuthContext has drop {}

// === Public functions ===

public fun digest(_actx: &AuthContext): &vector<u8> {
    native_digest()
}

public fun tx_inputs(_actx: &AuthContext): &vector<CallArg> {
    native_tx_inputs()
}

public fun tx_commands(_actx: &AuthContext): &vector<Command> {
    native_tx_commands()
}

// === Native functions ===

native fun native_digest(): &vector<u8>;

native fun native_tx_inputs(): &vector<CallArg>;

native fun native_tx_commands(): &vector<Command>;

// === Test-only functions ===

#[test_only]
public fun new_with_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
): AuthContext {
    assert!(auth_digest.length() == AUTH_DIGEST_LENGTH, EBadAuthDigestLength);

    native_replace(auth_digest, tx_inputs, tx_commands);

    AuthContext {}
}

#[test_only]
native fun native_replace(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
);
