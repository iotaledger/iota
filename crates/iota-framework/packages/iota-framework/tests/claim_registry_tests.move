// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::claim_registry_tests;

use iota::claim_registry::{Self, ClaimRegistry};
use iota::public_key;
use iota::signature_scheme;
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
    let sender = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            signature_scheme::ed25519(),
            ED25519_PK,
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_secp256k1_happy_path() {
    let mut scenario = setup();
    let sender = claim_registry::derive_address_for_testing(
        signature_scheme::secp256k1(),
        &SECP256K1_PK,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            signature_scheme::secp256k1(),
            SECP256K1_PK,
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_secp256r1_happy_path() {
    let mut scenario = setup();
    let sender = claim_registry::derive_address_for_testing(
        signature_scheme::secp256r1(),
        &SECP256R1_PK,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            signature_scheme::secp256r1(),
            SECP256R1_PK,
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

// ============================================================
// claim — custom account module uses the returned UID
// ============================================================

#[test]
fun test_custom_account_creation() {
    let mut scenario = setup();
    let sender = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        iota::test_account::create(
            &mut registry,
            signature_scheme::ed25519(),
            ED25519_PK,
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    // Verify the Account object was transferred to the sender.
    scenario.next_tx(sender);
    {
        let account = scenario.take_from_sender<iota::test_account::Account>();
        test_scenario::return_to_sender(&scenario, account);
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
        claim_registry::claim(
            &mut registry,
            signature_scheme::ed25519(),
            ED25519_PK,
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim_registry::EAlreadyClaimed)]
fun test_claim_double_claim() {
    let mut scenario = setup();
    let sender = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim(
            &mut registry,
            signature_scheme::ed25519(),
            ED25519_PK,
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim(
            &mut registry,
            signature_scheme::ed25519(),
            ED25519_PK,
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = public_key::EUnknownPublicKeyScheme)]
fun test_claim_invalid_scheme() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // Scheme 0xff is not recognized — public_key::create aborts inside claim.
        claim_registry::claim(
            &mut registry,
            signature_scheme::from_flag_for_testing(0xff),
            ED25519_PK,
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = public_key::EUnknownPublicKeyScheme)]
fun test_claim_move_authenticator_is_invalid() {
    let mut scenario = setup();
    scenario.next_tx(@0xcafe);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // Scheme 0x07 (MoveAuthenticator) has no standard address derivation rule.
        claim_registry::claim(
            &mut registry,
            signature_scheme::from_flag_for_testing(0x07),
            SECP256K1_PK,
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = public_key::EInvalidPublicKeyLength)]
fun test_claim_ed25519_wrong_key_length() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // 31 bytes instead of 32 — public_key::create aborts inside claim.
        claim_registry::claim(
            &mut registry,
            signature_scheme::ed25519(),
            x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd",
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

// ============================================================
// derive_address correctness
// ============================================================

#[test]
fun test_derive_address_ed25519_is_deterministic() {
    let addr1 = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );
    let addr2 = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );
    assert!(addr1 == addr2);
}

#[test]
fun test_derive_address_differs_by_scheme() {
    let addr_ed = claim_registry::derive_address_for_testing(
        signature_scheme::ed25519(),
        &ED25519_PK,
    );
    let addr_k1 = claim_registry::derive_address_for_testing(
        signature_scheme::secp256k1(),
        &SECP256K1_PK,
    );
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
