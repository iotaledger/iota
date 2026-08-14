// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::attestor_registry_tests;

use iota::balance;
use iota::event;
use iota_system::attestor_registry::{Self, AttestorsActivatedEvent, AttestorsExitedEvent};

fun min_joining_bond(): u64 { attestor_registry::min_joining_bond() }

fun low_bond_threshold(): u64 { attestor_registry::low_bond_threshold() }
const MAX_INACTIVITY_EPOCHS: u64 = 7;
const INACTIVITY_PENALTY: u64 = 500_000_000_000;

fun make_pubkey(flag: u8, len: u64): vector<u8> {
    let mut pk = vector[flag];
    let mut i = 0;
    while (i < len) {
        pk.push_back(0xAB);
        i = i + 1;
    };
    pk
}

fun ed25519_pubkey(): vector<u8> { ed25519_key() }
// Seed-derived `flag || raw_key` public keys with proofs of possession.
// Regenerate with:
// cargo nextest run -p iota-types --lib print_attestor_move_fixtures --no-capture
fun ed25519_key(): vector<u8> {
    x"00876edc0d843534980747592afce708167a0b6516b0b9be7fd6eb864d05c0ba61"
}

fun secp256k1_key(): vector<u8> {
    x"0102253bda0005e6d0332d8f59bfadc6c682ae3a6797acda0b01bfcd078e371977d9"
}

fun secp256r1_key(): vector<u8> {
    x"02029b0265bc7ce0a9d1303493aa0e7acee45fad24ea8b70664779ef0c9ac98ccb19"
}

fun ed25519_pop_a1(): vector<u8> {
    x"22c37e6607d82edc3a8d3882af50a3f57e83cb7b0070a362849b4000090073c725328eb7019d130c36525038c5e39631865d97233f63c789488dfba93e5da20a"
}

fun ed25519_pop_a2(): vector<u8> {
    x"c7f8391a213927491731a7a66313f8e05ec5d353025ef4d8076d333b20b19e9b4b6b3869591adf5515d3e45961a0e5c291964d53a1dda6f369335f049f2f2d08"
}

fun secp256k1_pop_a1(): vector<u8> {
    x"e891902ea9f087fd8999cdd46c90624130b5214d4b0110614d9a499994d2d7b970154aa37e91430d12e6006c27b0a0edc7af8254d939be7647645afcbd9aee72"
}

fun secp256k1_pop_a2(): vector<u8> {
    x"c303e6d9233fb8786b247a1b4e2aeaa2a3b44fa49d6fad53d86ac80ef978d22b16e2e006fd1b41a0542c1ada63c883aaa7a6a4d6cf4e02ed67689fc5e6e07574"
}

fun secp256r1_pop_a3(): vector<u8> {
    x"836bc4dd6711e234fa480a3c478dd49b161854aa5da7a5f9838ac244d853afdc7c851015628d6b61d6dce73cb2d80f6a7e7ccefc8392e77c7075c6d5e1fca2d1"
}

fun pubkey_a(): vector<u8> { ed25519_key() }
fun pubkey_b(): vector<u8> { secp256k1_key() }

// === Pubkey validation ===
//
// Validation lives in the `validate_attestor_pubkey` native (a private fn),
// so it is exercised indirectly through `register` / `rotate_key`. Valid
// keys for all three plain schemes are accepted here; rejection cases live
// in the register/rotate failure tests below.

#[test]
fun test_register_accepts_all_plain_schemes() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256k1_key(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256r1_key(),
        secp256r1_pop_a3(),
        @0xA3,
        5,
    );
    assert!(registry.pending_count() == 3);
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_wrong_length() {
    let mut registry = attestor_registry::new();
    // ed25519 flag with a 33-byte key (must be 32)
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        make_pubkey(0x00, 33),
        vector[],
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_empty_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(min_joining_bond()), vector[], vector[], @0xA1, 5);
    abort 0
}

// === Registration ===

#[test]
fun test_register_lands_in_pending() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    assert!(registry.pending_count() == 1);
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::EBondTooLow)]
fun test_register_rejects_low_bond() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond() - 1),
        ed25519_pubkey(),
        vector[],
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_bad_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        make_pubkey(0x03, 32),
        vector[],
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyRegistered)]
fun test_register_rejects_duplicate_pending() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::ETooManyAttestors)]
fun test_register_rejects_at_max_count() {
    let mut registry = attestor_registry::new();
    // MAX_ATTESTOR_COUNT = 1_000; cheaply fill pending to the cap (the
    // capacity assert in `register` fires before its O(n) duplicate scan).
    let mut i: u256 = 0;
    while (i < 1_000) {
        registry.push_pending_for_testing(iota::address::from_u256(0x10000 + i), min_joining_bond());
        i = i + 1;
    };
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        vector[],
        @0xFFFF,
        5,
    );
    abort 0
}

