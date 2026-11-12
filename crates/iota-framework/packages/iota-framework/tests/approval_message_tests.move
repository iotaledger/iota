// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::approval_message_tests;

use iota::approval_message;
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::test_utils::{assert_eq, destroy};
use iota::verified_nonce;

/// A check type, as a check passes it to `verify_signature`.
public struct TestCheck {}

/// Another check type.
public struct OtherCheck {}

// === Fixtures ===
//
// The signatures are Ed25519 signatures of IOTA personal messages, in `UserSignature` wire format.
// Generated with `iota-sdk-crypto`.

/// The raw Ed25519 key K.
const PUBLIC_KEY: vector<u8> = x"b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// The raw Ed25519 key K2.
const OTHER_PUBLIC_KEY: vector<u8> = x"aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";

const CHECK_LINES: vector<u8> = b"action: test";
const VALID_UNTIL_EPOCH: u64 = 5;

/// The message for `TestCheck` with `CHECK_LINES`, the first nonce of a transaction and
/// `VALID_UNTIL_EPOCH`, on the network whose chain identifier is all zeros.
const MESSAGE: vector<u8> =
    b"check: 0000000000000000000000000000000000000000000000000000000000000002::approval_message_tests::TestCheck\nnetwork: 0000000000000000000000000000000000000000000000000000000000000000\naction: test\nnonce: 8122e789e9387c8ee9dc4811c945e35a196e3c741a6210a5c7c1bbb58e018556\nvalid until epoch: 5";

/// K signs `MESSAGE`.
const SIGNATURE: vector<u8> =
    x"00a4bc2954e66cfe3a6848862246a0371bde28b8a557c519ccfe79a229fc9b9edb6a1c67303ee8ff01fcd8cc12e285bcb524f08eff9ffd08a1b65373928ed0bb06b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K2 signs `MESSAGE`.
const OTHER_KEY_SIGNATURE: vector<u8> =
    x"00b5cc7390f18f132c9c42f02f296c13dc68fefdce20762a02e4cfc933de51175211df06de5317117f5e4d03e08edc149bf02ccc3c7095738c36f33c749bb8df0eaef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K signs `MESSAGE` with the `network:` line of the chain identifier `2222…22`.
const OTHER_NETWORK_SIGNATURE: vector<u8> =
    x"006c88f76af673b28e56dc2fdf79444f193682046c3aebe6ce200c528aff87e36b18999c36bd46a3cb52ab280bd0ae9cb66f0f323ca5a08d5c3fb4fac957713209b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";

// === message ===

#[test]
fun message_has_the_expected_text() {
    let mut ctx = tx_context::dummy();
    let nonce = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);

    let message = approval_message::message_for_testing(
        nonce,
        CHECK_LINES,
        VALID_UNTIL_EPOCH,
        &ctx,
    );

    assert_eq(message, MESSAGE);
}

// === verify_signature ===

#[test]
fun verify_signature_accepts_the_key_holder_signature() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
fun verify_signature_accepts_the_approval_in_its_last_epoch() {
    let mut ctx = ctx_in_epoch(VALID_UNTIL_EPOCH);

    verify_signature(&mut ctx, key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EApprovalExpired)]
fun verify_signature_aborts_after_the_last_epoch() {
    let mut ctx = ctx_in_epoch(VALID_UNTIL_EPOCH + 1);

    verify_signature(&mut ctx, key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_a_changed_signature() {
    let mut ctx = tx_context::dummy();
    let mut signature = SIGNATURE;
    *&mut signature[10] = signature[10] ^ 0xff;

    verify_signature(&mut ctx, key(), CHECK_LINES, signature, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_a_signature_by_another_key() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, key(), CHECK_LINES, OTHER_KEY_SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_another_key_with_the_same_signature() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, other_key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_a_signature_made_for_another_network() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, key(), CHECK_LINES, OTHER_NETWORK_SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_a_signature_made_for_another_check() {
    let mut ctx = tx_context::dummy();
    let nonce = verified_nonce::take_from_tx_context<OtherCheck>(&mut ctx);

    approval_message::verify_signature(
        nonce,
        &key(),
        CHECK_LINES,
        SIGNATURE,
        VALID_UNTIL_EPOCH,
        &ctx,
    );
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_other_check_lines() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, key(), b"action: other", SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_another_nonce() {
    let mut ctx = tx_context::dummy();
    destroy(verified_nonce::take_from_tx_context<TestCheck>(&mut ctx));

    // The signature is over the first nonce of the transaction; this is the second.
    verify_signature(&mut ctx, key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH);
}

#[test]
#[expected_failure(abort_code = approval_message::EInvalidSignature)]
fun verify_signature_aborts_for_another_expiry() {
    let mut ctx = tx_context::dummy();

    verify_signature(&mut ctx, key(), CHECK_LINES, SIGNATURE, VALID_UNTIL_EPOCH + 1);
}

// === Helpers ===

fun key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), PUBLIC_KEY)
}

fun other_key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), OTHER_PUBLIC_KEY)
}

/// Returns the context of a transaction executed in `epoch`.
fun ctx_in_epoch(epoch: u64): TxContext {
    tx_context::new_from_hint(@0x0, 0, epoch, 0, 0)
}

/// Takes the next nonce of the transaction for `TestCheck` and checks `signature` with it.
fun verify_signature(
    ctx: &mut TxContext,
    public_key: PublicKey,
    check_lines: vector<u8>,
    signature: vector<u8>,
    valid_until_epoch: u64,
) {
    let nonce = verified_nonce::take_from_tx_context<TestCheck>(ctx);
    approval_message::verify_signature(
        nonce,
        &public_key,
        check_lines,
        signature,
        valid_until_epoch,
        ctx,
    );
}
