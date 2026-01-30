// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::account_tests;

use generic_keyed_authentication::owner_public_key;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::coin;
use iota::hex;
use iota::iota::IOTA;
use iota::ptb_call_arg;
use iota::ptb_command;
use iota::test_scenario::{Self, Scenario};
use spending_limit::account::{Self as spending_limit, SpendLimit};
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
        let auth_context = create_auth_context_for_testing(account_address, 500, scenario.ctx());
        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[
    expected_failure(
        abort_code = generic_keyed_authentication::owner_public_key::EEd25519VerificationFailed,
    ),
]
fun only_account_can_authenticate() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 1000, public_key);
    let test_ctx = tx_context::new(
        @0x9999,
        x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3",
        0,
        0,
        0,
    );
    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let signature: vector<u8> = b"32";
        let auth_context = create_auth_context_for_testing(account_address, 1001, &test_ctx);
        spending_limit::authenticate(
            &account,
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
        let auth_context = create_auth_context_for_testing(account_address, 1001, &test_ctx);

        // Try to spend 1001, which exceeds limit of 1000
        spending_limit::authenticate(
            &account,
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
    let initial_limit = 1000;
    let withdraw_amount = 500;

    // Create account with 1000 limit
    let account_address = create_spending_limit_for_testing(scenario, initial_limit, public_key);

    // Add balance to the reserve
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<spending_limit::SpendLimit>();

        // Mint coin and deposit to reserve
        let coin = coin::mint_for_testing<IOTA>(10000, scenario.ctx());
        spending_limit::deposit_to_reserve(&mut account, coin);

        test_scenario::return_shared(account);
    };

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<spending_limit::SpendLimit>();

        let test_ctx = tx_context::new(
            account_address,
            digest,
            0,
            0,
            0,
        );

        let signature =
            x"474686f447a998ccc6824bb05e69133de41b59999944e494a3ff5504abd9af86403aa7c240ac51d1d48e0b34a560ca7ee4542e25cfd7b090e4652dfb53941a04";

        let auth_context = create_auth_context_for_testing(
            account_address,
            withdraw_amount,
            &test_ctx,
        );

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        // Withdraw from balance reserve
        let coin = spending_limit::withdraw_from_balance_reserve(
            &mut account,
            withdraw_amount,
            scenario.ctx(),
        );

        iota::test_utils::destroy(coin);

        test_scenario::return_shared(account);
    };

    // Verify State Change
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let current_limit = account.spending_limit();

        // Expected: 1000 (initial) - 500 (withdrawn) = 500
        assert_eq!(current_limit, initial_limit - withdraw_amount);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EInvalidAmount)]
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
        let auth_context = create_auth_context_for_testing(account_address, 0, &test_ctx);
        // Spend 0 (should always pass)
        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_authenticator_function_ref_integrity() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_spending_limit_for_testing(scenario, 1000, b"42");

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();

        let fn_ref = account.authenticator_function_ref();

        let expected_fn_ref = create_authenticator_function_ref_v1_for_testing();

        assert!(fn_ref == &expected_fn_ref, 0);

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_missing_withdraw_call() {
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

        // AuthContext without withdraw_call
        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), vector[], vector[]);

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_multiple_withdraw_calls_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 3000, public_key);
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        // Create auth_context with 5 withdraw calls of 500 each (total 2500, within limit of 3000)
        let auth_context = create_auth_context_for_testing_multiple_withdraw_calls(
            account_address,
            500,
            5,
            &test_ctx,
        );

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_multiple_withdraw_calls_at_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 3000, public_key);
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        // Create auth_context with 6 withdraw calls of 500 each (total 3000, at limit of 3000)
        let auth_context = create_auth_context_for_testing_multiple_withdraw_calls(
            account_address,
            500,
            6,
            &test_ctx,
        );

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = limit::EOverspend)]
fun test_multiple_withdraw_calls_over_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_spending_limit_for_testing(scenario, 3000, public_key);
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<spending_limit::SpendLimit>();
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let test_ctx = tx_context::new(account_address, digest, 0, 0, 0);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        // Create auth_context with 3 withdraw calls of 1500 each (total 4500, over limit of 3000)
        let auth_context = create_auth_context_for_testing_multiple_withdraw_calls(
            account_address,
            1500,
            3,
            &test_ctx,
        );

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_withdraw_call_wrong_account() {
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

        // Create auth_context with a wrong account as the first argument
        let wrong_address = @0x9999;
        let auth_context = create_auth_context_for_testing(wrong_address, 500, &test_ctx);

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_withdraw_call_wrong_package_id() {
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

        // Create auth_context with a wrong package id
        let move_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@0x012345), // wrong package id
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                ptb_command::new_input_argument_for_testing(0), // account
                ptb_command::new_input_argument_for_testing(1), // amount
            ],
        );
        let command = ptb_command::new_move_call_command_for_testing(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
        let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_withdraw_call_wrong_module() {
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

        // Create auth_context with a wrong module name
        let move_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@spending_limit),
            ascii::string(b"wrong_module"), // wrong module name
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                ptb_command::new_input_argument_for_testing(0), // account
                ptb_command::new_input_argument_for_testing(1), // amount
            ],
        );
        let command = ptb_command::new_move_call_command_for_testing(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
        let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(amount_bytes);
        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_withdraw_call_wrong_function() {
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

        // Create auth_context with a wrong function name
        let move_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"wrong_function"), // wrong function name
            vector[],
            vector[
                ptb_command::new_input_argument_for_testing(0), // account
                ptb_command::new_input_argument_for_testing(1), // amount
            ],
        );
        let command = ptb_command::new_move_call_command_for_testing(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
        let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(amount_bytes);
        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
fun test_balance_reserve_structure() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_spending_limit_for_testing(scenario, 1000, b"42");

    // Switch sender to account address because borrow_field_mut checks
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<spending_limit::SpendLimit>();

        let key = spending_limit::get_balance_reserve_key_for_testing();

        let _reserve = spending_limit::borrow_field_mut<
            spending_limit::BalanceReserveKey,
            spending_limit::BalanceReserve,
        >(
            &mut account,
            key,
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EInvalidAmount)]
fun test_withdraw_invalid_bcs_amount() {
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

        // Create auth_context with an invalid BCS amount (e.g., empty vector)
        let account_id = object::id_from_address(account_address);
        let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
        let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

        let invalid_amount_bytes: vector<u8> = vector[]; // invalid BCS
        let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(invalid_amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let move_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                ptb_command::new_input_argument_for_testing(0), // account
                ptb_command::new_input_argument_for_testing(1), // amount
            ],
        );
        let command = ptb_command::new_move_call_command_for_testing(move_call);
        let commands = vector[command];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);
        spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<SpendLimit> {
    iota::authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(b"spending_limit"),
        ascii::string(b"authenticate"),
    )
}

