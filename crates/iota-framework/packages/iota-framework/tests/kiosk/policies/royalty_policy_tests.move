// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::royalty_policy_tests;

use iota::coin;
use iota::iota::IOTA;
use iota::royalty_policy;
use iota::transfer_policy as policy;
use iota::transfer_policy_tests as test;

#[test]
fun test_default_flow() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty
    royalty_policy::set(&mut policy, &cap, 100);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let mut payment = coin::mint_for_testing<IOTA>(2000, ctx);

    royalty_policy::pay(&mut policy, &mut request, &mut payment, ctx);
    policy::confirm_request(&policy, request);

    let remainder = coin::burn_for_testing(payment);
    let profits = test::wrapup(policy, cap, ctx);

    assert!(remainder == 1000);
    assert!(profits == 1000);
}

#[test]
#[expected_failure(abort_code = iota::royalty_policy::EIncorrectArgument)]
fun test_incorrect_config() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    royalty_policy::set(&mut policy, &cap, 11_000);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = iota::royalty_policy::EInsufficientAmount)]
fun test_insufficient_amount() {
    let ctx = &mut tx_context::dummy();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty
    royalty_policy::set(&mut policy, &cap, 100);

    // Requires 1_000 NANOS, coin has only 999
    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let mut payment = coin::mint_for_testing<IOTA>(999, ctx);

    royalty_policy::pay(&mut policy, &mut request, &mut payment, ctx);
    policy::confirm_request(&policy, request);

    coin::burn_for_testing(payment);
    test::wrapup(policy, cap, ctx);
}
