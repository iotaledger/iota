// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module time_locked::time_locked_iotaccount_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::clock;
use iota::dynamic_field as df;
use iota::test_scenario::{Self, Scenario};
use iotaccount::iotaccount::IOTAccount;
use public_key_authentication::public_key_authentication;
use public_key_authentication::public_key_iotaccount;
use std::ascii;
use std::unit_test::assert_eq;
use time_locked::time_locked_iotaccount;
use time_locked::unlock_time_authentication;

use fun public_key_iotaccount::borrow_public_key as IOTAccount.borrow_public_key;
use fun time_locked_iotaccount::add_unlock_time as IOTAccount.add_unlock_time;
use fun time_locked_iotaccount::remove_unlock_time as IOTAccount.remove_unlock_time;
use fun time_locked_iotaccount::rotate_unlock_time as IOTAccount.rotate_unlock_time;
use fun time_locked_iotaccount::has_unlock_time as IOTAccount.has_unlock_time;
use fun time_locked_iotaccount::borrow_unlock_time as IOTAccount.borrow_unlock_time;

// --------------------------------------- Time locked account ---------------------------------------

#[test]
fun account_creation() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_time_locked_account_for_testing(scenario, 3, b"42");

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        let public_key = account.borrow_public_key();
        assert_eq!(*public_key, b"42");

        let unlock_time = account.borrow_unlock_time();
        assert_eq!(*unlock_time, 3);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun unlock_time_clock_ed25519_authenticator_success() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_account_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let mut test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let mut clock = clock::create_for_testing(&mut test_ctx);
        clock.increment_for_testing(3);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked_iotaccount::unlock_time_clock_ed25519_authenticator(
            &account,
            &clock,
            signature,
            &auth_context,
            &test_ctx,
        );

        clock::destroy_for_testing(clock);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun unlock_time_epoch_ed25519_authenticator_success() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_account_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let epoch_timestamp = 4;
        let test_ctx = tx_context::new(account_address, digest, 0, epoch_timestamp, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked_iotaccount::unlock_time_epoch_ed25519_authenticator(
            &account,
            signature,
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = public_key_authentication::EEd25519VerificationFailed)]
fun account_fails_verification() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_account_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let clock = clock::create_for_testing(scenario.ctx());

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing();
        time_locked_iotaccount::unlock_time_clock_ed25519_authenticator(
            &account,
            &clock,
            signature,
            &auth_context,
            scenario.ctx(),
        );

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time_authentication::EAccountStillLocked)]
fun account_time_locked_clock() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_account_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let mut test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let clock = clock::create_for_testing(&mut test_ctx);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked_iotaccount::unlock_time_clock_ed25519_authenticator(
            &account,
            &clock,
            signature,
            &auth_context,
            &test_ctx,
        );

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = unlock_time_authentication::EAccountStillLocked)]
fun account_time_locked_epoch() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_account_for_testing(scenario, 3, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let epoch_timestamp = 2;
        let test_ctx = tx_context::new(account_address, digest, 0, epoch_timestamp, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        time_locked_iotaccount::unlock_time_epoch_ed25519_authenticator(
            &account,
            signature,
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun unlock_time_handling() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let account_address = create_public_key_account_for_testing(scenario, x"42");

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        account.add_unlock_time(5, scenario.ctx());
        assert_eq!(account.has_unlock_time(), true);
        assert_eq!(*account.borrow_unlock_time(), 5);

        account.rotate_unlock_time(
            option::none(),
            3,
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );
        assert_eq!(*account.borrow_unlock_time(), 3);

        account.remove_unlock_time(scenario.ctx());
        assert_eq!(account.has_unlock_time(), false);

        test_scenario::return_shared(account);
        test_scenario::end(scenario_val);
    }
}

#[test]
#[expected_failure(abort_code = df::EFieldAlreadyExists)]
fun duplicate_unlock_time_reported() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let account_address = create_public_key_account_for_testing(scenario, x"42");

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        account.add_unlock_time(5, scenario.ctx());
        account.add_unlock_time(5, scenario.ctx());

        test_scenario::return_shared(account);
        test_scenario::end(scenario_val);
    };
}

#[test]
#[expected_failure(abort_code = df::EFieldDoesNotExist)]
fun detach_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let account_address = create_public_key_account_for_testing(scenario, x"42");

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        account.remove_unlock_time(scenario.ctx());

        test_scenario::return_shared(account);
        test_scenario::end(scenario_val);
    };
}

#[test]
#[expected_failure(abort_code = df::EFieldDoesNotExist)]
fun rotate_unlock_time_fails_if_missing() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let account_address = create_public_key_account_for_testing(scenario, x"42");

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        account.rotate_unlock_time(
            option::none(),
            3,
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
        test_scenario::end(scenario_val);
    };
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<IOTAccount> {
    // The exact values doesn't matter in these tests.
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(b"time_locked"),
        ascii::string(b"authenticate"),
    )
}

fun create_time_locked_account_for_testing(
    scenario: &mut Scenario,
    unlock_time: u64,
    public_key: vector<u8>,
): address {
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    time_locked_iotaccount::create(
        public_key,
        option::none(),
        unlock_time,
        authenticator,
        scenario.ctx(),
    );

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_public_key_account_for_testing(
    scenario: &mut Scenario,
    public_key: vector<u8>,
): address {
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    public_key_iotaccount::create(public_key, authenticator, scenario.ctx());

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(
        b"00000000000000000000000000000000",
        vector::empty(),
        vector::empty(),
    )
}