fun create_spending_limit_for_testing(
    scenario: &mut Scenario,
    limit: u64,
    public_key: vector<u8>,
): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_function_ref_v1_for_testing();

    spending_limit::create(public_key, limit, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<spending_limit::SpendLimit>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_auth_context_for_testing(
    account_address: address,
    amount: u64,
    ctx: &TxContext,
): AuthContext {
    // Input 0: account (shared object)
    let account_id = object::id_from_address(account_address);
    let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
    let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

    // Input 1: amount (pure u64)
    let amount_bytes = iota::bcs::to_bytes(&amount);
    let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(amount_bytes);
    let inputs = vector[account_call_arg, amount_call_arg];

    // Command 1: withdraw_from_balance_reserve<IOTA>(Input(0), Input(1))
    let move_call = ptb_command::new_programmable_move_call_for_testing(
        object::id_from_address(@spending_limit),
        ascii::string(b"account"),
        ascii::string(b"withdraw_from_balance_reserve"),
        vector[], // type args
        vector[
            ptb_command::new_input_argument_for_testing(0), // account
            ptb_command::new_input_argument_for_testing(1), // amount
        ],
    );

    let command = ptb_command::new_move_call_command_for_testing(move_call);
    let commands = vector[command];

    auth_context::new_with_tx_inputs(*ctx.digest(), inputs, commands)
}

fun create_auth_context_for_testing_multiple_withdraw_calls(
    account_address: address,
    amount_per_withdraw: u64,
    num_withdraws: u64,
    ctx: &TxContext,
): AuthContext {
    // Input 0: account (shared object)
    let account_id = object::id_from_address(account_address);
    let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(account_id, 0, true);
    let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);

    // Input 1: amount (pure u64) for withdraw
    let amount_bytes = iota::bcs::to_bytes(&amount_per_withdraw);
    let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(amount_bytes);
    // Input 2: temp_number (pure u16) for random function
    let temp_number: u16 = 42;
    let temp_number_bytes = iota::bcs::to_bytes(&temp_number);
    let temp_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(temp_number_bytes);
    let inputs = vector[account_call_arg, amount_call_arg, temp_call_arg];

    // Create commands with one loop
    let mut commands = vector[];
    let mut i = 0;

    while (i < num_withdraws) {
        // Add random function call
        let random_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"random_function_that_does_nothing"),
            vector[],
            vector[ptb_command::new_input_argument_for_testing(2)], // temp_number
        );
        commands.push_back(ptb_command::new_move_call_command_for_testing(random_call));

        // Add withdraw call
        let withdraw_call = ptb_command::new_programmable_move_call_for_testing(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                ptb_command::new_input_argument_for_testing(0), // account
                ptb_command::new_input_argument_for_testing(1), // amount
            ],
        );
        commands.push_back(ptb_command::new_move_call_command_for_testing(withdraw_call));
        i = i + 1;
    };

    auth_context::new_with_tx_inputs(*ctx.digest(), inputs, commands)
}
