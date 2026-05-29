// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module whitelist_sponsorship::whitelist_sponsorship_account_tests;

use iota::auth_context::AuthenticatorFunctionInfoV1;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::bcs;
use iota::ptb_call_arg;
use iota::ptb_command;
use iota::test_scenario::{Self, Scenario};
use std::ascii;
use std::unit_test::assert_eq;
use whitelist_sponsorship::whitelists;
use whitelist_sponsorship::whitelist_sponsorship_account::{Self, WhitelistSponsorshipAccount};

// === Constants ===

const ADMIN: address = @0xAD;
const SENDER: address = @0xBEEF;
const NON_ADMIN: address = @0xDEAD;
const SENDER_AUTH_FN_PKG: address = @0xCAFE;
const DIGEST: vector<u8> = x"0000000000000000000000000000000000000000000000000000000000000001";

// === Helpers ===

fun sender_module_name(): ascii::String { ascii::string(b"sender_module") }

fun sender_function_name(): ascii::String { ascii::string(b"sender_authenticator") }

fun whitelisted_sender_ref(): AuthenticatorFunctionRefV1<WhitelistSponsorshipAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing<WhitelistSponsorshipAccount>(
        SENDER_AUTH_FN_PKG,
        sender_module_name(),
        sender_function_name(),
    )
}

fun whitelisted_sender_info(): AuthenticatorFunctionInfoV1 {
    auth_context::create_auth_function_info_v1_for_testing(
        SENDER_AUTH_FN_PKG,
        sender_module_name(),
        sender_function_name(),
    )
}

/// Creates a `WhitelistSponsorshipAccount` and returns its address.
fun create_account_for_testing(scenario: &mut Scenario): address {
    let authenticator = authenticator_function::create_auth_function_ref_v1_for_testing<
        WhitelistSponsorshipAccount,
    >(
        @whitelist_sponsorship,
        ascii::string(b"whitelist_sponsorship_account"),
        ascii::string(b"authenticator"),
    );
    whitelist_sponsorship_account::create(ADMIN, authenticator, scenario.ctx());

    scenario.next_tx(ADMIN);
    let account = scenario.take_shared<WhitelistSponsorshipAccount>();
    let account_addr = account.account_address();
    test_scenario::return_shared(account);
    account_addr
}

/// Whitelists the sender authenticator function and sets a gas allowance for `SENDER`.
fun setup_admin_state(scenario: &mut Scenario, sender_allowance: u64) {
    scenario.next_tx(ADMIN);
    let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();
    account.add_authenticator_function(whitelisted_sender_ref(), scenario.ctx());
    account.add_user_gas_allowance(SENDER, sender_allowance, scenario.ctx());
    test_scenario::return_shared(account);
}

fun make_tx_ctx_for_testing(
    sender: address,
    sponsor: Option<address>,
    gas_budget: u64,
): TxContext {
    tx_context::create(sender, DIGEST, 0, 0, 0, 1, 1, gas_budget, sponsor)
}

/// Builds an `AuthContext` containing a single PTB command that calls
/// `deduct_user_gas_allowance(sponsor_addr, sender_addr, deduct_amount)`.
fun make_auth_ctx_for_testing(
    sponsor_addr: address,
    sender_addr: address,
    deduct_amount: u64,
    sender_info: Option<AuthenticatorFunctionInfoV1>,
): AuthContext {
    let account_obj_arg = ptb_call_arg::new_object_arg_shared_for_testing(
        object::id_from_address(sponsor_addr),
        0,
        true,
    );
    let account_call_arg = ptb_call_arg::new_call_arg_object_for_testing(account_obj_arg);
    let user_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(bcs::to_bytes(&sender_addr));
    let amount_call_arg = ptb_call_arg::new_call_arg_pure_for_testing(bcs::to_bytes(&deduct_amount));
    let inputs = vector[account_call_arg, user_call_arg, amount_call_arg];

    let deduct_call = ptb_command::new_programmable_move_call_for_testing(
        object::id_from_address(@whitelist_sponsorship),
        ascii::string(b"whitelist_sponsorship_account"),
        ascii::string(b"deduct_user_gas_allowance"),
        vector[],
        vector[
            ptb_command::new_input_argument_for_testing(0),
            ptb_command::new_input_argument_for_testing(1),
            ptb_command::new_input_argument_for_testing(2),
        ],
    );
    let commands = vector[ptb_command::new_move_call_command_for_testing(deduct_call)];

    auth_context::new_for_testing(
        DIGEST,
        inputs,
        commands,
        vector[],
        DIGEST,
        option::some(DIGEST),
        sender_info,
        option::none(),
    )
}

// --------------------------------------- Creation & Admin ---------------------------------------

