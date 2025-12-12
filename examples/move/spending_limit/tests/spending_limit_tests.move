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

    let mut id = scenario.new_object();

    spending_limit::authenticate_with_amount(&mut id, 100);

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
    spending_limit::authenticate_with_amount(&mut id, 1001);

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
    spending_limit::authenticate_with_amount(&mut id, 1000);

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
    spending_limit::authenticate_with_amount(&mut id, 500);

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
    spending_limit::authenticate_with_amount(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun multiple_authentications_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    spending_limit::authenticate_with_amount(&mut id, 500);
    spending_limit::authenticate_with_amount(&mut id, 200);
    spending_limit::authenticate_with_amount(&mut id, 100);
    spending_limit::authenticate_with_amount(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EOverspend)]
fun multiple_authentications_over_the_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit::attach(&mut id, 1000);

    spending_limit::authenticate_with_amount(&mut id, 500);
    spending_limit::authenticate_with_amount(&mut id, 200);
    spending_limit::authenticate_with_amount(&mut id, 100);

    // This one will exceed the limit (500 + 200 + 100 + 300 = 1100 > 1000)
    spending_limit::authenticate_with_amount(&mut id, 300);

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
    assert_eq!(spending_limit::has(&id), true);

    let detached_value = spending_limit::detach(&mut id);
    assert_eq!(detached_value, 1000);
    assert_eq!(spending_limit::has(&id), false);

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
