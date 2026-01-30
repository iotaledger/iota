// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::keyed_iotaccount_tests;

use iota::authenticator_function;
use iota::ecdsa_k1;
use iota::hex;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use iotaccount::iotaccount::{Self, IOTAccount};
use iotaccount::keyed_iotaccount::{Self, borrow_public_key};
use iotaccount::test_utils::create_authenticator_function_ref_v1_for_testing;
use std::ascii;

// --------------------------------------- Create Basic Keyed Account ---------------------------------------

#[test]
fun account_created() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = test_scenario::ctx(scenario);

    let public_key = b"42";
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    keyed_iotaccount::create(public_key, authenticator, ctx);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        // Check if authenticator has been set.
        assert_ref_eq(
            account.borrow_auth_function_ref_v1(),
            &create_authenticator_function_ref_v1_for_testing(),
        );

        assert_eq(*borrow_public_key(&account), public_key);

        test_scenario::return_shared(account);
    };
    test_scenario::end(scenario_val);
}

// --------------------------------------- Ed25519 Authentication ---------------------------------------

#[test]
fun test_authenticate_ed25519() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        keyed_iotaccount::authenticate_ed25519(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun test_authenticate_ed25519_wrong_sender() {
    let sender = @0x1;
    let mut scenario_val = test_scenario::begin(sender);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";

    create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(sender);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(sender, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        keyed_iotaccount::authenticate_ed25519(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = keyed_iotaccount::EEd25519VerificationFailed)]
fun test_authenticate_ed25519_wrong_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc40561aa";

        keyed_iotaccount::authenticate_ed25519(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Secp256k1 Authentication ---------------------------------------

#[test]
fun test_authenticate_secp256k1() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let secret_key = x"42258dcda14cf111c602b8971b8cc843e91e46ca905151c02744a6b017e69316";
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature = ecdsa_k1::secp256k1_sign(&secret_key, &digest, 0, false);

        keyed_iotaccount::authenticate_secp256k1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun test_authenticate_secp256k1_wrong_sender() {
    let sender = @0x1;
    let mut scenario_val = test_scenario::begin(sender);
    let scenario = &mut scenario_val;
    let public_key = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";

    create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(sender);
    {
        let secret_key = x"42258dcda14cf111c602b8971b8cc843e91e46ca905151c02744a6b017e69316";
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(sender, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature = ecdsa_k1::secp256k1_sign(&secret_key, &digest, 0, false);

        keyed_iotaccount::authenticate_secp256k1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = keyed_iotaccount::ESecp256k1VerificationFailed)]
fun test_authenticate_secp256k1_wrong_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        keyed_iotaccount::authenticate_secp256k1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Secp256r1 Authentication ---------------------------------------

#[test]
fun test_authenticate_secp256r1() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"310d0ab3a8870f6ab3d775f3cdf0a60059293e431f3ded9d1f6efe2c70f12da5628c7853ae18464b4d426d8ff6d31ae50fe31e47886b13733ba2aae508541bcd";

        keyed_iotaccount::authenticate_secp256r1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun test_authenticate_secp256r1_wrong_sender() {
    let sender = @0x1;
    let mut scenario_val = test_scenario::begin(sender);
    let scenario = &mut scenario_val;
    let public_key = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";

    create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(sender);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(sender, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"310d0ab3a8870f6ab3d775f3cdf0a60059293e431f3ded9d1f6efe2c70f12da5628c7853ae18464b4d426d8ff6d31ae50fe31e47886b13733ba2aae508541bcd";

        keyed_iotaccount::authenticate_secp256r1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = keyed_iotaccount::ESecp256r1VerificationFailed)]
fun test_authenticate_secp256r1_wrong_signature() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, public_key);

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_for_testing();

        let signature =
            x"310d0ab3a8870f6ab3d775f3cdf0a60059293e431f3ded9d1f6efe2c70f12da5628c7853ae18464b4d426d8ff6d31ae50fe31e47886b13733ba2aae508541baa";

        keyed_iotaccount::authenticate_secp256r1(
            &account,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Public Key Rotation ---------------------------------------

#[test]
fun test_rotate_account_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let starting_public_key = b"42";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, starting_public_key);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        let public_key = b"24";
        let authenticator = authenticator_function::create_auth_function_ref_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        let ctx = test_scenario::ctx(scenario);

        keyed_iotaccount::rotate_public_key(&mut account, public_key, authenticator, ctx);

        assert_eq(*borrow_public_key(&account), public_key);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &authenticator);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun test_rotate_account_public_key_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let starting_public_key = b"42";
    create_iotaccount_with_pk_for_testing(scenario, starting_public_key);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        let public_key = b"24";
        let authenticator = authenticator_function::create_auth_function_ref_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        let ctx = test_scenario::ctx(scenario);

        keyed_iotaccount::rotate_public_key(&mut account, public_key, authenticator, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_iotaccount_with_pk_for_testing(
    scenario: &mut Scenario,
    public_key: vector<u8>,
): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_function_ref_v1_for_testing();

    keyed_iotaccount::create(public_key, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.account_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    tx_context::new(sender, digest, 0, 0, 0)
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}
