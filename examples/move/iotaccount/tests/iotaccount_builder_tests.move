// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iotaccount::iotaccount_builder_tests;

use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};
use iotaccount::iotaccount::{Self, IOTAccount};
use iotaccount::test_utils::create_authenticator_function_ref_v1_for_testing;

// -------------------------------- Create IOTAccount --------------------------------

public struct DynamicFieldKey has copy, drop, store {}

#[test]
fun builder_all_mandatory_fields_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let dynamic_field_key = DynamicFieldKey {};

    let authenticator = create_authenticator_function_ref_v1_for_testing();
    // Any field value can be set as a dynamic field, and for the purposes of this test
    // the exact value doesn't matter.
    iotaccount::builder(authenticator, ctx).add_dynamic_field(dynamic_field_key, 6).build();

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        // Check if authenticator has been set.
        assert_ref_eq(
            account.borrow_auth_function_ref_v1(),
            &create_authenticator_function_ref_v1_for_testing(),
        );

        // Check the added dynamic field contains the set value.
        assert!(account.has_field(dynamic_field_key));
        assert_eq(*account.borrow_field(dynamic_field_key), 6);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun attempting_to_add_same_dynamic_field_twice() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    let field_name = b"SomeData".to_ascii_string();
    iotaccount::builder(authenticator, ctx)
        .add_dynamic_field(field_name, 3)
        .add_dynamic_field(
            field_name,
            3,
        )
        .build();

    test_scenario::end(scenario_val);
}

#[test]
fun dynamic_fields_observe_the_value_not_just_the_type() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    // These fields will are considered different, because the value within the Strings
    // are different.
    let field_name = b"SomeData".to_ascii_string();
    let another_name = b"DifferentData".to_ascii_string();
    iotaccount::builder(authenticator, ctx)
        .add_dynamic_field(
            field_name,
            3,
        )
        .add_dynamic_field(
            another_name,
            3,
        )
        .build();

    test_scenario::end(scenario_val);
}
