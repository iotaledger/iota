// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module time_locked::unlock_time_tests;

use iota::clock;
use iota::test_scenario;
use iota::test_utils;
use std::unit_test::assert_eq;
use time_locked::unlock_time;

#[test]
fun unlock_time_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(unlock_time::has(&id), false);
    unlock_time::attach(&mut id, 5);
    assert_eq!(unlock_time::has(&id), true);
    assert_eq!(*unlock_time::borrow(&id), 5);

    unlock_time::rotate(&mut id, 3);
    assert_eq!(*unlock_time::borrow(&id), 3);

    unlock_time::detach(&mut id);
    assert_eq!(unlock_time::has(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeAttached)]
fun duplicate_unlock_time_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 5);
    unlock_time::attach(&mut id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeMissing)]
fun detach_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::detach(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeMissing)]
fun rotate_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::rotate(&mut id, 3);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ---------------------- authenticate_with_epoch_timestamp ------------------------

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeMissing)]
fun authenticate_with_epoch_timestamp_requires_unlock_time_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    unlock_time::authenticate_with_epoch_timestamp(&id, scenario.ctx());

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EAccountStillLocked)]
fun authenticate_with_epoch_timestamp_fails_if_time_not_passed() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);

    unlock_time::authenticate_with_epoch_timestamp(&id, scenario.ctx());

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_epoch_timestamp() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);

    let ctx = scenario.ctx();
    ctx.increment_epoch_timestamp(4);
    unlock_time::authenticate_with_epoch_timestamp(&id, ctx);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ---------------------- authenticate_with_clock ------------------------

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeMissing)]
fun authenticate_with_clock_requires_unlock_time_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    // default clock is at zero
    let clock = clock::create_for_testing(scenario.ctx());
    unlock_time::authenticate_with_clock(&id, &clock);

    clock::destroy_for_testing(clock);
    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EAccountStillLocked)]
fun authenticate_with_clock_fails_if_time_not_passed() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);

    // default clock is at zero
    let clock = clock::create_for_testing(scenario.ctx());
    unlock_time::authenticate_with_clock(&id, &clock);

    clock::destroy_for_testing(clock);
    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_with_clock() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);

    let mut clock = clock::create_for_testing(scenario.ctx());
    clock.set_for_testing(3);
    unlock_time::authenticate_with_clock(&id, &clock);

    clock::destroy_for_testing(clock);
    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// ---------------------- authenticate_unlock_time ------------------------

#[test]
#[expected_failure(abort_code = unlock_time::EUnlockTimeMissing)]
fun authenticate_unlock_time_requires_it_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    unlock_time::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time::EAccountStillLocked)]
fun authenticate_unlock_time_fails_if_time_not_passed() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);
    unlock_time::authenticate_unlock_time(&id, 2);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_unlock_time() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    unlock_time::attach(&mut id, 3);
    unlock_time::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