// === Deregistration ===

#[test]
fun test_deregister_pending_refunds_immediately() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let mut refund = registry.deregister(@0xA1, 5);
    assert!(refund.is_some());
    let bal = refund.extract();
    assert!(bal.value() == min_joining_bond());
    bal.destroy_for_testing();
    refund.destroy_none();
    assert!(registry.pending_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_deregister_active_is_requested_not_immediate() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    let refund = registry.deregister(@0xA1, 6);
    assert!(refund.is_none());
    refund.destroy_none();
    assert!(registry.active_count() == 1);
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyDeregistering)]
fun test_double_deregister_aborts() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    let r1 = registry.deregister(@0xA1, 6);
    r1.destroy_none();
    let _r2 = registry.deregister(@0xA1, 6);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::ENotAnAttestor)]
fun test_deregister_unknown_aborts() {
    let mut registry = attestor_registry::new();
    let _r = registry.deregister(@0xA1, 5);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyRegistered)]
fun test_reregister_while_exiting_rejected() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    let r = registry.deregister(@0xA1, 6);
    r.destroy_none();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        6,
    );
    abort 0
}

// === Deposit & rotation ===

#[test]
fun test_deposit_increases_bond_for_active_and_pending() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.deposit(@0xA1, balance::create_for_testing(500), 5);
    registry.activate_for_testing();
    registry.deposit(@0xA1, balance::create_for_testing(500), 6);
    assert!(registry.active_attestors()[0].bond_value() == min_joining_bond() + 1000);
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::ENotAnAttestor)]
fun test_deposit_unknown_aborts() {
    let mut registry = attestor_registry::new();
    registry.deposit(@0xA1, balance::create_for_testing(500), 5);
    abort 0
}

#[test]
fun test_rotate_key_stages_replacement() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, pubkey_b(), secp256k1_pop_a1());
    assert!(registry.active_attestors()[0].attestor_pubkey() == ed25519_pubkey());
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::ENotActiveAttestor)]
fun test_rotate_key_rejected_for_pending() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.rotate_key(@0xA1, pubkey_b(), vector[]);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyDeregistering)]
fun test_rotate_key_rejected_while_exiting() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    let r = registry.deregister(@0xA1, 6);
    r.destroy_none();
    registry.rotate_key(@0xA1, pubkey_b(), vector[]);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_rotate_key_rejects_bad_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_pubkey(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, make_pubkey(0x09, 32), vector[]);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidProofOfPossession)]
fun test_register_rejects_missing_pop() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        vector[],
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidProofOfPossession)]
fun test_register_rejects_pop_for_other_sender() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a2(),
        @0xA1,
        5,
    );
    abort 0
}

// === Epoch processing ===

#[test]
fun test_advance_epoch_activates_pending_in_registration_order() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_b(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    let (evicted, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == 0);
    evicted.destroy_zero();
    assert!(registry.active_count() == 2);
    assert!(registry.pending_count() == 0);
    assert!(registry.active_attestors()[0].attestor_address() == @0xA1);
    assert!(registry.active_attestors()[1].attestor_address() == @0xA2);
    assert!(registry.active_attestors()[0].activation_epoch() == 6);
    registry.destroy_for_testing();
}

#[test]
fun test_advance_epoch_processes_removals_preserving_order() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_b(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256r1_key(),
        secp256r1_pop_a3(),
        @0xA3,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.deregister(@0xA2, 6).destroy_none();
    let (evicted_bond, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_count() == 2);
    assert!(registry.active_attestors()[0].attestor_address() == @0xA1);
    assert!(registry.active_attestors()[1].attestor_address() == @0xA3);
    registry.destroy_for_testing();
}

#[test]
fun test_advance_epoch_applies_staged_rotation_in_place() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.rotate_key(@0xA1, pubkey_b(), secp256k1_pop_a1());
    let (evicted_bond, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_attestors()[0].attestor_pubkey() == pubkey_b());
    registry.destroy_for_testing();
}

