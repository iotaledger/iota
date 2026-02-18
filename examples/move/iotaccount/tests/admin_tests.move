#[test_only]
module iotaccount::admin_tests;

use iota::test_scenario;
use iotaccount::iotaccount::{Self, IOTAccount};
use iotaccount::public_key_iotaccount;
use iotaccount::test_utils::create_authenticator_function_ref_v1_for_testing;

#[test]
fun test_admin_rotate_auth_function_ref() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    // Admin account -> secp256k1 key
    let admin_public_key = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    public_key_iotaccount::create(
        admin_public_key,
        create_authenticator_function_ref_v1_for_testing(),
        scenario.ctx(),
    );
    scenario.next_tx(@0x0);
    let admin_account = scenario.take_shared<IOTAccount>();
    let admin_address = admin_account.account_address();
    test_scenario::return_shared(admin_account);

    // Main IOTAccount -> ed25519 key
    let iotaccount_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    public_key_iotaccount::create_with_admin(
        iotaccount_public_key,
        admin_address,
        create_authenticator_function_ref_v1_for_testing(),
        scenario.ctx(),
    );
    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: The IOTAccount rotates its authenticator to a fake one.
    scenario.next_tx(iotaccount_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        public_key_iotaccount::rotate_public_key(
            &mut account,
            x"0123",
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    // TX2: The admin rotates the IOTAccount's authenticator to an ed25519 one.
    scenario.next_tx(admin_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        public_key_iotaccount::rotate_public_key(
            &mut account,
            x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88",
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    // TX3: Test the IOTAccount authentication.
    scenario.next_tx(iotaccount_address);
    {
        let account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        let ctx = tx_context::new(
            iotaccount_address,
            x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3",
            0,
            0,
            0,
        );
        let auth_ctx = auth_context::new_with_tx_inputs(
            vector::empty(),
            vector::empty(),
            vector::empty(),
        );

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        public_key_iotaccount::ed25519_IOTAccount_authenticator(
            &account,
            signature,
            &auth_ctx,
            &ctx,
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
    let non_admin_public_key =
        x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    public_key_iotaccount::create(
        non_admin_public_key,
        create_authenticator_function_ref_v1_for_testing(),
        scenario.ctx(),
    );
    scenario.next_tx(@0x0);
    let non_admin_account = scenario.take_shared<IOTAccount>();
    let non_admin_address = non_admin_account.account_address();
    test_scenario::return_shared(non_admin_account);

    // Main IOTAccount -> created with no admin
    let iotaccount_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    public_key_iotaccount::create(
        iotaccount_public_key,
        create_authenticator_function_ref_v1_for_testing(),
        scenario.ctx(),
    );
    scenario.next_tx(@0x0);
    let iotaccount = scenario.take_shared<IOTAccount>();
    let iotaccount_address = iotaccount.account_address();
    test_scenario::return_shared(iotaccount);

    // TX1: The non admin tries to rotate the IOTAccount's authenticator to an ed25519 one.
    scenario.next_tx(non_admin_address);
    {
        let mut account = scenario.take_shared_by_id<IOTAccount>(iotaccount_address.to_id());

        public_key_iotaccount::rotate_public_key(
            &mut account,
            x"0123",
            create_authenticator_function_ref_v1_for_testing(),
            scenario.ctx(),
        );

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}
