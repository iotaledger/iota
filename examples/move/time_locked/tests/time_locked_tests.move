// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module time_locked::time_locked_tests;

use account_template::account_template::IOTAccount;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::{Self, AuthContext};
use iota::clock;
use iota::test_scenario::{Self, Scenario};
use iota::tx_context;
use std::ascii;
use std::unit_test::assert_eq;
use time_locked::time_locked;

// --------------------------------------- Basic Scenario ---------------------------------------

#[test]
fun test_account_creation() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_time_locked_for_testing(scenario, 3, option::none());

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        let public_key_df_name = time_locked::create_owner_public_key_for_testing();
        assert!(account.has_field(public_key_df_name));
        assert_eq!(*account.borrow_field(public_key_df_name), public_key_for_testing());

        let authenticator_df_name = account::authenticator_df_name();
        assert!(account.has_field(authenticator_df_name));
        assert_eq!(
            *account.borrow_field(authenticator_df_name),
            create_authenticator_info_v1_for_testing(),
        );

        let unlock_time_df_name = time_locked::create_unlock_time_key_for_testing();
        assert!(account.has_field(unlock_time_df_name));
        assert_eq!(*account.borrow_field(unlock_time_df_name), 3);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EAccountStillLocked)]
fun test_account_time_locked() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_time_locked_for_testing(scenario, 3, option::none());

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        let clock = clock::create_for_testing(scenario.ctx());
        let signature: vector<u8> = vector[];
        let auth_context = create_auth_context_for_testing();
        time_locked::authenticate(&account, &clock, signature, &auth_context, scenario.ctx());

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = time_locked::EEd25519VerificationFailed)]
fun test_account_time_unlocked_but_fails_verification() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_time_locked_for_testing(scenario, 3, option::none());

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        let mut clock = clock::create_for_testing(scenario.ctx());
        clock.increment_for_testing(3);

        let signature: vector<u8> = vector[];
        let auth_context = create_auth_context_for_testing();
        time_locked::authenticate(&account, &clock, signature, &auth_context, scenario.ctx());

        clock::destroy_for_testing(clock);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_account_unlocked() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_time_locked_for_testing(scenario, 3, option::some(public_key));

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

        time_locked::authenticate(&account, &clock, signature, &auth_context, &test_ctx);

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
    public_key: Option<vector<u8>>,
): address {
    let ctx = test_scenario::ctx(scenario);

    let public_key = public_key.destroy_or!(public_key_for_testing());
    let authenticator = create_authenticator_info_v1_for_testing();

    time_locked::create(unlock_time, public_key, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.get_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}

fun public_key_for_testing(): vector<u8> {
    b"42"
}
