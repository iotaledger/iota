// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::auth_context;

use iota::hash;
use iota::intent;
use iota::ptb_call_arg::CallArg;
use iota::ptb_command::Command;
use std::ascii;

// === Errors ===

#[test_only]
#[error(code = 0)]
const EBadDigestLength: vector<u8> =
    b"Expected a digest of length 32, but found a different length.";

// === Constants ===

#[test_only]
/// Number of bytes in a digest.
const DIGEST_LENGTH: u64 = 32;

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

/// Identifies the `authenticate` function used by a `MoveAuthenticator`
/// signature, without binding to a specific account type.
public struct AuthenticatorFunctionInfoV1 has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

// === Public functions ===

/// Returns the MoveAuthenticator digest.
public fun digest(_ctx: &AuthContext): &vector<u8> {
    native_digest()
}

/// Returns the sender's auth digest. For `MoveAuthenticator` signatures equals
/// its digest; for all other signature types it is the Blake2b256 of the
/// serialized (flag-prefixed) signature bytes.
public fun sender_auth_digest(_ctx: &AuthContext): &vector<u8> {
    native_sender_auth_digest()
}

/// Returns the sponsor's auth digest for sponsored transactions, `none`
/// otherwise. For `MoveAuthenticator` signatures equals its digest; for all
/// other signature types it is the Blake2b256 of the serialized
/// (flag-prefixed) signature bytes.
public fun sponsor_auth_digest(_ctx: &AuthContext): &Option<vector<u8>> {
    native_sponsor_auth_digest()
}

/// Returns the sender's `AuthenticatorFunctionInfoV1` if the sender uses a
/// `MoveAuthenticator` signature, `none` otherwise.
public fun sender_authenticator_function_info_v1(
    _ctx: &AuthContext,
): &Option<AuthenticatorFunctionInfoV1> {
    native_sender_authenticator_function_info_v1()
}

/// Returns the sponsor's `AuthenticatorFunctionInfoV1` if the transaction is
/// sponsored and the sponsor uses a `MoveAuthenticator` signature, `none`
/// otherwise.
public fun sponsor_authenticator_function_info_v1(
    _ctx: &AuthContext,
): &Option<AuthenticatorFunctionInfoV1> {
    native_sponsor_authenticator_function_info_v1()
}

/// Returns the transaction input objects or primitive values.
public fun tx_inputs(_ctx: &AuthContext): &vector<CallArg> {
    native_tx_inputs()
}

/// Returns the transaction commands to be executed sequentially.
public fun tx_commands(_ctx: &AuthContext): &vector<Command> {
    native_tx_commands()
}

/// Returns `bcs::to_bytes(TransactionData)`.
public fun tx_data_bytes(_ctx: &AuthContext): &vector<u8> {
    native_tx_data_bytes()
}

/// Returns `bcs::to_bytes(IntentMessage<TransactionData>)`, i.e., the IOTA
/// transaction intent bytes prepended to the BCS-serialized TransactionData.
public fun intent_tx_data_bytes(ctx: &AuthContext): vector<u8> {
    let mut result = intent::iota_transaction().to_bytes();
    result.append(*ctx.tx_data_bytes());
    result
}

/// Returns `Blake2b256(bcs::to_bytes(IntentMessage<TransactionData>))`.
/// This is the message that protocol generic signatures sign over.
public fun signing_digest(ctx: &AuthContext): vector<u8> {
    let intent_msg = ctx.intent_tx_data_bytes();
    hash::blake2b256(&intent_msg)
}

/// Returns the package ID of the `AuthenticatorFunctionInfoV1`.
public fun package(self: &AuthenticatorFunctionInfoV1): ID {
    self.package
}

/// Returns the module name of the `AuthenticatorFunctionInfoV1`.
public fun module_name(self: &AuthenticatorFunctionInfoV1): &ascii::String {
    &self.module_name
}

/// Returns the function name of the `AuthenticatorFunctionInfoV1`.
public fun function_name(self: &AuthenticatorFunctionInfoV1): &ascii::String {
    &self.function_name
}

// === Native functions ===

native fun native_digest(): &vector<u8>;

native fun native_sender_auth_digest(): &vector<u8>;

native fun native_sponsor_auth_digest(): &Option<vector<u8>>;

native fun native_sender_authenticator_function_info_v1<F>(): &Option<F>;

native fun native_sponsor_authenticator_function_info_v1<F>(): &Option<F>;

native fun native_tx_data_bytes(): &vector<u8>;

native fun native_tx_inputs<I>(): &vector<I>;

native fun native_tx_commands<C>(): &vector<C>;

// === Test-only functions ===

/// Creates a new `AuthContext` for testing.
#[test_only]
public fun new_for_testing(
    auth_digest: vector<u8>,
    tx_inputs: vector<CallArg>,
    tx_commands: vector<Command>,
    tx_data_bytes: vector<u8>,
    sender_auth_digest: vector<u8>,
    sponsor_auth_digest: Option<vector<u8>>,
    sender_authenticator_function_info_v1: Option<AuthenticatorFunctionInfoV1>,
    sponsor_authenticator_function_info_v1: Option<AuthenticatorFunctionInfoV1>,
): AuthContext {
    assert!(auth_digest.length() == DIGEST_LENGTH, EBadDigestLength);
    assert!(sender_auth_digest.length() == DIGEST_LENGTH, EBadDigestLength);
    if (sponsor_auth_digest.is_some()) {
        assert!(sponsor_auth_digest.borrow().length() == DIGEST_LENGTH, EBadDigestLength);
    };

    native_replace(
        auth_digest,
        tx_inputs,
        tx_commands,
        tx_data_bytes,
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_info_v1,
        sponsor_authenticator_function_info_v1,
    );

    // The fields of the returned `AuthContext` are not actually used,
    // since the native functions are used to manage the state.
    AuthContext {
        auth_digest: vector::empty(),
        tx_inputs: vector::empty(),
        tx_commands: vector::empty(),
    }
}

#[test_only]
native fun native_replace<I, C, F>(
    auth_digest: vector<u8>,
    tx_inputs: vector<I>,
    tx_commands: vector<C>,
    tx_data_bytes: vector<u8>,
    sender_auth_digest: vector<u8>,
    sponsor_auth_digest: Option<vector<u8>>,
    sender_authenticator_function_info_v1: Option<F>,
    sponsor_authenticator_function_info_v1: Option<F>,
);

/// Creates an `AuthenticatorFunctionInfoV1` for testing, skipping validation.
#[test_only]
public fun create_auth_function_info_v1_for_testing(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionInfoV1 {
    AuthenticatorFunctionInfoV1 { package: package.to_id(), module_name, function_name }
}
