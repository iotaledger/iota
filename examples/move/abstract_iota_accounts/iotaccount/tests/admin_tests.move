#[test_only]
module iotaccount::admin_tests;

use iota::test_scenario;
use iotaccount::iotaccount::{Self, IOTAccount};
use iotaccount::test_utils::{
    create_authenticator_function_ref_v1_for_testing,
    create_iotaccount_for_testing,
    create_iotaccount_with_admin_for_testing
};
use std::unit_test::assert_eq;

#[test]
fun test_admin_rotate_auth_function_ref() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    // Admin account
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let admin_account = scenario.take_shared<IOTAccount>();
    let admin_address = admin_account.account_address();
    test_scenario::return_shared(admin_account);

    // Main IOTAccount
    create_iotaccount_with_admin_for_testing(scenario, admin_address);

    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: The IOTAccount rotates its authenticator.
    scenario.next_tx(iotaccount_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        account.rotate_auth_function_ref_v1(
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    // TX2: The admin rotates the IOTAccount's authenticator.
    scenario.next_tx(admin_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        account.rotate_auth_function_ref_v1(
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccountOrAdmin)]
fun test_non_admin_rotate_auth_function_ref() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    // Non-Admin account
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let non_admin_account = scenario.take_shared<IOTAccount>();
    let non_admin_address = non_admin_account.account_address();
    test_scenario::return_shared(non_admin_account);

    // Main IOTAccount -> created with no admin
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: The non admin tries to rotate the IOTAccount's authenticator.
    scenario.next_tx(non_admin_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        account.rotate_auth_function_ref_v1(
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_add_admin() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    // Admin account
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let admin_account = scenario.take_shared<IOTAccount>();
    let admin_address = admin_account.account_address();
    test_scenario::return_shared(admin_account);

    // Main IOTAccount
    create_iotaccount_for_testing(scenario);

    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: add admin.
    scenario.next_tx(iotaccount_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        account.add_admin(admin_address, scenario.ctx());

        assert_eq!(account.borrow_admin(), option::some(admin_address));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun test_rotate_admin() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    // Admin account
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let admin_account = scenario.take_shared<IOTAccount>();
    let admin_address = admin_account.account_address();
    test_scenario::return_shared(admin_account);

    // Admin account 2
    create_iotaccount_for_testing(scenario);
    scenario.next_tx(@0x0);
    let admin_account_2 = scenario.take_shared<IOTAccount>();
    let admin_address_2 = admin_account_2.account_address();
    test_scenario::return_shared(admin_account_2);

    // Main IOTAccount
    create_iotaccount_with_admin_for_testing(scenario, admin_address);

    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: add admin.
    scenario.next_tx(iotaccount_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        assert_eq!(account.borrow_admin(), option::some(admin_address));

        account.rotate_admin(admin_address_2, scenario.ctx());

        assert_eq!(account.borrow_admin(), option::some(admin_address_2));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}
