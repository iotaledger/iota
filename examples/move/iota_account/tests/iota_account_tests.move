// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_account::iota_account_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};
use iota_account::iota_account::{
    Self,
    IOTAccount,
    DfKey,
    create_iotaccount_for_testing,
    create_authenticator_info_v1_for_testing
};
use std::string;

// ##########################################################################################
// #                                    IOTAccount                                          #
// ##########################################################################################

// ######################## Dynamic Fields Management By The Account ########################

public struct TestObject has copy, drop, store {}

#[test]
fun add_read_remove_regular_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        check_dynamic_field(&mut account, 42, 42, ctx);
        check_dynamic_field(&mut account, b"vector", b"vector", ctx);
        check_dynamic_field(
            &mut account,
            string::utf8(b"std::string"),
            string::utf8(b"std::string"),
            ctx,
        );
        check_dynamic_field(&mut account, TestObject {}, TestObject {}, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// #[test]
// fun account_cant_add_dynamic_field_matching_reserved_one() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         check_dynamic_field(&mut account, 42, 42, ctx);
//         check_dynamic_field(&mut account, b"vector", b"vector", ctx);
//         check_dynamic_field(
//             &mut account,
//             string::utf8(b"std::string"),
//             string::utf8(b"std::string"),
//             ctx,
//         );
//         check_dynamic_field(&mut account, TestObject {}, TestObject {}, ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

// #[test]
// fun account_cannot_modify_reserved_dynamic_fields() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         check_dynamic_field(&mut account, 42, 42, ctx);
//         check_dynamic_field(&mut account, b"vector", b"vector", ctx);
//         check_dynamic_field(
//             &mut account,
//             string::utf8(b"std::string"),
//             string::utf8(b"std::string"),
//             ctx,
//         );
//         check_dynamic_field(&mut account, TestObject {}, TestObject {}, ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

// #[test]
// fun account_cannot_delete_reserved_dynamic_fields() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         check_dynamic_field(&mut account, 42, 42, ctx);
//         check_dynamic_field(&mut account, b"vector", b"vector", ctx);
//         check_dynamic_field(
//             &mut account,
//             string::utf8(b"std::string"),
//             string::utf8(b"std::string"),
//             ctx,
//         );
//         check_dynamic_field(&mut account, TestObject {}, TestObject {}, ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

// todo has

// ---------------------------------- Rotate reserved field -------------------------------------

#[test]
fun account_can_rotate_reserved_field() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.rotate_reserved(
            account::authenticator_df_name(),
            create_authenticator_info_v1_for_testing(), // only for the AuthInfoV1 type, the value doesn't matter
            ctx,
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::EMustModifyReservedDynamicField)]
fun account_cant_rotate_regular_field() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.rotate_reserved(b"SomeData".to_ascii_string(), 3, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// ######################## Dynamic Fields Access By Non-Account Party ########################

#[test]
#[expected_failure(abort_code = iota_account::ECantModifyReservedDynamicField)]
fun non_account_cant_add_reserved_dynamic_fields() {
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

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_add_regular_dynamic_fields() {
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

// --------------------------------------- Borrow Dynamic Field ---------------------------------------

#[test]
fun non_account_can_read_regular_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        let value: &u8 = account.borrow_field(b"SomeData".to_ascii_string());
        assert_eq(*value, 3u8);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun non_account_can_read_reserved_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        let authenticator: &AuthenticatorInfoV1 = account.borrow_field(
            account::authenticator_df_name(),
        );
        assert_eq(*authenticator, create_authenticator_info_v1_for_testing());

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Borrow Mut Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_modify_regular_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_modify_reserved_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        let _: &mut u8 = account.borrow_field_mut(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Remove Dynamic Field ---------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_remove_regular_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(b"SomeData".to_ascii_string(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_remove_reserved_dynamic_fields() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.remove_field<_, u8>(account::authenticator_df_name(), ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Has Dynamic Field ---------------------------------------

#[test]
fun non_account_can_query_regular_dynamic_field_existence() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(b"SomeData".to_ascii_string()));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun non_account_can_query_reserved_dynamic_field_existence() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert!(account.has_field(account::authenticator_df_name()));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// ---------------------------------- Rotate reserved field -------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::ETransactionSenderIsNotTheAccount)]
fun non_account_cant_rotate_reserved_field() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = test_scenario::ctx(scenario);

        account.rotate_reserved(account::authenticator_df_name(), 3, ctx);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

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
