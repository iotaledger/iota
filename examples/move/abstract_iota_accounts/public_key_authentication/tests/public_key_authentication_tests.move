// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module public_key_authentication::public_key_authentication_tests;

use iota::test_scenario;
use iota::test_utils;
use public_key_authentication::public_key_authentication;
use std::unit_test::assert_eq;

#[test]
fun unlock_time_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(public_key_authentication::has_public_key(&id), false);
    public_key_authentication::attach_public_key(&mut id, x"52");
    assert_eq!(public_key_authentication::has_public_key(&id), true);
    assert_eq!(*public_key_authentication::borrow_public_key(&id), x"52");

    public_key_authentication::rotate_public_key(&mut id, x"32");
    assert_eq!(*public_key_authentication::borrow_public_key(&id), x"32");

    public_key_authentication::detach_public_key(&mut id);
    assert_eq!(public_key_authentication::has_public_key(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = public_key_authentication::EPublicKeyAlreadyAttached)]
fun duplicate_public_key_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    public_key_authentication::attach_public_key(&mut id, x"52");
    public_key_authentication::attach_public_key(&mut id, x"52");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = public_key_authentication::EPublicKeyMissing)]
fun detach_public_key_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    public_key_authentication::detach_public_key(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = public_key_authentication::EPublicKeyMissing)]
fun rotate_public_key_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    public_key_authentication::rotate_public_key(&mut id, x"32");

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = public_key_authentication::EPublicKeyMissing)]
fun authenticate_with_epoch_timestamp_requires_public_key_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    let signature = x"00";
    public_key_authentication::authenticate_ed25519(&id, signature, scenario.ctx());

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}
