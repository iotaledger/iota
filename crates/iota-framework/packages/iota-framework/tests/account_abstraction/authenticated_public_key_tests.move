// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::authenticated_public_key_tests;

use iota::authenticated_public_key::{Self, AuthenticatedPublicKey};
use iota::protocol_config;
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::test_utils::{assert_eq, assert_ref_eq, destroy};
use iota::verified_nonce;

// === Fixtures ===
//
// The signatures are Ed25519 signatures of IOTA personal messages, in `UserSignature` wire format.
// Generated with `iota-sdk-crypto`.
//
// Each message is for the first nonce of a transaction and `VALID_UNTIL_EPOCH`, on the network
// whose chain identifier is all zeros. The one that approves K for `ACCOUNT` is:
//
// check: 0000000000000000000000000000000000000000000000000000000000000002::authenticated_public_key::AuthenticatedPublicKey
// network: 0000000000000000000000000000000000000000000000000000000000000000
// public key: 00b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200
// account: 0x0000000000000000000000000000000000000000000000000000000000000acc
// nonce: 8122e789e9387c8ee9dc4811c945e35a196e3c741a6210a5c7c1bbb58e018556
// valid until epoch: 5
//
// The one that approves K for a new `SmartAccount` created by `SENDER` has these lines in place
// of the `account:` line:
//
// account: new SmartAccount with built-in authenticator
// sender: 0x00000000000000000000000000000000000000000000000000000000000a11ce

/// The raw Ed25519 key K.
const PUBLIC_KEY: vector<u8> = x"b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// The raw Ed25519 key K2.
const OTHER_PUBLIC_KEY: vector<u8> =
    x"aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";

/// K approves K for a new `SmartAccount` created by `SENDER`.
const SIGNATURE_FOR_NEW_SMART_ACCOUNT: vector<u8> =
    x"00a30e7eb739773b8232a31e5b0ad92f3bdc56cb9eeba468b93a8f61c8b15c5b3dda5523b4f3157ba585d98d02927e5f68ff49e29ca2a6d31d9c8f0603b92d0d0bb9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K approves K for `ACCOUNT`.
const SIGNATURE_FOR_EXISTING_ACCOUNT: vector<u8> =
    x"00de11637db60dc0f711cc3d219e1876364d94ebf49f1da1cbd74efe68e25d8c8538bc0890404aac8f66db9c5794789cba412f3e1e0c997d3eec4383c0fc50fd08b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
/// K2 signs the message that approves K for `ACCOUNT`.
const OTHER_KEY_SIGNATURE_FOR_EXISTING_ACCOUNT: vector<u8> =
    x"0033ff8113069ae9a5cb5401ed8fca761c5abbb78b0d82d58940c9a1ef251faf49526f9fec4b7c7367681ff490adac834b2c41401497447818d00b7bdcbcdd8603aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
/// K approves K for `ACCOUNT` on the network whose chain identifier is `2222…22`.
const OTHER_NETWORK_SIGNATURE_FOR_EXISTING_ACCOUNT: vector<u8> =
    x"0085804e8e76b5e13af14d7ff37880c45e6b018c4488f735d38c70812d5e1033db214e6f72b32a2d2573a7c27e04e9acef3f31f718cc2622de3f86e45826355d02b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";

const VALID_UNTIL_EPOCH: u64 = 5;
const SENDER: address = @0xa11ce;
const ACCOUNT: address = @0xacc;

// === verify_for_new_smart_account ===

#[test]
fun verify_for_new_smart_account_accepts_the_key_holder_approval() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_NEW_SMART_ACCOUNT);

    assert_ref_eq(public_key.public_key(), &key());
    assert_eq(*public_key.account(), option::none());
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_new_smart_account_aborts_for_another_sender() {
    let mut ctx = ctx_in_epoch(@0xb0b, 0);

    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_NEW_SMART_ACCOUNT);
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_new_smart_account_aborts_for_an_approval_of_an_existing_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_EXISTING_ACCOUNT);
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = authenticated_public_key::EBuiltinAuthenticatorsNotEnabled)]
fun verify_for_new_smart_account_aborts_when_builtin_authenticators_are_disabled() {
    protocol_config::set_feature_enabled_for_testing(b"enable_builtin_move_authenticators", false);
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_NEW_SMART_ACCOUNT);
    destroy(public_key);
}

// === verify_for_existing_account ===

#[test]
fun verify_for_existing_account_accepts_the_key_holder_approval() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );

    assert_ref_eq(public_key.public_key(), &key());
    assert_eq(*public_key.account(), option::some(ACCOUNT));
    destroy(public_key);
}