#[test]
fun test_low_bond_eviction_burns_remaining_bond() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    let slashed = registry.slash(@0xA1, min_joining_bond() - low_bond_threshold() + 1);
    assert!(slashed.value() == min_joining_bond() - low_bond_threshold() + 1);
    slashed.destroy_for_testing();
    let (evicted, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == low_bond_threshold() - 1);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_eviction_wins_over_voluntary_removal() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.deregister(@0xA1, 6).destroy_none();
    registry.slash(@0xA1, min_joining_bond()).destroy_for_testing();
    let (evicted, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_topup_prevents_eviction() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.slash(@0xA1, min_joining_bond() - 1).destroy_for_testing();
    registry.deposit(@0xA1, balance::create_for_testing(low_bond_threshold()), 6);
    let (evicted, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted.destroy_zero();
    assert!(registry.active_count() == 1);
    registry.destroy_for_testing();
}

// === Inactivity ===

#[test]
fun test_last_active_epoch_initialized_to_activation_epoch() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_attestors()[0].last_active_epoch() == 6);
    registry.destroy_for_testing();
}

#[test]
fun test_refresh_activity_updates_last_active_epoch() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.refresh_activity(vector[0], 9);
    assert!(registry.active_attestors()[0].last_active_epoch() == 9);
    registry.destroy_for_testing();
}

#[test]
fun test_refresh_activity_skips_out_of_range_and_tolerates_duplicates() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    // Out-of-range index 7 is skipped, duplicate 0s are idempotent; no abort.
    registry.refresh_activity(vector[7, 0, 0], 9);
    assert!(registry.active_attestors()[0].last_active_epoch() == 9);
    registry.destroy_for_testing();
}

#[test]
fun test_refresh_activity_empty_list_is_noop() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.refresh_activity(vector[], 9);
    assert!(registry.active_attestors()[0].last_active_epoch() == 6);
    registry.destroy_for_testing();
}

#[test]
fun test_attestor_survives_exactly_the_inactivity_window() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    // last_active = 6; at the boundary starting 6 + window the gap is not
    // yet strictly greater than the window, so the attestor survives.
    let (evicted_bond, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_count() == 1);
    registry.destroy_for_testing();
}

#[test]
fun test_inactive_attestor_dropped_with_penalty_after_window() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    // Gap is window + 1: dropped; only the penalty is burned.
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == INACTIVITY_PENALTY);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_refreshed_attestor_survives_past_the_window() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.refresh_activity(vector[0], 13);
    let (evicted_bond, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_count() == 1);
    // A later boundary past the refreshed epoch's window drops it.
    let (evicted, _departed) = registry.advance_epoch(13 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == INACTIVITY_PENALTY);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_inactivity_penalty_beats_pending_deregistration() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.deregister(@0xA1, 6).destroy_none();
    // Inactive AND deregistering: the penalty is still charged.
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == INACTIVITY_PENALTY);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_eviction_beats_inactivity() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.slash(@0xA1, min_joining_bond() - low_bond_threshold() + 1).destroy_for_testing();
    // Low bond AND inactive: full remaining bond is burned, not just the penalty.
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == low_bond_threshold() - 1);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_deregistration_within_window_refunds_in_full() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.deregister(@0xA1, 6).destroy_none();
    // Still within the window: a plain voluntary removal, nothing burned.
    let (evicted_bond, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_penalty_charged_from_bond_at_exactly_the_eviction_threshold() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    // Bond exactly at the threshold is NOT evicted (the check is strict
    // less-than); the inactivity penalty applies instead.
    registry.slash(@0xA1, min_joining_bond() - low_bond_threshold()).destroy_for_testing();
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == INACTIVITY_PENALTY);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_inactivity_drop_discards_staged_rotation() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.rotate_key(@0xA1, pubkey_b(), secp256k1_pop_a1());
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_dropped_address_can_reregister() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    let dropped_at = 6 + MAX_INACTIVITY_EPOCHS + 1;
    let (evicted_bond, _departed) = registry.advance_epoch(dropped_at, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_for_testing();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        pubkey_a(),
        ed25519_pop_a1(),
        @0xA1,
        dropped_at,
    );
    assert!(registry.pending_count() == 1);
    registry.destroy_for_testing();
}

// === Pubkey uniqueness ===

#[test, expected_failure(abort_code = attestor_registry::EDuplicatePubkey)]
fun test_register_rejects_pubkey_of_active_attestor() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a2(),
        @0xA2,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EDuplicatePubkey)]
