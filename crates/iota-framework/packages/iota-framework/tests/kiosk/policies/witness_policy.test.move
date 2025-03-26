// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::witness_policy;

use iota::transfer_policy::{Self as policy, TransferPolicy, TransferPolicyCap, TransferRequest};
use iota::transfer_policy_tests::{Self as test, Asset};

/// When a Proof does not find its Rule<Proof>.
const ERuleNotFound: u64 = 0;

/// Custom witness-key for the "proof policy".
public struct Rule<phantom Proof: drop> has drop {}

/// Creator action: adds the Rule.
/// Requires a "Proof" witness confirmation on every transfer.
public fun set<T: key + store, Proof: drop>(
    policy: &mut TransferPolicy<T>,
    cap: &TransferPolicyCap<T>,
) {
    policy::add_rule(Rule<Proof> {}, policy, cap, true);
}

/// Buyer action: follow the policy.
/// Present the required "Proof" instance to get a receipt.
public fun prove<T: key + store, Proof: drop>(
    _proof: Proof,
    policy: &TransferPolicy<T>,
    request: &mut TransferRequest<T>,
) {
    assert!(policy::has_rule<T, Rule<Proof>>(policy), ERuleNotFound);
    policy::add_receipt(Rule<Proof> {}, request)
}

/// Confirmation of an action to use in Policy.
public struct Proof has drop {}

/// Malicious attempt to use a different proof.
public struct Cheat has drop {}

#[test]
fun test_default_flow() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // set the lock policy and require `Proof` on every transfer.
    set<Asset, Proof>(&mut policy, &cap);

    let mut request = policy::new_request(test::fresh_id(ctx), 0, test::fresh_id(ctx));

    prove(Proof {}, &policy, &mut request);
    policy.confirm_request(request);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = iota::transfer_policy::EPolicyNotSatisfied)]
fun test_no_proof() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // set the lock policy and require `Proof` on every transfer.
    set<Asset, Proof>(&mut policy, &cap);
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
    set<Asset, Proof>(&mut policy, &cap);

    let mut request = policy::new_request(test::fresh_id(ctx), 0, test::fresh_id(ctx));

    prove(Cheat {}, &policy, &mut request);
    policy.confirm_request(request);
    test::wrapup(policy, cap, ctx);
}
