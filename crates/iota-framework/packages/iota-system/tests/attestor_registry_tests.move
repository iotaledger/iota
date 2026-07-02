// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::attestor_registry_tests;

use iota::balance;
use iota_system::attestor_registry;

const MIN_JOINING_BOND: u64 = 2_000_000_000_000;
const LOW_BOND_THRESHOLD: u64 = 1_000_000_000_000;

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
// Real `flag || raw_key` public keys (the native does on-curve validation,
// so arbitrary bytes are rejected). Generated from seeded keypairs.
fun ed25519_key(): vector<u8> {
    x"00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f"
}

fun secp256k1_key(): vector<u8> {
    x"0102770632ba449f7f0f6d7e8173ee8cdeee0c1676a4f02a9c10b877b2c022126a1d"
}

fun secp256r1_key(): vector<u8> {
    x"0202187de95d431e456a4a1a6837f732d94c21ceec701ae551025042d2f2e96ae05c"
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
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_key(), @0xA1, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), secp256k1_key(), @0xA2, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), secp256r1_key(), @0xA3, 5);
    assert!(registry.pending_count() == 3);
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_wrong_length() {
    let mut registry = attestor_registry::new();
    // ed25519 flag with a 33-byte key (must be 32)
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), make_pubkey(0x00, 33), @0xA1, 5);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_empty_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), vector[], @0xA1, 5);
    abort 0
}

// === Registration ===

#[test]
fun test_register_lands_in_pending() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(MIN_JOINING_BOND),
        ed25519_pubkey(),
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
        balance::create_for_testing(MIN_JOINING_BOND - 1),
        ed25519_pubkey(),
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_register_rejects_bad_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(
        balance::create_for_testing(MIN_JOINING_BOND),
        make_pubkey(0x03, 32),
        @0xA1,
        5,
    );
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyRegistered)]
fun test_register_rejects_duplicate_pending() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::ETooManyAttestors)]
fun test_register_rejects_at_max_count() {
    let mut registry = attestor_registry::new();
    // MAX_ATTESTOR_COUNT = 1_000; cheaply fill pending to the cap (the
    // capacity assert in `register` fires before its O(n) duplicate scan).
    let mut i: u256 = 0;
    while (i < 1_000) {
        registry.push_pending_for_testing(iota::address::from_u256(0x10000 + i), MIN_JOINING_BOND);
        i = i + 1;
    };
    registry.register(
        balance::create_for_testing(MIN_JOINING_BOND),
        ed25519_pubkey(),
        @0xFFFF,
        5,
    );
    abort 0
}

// === Deregistration ===

#[test]
fun test_deregister_pending_refunds_immediately() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    let mut refund = registry.deregister(@0xA1, 5);
    assert!(refund.is_some());
    let bal = refund.extract();
    assert!(bal.value() == MIN_JOINING_BOND);
    bal.destroy_for_testing();
    refund.destroy_none();
    assert!(registry.pending_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_deregister_active_is_requested_not_immediate() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
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
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
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
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.activate_for_testing();
    let r = registry.deregister(@0xA1, 6);
    r.destroy_none();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 6);
    abort 0
}

// === Deposit & rotation ===

#[test]
fun test_deposit_increases_bond_for_active_and_pending() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.deposit(@0xA1, balance::create_for_testing(500), 5);
    registry.activate_for_testing();
    registry.deposit(@0xA1, balance::create_for_testing(500), 6);
    assert!(registry.active_attestors()[0].bond_value() == MIN_JOINING_BOND + 1000);
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
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, pubkey_b(), 6);
    assert!(registry.active_attestors()[0].attestor_pubkey() == ed25519_pubkey());
    registry.destroy_for_testing();
}

#[test, expected_failure(abort_code = attestor_registry::ENotActiveAttestor)]
fun test_rotate_key_rejected_for_pending() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.rotate_key(@0xA1, pubkey_b(), 5);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EAlreadyDeregistering)]
fun test_rotate_key_rejected_while_exiting() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.activate_for_testing();
    let r = registry.deregister(@0xA1, 6);
    r.destroy_none();
    registry.rotate_key(@0xA1, pubkey_b(), 6);
    abort 0
}

#[test, expected_failure(abort_code = attestor_registry::EInvalidPubkey)]
fun test_rotate_key_rejects_bad_pubkey() {
    let mut registry = attestor_registry::new();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), ed25519_pubkey(), @0xA1, 5);
    registry.activate_for_testing();
    registry.rotate_key(@0xA1, make_pubkey(0x09, 32), 6);
    abort 0
}

// === Epoch processing ===

#[test]
fun test_advance_epoch_activates_pending_in_registration_order() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_b(), @0xA2, 5);
    let evicted = registry.advance_epoch(6, &mut ctx);
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
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA2, 5);
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA3, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    registry.deregister(@0xA2, 6).destroy_none();
    registry.advance_epoch(7, &mut ctx).destroy_zero();
    assert!(registry.active_count() == 2);
    assert!(registry.active_attestors()[0].attestor_address() == @0xA1);
    assert!(registry.active_attestors()[1].attestor_address() == @0xA3);
    registry.destroy_for_testing();
}

#[test]
fun test_advance_epoch_applies_staged_rotation_in_place() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    registry.rotate_key(@0xA1, pubkey_b(), 6);
    registry.advance_epoch(7, &mut ctx).destroy_zero();
    assert!(registry.active_attestors()[0].attestor_pubkey() == pubkey_b());
    registry.destroy_for_testing();
}

#[test]
fun test_low_bond_eviction_burns_remaining_bond() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    let slashed = registry.slash(@0xA1, MIN_JOINING_BOND - LOW_BOND_THRESHOLD + 1);
    assert!(slashed.value() == MIN_JOINING_BOND - LOW_BOND_THRESHOLD + 1);
    slashed.destroy_for_testing();
    let evicted = registry.advance_epoch(7, &mut ctx);
    assert!(evicted.value() == LOW_BOND_THRESHOLD - 1);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_eviction_wins_over_voluntary_removal() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    registry.deregister(@0xA1, 6).destroy_none();
    registry.slash(@0xA1, MIN_JOINING_BOND).destroy_for_testing();
    let evicted = registry.advance_epoch(7, &mut ctx);
    evicted.destroy_for_testing();
    assert!(registry.active_count() == 0);
    registry.destroy_for_testing();
}

#[test]
fun test_topup_prevents_eviction() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    registry.slash(@0xA1, MIN_JOINING_BOND - 1).destroy_for_testing();
    registry.deposit(@0xA1, balance::create_for_testing(LOW_BOND_THRESHOLD), 6);
    let evicted = registry.advance_epoch(7, &mut ctx);
    evicted.destroy_zero();
    assert!(registry.active_count() == 1);
    registry.destroy_for_testing();
}

// === Inactivity ===

#[test]
fun test_last_active_epoch_initialized_to_activation_epoch() {
    let mut registry = attestor_registry::new();
    let mut ctx = tx_context::dummy();
    registry.register(balance::create_for_testing(MIN_JOINING_BOND), pubkey_a(), @0xA1, 5);
    registry.advance_epoch(6, &mut ctx).destroy_zero();
    assert!(registry.active_attestors()[0].last_active_epoch() == 6);
    registry.destroy_for_testing();
}
