// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Scenario-style tests that mirror the structure and tone of `iotaccount` tests.
///
/// Coverage:
/// - Happy path (init → grant → authenticate OK)
/// - Unauthorized function
/// - Invalid amount of commands
/// - Revoke then fail
/// - Double add (store error)
/// - Remove missing (store error)
/// - Authenticate without init
///
#[test_only]
module function_keys::function_keys_scenario_tests;

use function_keys::fk_store::{Self as store, make_func_key};
use function_keys::function_keys;
use iota::account::{Self as account_pkg, AuthenticatorInfoV1};
use iota::auth_context::{Self as acx, AuthContext};
use iota::hex;
use iota::programmable_transaction::{Self as ptb, Command};
use iota::test_scenario::{Self as scen, Scenario};
use iota::tx_context as txc;
use iotaccount::basic_keyed_account as iacc;
use iotaccount::iotaccount::{account_address, IOTAccount};
use std::ascii;

// ----------------------------------------------------------------------------
// Happy path: create → grant → authenticate OK
// ----------------------------------------------------------------------------
#[test]
fun test_fk_authenticate_happy_path() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    // Use the same fixed public key / digest / signature pattern as iotaccount tests
    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // TX 1: init store + grant permission for this pub_key
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        // initialize FK store
        function_keys::create(&mut account, ctx);

        // allow @0x123::wallet::withdraw for ed25519_pk
        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);

        scen::return_shared(account);
    };

    // TX 2: authenticate a PTB with exactly one matching MoveCall
    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"withdraw".to_ascii_string(),
            ),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            ed25519_pk,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Unauthorized function
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EUnauthorized)]
fun test_fk_authenticate_unauthorized() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // setup
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);
        function_keys::create(&mut account, ctx);

        // allow withdraw only for this pub_key
        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);

        scen::return_shared(account);
    };

    // authenticate calling DEPOSIT instead
    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let account = scenario.take_shared<IOTAccount>();

        let ctx = create_tx_context_for_testing(account_address, digest);
        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"deposit".to_ascii_string(),
            ),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            ed25519_pk,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Too many commands
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EInvalidAmountOfCommands)]
fun test_fk_authenticate_too_many_commands() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // setup
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);
        function_keys::create(&mut account, ctx);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);

        scen::return_shared(account);
    };

    // authenticate with 2 commands
    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let account = scenario.take_shared<IOTAccount>();

        let ctx = create_tx_context_for_testing(account_address, digest);
        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"withdraw".to_ascii_string(),
            ),
        );
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"deposit".to_ascii_string(),
            ),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            ed25519_pk,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Revoke → authenticate fails
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EUnauthorized)]
fun test_fk_revoke_then_fails() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // setup & revoke
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        function_keys::create(&mut account, ctx);
        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);
        function_keys::revoke_permission(&mut account, ed25519_pk, &fk, ctx);

        scen::return_shared(account);
    };

    // authenticate now should fail
    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let account = scenario.take_shared<IOTAccount>();

        let ctx = create_tx_context_for_testing(account_address, digest);
        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"withdraw".to_ascii_string(),
            ),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            ed25519_pk,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Double add attempt
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = store::EFunctionKeyAlreadyAdded)]
fun test_fk_double_add_should_fail() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        function_keys::create(&mut account, ctx);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");

        // First add OK
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);

        // Second add must fail with EFunctionKeyAlreadyAdded for the same pub_key
        function_keys::grant_permission(&mut account, ed25519_pk, fk, ctx);

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Try to remove function key that hasn't been added → EFunctionKeyDoesNotExist
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = store::EFunctionKeyDoesNotExist)]
fun test_fk_remove_missing_should_fail() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        function_keys::create(&mut account, ctx);

        // Ensure the pub_key bucket exists by granting a different function first
        let fk_granted = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, ed25519_pk, fk_granted, ctx);

        // Now try to remove a function key that was never added (same pub_key)
        let fk_other = make_func_key(package_id.to_address(), b"wallet", b"deposit");
        function_keys::revoke_permission(&mut account, ed25519_pk, &fk_other, ctx);

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Authenticate before init → EFunctionKeysNotInitialized
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EFunctionKeysNotInitialized)]
fun test_fk_authenticate_without_init() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let ed25519_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, ed25519_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();

        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let ctx = create_tx_context_for_testing(account_address, digest);

        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_move_call_for_testing(
                package_id,
                b"wallet".to_ascii_string(),
                b"withdraw".to_ascii_string(),
            ),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            ed25519_pk,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ============================================================================
// Utilities (mirroring iotaccount test style)
// ============================================================================

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account_pkg::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun create_iotaccount_with_pk_for_testing(
    scenario: &mut Scenario,
    public_key: vector<u8>,
): address {
    create_iotaccount_for_testing_impl(scenario, option::some(public_key))
}

fun create_iotaccount_for_testing_impl(
    scenario: &mut Scenario,
    public_key: option::Option<vector<u8>>,
): address {
    let ctx = scen::ctx(scenario);

    let public_key = public_key.destroy_or!(public_key_for_testing());
    let authenticator = create_authenticator_info_v1_for_testing();

    iacc::create(public_key, authenticator, ctx);

    scen::next_tx(scenario, @0x0);

    let account = scen::take_shared<IOTAccount>(scenario);
    let account_address = account.account_address();
    scen::return_shared(account);

    account_address
}

fun create_tx_context_for_testing(sender: address, digest: vector<u8>): TxContext {
    txc::new(sender, digest, 0, 0, 0)
}

/// Build an AuthContext for tests.
fun create_auth_context_with_commands_for_testing(cmds: vector<Command>): AuthContext {
    acx::new_with_tx_inputs(vector::empty(), vector::empty(), cmds)
}

fun public_key_for_testing(): vector<u8> { b"42" }

/// Build a MoveCall `Command` aligned with `fk_store::extract_func_key`.
fun make_move_call_for_testing(
    pkg: ID,
    module_name: std::ascii::String,
    function_name: std::ascii::String,
): Command {
    ptb::new_move_call(
        ptb::new_programmable_move_call(pkg, module_name, function_name, vector[], vector[]),
    )
}
