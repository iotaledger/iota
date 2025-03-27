// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
/// A `TransferPolicy` Rule which implements percentage-based royalty fee.
module iota::royalty_policy;

use iota::coin::{Self, Coin};
use iota::iota::IOTA;
use iota::transfer_policy::{Self as policy, TransferPolicy, TransferPolicyCap, TransferRequest};
use iota::transfer_policy_tests as test;

/// The `amount_bp` passed is more than 100%.
const EIncorrectArgument: u64 = 0;
/// The `Coin` used for payment is not enough to cover the fee.
const EInsufficientAmount: u64 = 1;

/// Max value for the `amount_bp`.
const MAX_BPS: u16 = 10_000;

/// The "Rule" witness to authorize the policy.
public struct Rule has drop {}

/// Configuration for the Rule.
public struct Config has drop, store {
    amount_bp: u16,
}

/// Creator action: Set the Royalty policy for the `T`.
public fun set<T: key + store>(
    policy: &mut TransferPolicy<T>,
    cap: &TransferPolicyCap<T>,
    amount_bp: u16,
) {
    assert!(amount_bp < MAX_BPS, EIncorrectArgument);
    policy::add_rule(Rule {}, policy, cap, Config { amount_bp })
}

/// Buyer action: Pay the royalty fee for the transfer.
public fun pay<T: key + store>(
    policy: &mut TransferPolicy<T>,
    request: &mut TransferRequest<T>,
    payment: &mut Coin<IOTA>,
    ctx: &mut TxContext,
) {
    let config: &Config = policy::get_rule(Rule {}, policy);
    let paid = policy::paid(request);
    let amount = (paid as u128 * (config.amount_bp as u128) / 10_000) as u64;

    assert!(coin::value(payment) >= amount, EInsufficientAmount);

    let fee = coin::split(payment, amount, ctx);
    policy::add_to_balance(Rule {}, policy, fee);
    policy::add_receipt(Rule {}, request)
}

#[test]
fun test_default_flow() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty
    set(&mut policy, &cap, 100);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let mut payment = coin::mint_for_testing<IOTA>(2000, ctx);

    pay(&mut policy, &mut request, &mut payment, ctx);
    policy::confirm_request(&policy, request);

    let remainder = coin::burn_for_testing(payment);
    let profits = test::wrapup(policy, cap, ctx);

    assert!(remainder == 1000);
    assert!(profits == 1000);
}

#[test]
#[expected_failure(abort_code = EIncorrectArgument)]
fun test_incorrect_config() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    set(&mut policy, &cap, 11_000);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = EInsufficientAmount)]
fun test_insufficient_amount() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty
    set(&mut policy, &cap, 100);

    // Requires 1_000 NANOS, coin has only 999
    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let mut payment = coin::mint_for_testing<IOTA>(999, ctx);

    pay(&mut policy, &mut request, &mut payment, ctx);
    policy::confirm_request(&policy, request);

    coin::burn_for_testing(payment);
    test::wrapup(policy, cap, ctx);
}
