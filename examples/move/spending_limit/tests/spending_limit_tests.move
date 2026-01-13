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

    assert_eq!(spending_limit::exists(&id), false);
    spending_limit::attach(&mut id, 5000);
    assert_eq!(spending_limit::exists(&id), true);
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

// ---------------------- check_amount_against_spending_limit ------------------------

#[test]
#[expected_failure(abort_code = spending_limit::ESpendingLimitMissing)]
fun check_amount_against_spending_limit_requires_limit_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    spending_limit::check_amount_against_spending_limit( &id, 100);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EOverspend)]
fun check_amount_against_spending_limit_fails_if_exceeds_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Try to spend 1001
    spending_limit::check_amount_against_spending_limit( &id, 1001);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun check_amount_against_spending_limit_at_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Spend exactly at limit
    spending_limit::check_amount_against_spending_limit( &id, 1000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun check_amount_against_spending_limit_below_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    // Spend below limit
    spending_limit::check_amount_against_spending_limit( &id, 500);

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
    spending_limit::check_amount_against_spending_limit( &id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun multiple_authentications_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    spending_limit::check_amount_against_spending_limit( &id, 500);
    spending_limit::check_amount_against_spending_limit( &id, 200);
    spending_limit::check_amount_against_spending_limit( &id, 100);
    spending_limit::check_amount_against_spending_limit( &id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EOverspend)]
fun multiple_withdrawals_over_the_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    // Attach spending limit of 1000
    spending_limit::attach(&mut id, 1000);

    // Decrementing the spending limit to simulate withdrawals in 3 steps
    let spending_limit_ref = spending_limit::borrow_mut(&mut id);
    *spending_limit_ref = *spending_limit_ref - 500;

    let spending_limit_ref = spending_limit::borrow_mut(&mut id);
    *spending_limit_ref = *spending_limit_ref - 200;

    let spending_limit_ref = spending_limit::borrow_mut(&mut id);
    *spending_limit_ref = *spending_limit_ref - 100;

    // Now remaining is 200, authenticate with 300 should fail
    spending_limit::check_amount_against_spending_limit(&id, 300);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EInvalidLimit)]
fun attach_with_zero_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EInvalidLimit)]
fun rotate_to_zero_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    spending_limit::rotate(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun rotate_spending_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);
    assert_eq!(*spending_limit::borrow(&id), 1000);

    // Rotate to new limit
    let old_limit = spending_limit::rotate(&mut id, 2000);
    assert_eq!(old_limit, 1000);
    assert_eq!(*spending_limit::borrow(&id), 2000);

    // Rotate back
    let old_limit = spending_limit::rotate(&mut id, 500);
    assert_eq!(old_limit, 2000);
    assert_eq!(*spending_limit::borrow(&id), 500);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun detach_spending_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);
    assert_eq!(spending_limit::exists(&id), true);

    let detached_value = spending_limit::detach(&mut id);
    assert_eq!(detached_value, 1000);
    assert_eq!(spending_limit::exists(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::ESpendingLimitMissing)]
fun detach_nonexistent_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::detach(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::ESpendingLimitMissing)]
fun rotate_nonexistent_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::rotate(&mut id, 1000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
