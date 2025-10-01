// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_account::iota_account_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::{Self, AuthContext};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use iota_account::iota_account::{Self, IOTAccount, DfKey, make_key};
use std::ascii;
use std::string;

// --------------------------------------- Create IOTAccount ---------------------------------------------------

#[test]
#[expected_failure(abort_code = iota_account::EReservedDynamicFieldsListCannotBeSet)]
fun builder_reserved_fields_list_cannot_be_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();
    let account = iota_account::builder(authenticator, ctx)
        .add_reserved_field(
            iota_account::get_reserved_dynamic_fields(),
            vector<DfKey>[],
        )
        .finish();
    account.share();

    test_scenario::end(scenario_val);
}

public struct ReservedDfName has copy, drop, store {}

#[test]
fun builder_all_mandatory_fields_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let reserved_df_example_name = ReservedDfName {};

    let authenticator = create_authenticator_info_v1_for_testing();
    // Any field value can be set as a reserved, and for the purposes of this test
    // the exact value doesn't matter.
    let account = iota_account::builder(authenticator, ctx)
        .add_reserved_field(reserved_df_example_name, 6)
        .finish();
    account.share();

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        // Check if authenticator has been set.
        let authenticator_df_name = account::authenticator_df_name();
        assert!(account.has_field(authenticator_df_name));
        assert_ref_eq(
            account.borrow_field(authenticator_df_name),
            &create_authenticator_info_v1_for_testing(),
        );

        // Check if reserved dynamic fields list has been set.
        let reserved_df_name = iota_account::get_reserved_dynamic_fields();
        assert!(account.has_field(reserved_df_name));
        // and if it contains the appropriate values.
        let reserved_df_keys: &vector<DfKey> = account.borrow_field(
            reserved_df_name,
        );
        assert!(reserved_df_keys.length() == 2);
        assert!(reserved_df_keys.contains(&make_key(authenticator_df_name)));
        assert!(reserved_df_keys.contains(&make_key(reserved_df_example_name)));

        // Check the ReservedDfName contains the set value.
        assert!(account.has_field(reserved_df_example_name));
        assert_eq(*account.borrow_field(reserved_df_example_name), 6);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun attempting_to_add_a_field_as_reserved_then_regular() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);
    let authenticator = create_authenticator_info_v1_for_testing();

    let field_name = b"SomeData".to_ascii_string();
    let account = iota_account::builder(authenticator, ctx)
        .add_reserved_field(
            field_name,
            3,
        )
        .add_regular_field(field_name, 3)
        .finish();
    account.share();

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun attempting_to_add_a_field_as_regular_then_reserved() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);
    let authenticator = create_authenticator_info_v1_for_testing();

    let field_name = b"SomeData".to_ascii_string();
    let account = iota_account::builder(authenticator, ctx)
        .add_regular_field(field_name, 3)
        .add_reserved_field(
            field_name,
            3,
        )
        .finish();
    account.share();

    test_scenario::end(scenario_val);
}

#[test]
fun reserved_fields_list_observe_the_value_not_just_the_type() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);
    let authenticator = create_authenticator_info_v1_for_testing();

    // These fields will are considered different, because the value within the Strings
    // are different.
    let field_name = b"SomeData".to_ascii_string();
    let another_name = b"DifferentData".to_ascii_string();
    let account = iota_account::builder(authenticator, ctx)
        .add_reserved_field(
            field_name,
            3,
        )
        .add_reserved_field(
            another_name,
            3,
        )
        .finish();
    account.share();

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

// #[test]
// #[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
// fun test_add_user_defined_dynamic_field_authenticator_df_name() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         account.add_field(account::authenticator_df_name(), 42, ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

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

// #[test]
// #[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
// fun test_remove_user_defined_dynamic_field_authenticator_df_name() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         account.remove_field<_, AuthenticatorInfoV1>(account::authenticator_df_name(), ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

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
fun test_borrow_user_defined_dynamic_field_authenticator_df_name() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account_address = create_iotaccount_for_testing(scenario);

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        assert_ref_eq(
            account.borrow_field(account::authenticator_df_name()),
            &create_authenticator_info_v1_for_testing(),
        );

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

// #[test]
// #[expected_failure(abort_code = iota_account::EAuthenticatorDynamicFieldNameCannotBeUsed)]
// fun test_borrow_mut_user_defined_dynamic_field_authenticator_df_name() {
//     let mut scenario_val = test_scenario::begin(@0x0);
//     let scenario = &mut scenario_val;
//     let account_address = create_iotaccount_for_testing(scenario);

//     scenario.next_tx(account_address);
//     {
//         let mut account = scenario.take_shared<IOTAccount>();
//         let ctx = test_scenario::ctx(scenario);

//         account.borrow_field_mut<_, AuthenticatorInfoV1>(account::authenticator_df_name(), ctx);

//         test_scenario::return_shared(account);
//     };

//     test_scenario::end(scenario_val);
// }

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

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    // The exact values don't matter in these tests.
    account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun create_iotaccount_for_testing(scenario: &mut Scenario): address {
    let ctx = test_scenario::ctx(scenario);

    let authenticator = create_authenticator_info_v1_for_testing();

    let account = iota_account::builder(authenticator, ctx).finish();
    let account_address = account.get_address();

    iota_account::share(account);

    account_address
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