#[test]
fun verify_for_existing_account_accepts_the_approval_from_any_sender() {
    let mut ctx = ctx_in_epoch(@0xb0b, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
fun verify_for_existing_account_accepts_the_approval_in_its_last_epoch() {
    let mut ctx = ctx_in_epoch(SENDER, VALID_UNTIL_EPOCH);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EApprovalExpired)]
fun verify_for_existing_account_aborts_after_the_last_epoch() {
    let mut ctx = ctx_in_epoch(SENDER, VALID_UNTIL_EPOCH + 1);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_aborts_for_another_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(&mut ctx, @0xacd, SIGNATURE_FOR_EXISTING_ACCOUNT);
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_aborts_for_an_approval_of_a_new_smart_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_NEW_SMART_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_aborts_for_a_signature_by_another_key() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        OTHER_KEY_SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_aborts_for_another_key_with_the_same_signature() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let nonce = verified_nonce::take_from_tx_context<AuthenticatedPublicKey>(&mut ctx);

    let public_key = authenticated_public_key::verify_for_existing_account(
        nonce,
        other_key(),
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
        VALID_UNTIL_EPOCH,
        &ctx,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_aborts_for_an_approval_made_for_another_network() {
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        OTHER_NETWORK_SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = iota::approval_message::EInvalidSignature)]
fun verify_for_existing_account_accepts_an_approval_once_in_a_transaction() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);

    // The approval is for the first nonce of the transaction; this call takes the second.
    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

#[test]
#[expected_failure(abort_code = authenticated_public_key::EBuiltinAuthenticatorsNotEnabled)]
fun verify_for_existing_account_aborts_when_builtin_authenticators_are_disabled() {
    protocol_config::set_feature_enabled_for_testing(b"enable_builtin_move_authenticators", false);
    let mut ctx = ctx_in_epoch(SENDER, 0);

    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );
    destroy(public_key);
}

// === into_public_key_for_existing_account ===

#[test]
fun into_public_key_for_existing_account_returns_the_key_for_the_approved_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );

    assert_eq(public_key.into_public_key_for_existing_account(ACCOUNT), key());
}

#[test]
#[expected_failure(abort_code = authenticated_public_key::EApprovalForAnotherAccount)]
fun into_public_key_for_existing_account_aborts_for_another_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );

    public_key.into_public_key_for_existing_account(@0xacd);
}

#[test]
#[expected_failure(abort_code = authenticated_public_key::EApprovalForNewSmartAccount)]
fun into_public_key_for_existing_account_aborts_for_an_approval_of_a_new_smart_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_NEW_SMART_ACCOUNT);

    // Not even for the sender the approval names.
    public_key.into_public_key_for_existing_account(SENDER);
}

// === into_public_key_for_new_smart_account ===

#[test]
fun into_public_key_for_new_smart_account_returns_the_key() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_new_smart_account(&mut ctx, SIGNATURE_FOR_NEW_SMART_ACCOUNT);

    assert_eq(public_key.into_public_key_for_new_smart_account(), key());
}

#[test]
#[expected_failure(abort_code = authenticated_public_key::EApprovalForExistingAccount)]
fun into_public_key_for_new_smart_account_aborts_for_an_approval_of_an_existing_account() {
    let mut ctx = ctx_in_epoch(SENDER, 0);
    let public_key = verify_for_existing_account(
        &mut ctx,
        ACCOUNT,
        SIGNATURE_FOR_EXISTING_ACCOUNT,
    );

    public_key.into_public_key_for_new_smart_account();
}

// === Helpers ===

fun key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), PUBLIC_KEY)
}

fun other_key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), OTHER_PUBLIC_KEY)
}

/// Returns the context of a transaction sent by `sender` and executed in `epoch`.
fun ctx_in_epoch(sender: address, epoch: u64): TxContext {
    tx_context::new_from_hint(sender, 0, epoch, 0, 0)
}

/// Takes the next nonce of the transaction and checks K's approval for a new `SmartAccount`,
/// with `signature`.
fun verify_for_new_smart_account(
    ctx: &mut TxContext,
    signature: vector<u8>,
): AuthenticatedPublicKey {
    let nonce = verified_nonce::take_from_tx_context<AuthenticatedPublicKey>(ctx);
    authenticated_public_key::verify_for_new_smart_account(
        nonce,
        key(),
        signature,
        VALID_UNTIL_EPOCH,
        ctx,
    )
}

/// Takes the next nonce of the transaction and checks K's approval for `account`, with
/// `signature`.
fun verify_for_existing_account(
    ctx: &mut TxContext,
    account: address,
    signature: vector<u8>,
): AuthenticatedPublicKey {
    let nonce = verified_nonce::take_from_tx_context<AuthenticatedPublicKey>(ctx);
    authenticated_public_key::verify_for_existing_account(
        nonce,
        key(),
        account,
        signature,
        VALID_UNTIL_EPOCH,
        ctx,
    )
}
