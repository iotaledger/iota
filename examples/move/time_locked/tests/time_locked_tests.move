// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module time_locked::time_locked_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::{Self, AuthContext};
use iota::clock;
use iota::hex;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils;
use iotaccount::basic_keyed_account;
use iotaccount::iotaccount;
use std::ascii;
use std::unit_test::assert_eq;
use time_locked::time_locked::{Self, TimeLocked, borrow_unlock_time};

// --------------------------------------- Time locked tools -----------------------------------------

#[test]
fun unlock_time_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    assert_eq!(time_locked::has_unlock_time(&id), false);
    time_locked::attach_unlock_time(&mut id, 5);
    assert_eq!(time_locked::has_unlock_time(&id), true);
    assert_eq!(*time_locked::borrow_unlock_time(&id), 5);

    time_locked::rotate_unlock_time(&mut id, 3);
    assert_eq!(*time_locked::borrow_unlock_time(&id), 3);

    time_locked::detach_unlock_time(&mut id);
    assert_eq!(time_locked::has_unlock_time(&id), false);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EUnlockTimeAttached)]
fun duplicate_unlock_time_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    time_locked::attach_unlock_time(&mut id, 5);
    time_locked::attach_unlock_time(&mut id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EUnlockTimeMissing)]
fun detach_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    time_locked::detach_unlock_time(&mut id);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EUnlockTimeMissing)]
fun rotate_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    time_locked::rotate_unlock_time(&mut id, 3);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EUnlockTimeMissing)]
fun authenticate_unlock_time_requires_it_to_be_set() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let id = scenario.new_object();

    time_locked::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
fun authenticate_unlock_time() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    time_locked::attach_unlock_time(&mut id, 3);
    time_locked::authenticate_unlock_time(&id, 5);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EAccountStillLocked)]
fun authenticate_unlock_time_fails_if_time_not_passed() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let mut id = scenario.new_object();

    time_locked::attach_unlock_time(&mut id, 3);
    time_locked::authenticate_unlock_time(&id, 2);

    test_utils::destroy(id);
    test_scenario::end(scenario_val);
}

// --------------------------------------- Time locked account ---------------------------------------

#[test]
fun account_creation() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_time_locked_for_testing(scenario, 3, b"42");

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<TimeLocked>();
        let account_id = account.borrow_uid();

        let public_key = basic_keyed_account::borrow_public_key_id(account_id);
        assert_eq!(*public_key, b"42");

        let unlock_time = borrow_unlock_time(account_id);
        assert_eq!(*unlock_time, 3);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = basic_keyed_account::EEd25519VerificationFailed)]
fun account_fails_verification() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<TimeLocked>();
        let clock = clock::create_for_testing(scenario.ctx());

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing();
        time_locked::authenticate(
            &account,
            &clock,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun only_account_can_authenticate() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    create_time_locked_for_testing(scenario, 3, public_key);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<TimeLocked>();
        let clock = clock::create_for_testing(scenario.ctx());

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing();
        time_locked::authenticate(
            &account,
            &clock,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EAccountStillLocked)]
fun account_time_locked() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<TimeLocked>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let mut test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let clock = clock::create_for_testing(&mut test_ctx);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked::authenticate(
            &account,
            &clock,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun account_unlocked() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<TimeLocked>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let mut test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let mut clock = clock::create_for_testing(&mut test_ctx);
        clock.increment_for_testing(3);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked::authenticate(
            &account,
            &clock,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        clock::destroy_for_testing(clock);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"time_locked"),
        ascii::string(b"authenticate_time"),
    )
}

fun create_time_locked_for_testing(
    scenario: &mut Scenario,
    unlock_time: u64,
    public_key: vector<u8>,
): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();

    time_locked::create(public_key, unlock_time, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<time_locked::TimeLocked>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}
