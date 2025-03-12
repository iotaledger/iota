// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::witness_policy;

use iota::transfer_policy::{Self as policy, TransferPolicy, TransferPolicyCap, TransferRequest};

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
