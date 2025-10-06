// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::iotaccount_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use iotaccount::iotaccount::{Self, IOTAccount};
use iotaccount::test_utils::{
    create_iotaccount_for_testing,
    create_authenticator_info_v1_for_testing
};

// --------------------------------------- Add Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun account_cant_add_reserved_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(account::authenticator_df_name(), 42, ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
fun account_can_add_regular_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(42, 42, ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Borrow Dynamic Field ---------------------------------------

#[test]
fun account_can_read_regular_dynamic_fields() {
    account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        let value: &u8 = account.borrow_field(b"SomeData".to_ascii_string());
        assert_eq(*value, 3u8);

        test_scenario::return_shared(account);
    })
}

#[test]
fun account_can_read_reserved_dynamic_fields() {
    account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        let authenticator: &AuthenticatorInfoV1 = account.borrow_field(
            account::authenticator_df_name(),
        );
        assert_eq(*authenticator, create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Borrow Mut Dynamic Field ---------------------------------------

#[test]
fun account_can_modify_regular_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::EAuthenticatorDynamicFieldNameCannotBeUsed)]
fun account_cant_modify_reserved_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Remove Dynamic Field ---------------------------------------

#[test]
fun account_can_remove_regular_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::EAuthenticatorDynamicFieldNameCannotBeUsed)]
fun account_cant_remove_reserved_dynamic_fields() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Has Dynamic Field ---------------------------------------

#[test]
fun account_can_query_regular_dynamic_field_existence() {
    account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(b"SomeData".to_ascii_string()));

        test_scenario::return_shared(account);
    })
}

#[test]
fun account_can_query_reserved_dynamic_field_existence() {
    account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(account::authenticator_df_name()));

        test_scenario::return_shared(account);
    })
}

// ---------------------------------- Rotate reserved field -------------------------------------

#[test]
fun account_can_rotate_reserved_field() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let value = account.rotate(
            account::authenticator_df_name(),
            create_authenticator_info_v1_for_testing(),
            ctx,
        );
        assert_eq(value, create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    })
}

#[test]
fun account_can_rotate_regular_field() {
    account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let value = account.rotate(
            b"SomeData".to_ascii_string(),
            2u8,
            ctx,
        );
        assert_eq(value, 3u8);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Test Utilities ---------------------------------------

macro fun account_sender($f: |&mut Scenario|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        $f(scenario);
    };
    test_scenario::end(scenario_val);
}

// ###############################################################################################################
//                                          Non-Account sender tests
// ###############################################################################################################

// --------------------------------------- Add Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_add_reserved_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(account::authenticator_df_name(), 42, ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_add_regular_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.add_field(42, 42, ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Borrow Dynamic Field ---------------------------------------

#[test]
fun non_account_can_read_regular_dynamic_fields() {
    non_account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        let value: &u8 = account.borrow_field(b"SomeData".to_ascii_string());
        assert_eq(*value, 3u8);

        test_scenario::return_shared(account);
    })
}

#[test]
fun non_account_can_read_reserved_dynamic_fields() {
    non_account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        let authenticator: &AuthenticatorInfoV1 = account.borrow_field(
            account::authenticator_df_name(),
        );
        assert_eq(*authenticator, create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Borrow Mut Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_modify_regular_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_modify_reserved_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Remove Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_remove_regular_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_remove_reserved_dynamic_fields() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Has Dynamic Field ---------------------------------------

#[test]
fun non_account_can_query_regular_dynamic_field_existence() {
    non_account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(b"SomeData".to_ascii_string()));

        test_scenario::return_shared(account);
    })
}

#[test]
fun non_account_can_query_reserved_dynamic_field_existence() {
    non_account_sender!(|scenario| {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(account::authenticator_df_name()));

        test_scenario::return_shared(account);
    })
}

// ---------------------------------- Rotate reserved field -------------------------------------

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_rotate_reserved_field() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.rotate(
            account::authenticator_df_name(),
            create_authenticator_info_v1_for_testing(),
            ctx,
        );

        test_scenario::return_shared(account);
    })
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_rotate_regular_field() {
    non_account_sender!(|scenario| {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.rotate(
            b"SomeData".to_ascii_string(),
            2u8,
            ctx,
        );

        test_scenario::return_shared(account);
    })
}

// --------------------------------------- Test Utilities ---------------------------------------

macro fun non_account_sender($f: |&mut Scenario|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        $f(scenario);
    };
    test_scenario::end(scenario_val);
}
