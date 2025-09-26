// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module kiosk::royalty_rule_tests;

use iota::coin;
use iota::iota::IOTA;
use iota::transfer_policy as policy;
use iota::transfer_policy_tests as test;
use iota::tx_context::dummy as ctx;
use kiosk::royalty_rule;

#[test]
fun test_default_flow_0() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 0% royalty; min 0 nano
    royalty_rule::add(&mut policy, &cap, 0, 0);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(0, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);

    let profits = test::wrapup(policy, cap, ctx);
    assert!(profits == 0, 1);
}

#[test]
#[expected_failure(abort_code = kiosk::royalty_rule::EInsufficientAmount)]
fun test_default_flow_0_invalid_amount_fail() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 0% royalty; min 0 nano
    royalty_rule::add(&mut policy, &cap, 0, 0);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(10, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);

    let profits = test::wrapup(policy, cap, ctx);
    assert!(profits == 0, 1);
}

#[test]
fun test_default_flow_1() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty; min 0 nano
    royalty_rule::add(&mut policy, &cap, 100, 0);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(1000, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);

    let profits = test::wrapup(policy, cap, ctx);
    assert!(profits == 1000, 1);
}

#[test]
fun test_default_flow_100() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 100% royalty; min 0 nano
    royalty_rule::add(&mut policy, &cap, 10_000, 0);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(100_000, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);

    let profits = test::wrapup(policy, cap, ctx);
    assert!(profits == 100_000, 1);
}

#[test]
fun test_default_flow_1_min_10_000() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty; min 10_000 nano
    royalty_rule::add(&mut policy, &cap, 100, 10_000);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(10_000, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);
    assert!(test::wrapup(policy, cap, ctx) == 10_000, 1);
}

#[test]
fun test_default_flow_10_min_10_000() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 10% royalty; min 10_000 nano
    royalty_rule::add(&mut policy, &cap, 1000, 10_000);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(10_000, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);
    assert!(test::wrapup(policy, cap, ctx) == 10_000, 1);
}

#[test]
fun test_default_flow_20_min_10_000() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 20% royalty; min 10_000 nano
    royalty_rule::add(&mut policy, &cap, 20_00, 10_000);

    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(20_000, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);
    assert!(test::wrapup(policy, cap, ctx) == 20_000, 1);
}

#[test]
#[expected_failure(abort_code = kiosk::royalty_rule::EIncorrectArgument)]
fun test_incorrect_config() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    royalty_rule::add(&mut policy, &cap, 11_000, 0);
    test::wrapup(policy, cap, ctx);
}

#[test]
#[expected_failure(abort_code = kiosk::royalty_rule::EInsufficientAmount)]
fun test_insufficient_amount() {
    let ctx = &mut ctx();
    let (mut policy, cap) = test::prepare(ctx);

    // 1% royalty
    royalty_rule::add(&mut policy, &cap, 100, 0);

    // Requires 1_000 nano, coin has only 999
    let mut request = policy::new_request(test::fresh_id(ctx), 100_000, test::fresh_id(ctx));
    let payment = coin::mint_for_testing<IOTA>(999, ctx);

    royalty_rule::pay(&mut policy, &mut request, payment);
    policy::confirm_request(&policy, request);
    test::wrapup(policy, cap, ctx);
}
