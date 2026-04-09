// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::claim_registry_tests;

use iota::claim_registry::{Self, ClaimRegistry};
use iota::test_scenario::{Self, Scenario};

// Pre-computed Ed25519 public key from fastcrypto test vectors.
// address = Blake2b256(pk)
const ED25519_PK: vector<u8> =
    x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";

// Pre-computed Secp256k1 compressed public key from fastcrypto test vectors.
// address = Blake2b256([0x01] || pk)
const SECP256K1_PK: vector<u8> =
    x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";

// Pre-computed Secp256r1 compressed public key from fastcrypto test vectors.
// address = Blake2b256([0x02] || pk)
const SECP256R1_PK: vector<u8> =
    x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";

// ============================================================
// Helpers
// ============================================================

fun setup(): Scenario {
    let mut scenario = test_scenario::begin(@0x0);
    {
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::create_for_testing(ctx);
    };
    scenario.next_tx(@0x0);
    scenario
}

// ============================================================
// Registry creation
// ============================================================

#[test]
fun test_registry_created() {
    let mut scenario = setup();
    scenario.next_tx(@0x0);
    let registry = scenario.take_shared<ClaimRegistry>();
    test_scenario::return_shared(registry);
    test_scenario::end(scenario);
}

// ============================================================
// claim — happy paths
// ============================================================

#[test]
fun test_claim_ed25519_happy_path() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(claim_registry::scheme_ed25519(), &pk);

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_ed25519(), pk, ctx);
        let (account, returned_pk, flag) = claim_registry::consume_ticket_for_testing(ticket);
        assert!(account == sender);
        assert!(flag == claim_registry::scheme_ed25519());
        let expected_pk = ED25519_PK;
        assert!(returned_pk == expected_pk);
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_secp256k1_happy_path() {
    let mut scenario = setup();
    let pk = SECP256K1_PK;
    let sender = claim_registry::derive_address_for_testing(claim_registry::scheme_secp256k1(), &pk);

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_secp256k1(), pk, ctx);
        let (account, returned_pk, flag) = claim_registry::consume_ticket_for_testing(ticket);
        assert!(account == sender);
        assert!(flag == claim_registry::scheme_secp256k1());
        let expected_pk = SECP256K1_PK;
        assert!(returned_pk == expected_pk);
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_secp256r1_happy_path() {
    let mut scenario = setup();
    let pk = SECP256R1_PK;
    let sender = claim_registry::derive_address_for_testing(claim_registry::scheme_secp256r1(), &pk);

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_secp256r1(), pk, ctx);
        let (account, returned_pk, flag) = claim_registry::consume_ticket_for_testing(ticket);
        assert!(account == sender);
        assert!(flag == claim_registry::scheme_secp256r1());
        let expected_pk = SECP256R1_PK;
        assert!(returned_pk == expected_pk);
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_move_authenticator_happy_path() {
    let mut scenario = setup();
    // For SCHEME_MOVE_AUTHENTICATOR, public_key may be empty and any sender is valid.
    scenario.next_tx(@0xcafe);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(
            &mut registry,
            claim_registry::scheme_move_authenticator(),
            vector[],
            ctx,
        );
        let (account, _pk, flag) = claim_registry::consume_ticket_for_testing(ticket);
        assert!(account == @0xcafe);
        assert!(flag == claim_registry::scheme_move_authenticator());
        assert!(claim_registry::is_claimed(&registry, @0xcafe));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

// ============================================================
// Error paths
// ============================================================

#[test]
#[expected_failure(abort_code = claim_registry::EAddressMismatch)]
fun test_claim_address_mismatch() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_ed25519(), ED25519_PK, ctx);
        claim_registry::consume_ticket_for_testing(ticket);
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim_registry::EAlreadyClaimed)]
fun test_claim_double_claim() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(claim_registry::scheme_ed25519(), &pk);

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_ed25519(), pk, ctx);
        claim_registry::consume_ticket_for_testing(ticket);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_ed25519(), pk, ctx);
        claim_registry::consume_ticket_for_testing(ticket);
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim_registry::EInvalidScheme)]
fun test_claim_invalid_scheme() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, 0xff, vector[], ctx);
        claim_registry::consume_ticket_for_testing(ticket);
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim_registry::EInvalidPublicKeyLength)]
fun test_claim_ed25519_wrong_key_length() {
    let mut scenario = setup();
    // 31 bytes instead of 32.
    let short_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd";
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let ticket = claim_registry::claim(&mut registry, claim_registry::scheme_ed25519(), short_pk, ctx);
        claim_registry::consume_ticket_for_testing(ticket);
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

// ============================================================
// derive_address correctness
// ============================================================

#[test]
fun test_derive_address_ed25519_is_deterministic() {
    let pk = ED25519_PK;
    let addr1 = claim_registry::derive_address_for_testing(claim_registry::scheme_ed25519(), &pk);
    let addr2 = claim_registry::derive_address_for_testing(claim_registry::scheme_ed25519(), &pk);
    assert!(addr1 == addr2);
}

#[test]
fun test_derive_address_differs_by_scheme() {
    let pk = ED25519_PK;
    let addr_ed = claim_registry::derive_address_for_testing(claim_registry::scheme_ed25519(), &pk);
    let addr_k1 = claim_registry::derive_address_for_testing(claim_registry::scheme_secp256k1(), &pk);
    assert!(addr_ed != addr_k1);
}

#[test]
fun test_is_not_claimed_initially() {
    let mut scenario = setup();
    scenario.next_tx(@0x0);
    let registry = scenario.take_shared<ClaimRegistry>();
    assert!(!claim_registry::is_claimed(&registry, @0xcafe));
    test_scenario::return_shared(registry);
    test_scenario::end(scenario);
}
