// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::account_tests;

use generic_keyed_authentication::owner_public_key;
use iota::account::AuthenticatorInfoV1;
use iota::auth_context::{Self, AuthContext};
use iota::hex;
use iota::programmable_transaction;
use iota::test_scenario::{Self, Scenario};
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
        let auth_context = create_auth_context_for_testing(account_address, 500, scenario.ctx());
        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);
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
        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            scenario.ctx(),
        );

        spending_limit::destroy_withdraw_proof_for_testing(proof);

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
        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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
        let auth_context = create_auth_context_for_testing(account_address, 1000, &test_ctx);

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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
        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EUnauthorizedWithdrawCall)]
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

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EUnauthorizedWithdrawCall)]
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

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EUnauthorizedWithdrawCall)]
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
        let move_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@0x012345), // wrong package id
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                programmable_transaction::new_input_argument(0), // account
                programmable_transaction::new_input_argument(1), // amount
            ],
        );
        let command = programmable_transaction::new_move_call(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
        let account_call_arg = programmable_transaction::new_object(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = programmable_transaction::new_pure(amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EUnauthorizedWithdrawCall)]
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
        let move_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@spending_limit),
            ascii::string(b"wrong_module"), // wrong module name
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                programmable_transaction::new_input_argument(0), // account
                programmable_transaction::new_input_argument(1), // amount
            ],
        );
        let command = programmable_transaction::new_move_call(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
        let account_call_arg = programmable_transaction::new_object(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = programmable_transaction::new_pure(amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EUnauthorizedWithdrawCall)]
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
        let move_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"wrong_function"), // wrong function name
            vector[],
            vector[
                programmable_transaction::new_input_argument(0), // account
                programmable_transaction::new_input_argument(1), // amount
            ],
        );
        let command = programmable_transaction::new_move_call(move_call);
        let commands = vector[command];

        let account_id = object::id_from_address(account_address);
        let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
        let account_call_arg = programmable_transaction::new_object(account_obj_arg);

        let amount: u64 = 500;
        let amount_bytes = iota::bcs::to_bytes(&amount);
        let amount_call_arg = programmable_transaction::new_pure(amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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
        let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
        let account_call_arg = programmable_transaction::new_object(account_obj_arg);

        let invalid_amount_bytes: vector<u8> = vector[]; // invalid BCS
        let amount_call_arg = programmable_transaction::new_pure(invalid_amount_bytes);

        let inputs = vector[account_call_arg, amount_call_arg];

        let move_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                programmable_transaction::new_input_argument(0), // account
                programmable_transaction::new_input_argument(1), // amount
            ],
        );
        let command = programmable_transaction::new_move_call(move_call);
        let commands = vector[command];

        let auth_context = auth_context::new_with_tx_inputs(*test_ctx.digest(), inputs, commands);

        let proof = spending_limit::authenticate(
            &account,
            hex::encode(signature),
            &auth_context,
            &test_ctx,
        );
        spending_limit::destroy_withdraw_proof_for_testing(proof);

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

fun create_auth_context_for_testing(
    account_address: address,
    amount: u64,
    ctx: &TxContext,
): AuthContext {
    // Input 0: account (shared object)
    let account_id = object::id_from_address(account_address);
    let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
    let account_call_arg = programmable_transaction::new_object(account_obj_arg);

    // Input 1: amount (pure u64)
    let amount_bytes = iota::bcs::to_bytes(&amount);
    let amount_call_arg = programmable_transaction::new_pure(amount_bytes);

    let inputs = vector[account_call_arg, amount_call_arg];

    // Comando: withdraw_from_balance_reserve<IOTA>(Input(0), Input(1))
    let move_call = programmable_transaction::new_programmable_move_call(
        object::id_from_address(@spending_limit),
        ascii::string(b"account"),
        ascii::string(b"withdraw_from_balance_reserve"),
        vector[], // type args
        vector[
            programmable_transaction::new_input_argument(0), // account
            programmable_transaction::new_input_argument(1), // amount
        ],
    );

    let command = programmable_transaction::new_move_call(move_call);
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
    let account_obj_arg = programmable_transaction::new_shared_object(account_id, 0, true);
    let account_call_arg = programmable_transaction::new_object(account_obj_arg);

    // Input 1: amount (pure u64) for withdraw
    let amount_bytes = iota::bcs::to_bytes(&amount_per_withdraw);
    let amount_call_arg = programmable_transaction::new_pure(amount_bytes);
    // Input 2: temp_number (pure u16) for random function
    let temp_number: u16 = 42;
    let temp_number_bytes = iota::bcs::to_bytes(&temp_number);
    let temp_call_arg = programmable_transaction::new_pure(temp_number_bytes);

    let inputs = vector[account_call_arg, amount_call_arg, temp_call_arg];

    // Create commands with one loop
    let mut commands = vector[];
    let mut i = 0;

    while (i < num_withdraws) {
        // Add random function call
        let random_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"random_function_that_does_nothing"),
            vector[],
            vector[programmable_transaction::new_input_argument(2)], // temp_number
        );
        commands.push_back(programmable_transaction::new_move_call(random_call));

        // Add withdraw call
        let withdraw_call = programmable_transaction::new_programmable_move_call(
            object::id_from_address(@spending_limit),
            ascii::string(b"account"),
            ascii::string(b"withdraw_from_balance_reserve"),
            vector[],
            vector[
                programmable_transaction::new_input_argument(0), // account
                programmable_transaction::new_input_argument(1), // amount
            ],
        );
        commands.push_back(programmable_transaction::new_move_call(withdraw_call));

        i = i + 1;
    };

    auth_context::new_with_tx_inputs(*ctx.digest(), inputs, commands)
}
