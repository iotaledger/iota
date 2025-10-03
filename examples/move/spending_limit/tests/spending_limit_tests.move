// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module spending_limit::spending_limit_tests;

use account_template::account_template::IOTAccount;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::coin;
use iota::test_scenario::{Self, Scenario};
use std::ascii;
use std::unit_test::assert_eq;
use spending_limit::spending_limit;

// --------------------------------------- Basic Tests ---------------------------------------

#[test]
fun test_account_creation() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    
    let limit = 1000u64;
    let gas_amount = 500u64;
    let account_address = create_spending_limit_account(scenario, limit, option::none(), gas_amount);
    
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        
        // Verify spending state is initialized
        let (stored_limit, used, _epoch) = spending_limit::get_spending_state(&account);
        assert_eq!(stored_limit, limit);
        assert_eq!(used, 0);
        
        // Verify gas reserve is initialized
        assert_eq!(spending_limit::get_gas_reserve_balance(&account), gas_amount);
        
        test_scenario::return_shared(account);
    };
    
    test_scenario::end(scenario_val);
}

#[test]
fun test_spending_within_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    
    let limit = 1000u64;
    let account_address = create_spending_limit_account(scenario, limit, option::none(), 500);
    
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        
        // Spend 300 (within limit of 1000)
        let effects = spending_limit::new_tx_effects(300);
        spending_limit::post_execution(&mut account, &effects, scenario.ctx());
        
        // Verify spending was recorded
        let (_, used, _) = spending_limit::get_spending_state(&account);
        assert_eq!(used, 300);
        
        test_scenario::return_shared(account);
    };
    
    test_scenario::end(scenario_val);
}

#[test]
#[expected_failure(abort_code = spending_limit::EOverLimit)]
fun test_spending_exceeds_limit() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    
    let limit = 1000u64;
    let account_address = create_spending_limit_account(scenario, limit, option::none(), 500);
    
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        
        // Try to spend 1500 (exceeds limit of 1000)
        let effects = spending_limit::new_tx_effects(1500);
        spending_limit::post_execution(&mut account, &effects, scenario.ctx());
        
        test_scenario::return_shared(account);
    };
    
    test_scenario::end(scenario_val);
}

#[test]
fun test_capacity_check() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    
    let limit = 1000u64;
    let account_address = create_spending_limit_account(scenario, limit, option::none(), 500);
    
    scenario.next_tx(account_address);
    {
        let account = scenario.take_shared<IOTAccount>();
        
        // Check capacity with 20% safety margin
        let can_spend_500 = spending_limit::check_spending_capacity(&account, 500, 20, scenario.ctx());
        assert!(can_spend_500); // 500 + 20% = 600, which is < 1000
        
        let can_spend_900 = spending_limit::check_spending_capacity(&account, 900, 20, scenario.ctx());
        assert!(!can_spend_900); // 900 + 20% = 1080, which exceeds 1000
        
        test_scenario::return_shared(account);
    };
    
    test_scenario::end(scenario_val);
}

#[test]
fun test_gas_reserve_operations() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let public_key = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";

    let account_address = create_spending_limit_account(scenario, 1000, option::some(public_key), 500);
    
    scenario.next_tx(account_address);
    {
        let mut account = scenario.take_shared<IOTAccount>();
        
        // Deposit more gas
        let additional_gas = coin::mint_for_testing<iota::iota::IOTA>(300, scenario.ctx());
        spending_limit::deposit_to_gas_reserve(&mut account, additional_gas, scenario.ctx());
        
        assert_eq!(spending_limit::get_gas_reserve_balance(&account), 800);
        
        // Withdraw some gas
        let withdrawn = spending_limit::withdraw_from_gas_reserve(&mut account, 200, scenario.ctx());
        coin::burn_for_testing(withdrawn);
        
        assert_eq!(spending_limit::get_gas_reserve_balance(&account), 600);
        
        test_scenario::return_shared(account);
    };
    
    test_scenario::end(scenario_val);
}

// --------------------------------------- Test Utilities ---------------------------------------

fun create_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"spending_limit"),
        ascii::string(b"authenticate"),
    )
}

fun create_spending_limit_account(
    scenario: &mut Scenario,
    limit: u64,
    public_key: Option<vector<u8>>,
    gas_amount: u64
): address {
    let public_key = public_key.destroy_or!(public_key_for_testing());
    let authenticator = create_authenticator_info_v1_for_testing();
    
    let ctx = test_scenario::ctx(scenario);
    let gas_coin = coin::mint_for_testing<iota::iota::IOTA>(gas_amount, ctx);
    
    // Pass gas_coin directly to create
    spending_limit::create(limit, gas_coin, public_key, authenticator, ctx);
    
    // Get account address in next transaction
    scenario.next_tx(@0x0);
    let account = scenario.take_shared<IOTAccount>();
    let account_address = account.get_address();
    test_scenario::return_shared(account);
    
    account_address
}





fun public_key_for_testing(): vector<u8> {
    b"24"
}