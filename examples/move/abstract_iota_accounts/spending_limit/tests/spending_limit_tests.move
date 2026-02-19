// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::spending_limit_tests;

use iota::test_scenario;
use iota::test_utils;
use spending_limit::spending_limit_authentication;
use std::unit_test::assert_eq;

#[test]
fun spending_limit_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(spending_limit_authentication::has_spending_limit(&id), false);
    spending_limit_authentication::attach_spending_limit(&mut id, 5000);
    assert_eq!(spending_limit_authentication::has_spending_limit(&id), true);
    assert_eq!(*spending_limit_authentication::borrow_spending_limit(&id), 5000);

    // Update the limit
    let limit_ref = spending_limit_authentication::borrow_mut_spending_limit(&mut id);
    *limit_ref = 3000;
    assert_eq!(*spending_limit_authentication::borrow_spending_limit(&id), 3000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::ESpendingLimitAlreadyAttached)]
fun duplicate_spending_limit_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 5000);
    spending_limit_authentication::attach_spending_limit(&mut id, 5000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ---------------------- authenticate_spending_limit ------------------------

#[test]
#[expected_failure(abort_code = spending_limit_authentication::ESpendingLimitMissing)]
fun authenticate_spending_limit_requires_limit_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    spending_limit_authentication::authenticate_spending_limit(&id, 100);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::EOverspend)]
fun authenticate_spending_limit_fails_if_exceeds_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    // Try to spend 1001
    spending_limit_authentication::authenticate_spending_limit(&id, 1001);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_spending_limit_at_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    // Spend exactly at limit
    spending_limit_authentication::authenticate_spending_limit(&id, 1000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_spending_limit_below_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    // Spend below limit
    spending_limit_authentication::authenticate_spending_limit(&id, 500);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_zero_amount() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    // Spend zero (should always pass)
    spending_limit_authentication::authenticate_spending_limit(&id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun multiple_authentications_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    spending_limit_authentication::authenticate_spending_limit(&id, 500);
    spending_limit_authentication::authenticate_spending_limit(&id, 200);
    spending_limit_authentication::authenticate_spending_limit(&id, 100);
    spending_limit_authentication::authenticate_spending_limit(&id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::EOverspend)]
fun multiple_withdrawals_over_the_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    // Attach spending limit of 1000
    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    // Decrementing the spending limit to simulate withdrawals in 3 steps
    let spending_limit_ref = spending_limit_authentication::borrow_mut_spending_limit(&mut id);
    *spending_limit_ref = *spending_limit_ref - 500;

    let spending_limit_ref = spending_limit_authentication::borrow_mut_spending_limit(&mut id);
    *spending_limit_ref = *spending_limit_ref - 200;

    let spending_limit_ref = spending_limit_authentication::borrow_mut_spending_limit(&mut id);
    *spending_limit_ref = *spending_limit_ref - 100;

    // Now remaining is 200, authenticate with 300 should fail
    spending_limit_authentication::authenticate_spending_limit(&id, 300);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::EInvalidLimit)]
fun attach_with_zero_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::EInvalidLimit)]
fun rotate_to_zero_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);

    spending_limit_authentication::rotate_spending_limit(&mut id, 0);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun rotate_spending_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);
    assert_eq!(*spending_limit_authentication::borrow_spending_limit(&id), 1000);

    // Rotate to new limit
    let old_limit = spending_limit_authentication::rotate_spending_limit(&mut id, 2000);
    assert_eq!(old_limit, 1000);
    assert_eq!(*spending_limit_authentication::borrow_spending_limit(&id), 2000);

    // Rotate back
    let old_limit = spending_limit_authentication::rotate_spending_limit(&mut id, 500);
    assert_eq!(old_limit, 2000);
    assert_eq!(*spending_limit_authentication::borrow_spending_limit(&id), 500);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun detach_spending_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::attach_spending_limit(&mut id, 1000);
    assert_eq!(spending_limit_authentication::has_spending_limit(&id), true);

    let detached_value = spending_limit_authentication::detach_spending_limit(&mut id);
    assert_eq!(detached_value, 1000);
    assert_eq!(spending_limit_authentication::has_spending_limit(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::ESpendingLimitMissing)]
fun detach_nonexistent_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::detach_spending_limit(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit_authentication::ESpendingLimitMissing)]
fun rotate_nonexistent_limit_fails() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    spending_limit_authentication::rotate_spending_limit(&mut id, 1000);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
