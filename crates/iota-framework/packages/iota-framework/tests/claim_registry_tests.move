// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::claim_registry_tests;

use iota::claim_registry::{Self, ClaimRegistry};
use iota::iota_default_account::{Self, IotaDefaultAccount};
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

/// Set up a fresh `ClaimRegistry` and return the scenario.
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
// claim_ed25519
// ============================================================

#[test]
fun test_claim_ed25519_happy_path() {
    let mut scenario = setup();
    let pk = ED25519_PK;

    // Derive the address that corresponds to this Ed25519 public key.
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    // Account must now exist as a shared object.
    scenario.next_tx(sender);
    {
        let account = scenario.take_shared<IotaDefaultAccount>();
        // ObjectID must equal the sender address.
        assert!(object::id_address(&account) == sender);
        // Stored scheme must be Ed25519.
        assert!(account.scheme() == iota_default_account::scheme_ed25519());
        // Stored public key must match what was provided.
        let expected_pk = ED25519_PK;
        assert!(account.public_key() == &expected_pk);
        test_scenario::return_shared(account);
    };

    // Registry must mark the address as claimed.
    scenario.next_tx(sender);
    {
        let registry = scenario.take_shared<ClaimRegistry>();
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

// ============================================================
// claim_secp256k1
// ============================================================

#[test]
fun test_claim_secp256k1_happy_path() {
    let mut scenario = setup();
    let pk = SECP256K1_PK;

    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_secp256k1(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_secp256k1(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let account = scenario.take_shared<IotaDefaultAccount>();
        assert!(object::id_address(&account) == sender);
        assert!(account.scheme() == iota_default_account::scheme_secp256k1());
        let expected_pk = SECP256K1_PK;
        assert!(account.public_key() == &expected_pk);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario);
}

// ============================================================
// claim_secp256r1
// ============================================================

#[test]
fun test_claim_secp256r1_happy_path() {
    let mut scenario = setup();
    let pk = SECP256R1_PK;

    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_secp256r1(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_secp256r1(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let account = scenario.take_shared<IotaDefaultAccount>();
        assert!(object::id_address(&account) == sender);
        assert!(account.scheme() == iota_default_account::scheme_secp256r1());
        let expected_pk = SECP256R1_PK;
        assert!(account.public_key() == &expected_pk);
        test_scenario::return_shared(account);
    };

    // Registry must mark the address as claimed.
    scenario.next_tx(sender);
    {
        let registry = scenario.take_shared<ClaimRegistry>();
        assert!(claim_registry::is_claimed(&registry, sender));
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

// ============================================================
// Error paths — claim
// ============================================================

#[test]
#[expected_failure(abort_code = claim_registry::EAddressMismatch)]
fun test_claim_address_mismatch() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    // Use a different sender — not the one derived from the pubkey.
    let wrong_sender = @0xdead;

    scenario.next_tx(wrong_sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim_registry::EAlreadyClaimed)]
fun test_claim_double_claim() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    // First claim — succeeds.
    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    // Second claim — must abort with EAlreadyClaimed.
    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

// ============================================================
// Key rotation
// ============================================================

#[test]
fun test_rotate_key_happy_path() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    // Rotate to a different Ed25519 key (any 32 distinct bytes).
    let new_pk = x"aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";

    scenario.next_tx(sender);
    {
        let mut account = scenario.take_shared<IotaDefaultAccount>();
        let ctx = test_scenario::ctx(&mut scenario);
        iota_default_account::rotate_key(
            &mut account,
            new_pk,
            iota_default_account::scheme_ed25519(),
            ctx,
        );
        assert!(account.scheme() == iota_default_account::scheme_ed25519());
        assert!(account.public_key() == &new_pk);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = iota_default_account::ENotAccountOwner)]
fun test_rotate_key_wrong_sender() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(@0xbad);
    {
        let mut account = scenario.take_shared<IotaDefaultAccount>();
        let ctx = test_scenario::ctx(&mut scenario);
        iota_default_account::rotate_key(&mut account, ED25519_PK, iota_default_account::scheme_ed25519(), ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = iota_default_account::EInvalidScheme)]
fun test_rotate_key_invalid_scheme() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let mut account = scenario.take_shared<IotaDefaultAccount>();
        let ctx = test_scenario::ctx(&mut scenario);
        iota_default_account::rotate_key(&mut account, pk, 0xff, ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = iota_default_account::EInvalidPublicKeyLength)]
fun test_rotate_key_wrong_key_length() {
    let mut scenario = setup();
    let pk = ED25519_PK;
    let sender = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, pk, ctx);
        test_scenario::return_shared(registry);
    };

    scenario.next_tx(sender);
    {
        let mut account = scenario.take_shared<IotaDefaultAccount>();
        let ctx = test_scenario::ctx(&mut scenario);
        // 31 bytes — abort at key-length check.
        let short_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd";
        iota_default_account::rotate_key(&mut account, short_pk, iota_default_account::scheme_ed25519(), ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario);
}

// ============================================================
// Public key length validation
// ============================================================

#[test]
#[expected_failure(abort_code = claim_registry::EInvalidPublicKeyLength)]
fun test_claim_ed25519_wrong_key_length() {
    let mut scenario = setup();
    // 31 bytes instead of 32 — must abort with EInvalidPublicKeyLength.
    let short_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd";
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim_ed25519(&mut registry, short_pk, ctx);
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
    let addr1 = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );
    let addr2 = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );
    assert!(addr1 == addr2);
}

#[test]
fun test_derive_address_differs_by_scheme() {
    let pk = ED25519_PK;
    let addr_ed = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_ed25519(),
        &pk,
    );
    let addr_k1 = claim_registry::derive_address_for_testing(
        iota_default_account::scheme_secp256k1(),
        &pk,
    );
    // Different scheme flags must produce different addresses for the same key bytes.
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