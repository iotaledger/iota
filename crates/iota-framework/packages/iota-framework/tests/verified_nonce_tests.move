// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::verified_nonce_tests;

use iota::test_utils::{assert_eq, destroy};
use iota::verified_nonce;

/// A check type, as a check passes it to `take_from_tx_context`.
public struct TestCheck {}

/// Another check type.
public struct OtherCheck {}

// Blake2b-256 of the coin ID `@0xc01`, the version 412 and the index, computed outside Move.
const HASH_INDEX_0: vector<u8> =
    x"8122e789e9387c8ee9dc4811c945e35a196e3c741a6210a5c7c1bbb58e018556";
const HASH_INDEX_1: vector<u8> =
    x"f126b16c850fe027b9408972e31635d9db931b06dabce35cab4bfe025b9075a6";
const HASH_INDEX_2: vector<u8> =
    x"bbd40664e9775ca270c9608bd698bf4a821c682e6aa1666237213ba9dd3fe7ec";

// === take_from_tx_context ===

#[test]
fun take_from_tx_context_hashes_the_gas_coin_its_version_and_index() {
    let mut ctx = tx_context::dummy();

    let nonce = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);

    assert_eq(*nonce.hash_for_testing(), HASH_INDEX_0);
    destroy(nonce);
}

#[test]
fun take_from_tx_context_takes_the_next_index_on_each_call() {
    let mut ctx = tx_context::dummy();

    let first = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);
    let second = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);
    let third = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);

    assert_eq(*first.hash_for_testing(), HASH_INDEX_0);
    assert_eq(*second.hash_for_testing(), HASH_INDEX_1);
    assert_eq(*third.hash_for_testing(), HASH_INDEX_2);
    destroy(first);
    destroy(second);
    destroy(third);
}

#[test]
fun take_from_tx_context_shares_the_index_between_checks() {
    let mut ctx = tx_context::dummy();

    let first = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);
    let second = verified_nonce::take_from_tx_context<OtherCheck>(&mut ctx);

    assert_eq(*first.hash_for_testing(), HASH_INDEX_0);
    assert_eq(*second.hash_for_testing(), HASH_INDEX_1);
    destroy(first);
    destroy(second);
}

// === into_hash ===

#[test]
fun into_hash_returns_the_hash_of_the_nonce() {
    let mut ctx = tx_context::dummy();
    let nonce = verified_nonce::take_from_tx_context<TestCheck>(&mut ctx);

    let hash = nonce.into_hash();

    assert_eq(hash, HASH_INDEX_0);
}
