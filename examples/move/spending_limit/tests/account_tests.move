// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::account_tests;

use generic_keyed_authentication::owner_public_key;
use iota::account::AuthenticatorInfoV1;
use iota::auth_context::{Self, AuthContext};
use iota::hex;
use iota::test_scenario::{Self, Scenario};
use iotaccount::iotaccount;
use spending_limit::account as spending_limit;
use spending_limit::spending_limit as limit;
use std::ascii;
use std::unit_test::assert_eq;

// --------------------------------------- Spending limit account ---------------------------------------

#[test]
fun account_creation() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_spending_limit_for_testing(scenario, 1000, b"42");

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let public_key = account.public_key();
        assert_eq!(*public_key, b"42");

        let spending_limit = account.spending_limit();
        assert_eq!(spending_limit, 1000);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = owner_public_key::EEd25519VerificationFailed)]
fun account_fails_verification() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 1000, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing();
        spending_limit::authenticate(
            &account,
            500,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );

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
    create_spending_limit_for_testing(scenario, 1000, public_key);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing();
        spending_limit::authenticate(
            &account,
            500,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = limit::EOverspend)]
fun account_spending_limit_exceeded() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 1000, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        // Try to spend 1001, which exceeds limit of 1000
        spending_limit::authenticate(
            &account,
            1001,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun account_within_spending_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
    let public_key = x"28851fafd2cbe27170bdae5a24029b2accfb1ede8b364811a808fe2275c82b59";

    let account_address = create_spending_limit_for_testing(scenario, 1000, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let test_ctx = tx_context::new(
            account_address,
            digest,
            0,
            0,
            0,
        );

        let signature =
            x"474686f447a998ccc6824bb05e69133de41b59999944e494a3ff5504abd9af86403aa7c240ac51d1d48e0b34a560ca7ee4542e25cfd7b090e4652dfb53941a04";
        let auth_context = create_auth_context_for_testing();

        spending_limit::authenticate(
            &account,
            1000,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun account_zero_spending() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 1000, public_key);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
        let auth_context = create_auth_context_for_testing();

        // Spend 0 (should always pass)
        spending_limit::authenticate(
            &account,
            0,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    iota::account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"spending_limit"),
        ascii::string(b"authenticate_spending_limit"),
    )
}

fun create_spending_limit_for_testing(
    scenario: &mut Scenario,
    limit: u64,
    public_key: vector<u8>,
): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();

    spending_limit::create(public_key, limit, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<spending_limit::SpendLimit>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}