fun test_register_rejects_pubkey_of_pending_attestor() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a2(),
        @0xA2,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EDuplicatePubkey)]
fun test_register_rejects_pubkey_staged_for_rotation() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, secp256k1_key(), secp256k1_pop_a1());
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256k1_key(),
        secp256k1_pop_a2(),
        @0xA2,
        6,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EDuplicatePubkey)]
fun test_rotate_rejects_pubkey_of_other_attestor() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256k1_key(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, secp256k1_key(), secp256k1_pop_a1());
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EDuplicatePubkey)]
fun test_rotate_rejects_own_current_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, ed25519_key(), ed25519_pop_a1());
    abort 0
}

#[test]
fun test_pubkey_reusable_after_removal() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.deregister(@0xA1, 6).destroy_none();
    let (evicted_bond, _departed) = registry.advance_epoch(7, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a2(),
        @0xA2,
        7,
    );
    assert!(registry.pending_count() == 1);
    registry.destroy_for_testing();
}

// === Metadata ===

#[test]
fun test_add_update_and_remove_metadata() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    attestor_registry::add_metadata(&mut uid, @0xA1, b"name", b"desc", b"https://a", b"https://l");
    attestor_registry::update_metadata_name(&mut uid, @0xA1, b"name2");
    attestor_registry::update_metadata_description(&mut uid, @0xA1, b"desc2");
    attestor_registry::update_metadata_url(&mut uid, @0xA1, b"https://b");
    attestor_registry::update_metadata_logo(&mut uid, @0xA1, b"https://m");
    attestor_registry::remove_metadata(&mut uid, @0xA1);
    // removing again must abort, proving the entry is gone — covered below;
    // here the UID must be cleanly deletable (no children left).
    uid.delete();
}

#[test, expected_failure(abort_code = attestor_registry::ENoMetadataEntry)]
fun test_update_metadata_without_entry_aborts() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    attestor_registry::update_metadata_name(&mut uid, @0xA1, b"name");
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAttestorMetadataExceedingLengthLimit)]
fun test_add_metadata_rejects_long_name() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    let mut long = vector<u8>[];
    257u64.do!(|_| long.push_back(0x61));
    attestor_registry::add_metadata(&mut uid, @0xA1, long, b"", b"", b"");
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAttestorMetadataExceedingLengthLimit)]
fun test_update_metadata_rejects_long_description() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    attestor_registry::add_metadata(&mut uid, @0xA1, b"n", b"d", b"u", b"l");
    let mut long = vector<u8>[];
    257u64.do!(|_| long.push_back(0x61));
    attestor_registry::update_metadata_description(&mut uid, @0xA1, long);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAttestorMetadataExceedingLengthLimit)]
fun test_update_metadata_rejects_long_url() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    attestor_registry::add_metadata(&mut uid, @0xA1, b"n", b"d", b"u", b"l");
    let mut long = vector<u8>[];
    257u64.do!(|_| long.push_back(0x61));
    attestor_registry::update_metadata_url(&mut uid, @0xA1, long);
    abort 0
}

#[test, expected_failure(abort_code = std::ascii::EInvalidASCIICharacter)]
fun test_update_metadata_rejects_non_ascii_name() {
    let mut ctx = tx_context::dummy();
    let mut uid = iota::object::new(&mut ctx);
    attestor_registry::add_metadata(&mut uid, @0xA1, b"n", b"d", b"u", b"l");
    // 0xC3 0xA9 = UTF-8 "é": valid UTF-8, not ASCII — to_ascii_string aborts.
    attestor_registry::update_metadata_name(&mut uid, @0xA1, x"C3A9");
    abort 0
}

#[test]
fun test_advance_epoch_reports_departed_addresses() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(min_joining_bond()), ed25519_key(), ed25519_pop_a1(), @0xA1, 5);
    registry.register(balance::create_for_testing(min_joining_bond()), secp256k1_key(), secp256k1_pop_a2(), @0xA2, 5);
    let (b, departed) = registry.advance_epoch(6, &mut ctx);
    b.destroy_zero();
    assert!(departed.unpack_for_testing().is_empty());
    // A1 leaves voluntarily, A2 is evicted for low bond: both reported.
    registry.deregister(@0xA1, 6).destroy_none();
    registry.slash(@0xA2, min_joining_bond()).destroy_for_testing();
    let (evicted, departed) = registry.advance_epoch(7, &mut ctx);
    evicted.destroy_for_testing();
    let departed = departed.unpack_for_testing();
    assert!(departed.length() == 2);
    assert!(departed.contains(&@0xA1));
    assert!(departed.contains(&@0xA2));
    registry.destroy_for_testing();
}

