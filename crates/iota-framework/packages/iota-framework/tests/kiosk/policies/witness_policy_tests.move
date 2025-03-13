// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::witness_policy_tests;

use iota::transfer_policy as policy;
use iota::transfer_policy_tests::{Self as test, Asset};
use iota::witness_policy;

/// Confirmation of an action to use in Policy.
public struct Proof has drop {}

/// Malicious attempt to use a different proof.
public struct Cheat has drop {}

#[test]
fun test_default_flow() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // set the lock policy and require `Proof` on every transfer.
    witness_policy::set<Asset, Proof>(&mut policy, &cap);

    let mut request = policy::new_request(test::fresh_id(ctx), 0, test::fresh_id(ctx));

    witness_policy::prove(Proof {}, &policy, &mut request);
    policy.confirm_request(request);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = iota::transfer_policy::EPolicyNotSatisfied)]
fun test_no_proof() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // set the lock policy and require `Proof` on every transfer.
    witness_policy::set<Asset, Proof>(&mut policy, &cap);
    let request = policy::new_request(test::fresh_id(ctx), 0, test::fresh_id(ctx));

    policy.confirm_request(request);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = iota::witness_policy::ERuleNotFound)]
fun test_wrong_proof() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // set the lock policy and require `Proof` on every transfer.
    witness_policy::set<Asset, Proof>(&mut policy, &cap);

    let mut request = policy::new_request(test::fresh_id(ctx), 0, test::fresh_id(ctx));

    witness_policy::prove(Cheat {}, &policy, &mut request);
    policy.confirm_request(request);
    test::wrapup(policy, cap, ctx);
}
