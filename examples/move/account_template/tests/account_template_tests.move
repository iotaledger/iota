#[test_only]
module account_template::account_template_tests;

use account_template::account_template::{Self, IOTAccount};
use iota::account::{Self, AuthenticatorInfoV1};
use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

public struct ReservedDfExample has copy, drop, store {}

#[test]
#[expected_failure(abort_code = account_template::EAuthenticatorNotSet)]
fun builder_authenticator_not_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let builder = account_template::init_account_builder(ctx);
    builder.finish_and_share();

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = account_template::EReservedDynamicFieldsListCannotBeSet)]
fun builder_reserved_fields_cannot_be_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let mut builder = account_template::init_account_builder(ctx);
    builder.add_reserved_field(
        account_template::get_reserved_df_names(),
        vector<std::type_name::TypeName>[],
    );
    builder.finish_and_share();

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = account_template::EReservedDynamicFieldsNotSet)]
fun builder_no_reserved_field() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let mut builder = account_template::init_account_builder(ctx);

    let authenticator = create_authenticator_info_v1_for_testing();
    builder.set_authenticator(authenticator);

    builder.finish_and_share();

    test_scenario::end(scenario_val);
}

#[test]
fun builder_all_fields_set() {
    let test_sender = @0x0;
    let mut scenario_val = test_scenario::begin(test_sender);
    let scenario = &mut scenario_val;

    let ctx = test_scenario::ctx(scenario);

    let mut builder = account_template::init_account_builder(ctx);

    let authenticator = create_authenticator_info_v1_for_testing();
    builder.set_authenticator(authenticator);
    // Any field value can be set as a reserved, and for the purposes of this test
    // the exact value doesn't matter.
    let reserved_df_example = ReservedDfExample {};
    builder.add_reserved_field(reserved_df_example, 6);

    builder.finish_and_share();

    scenario.next_tx(@0x0);
    {
        let account = scenario.take_shared<IOTAccount>();

        let authenticator_df_name = account::authenticator_df_name();
        assert!(account.has_field(authenticator_df_name));
        assert_ref_eq(
            account.borrow_field(authenticator_df_name),
            &create_authenticator_info_v1_for_testing(),
        );

        let reserved_df_name = account_template::get_reserved_df_names();
        assert!(account.has_field(reserved_df_name));
        let reserved_df_names: &vector<std::type_name::TypeName> = account.borrow_field(
            reserved_df_name,
        );
        assert!(reserved_df_names.length() == 1);
        assert!(reserved_df_names.contains(&std::type_name::get<ReservedDfExample>()));

        // check the ReservedKey value as well
        assert!(account.has_field(reserved_df_example));
        assert_eq(*account.borrow_field(reserved_df_example), 6);

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(
        @0x0,
        ascii::string(b"account_template"),
        ascii::string(b"authenticator"),
    )
}