#[test]
fun test_mixed_exit_reasons_in_one_boundary() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256k1_key(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256r1_key(),
        secp256r1_pop_a3(),
        @0xA3,
        5,
    );
    let (evicted_bond, _departed) = registry.advance_epoch(6, &mut ctx);
    _departed.unpack_for_testing();
    evicted_bond.destroy_zero();
    // A1: low bond -> evicted (burn all). A2: untouched -> inactivity
    // (penalty). A3: refreshed + deregistering -> voluntary (full refund).
    registry.slash(@0xA1, min_joining_bond() - low_bond_threshold() + 1).destroy_for_testing();
    registry.deregister(@0xA3, 6).destroy_none();
    registry.refresh_activity(vector[2], 13);
    let (evicted, _departed) = registry.advance_epoch(6 + MAX_INACTIVITY_EPOCHS + 1, &mut ctx);
    _departed.unpack_for_testing();
    assert!(evicted.value() == (low_bond_threshold() - 1) + INACTIVITY_PENALTY);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_advance_epoch_batches_boundary_events() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        ed25519_key(),
        ed25519_pop_a1(),
        @0xA1,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256k1_key(),
        secp256k1_pop_a2(),
        @0xA2,
        5,
    );
    registry.register(
        balance::create_for_testing(min_joining_bond()),
        secp256r1_key(),
        secp256r1_pop_a3(),
        @0xA3,
        5,
    );
    let (evicted_bond, departed) = registry.advance_epoch(6, &mut ctx);
    // The first boundary only activates: no exits yet, so no
    // AttestorsExitedEvent should fire alongside the activation.
    assert!(departed.unpack_for_testing().is_empty());
    evicted_bond.destroy_zero();
    assert!(event::events_by_type<AttestorsExitedEvent>().is_empty());

    // A1: low bond -> evicted (burn all). A2: untouched -> inactivity
    // (penalty). A3: refreshed + deregistering -> voluntary (full refund).
    // B1: freshly pending -> activates at the same boundary.
    registry.slash(@0xA1, min_joining_bond() - low_bond_threshold() + 1).destroy_for_testing();
    registry.deregister(@0xA3, 6).destroy_none();
    registry.refresh_activity(vector[2], 13);
    registry.push_pending_for_testing(@0xB1, min_joining_bond());
    let boundary_epoch = 6 + MAX_INACTIVITY_EPOCHS + 1;
    let (evicted, departed) = registry.advance_epoch(boundary_epoch, &mut ctx);
    assert!(departed.unpack_for_testing().length() == 3);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 1);

    let activated_events = event::events_by_type<AttestorsActivatedEvent>();
    // One from the epoch-6 boundary (A1, A2, A3), one from this boundary (B1).
    assert!(activated_events.length() == 2);
    let (activated_epoch, activated) = activated_events[1].unpack_activated_event_for_testing();
    assert!(activated_epoch == boundary_epoch);
    assert!(activated == vector[@0xB1]);

    let exited_events = event::events_by_type<AttestorsExitedEvent>();
    assert!(exited_events.length() == 1);
    let (exited_epoch, mut exited) = exited_events[0].unpack_exited_event_for_testing();
    assert!(exited_epoch == boundary_epoch);
    assert!(exited.length() == 3);
    // Popped from the back of the sorted index list, so the highest active
    // index (A3) is processed first, then A2, then A1.
    let (addr, reason, refunded, burned) = exited.remove(0).unpack_exit_info_for_testing();
    assert!(addr == @0xA3 && reason == 2 && refunded == min_joining_bond() && burned == 0);
    let (addr, reason, refunded, burned) = exited.remove(0).unpack_exit_info_for_testing();
    assert!(
        addr == @0xA2 && reason == 1 && refunded == min_joining_bond() - INACTIVITY_PENALTY && burned == INACTIVITY_PENALTY,
    );
    let (addr, reason, refunded, burned) = exited.remove(0).unpack_exit_info_for_testing();
    assert!(addr == @0xA1 && reason == 0 && refunded == 0 && burned == low_bond_threshold() - 1);

    registry.destroy_for_testing();
}
