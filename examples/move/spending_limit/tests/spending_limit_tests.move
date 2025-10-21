// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::spending_limit_tests;

use iota::test_scenario;
use iota::test_utils;
use spending_limit::spending_limit;
use std::unit_test::assert_eq;

#[test]
fun spending_limit_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(spending_limit::has(&id), false);
    spending_limit::attach(&mut id, 5000);
    assert_eq!(spending_limit::has(&id), true);
    assert_eq!(*spending_limit::borrow(&id), 5000);

    // Update the limit
    let limit_ref = spending_limit::borrow_mut(&mut id);
    *limit_ref = 3000;
    assert_eq!(*spending_limit::borrow(&id), 3000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::ESpendingLimitAlreadyAttached)]
fun duplicate_spending_limit_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 5000);
    spending_limit::attach(&mut id, 5000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ---------------------- authenticate_with_amount ------------------------

#[test]
#[expected_failure(abort_code = spending_limit::ESpendingLimitMissing)]
fun authenticate_with_amount_requires_limit_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    spending_limit::authenticate_with_amount(&id, 100);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EOverspend)]
fun authenticate_with_amount_fails_if_exceeds_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Try to spend 1001
    spending_limit::authenticate_with_amount(&id, 1001);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_amount_at_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Spend exactly at limit
    spending_limit::authenticate_with_amount(&id, 1000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_amount_below_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Spend below limit
    spending_limit::authenticate_with_amount(&id, 500);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_zero_amount() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Spend zero (should always pass)
    spending_limit::authenticate_with_amount(&id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun multiple_authentications_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    spending_limit::authenticate_with_amount(&id, 500);
    spending_limit::authenticate_with_amount(&id, 300);
    spending_limit::authenticate_with_amount(&id, 1000);
    spending_limit::authenticate_with_amount(&id, 100);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
