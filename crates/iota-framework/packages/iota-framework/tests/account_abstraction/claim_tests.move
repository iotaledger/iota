// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::claim_tests;

use iota::claim;
use iota::public_key;
use iota::test_scenario;

// Pre-computed Ed25519 public key from fastcrypto test vectors.
// Layout: [0x00 (Ed25519 flag)] || [32-byte key]
// address = Blake2b256(raw_bytes)
const ED25519_PK: vector<u8> =
    x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const ED25519_ADDR: address = @0xcef6bafea1d59edb73ff5ec9e8aa58354796e1b572b695d64237ce9c15a34a03;

#[test]
fun test_claim_address_mints_uid_for_sender() {
    let mut scenario = test_scenario::begin(ED25519_ADDR);
    {
        let ctx = test_scenario::ctx(&mut scenario);
        let uid = claim::claim_address(public_key::from_prefixed_bytes(ED25519_PK), ctx);
        assert!(uid.to_address() == ED25519_ADDR);
        uid.delete();
    };
    test_scenario::end(scenario);
}

#[test]
fun test_claim_address_can_mint_the_same_uid_twice() {
    let mut scenario = test_scenario::begin(ED25519_ADDR);
    {
        let ctx = test_scenario::ctx(&mut scenario);
        claim::claim_address(public_key::from_prefixed_bytes(ED25519_PK), ctx).delete();
        claim::claim_address(public_key::from_prefixed_bytes(ED25519_PK), ctx).delete();
    };
    test_scenario::end(scenario);
}

#[test]
#[expected_failure(abort_code = claim::EAddressMismatch)]
fun test_claim_address_aborts_on_address_mismatch() {
    let mut scenario = test_scenario::begin(@0xdead);
    {
        let ctx = test_scenario::ctx(&mut scenario);
        claim::claim_address(public_key::from_prefixed_bytes(ED25519_PK), ctx).delete();
    };
    test_scenario::end(scenario);
}
