// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::verified_nonce_tests;

use iota::verified_nonce;
use std::unit_test::assert_eq;

/// Stands for the check a nonce is taken for.
public struct TestCheck {}

/// Stands for another check.
public struct OtherCheck {}

const SENDER: address = @0xa11ce;
const LEASE: vector<u8> = x"00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

/// A context for `SENDER` in epoch 4, whose transaction may run in epochs 4
/// and 5 and holds `LEASE`.
fun leased_ctx(): TxContext {
    let mut ctx = tx_context::new_from_hint(SENDER, 0, 4, 0, 0);
    ctx.set_valid_window_for_testing(4, 5, 0, option::some(LEASE));
    ctx
}

#[test]
fun verify_lease_nonce_writes_the_lease_its_sender_and_the_nonce_key() {
    let mut ctx = leased_ctx();

    let nonce = verified_nonce::verify_lease_nonce<TestCheck>(7, 5, &mut ctx);
    let (lines, valid_until_epoch) = nonce.destroy_for_testing();

    assert_eq!(
        lines.to_string(),
        b"nonce source: lease 0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff of 0x00000000000000000000000000000000000000000000000000000000000a11ce\nnonce key: 7".to_string(),
    );
    assert_eq!(valid_until_epoch, 5);
}

#[test]
fun an_expiry_before_the_last_valid_epoch_is_accepted() {
    let mut ctx = leased_ctx();

    let nonce = verified_nonce::verify_lease_nonce<TestCheck>(0, 4, &mut ctx);
    let (_, valid_until_epoch) = nonce.destroy_for_testing();

    assert_eq!(valid_until_epoch, 4);
}

#[test]
fun different_nonce_keys_are_taken_in_one_transaction() {
    let mut ctx = leased_ctx();

    let (first, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
    let (second, _) = verified_nonce::verify_lease_nonce<TestCheck>(1, 5, &mut ctx).destroy_for_testing();

    assert!(first != second);
}

#[test, expected_failure(abort_code = verified_nonce::ENonceKeyAlreadyTaken)]
fun a_nonce_key_is_taken_once_per_transaction() {
    let mut ctx = leased_ctx();

    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
}

#[test, expected_failure(abort_code = verified_nonce::ENonceKeyAlreadyTaken)]
fun a_nonce_key_taken_for_one_check_is_taken_for_every_check() {
    let mut ctx = leased_ctx();

    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
    let (_, _) = verified_nonce::verify_lease_nonce<OtherCheck>(0, 5, &mut ctx).destroy_for_testing();
}

#[test]
fun a_nonce_key_can_be_taken_again_in_another_transaction() {
    let mut ctx = leased_ctx();
    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();

    let mut ctx = leased_ctx();
    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
}

#[test, expected_failure(abort_code = verified_nonce::ENoLease)]
fun verify_lease_nonce_aborts_without_a_valid_window() {
    let mut ctx = tx_context::new_from_hint(SENDER, 0, 4, 0, 0);

    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
}

#[test, expected_failure(abort_code = verified_nonce::ENoLease)]
fun verify_lease_nonce_aborts_when_the_valid_window_has_no_lease() {
    let mut ctx = tx_context::new_from_hint(SENDER, 0, 4, 0, 0);
    ctx.set_valid_window_for_testing(4, 5, 0, option::none());

    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 5, &mut ctx).destroy_for_testing();
}

#[test, expected_failure(abort_code = verified_nonce::ELeaseEndsBeforeExpiry)]
fun verify_lease_nonce_aborts_when_the_expiry_is_after_the_last_valid_epoch() {
    let mut ctx = leased_ctx();

    let (_, _) = verified_nonce::verify_lease_nonce<TestCheck>(0, 6, &mut ctx).destroy_for_testing();
}
