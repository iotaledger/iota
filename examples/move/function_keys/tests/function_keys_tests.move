// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Scenario-style tests that mirror the structure and tone of `iotaccount` tests.
///
/// Coverage:
/// - Happy path (attach → grant → delegated authenticate OK)
/// - Happy path (attach → grant → owner authenticate OK)
/// - Unauthorized function (delegated)
/// - Invalid amount of commands (delegated)
/// - Revoke then fail (delegated)
/// - Double add (store error)
/// - Remove missing (store error)
/// - Authenticate without init (delegated)
/// - Attempt of granting a permission by non-owner
#[test_only]
module function_keys::function_keys_scenario_tests;

use function_keys::fk_store::{Self as store, make_func_key};
use function_keys::function_keys;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::hex;
use iota::ptb_command::{Self, Command};
use iota::test_scenario::{Self as scen, Scenario};
use iota::tx_context as txc;
use iotaccount::iotaccount::{Self, IOTAccount};
use std::ascii;

// ----------------------------------------------------------------------------
// Happy path (delegated): attach → grant(pubkey, function) → authenticate OK
// ----------------------------------------------------------------------------
#[test]
fun test_fk_authenticate_happy_path() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";

    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // TX 1: attach FK store + grant permission for this pub_key
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");

        function_keys::grant_permission(&mut account, user_public_key, fk, ctx);

        scen::return_shared(account);
    };

    // TX 2: exactly one matching MoveCall
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
            user_public_key, // delegated pubkey
            hex::encode(signature), // hex-encoded sig (like iotaccount tests)
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Happy path (owner): attach → grant(pubkey, function) → authenticate OK
// ----------------------------------------------------------------------------
#[test]
fun test_fk_authenticate_happy_path_owner() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);

    // TX 1
    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);
        let auth_ctx = create_auth_context_with_commands_for_testing(vector::empty<Command>());

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            owner_pk, // owner pubkey
            hex::encode(signature), // hex-encoded sig (like iotaccount tests)
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EUnauthorized)]
fun test_fk_authenticate_unauthorized() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // attach + allow withdraw only for this pub_key
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, owner_pk, fk, ctx);

        scen::return_shared(account);
    };

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
            user_public_key,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EInvalidAmountOfCommands)]
fun test_fk_authenticate_too_many_commands() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // attach + allow withdraw
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, owner_pk, fk, ctx);

        scen::return_shared(account);
    };

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
            user_public_key,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = store::EProgrammableMoveCallExpected)]
fun test_fk_authenticate_wrong_command() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // attach + allow withdraw
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, owner_pk, fk, ctx);

        scen::return_shared(account);
    };

    scenario.next_tx(account_address);
    {
        let digest = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";

        let account = scenario.take_shared<IOTAccount>();
        let ctx = create_tx_context_for_testing(account_address, digest);

        let mut cmds = vector::empty<Command>();
        vector::push_back(
            &mut cmds,
            make_publish_for_testing(),
        );
        let auth_ctx = create_auth_context_with_commands_for_testing(cmds);

        let signature =
            x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

        function_keys::authenticate(
            &account,
            user_public_key,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EUnauthorized)]
fun test_fk_revoke_then_fails() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    // attach, grant, revoke
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, user_public_key, fk, ctx);
        function_keys::revoke_permission(&mut account, user_public_key, &fk, ctx);

        scen::return_shared(account);
    };

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
            user_public_key,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Double add attempt → store::EFunctionKeyAlreadyAdded
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = store::EFunctionKeyAlreadyAdded)]
fun test_fk_double_add_should_fail() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        let fk = make_func_key(package_id.to_address(), b"wallet", b"withdraw");

        // First add OK
        function_keys::grant_permission(&mut account, user_public_key, fk, ctx);

        // Second add (same pubkey, same function) must fail
        function_keys::grant_permission(&mut account, user_public_key, fk, ctx);

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Remove missing → store::EFunctionKeyDoesNotExist
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = store::EFunctionKeyDoesNotExist)]
fun test_fk_remove_missing_should_fail() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let account_address = create_iotaccount_with_pk_for_testing(scenario, owner_pk);
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));

    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        let ctx = scen::ctx(scenario);

        // Prime the pubkey bucket with a different function
        let fk_granted = make_func_key(package_id.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut account, user_public_key, fk_granted, ctx);

        // Attempt to revoke a function that was never granted
        let fk_other = make_func_key(package_id.to_address(), b"wallet", b"deposit");
        function_keys::revoke_permission(&mut account, user_public_key, &fk_other, ctx);

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