#[test]
fun account_creation_succeeds() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;

    let account_addr = create_account_for_testing(scenario);

    scenario.next_tx(ADMIN);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        assert_eq!(account.account_address(), account_addr);
        assert_eq!(account.borrow_admin(), ADMIN);
        assert!(account.has_whitelists());
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun admin_can_add_and_remove_authenticator_function() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    create_account_for_testing(scenario);

    let key = whitelists::new_authenticator_function_key(
        object::id_from_address(SENDER_AUTH_FN_PKG),
        sender_module_name(),
        sender_function_name(),
    );

    scenario.next_tx(ADMIN);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();

        account.add_authenticator_function(whitelisted_sender_ref(), scenario.ctx());
        assert!(account.borrow_authenticator_functions_whitelist().contains(key));

        let ref = whitelisted_sender_ref();
        account.remove_authenticator_function(&ref, scenario.ctx());
        assert!(!account.borrow_authenticator_functions_whitelist().contains(key));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
fun admin_can_add_rotate_and_remove_user_gas_allowance() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    create_account_for_testing(scenario);

    scenario.next_tx(ADMIN);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();

        account.add_user_gas_allowance(SENDER, 1000, scenario.ctx());
        assert_eq!(*account.borrow_user_gas_allowances().borrow(SENDER), 1000);

        let prev = account.rotate_user_gas_allowance(SENDER, 500, scenario.ctx());
        assert_eq!(prev, 1000);
        assert_eq!(*account.borrow_user_gas_allowances().borrow(SENDER), 500);

        let removed = account.remove_user_gas_allowance(SENDER, scenario.ctx());
        assert_eq!(removed, 500);
        assert!(!account.borrow_user_gas_allowances().contains(SENDER));

        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::ENotAdmin)]
fun non_admin_cannot_add_user_gas_allowance() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    create_account_for_testing(scenario);

    scenario.next_tx(NON_ADMIN);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();
        account.add_user_gas_allowance(SENDER, 1000, scenario.ctx());
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Authenticator ---------------------------------------

#[test]
fun authenticator_passes_with_correct_setup() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);
    setup_admin_state(scenario, 5000);

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::some(account_addr), 1000);
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            1000,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::ENotASponsoredTransaction)]
fun authenticator_fails_if_not_sponsored() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);
    setup_admin_state(scenario, 5000);

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        // No sponsor.
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::none(), 1000);
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            1000,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::EAuthenticatorFunctionNotWhitelisted)]
fun authenticator_fails_if_sender_auth_fn_not_whitelisted() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);

    // Admin only sets the allowance; the sender auth fn is NOT whitelisted.
    scenario.next_tx(ADMIN);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();
        account.add_user_gas_allowance(SENDER, 5000, scenario.ctx());
        test_scenario::return_shared(account);
    };

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::some(account_addr), 1000);
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            1000,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::EUserGasAllowanceMissing)]
fun authenticator_fails_if_sender_has_no_allowance() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);

    // Admin only whitelists the auth fn; no allowance for SENDER.
    scenario.next_tx(ADMIN);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();
        account.add_authenticator_function(whitelisted_sender_ref(), scenario.ctx());
        test_scenario::return_shared(account);
    };

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::some(account_addr), 1000);
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            1000,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::EGasBudgetExceedsAllowance)]
fun authenticator_fails_if_gas_budget_exceeds_allowance() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);
    setup_admin_state(scenario, 500); // allowance < gas budget

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::some(account_addr), 1000);
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            1000,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = whitelist_sponsorship_account::EInsufficientAllowanceDeducted)]
fun authenticator_fails_if_ptb_under_deducts() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    let account_addr = create_account_for_testing(scenario);
    setup_admin_state(scenario, 5000);

    scenario.next_tx(SENDER);
    {
        let account = scenario.take_shared<WhitelistSponsorshipAccount>();
        let test_ctx = make_tx_ctx_for_testing(SENDER, option::some(account_addr), 1000);
        // Deduct only 500 in the PTB while the gas budget is 1000.
        let auth_ctx = make_auth_ctx_for_testing(
            account_addr,
            SENDER,
            500,
            option::some(whitelisted_sender_info()),
        );
        whitelist_sponsorship_account::authenticator(&account, &auth_ctx, &test_ctx);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}

// --------------------------------------- Deduct ---------------------------------------

#[test]
fun deduct_user_gas_allowance_reduces_allowance() {
    let mut scenario_val = test_scenario::begin(ADMIN);
    let scenario = &mut scenario_val;
    create_account_for_testing(scenario);
    setup_admin_state(scenario, 1000);

    scenario.next_tx(SENDER);
    {
        let mut account = scenario.take_shared<WhitelistSponsorshipAccount>();
        account.deduct_user_gas_allowance(SENDER, 300, scenario.ctx());
        assert_eq!(*account.borrow_user_gas_allowances().borrow(SENDER), 700);
        test_scenario::return_shared(account);
    };

    test_scenario::end(scenario_val);
}
