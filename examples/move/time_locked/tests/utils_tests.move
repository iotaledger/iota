// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module time_locked::utils_tests;

use iota::test_scenario;
use iota::test_utils;
use std::unit_test::assert_eq;
use time_locked::utils;

// --------------------------------------- Time locked tools -----------------------------------------

#[test]
fun unlock_time_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(utils::has_unlock_time(&id), false);
    utils::attach_unlock_time(&mut id, 5);
    assert_eq!(utils::has_unlock_time(&id), true);
    assert_eq!(*utils::borrow_unlock_time(&id), 5);

    utils::rotate_unlock_time(&mut id, 3);
    assert_eq!(*utils::borrow_unlock_time(&id), 3);

    utils::detach_unlock_time(&mut id);
    assert_eq!(utils::has_unlock_time(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = utils::EUnlockTimeAttached)]
fun duplicate_unlock_time_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    utils::attach_unlock_time(&mut id, 5);
    utils::attach_unlock_time(&mut id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = utils::EUnlockTimeMissing)]
fun detach_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    utils::detach_unlock_time(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = utils::EUnlockTimeMissing)]
fun rotate_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    utils::rotate_unlock_time(&mut id, 3);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = utils::EUnlockTimeMissing)]
fun authenticate_unlock_time_requires_it_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    utils::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_unlock_time() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    utils::attach_unlock_time(&mut id, 3);
    utils::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = utils::EAccountStillLocked)]
fun authenticate_unlock_time_fails_if_time_not_passed() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    utils::attach_unlock_time(&mut id, 3);
    utils::authenticate_unlock_time(&id, 2);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
