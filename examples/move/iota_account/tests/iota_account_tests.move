// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_account::iota_account_tests;

use std::ascii;
use std::string;

use iota::hex;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::ecdsa_k1;
use iota::auth_context::{Self, AuthContext};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};

use iota_account::iota_account::{Self, IOTAccount};

// --------------------------------------- Basic Scenario ---------------------------------------

#[test]
fun test_account_creation() {

    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        let public_key_df_name = iota_account::create_owner_public_key_for_testing();

        assert!(account.has_field(public_key_df_name));
        assert_ref_eq(account.borrow_field(public_key_df_name), &public_key_for_testing());

        let authenticator_df_name = account::authenticator_df_name();

        assert!(account.has_field(authenticator_df_name));
        assert_ref_eq(account.borrow_field(authenticator_df_name), &create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Public Key Rotation ---------------------------------------

#[test]
fun test_rotate_account_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        let public_key = b"24";
        let authenticator = account::create_auth_info_v1_for_testing(@0x2, ascii::string(b"module2"), ascii::string(b"function2"));
        let ctx = test_scenario::ctx(scenario);

        account.rotate_public_key(public_key, authenticator, ctx);

        let public_key_df_name = iota_account::create_owner_public_key_for_testing();

        assert!(account.has_field(public_key_df_name));
        assert_ref_eq(account.borrow_field(public_key_df_name), &public_key);

        let authenticator_df_name = account::authenticator_df_name();

        assert!(account.has_field(authenticator_df_name));
        assert_ref_eq(account.borrow_field(authenticator_df_name), &authenticator);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun test_rotate_account_public_key_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();

        let public_key = b"24";
        let authenticator = account::create_auth_info_v1_for_testing(@0x2, ascii::string(b"module2"), ascii::string(b"function2"));
        let ctx = test_scenario::ctx(scenario);

        account.rotate_public_key(public_key, authenticator, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Dynamic Fields Basic Scenario ---------------------------------------

public struct TestObject has copy, drop, store {}

#[test]
fun test_user_defined_dynamic_field() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        check_dynamic_field(&mut account, 42, 42, ctx);
        check_dynamic_field(&mut account, b"vector", b"vector", ctx);
        check_dynamic_field(&mut account, string::utf8(b"std::string"), string::utf8(b"std::string"), ctx);
        check_dynamic_field(&mut account, TestObject{}, TestObject{}, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Add Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun test_add_user_defined_dynamic_field_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(42, 42, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EOwnerPublicKeyCannotBeUsed)]
fun test_add_user_defined_dynamic_field_owner_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(iota_account::create_owner_public_key_for_testing(), 42, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
fun test_add_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(account::authenticator_df_name(), 42, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Remove Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun test_remove_user_defined_dynamic_field_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u64>(42, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EOwnerPublicKeyCannotBeUsed)]
fun test_remove_user_defined_dynamic_field_owner_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, vector<u64>>(iota_account::create_owner_public_key_for_testing(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
fun test_remove_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, AuthenticatorInfoV1>(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Borrow Dynamic Field ---------------------------------------

#[test]
fun test_borrow_user_defined_dynamic_field_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    let name = 42;
    let value = 42;

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(name, value, ctx);

        test_scenario::return_shared(account);
    };

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert_ref_eq(account.borrow_field(name), &value);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_borrow_user_defined_dynamic_field_owner_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert_ref_eq(account.borrow_field(iota_account::create_owner_public_key_for_testing()), &public_key_for_testing());

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_borrow_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert_ref_eq(account.borrow_field(account::authenticator_df_name()), &create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Borrow Mut Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun test_borrow_mut_user_defined_dynamic_field_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    let name = 42;
    let value = 42;

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(name, value, ctx);

        test_scenario::return_shared(account);
    };

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.borrow_field_mut<u64, u64>(name, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EOwnerPublicKeyCannotBeUsed)]
fun test_borrow_mut_user_defined_dynamic_field_owner_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.borrow_field_mut<_, vector<u64>>(iota_account::create_owner_public_key_for_testing(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
fun test_borrow_mut_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.borrow_field_mut<_, AuthenticatorInfoV1>(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Has Dynamic Field ---------------------------------------

#[test]
fun test_has_user_defined_dynamic_field_wrong_sender() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    let name = 42;
    let value = 42;

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(name, value, ctx);

        test_scenario::return_shared(account);
    };

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(name));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_has_user_defined_dynamic_field_owner_public_key() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(iota_account::create_owner_public_key_for_testing()));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_has_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(account::authenticator_df_name()));

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

        iota_account::authenticate_ed25519(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
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

        iota_account::authenticate_ed25519(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EEd25519VerificationFailed)]
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

        iota_account::authenticate_ed25519(&account, hex::encode(signature), &auth_ctx, &ctx);

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

        iota_account::authenticate_secp256k1(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
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

        iota_account::authenticate_secp256k1(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ESecp256k1VerificationFailed)]
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

        iota_account::authenticate_secp256k1(&account, hex::encode(signature), &auth_ctx, &ctx);

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

        iota_account::authenticate_secp256r1(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
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

        iota_account::authenticate_secp256k1(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ESecp256r1VerificationFailed)]
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

        iota_account::authenticate_secp256r1(&account, hex::encode(signature), &auth_ctx, &ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(@0x1, ascii::string(b"module"), ascii::string(b"function"))
}

fun create_iotaccount_for_testing(scenario: &mut Scenario): address {
    create_iotaccount_for_testing_impl(scenario, option::none())
}

fun create_iotaccount_with_pk_for_testing(scenario: &mut Scenario, public_key: vector<u8>): address {
    create_iotaccount_for_testing_impl(scenario, option::some(public_key))
}

fun create_iotaccount_for_testing_impl(scenario: &mut Scenario, public_key: Option<vector<u8>>): address {
    let ctx = test_scenario::ctx(scenario);

    let public_key = public_key.destroy_or!(public_key_for_testing());
    let authenticator = create_authenticator_info_v1_for_testing();

    iota_account::create(public_key, authenticator, ctx);

    scenario.next_tx(@0x0);

    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.get_address();

    test_scenario::return_shared(account);

    account_address
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    tx_context::new(sender, digest, 0, 0, 0)
}

fun create_auth_context_for_testing(): AuthContext {
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), vector::empty())
}

fun public_key_for_testing(): vector<u8> {
    b"42"
}

fun check_dynamic_field<Name: copy + drop + store, Value: store + copy + drop>(
    account: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    account.add_field(name, value, ctx);

    assert!(account.has_field(name));
    assert_ref_eq(account.borrow_field(name), &value);
    assert_ref_eq(account.borrow_field_mut(name, ctx), &value);

    assert_eq(account.remove_field(name, ctx), value);

    assert!(!account.has_field(name));
}