// ----------------------------------------------------------------------------
// Authenticate without init → function_keys::EFunctionKeysNotInitialized (delegated)
// ----------------------------------------------------------------------------
#[test]
#[expected_failure(abort_code = function_keys::EFunctionKeysNotInitialized)]
fun test_fk_authenticate_without_init() {
    let mut scenario_val = scen::begin(@0x0);
    let scenario = &mut scenario_val;

    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let owner_pk = x"1ea6f0f467574295a2cd5d21a3fd3a712ade354d520d3bd0fe6088d7b7c2e00e";
    let account_address = create_iotaccount_for_testing_without_fk_store(
        scenario,
        option::some(owner_pk),
    );
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
            user_public_key,
            hex::encode(signature),
            &auth_ctx,
            &ctx,
        );

        scen::return_shared(account);
    };

    scen::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = iotaccount::ETransactionSenderIsNotTheAccount)]
fun test_fk_authenticate_unauthorized_granted_permission() {
    let mut sc = scen::begin(@0x0);
    let user_public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    let owner_pk = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
    create_iotaccount_with_pk_for_testing(&mut sc, owner_pk);

    // Try to grant from a different sender → must fail
    scen::next_tx(&mut sc, @0xA);
    {
        let mut acc = scen::take_shared<IOTAccount>(&sc);
        let ctx = scen::ctx(&mut sc);
        let pkg = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
        let fk = make_func_key(pkg.to_address(), b"wallet", b"withdraw");
        function_keys::grant_permission(&mut acc, user_public_key, fk, ctx);
        scen::return_shared(acc);
    };
    scen::end(sc);
}

// ============================================================================
// Utilities (mirroring iotaccount test style)
// ============================================================================

fun create_authenticator_function_ref_v1_for_testing(): AuthenticatorFunctionRefV1<IOTAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
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
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    function_keys::create(public_key, authenticator, ctx);

    scen::next_tx(scenario, @0x0);

    let account = scen::take_shared<IOTAccount>(scenario);
    let account_address = account.account_address();
    scen::return_shared(account);

    account_address
}

fun create_iotaccount_for_testing_without_fk_store(
    scenario: &mut Scenario,
    public_key: option::Option<vector<u8>>,
): address {
    let ctx = scen::ctx(scenario);

    let public_key = public_key.destroy_or!(public_key_for_testing());
    let authenticator = create_authenticator_function_ref_v1_for_testing();

    function_keys::create_without_fk_store(public_key, authenticator, ctx);

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
    auth_context::new_with_tx_inputs(vector::empty(), vector::empty(), cmds)
}

fun public_key_for_testing(): vector<u8> { b"42" }

/// Build a MoveCall `Command` aligned with `fk_store::extract_func_key`.
fun make_move_call_for_testing(
    pkg: ID,
    module_name: std::ascii::String,
    function_name: std::ascii::String,
): Command {
    ptb_command::new_move_call_command_for_testing(
        ptb_command::new_programmable_move_call_for_testing(
            pkg,
            module_name,
            function_name,
            vector[],
            vector[],
        ),
    )
}

/// Build a Publish `Command`.
fun make_publish_for_testing(): Command {
    ptb_command::new_publish_command_for_testing(
        ptb_command::new_publish_for_testing(vector[], vector[]),
    )
}
