// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::enriched_call_arg::EnrichedCallArg;
use iota::enriched_command::EnrichedCommand;
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

#[allow(unused_field)]
public struct AuthContext has drop {
    /// The digest of the MoveAuthenticator
    auth_digest: vector<u8>,
    /// The transaction input objects or primitive values
    tx_inputs: vector<CallArg>,
    /// The transaction commands to be executed sequentially.
    tx_commands: vector<Command>,
}

// === Public functions ===

public fun digest(_ctx: &AuthContext): &vector<u8> {
    native_digest()
}

public fun tx_inputs(_ctx: &AuthContext): &vector<CallArg> {
    native_tx_inputs()
}

public fun tx_commands(_ctx: &AuthContext): &vector<Command> {
    native_tx_commands()
}

/// Returns the enriched transaction inputs for this authentication context.
///
/// Each [`EnrichedCallArg`] carries additional metadata (type name, mutability)
/// compared to the plain [`CallArg`] returned by [`tx_inputs`].
public fun enriched_tx_inputs(_ctx: &AuthContext): &vector<EnrichedCallArg> {
    native_tx_inputs()
}

/// Returns the enriched transaction commands for this authentication context.
///
/// Each [`MoveEnrichedCommand`] carries additional metadata (e.g. `is_entry`,
/// return types) compared to the plain [`Command`] returned by [`tx_commands`].
public fun enriched_tx_commands(_ctx: &AuthContext): &vector<EnrichedCommand> {
    native_tx_commands()
}

// === Native functions ===

native fun native_digest(): &vector<u8>;

native fun native_tx_inputs<I>(): &vector<I>;

native fun native_tx_commands<C>(): &vector<C>;

// === Test-only functions ===

#[test_only]
public fun new_with_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
): AuthContext {
    assert!(auth_digest.length() == AUTH_DIGEST_LENGTH, EBadAuthDigestLength);

    native_replace(auth_digest, tx_inputs, tx_commands);

    // The fields of the returned `AuthContext` are not actually used,
    // since the native functions are used to manage the state.
    AuthContext {
        auth_digest: vector::empty(),
        tx_inputs: vector::empty(),
        tx_commands: vector::empty(),
    }
}

#[test_only]
native fun native_replace<I, C>(
    auth_digest: vector<u8>,
    tx_inputs: vector<I>,
    tx_commands: vector<C>,
);

/// Test helper that injects pre-built enriched inputs/commands into the
/// `AuthContext`.  Unlike [`new_with_tx_inputs`], the enriched fields
/// (`is_entry`, `mutable`, `type_name`, `returns`) are stored as-is — no
/// plain→enriched conversion is performed.  Use this variant when a test
/// needs to verify logic that depends on those enriched values.
#[test_only]
public fun new_with_enriched_tx_inputs(
    auth_digest: vector<u8>,
    tx_inputs: vector<EnrichedCallArg>,
    tx_commands: vector<EnrichedCommand>,
): AuthContext {
    assert!(auth_digest.length() == AUTH_DIGEST_LENGTH, EBadAuthDigestLength);

    native_replace_enriched(auth_digest, tx_inputs, tx_commands);

    AuthContext {
        auth_digest: vector::empty(),
        tx_inputs: vector::empty(),
        tx_commands: vector::empty(),
    }
}

#[test_only]
native fun native_replace_enriched<I, C>(
    auth_digest: vector<u8>,
    tx_inputs: vector<I>,
    tx_commands: vector<C>,
);
