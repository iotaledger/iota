// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::claim_registry_tests;

use iota::claim_registry::{Self, ClaimRegistry};
use iota::public_key;
use iota::signature_scheme;
use iota::test_scenario::{Self, Scenario};

// Pre-computed Ed25519 public key from fastcrypto test vectors.
// Layout: [0x00 (Ed25519 flag)] || [32-byte key]
// address = Blake2b256(raw_bytes)
const ED25519_PK: vector<u8> =
    x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";

// Pre-computed Secp256k1 compressed public key from fastcrypto test vectors.
// Layout: [0x01 (Secp256k1 flag)] || [33-byte compressed key]
// address = Blake2b256([0x01] || raw_bytes)
const SECP256K1_PK: vector<u8> =
    x"0102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";

// Pre-computed Secp256r1 compressed public key from fastcrypto test vectors.
// Layout: [0x02 (Secp256r1 flag)] || [33-byte compressed key]
// address = Blake2b256([0x02] || raw_bytes)
const SECP256R1_PK: vector<u8> =
    x"020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";

// Minimal BCS-encoded MultiSigPublicKey: 1 Ed25519 signer (ED25519_PK raw), weight=1, threshold=1.
// Layout: [0x03 (MultiSig flag)] || ULEB128(num_signers=1) | ULEB128(tag=0 Ed25519) | 32-byte key | u8(weight=1) | u16-LE(threshold=1)
// address = Blake2b256([0x03] || threshold_le16 || pk || weight)  ← no flag for Ed25519
const MULTISIG_PK: vector<u8> =
    x"030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100";

// BCS-encoded MultiSigPublicKey: 1 Ed25519 signer + 1 Secp256k1 signer, weight=1 each, threshold=1.
// Layout: [0x03] || vec_len(2) | tag(0) | 32-byte ed25519 key | weight(1) | tag(1) | 33-byte secp256k1 key | weight(1) | threshold_le16(1)
// address = Blake2b256([0x03] || threshold_le16 || ed25519_pk || weight || 0x01 || secp256k1_pk || weight)
const MULTISIG_MIXED_PK: vector<u8> =
    x"030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100";

// Passkey uses a 33-byte compressed secp256r1 (P-256) public key — same wire format as Secp256r1.
// Layout: [0x06 (Passkey flag)] || [33-byte compressed key]
// address = Blake2b256([0x06] || raw_bytes)
const PASSKEY_PK: vector<u8> =
    x"060227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";

// Expected IOTA addresses for the keys above.
// Computed independently by the Rust node (Blake2b256 via fastcrypto), not by
// the Move to_iota_address() under test.
// Ed25519:  Blake2b256(raw)
// Others:   Blake2b256([flag] || raw)
// MultiSig: Blake2b256([0x03] || threshold_le16 || (scheme_flag || pk || weight)*)
const ED25519_ADDR: address = @0xcef6bafea1d59edb73ff5ec9e8aa58354796e1b572b695d64237ce9c15a34a03;
const SECP256K1_ADDR: address = @0x2fecbdf2652b089c64d127158d388621fdbbd156533fbcca5a0082aa0d2939fa;
const SECP256R1_ADDR: address = @0x318f591092f10b67a81963954fb9539ea3919444417726be4e1b95ce44fe2fc0;
const PASSKEY_ADDR: address = @0xa2f90cd2552d45ab5ba157dacf19597e2018108c6a80e4d7a4a5680d1542a7e8;
const MULTISIG_ADDR: address = @0x1cc23b51b2e3c8641eea35b29114a53ad7a76643dcb2763d12290a7b83cac525;
const MULTISIG_MIXED_ADDR: address =
    @0x2e6c30799340fef9d382542ff0cad8e2a20f766da8b71a25c2443eda658104e4;

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
    let sender = ED25519_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(ED25519_PK),
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
    let sender = SECP256K1_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(SECP256K1_PK),
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
    let sender = SECP256R1_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(SECP256R1_PK),
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_multisig_happy_path() {
    let mut scenario = setup();
    let sender = MULTISIG_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(MULTISIG_PK),
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_multisig_mixed_happy_path() {
    let mut scenario = setup();
    let sender = MULTISIG_MIXED_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(MULTISIG_MIXED_PK),
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
fun test_claim_passkey_happy_path() {
    let mut scenario = setup();
    let sender = PASSKEY_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(PASSKEY_PK),
            ctx,
        );
        assert!(claim_registry::is_claimed(&registry, sender));
        uid.delete();
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
        claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(ED25519_PK),
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
    let sender = ED25519_ADDR;

    scenario.next_tx(sender);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(ED25519_PK),
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
            public_key::from_prefixed_bytes(ED25519_PK),
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };

    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = signature_scheme::EUnknownScheme)]
fun test_claim_invalid_scheme() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // Flag 0xff is not recognized — scheme_from_flag aborts inside from_prefixed_bytes.
        claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(
                x"ffcc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88",
            ),
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = signature_scheme::EUnknownScheme)]
fun test_claim_move_authenticator_is_invalid() {
    let mut scenario = setup();
    scenario.next_tx(@0xcafe);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // Flag 0x07 (MoveAuthenticator) has no standard address derivation rule.
        claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(
                x"0702337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c",
            ),
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = public_key::EInvalidPublicKeyBytes)]
fun test_claim_ed25519_wrong_key_length() {
    let mut scenario = setup();
    scenario.next_tx(@0xdead);
    {
        let mut registry = scenario.take_shared<ClaimRegistry>();
        let ctx = test_scenario::ctx(&mut scenario);
        // 31 raw bytes instead of 32 — create aborts inside from_prefixed_bytes.
        claim_registry::claim(
            &mut registry,
            public_key::from_prefixed_bytes(
                x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd",
            ),
            ctx,
        ).delete();
        test_scenario::return_shared(registry);
    };
    test_scenario::end(scenario);
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
