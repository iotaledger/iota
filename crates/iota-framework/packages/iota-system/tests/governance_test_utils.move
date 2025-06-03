// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::governance_test_utils;

use iota::address;
use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::iota::{Self, IOTA};
use iota::system_admin_cap;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{Self, assert_eq};
use iota_system::iota_system::{Self, IotaSystemState};
use iota_system::iota_system_state_inner;
use iota_system::staking_pool::{StakedIota, StakingPoolV1};
use iota_system::validator::{Self, ValidatorV1};

const NANOS_PER_IOTA: u64 = 1_000_000_000;

public fun create_validator_for_testing(
    addr: address,
    init_stake_amount_in_iota: u64,
    ctx: &mut TxContext,
): ValidatorV1 {
    let validator = validator::new_for_testing(
        addr,
        x"AA",
        x"BB",
        x"CC",
        x"DD",
        b"ValidatorName",
        b"description",
        b"image_url",
        b"project_url",
        b"/ip4/127.0.0.1/tcp/80",
        b"/ip4/127.0.0.1/udp/80",
        b"/ip4/127.0.0.1/udp/80",
        option::some(balance::create_for_testing<IOTA>(init_stake_amount_in_iota * NANOS_PER_IOTA)),
        1,
        0,
        true,
        ctx,
    );
    validator
}

/// Create a validator set with the given stake amounts
public fun create_validators_with_stakes(
    stakes: vector<u64>,
    ctx: &mut TxContext,
): (vector<u64>, vector<ValidatorV1>) {
    let mut i = 0;
    let mut validators = vector[];
    let mut committee_members = vector[];

    while (i < stakes.length()) {
        let validator = create_validator_for_testing(address::from_u256(i as u256), stakes[i], ctx);
        validators.push_back(validator);
        committee_members.push_back(i);
        i = i + 1
    };

    (committee_members, validators)
}

public fun create_iota_system_state_for_testing(
    validators: vector<ValidatorV1>,
    iota_supply_amount: u64,
    storage_fund_amount: u64,
    ctx: &mut TxContext,
) {
    let system_parameters = iota_system_state_inner::create_system_parameters(
        42, // epoch_duration_ms, doesn't matter what number we put here
        150, // max_validator_count
        1, // min_validator_joining_stake
        1, // validator_low_stake_threshold
        0, // validator_very_low_stake_threshold
        7, // validator_low_stake_grace_period
        ctx,
    );

    let mut iota_treasury_cap = iota::create_for_testing(ctx);

    // We mint the given amount so the system appears to have a total supply of iota_supply_amount,
    // but we burn it since we cannot put it anywhere.
    let iota_total_supply_balance = iota_treasury_cap.mint_balance(
        iota_supply_amount * NANOS_PER_IOTA,
        ctx,
    );
    iota_total_supply_balance.destroy_for_testing();

    let storage_fund = iota_treasury_cap.mint_balance(
        storage_fund_amount * NANOS_PER_IOTA,
        ctx,
    );

    iota_system::create(
        object::new(ctx), // it doesn't matter what ID iota system state has in tests
        iota_treasury_cap,
        validators,
        storage_fund, // storage_fund
        1, // protocol version
        0, // chain_start_timestamp_ms
        system_parameters,
        system_admin_cap::new_system_admin_cap_for_testing(),
        ctx,
    )
}

public fun set_up_iota_system_state(mut addrs: vector<address>) {
    let mut scenario = test_scenario::begin(@0x0);
    let ctx = scenario.ctx();
    let mut validators = vector[];

    while (!addrs.is_empty()) {
        validators.push_back(
            create_validator_for_testing(addrs.pop_back(), 100, ctx),
        );
    };

    create_iota_system_state_for_testing(validators, 1000, 0, ctx);
    scenario.end();
}

public fun advance_epoch(scenario: &mut Scenario) {
    advance_epoch_with_balanced_reward_amounts(0, 0, scenario);
}

public fun advance_epoch_with_max_committee_members_count(
    max_committee_members_count: u64,
    scenario: &mut Scenario,
) {
    let storage_rebate = advance_epoch_with_reward_amounts_return_rebate_and_max_committee_members_count(
        0,
        0,
        0,
        0,
        0,
        0,
        max_committee_members_count,
        scenario,
    );
    test_utils::destroy(storage_rebate)
}

public fun advance_epoch_with_reward_amounts_return_rebate(
    validator_subsidy: u64,
    storage_charge: u64,
    computation_charge: u64,
    computation_charge_burned: u64,
    storage_rebate: u64,
    non_refundable_storage_rebate: u64,
    scenario: &mut Scenario,
): Balance<IOTA> {
    // Use the same value as the default value of max_active_validators.
    let max_committee_members_count = 150;

    advance_epoch_with_reward_amounts_return_rebate_and_max_committee_members_count(
        validator_subsidy,
        storage_charge,
        computation_charge,
        computation_charge_burned,
        storage_rebate,
        non_refundable_storage_rebate,
        max_committee_members_count,
        scenario,
    )
}

public fun advance_epoch_with_reward_amounts_return_rebate_and_max_committee_members_count(
    validator_subsidy: u64,
    storage_charge: u64,
    computation_charge: u64,
    computation_charge_burned: u64,
    storage_rebate: u64,
    non_refundable_storage_rebate: u64,
    max_committee_members_count: u64,
    scenario: &mut Scenario,
): Balance<IOTA> {
    scenario.next_tx(@0x0);
    let new_epoch = scenario.ctx().epoch() + 1;
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();

    let storage_rebate = system_state.advance_epoch_for_testing(
        new_epoch,
        1,
        validator_subsidy,
        storage_charge,
        computation_charge,
        computation_charge_burned,
        storage_rebate,
        non_refundable_storage_rebate,
        0,
        0,
        max_committee_members_count,
        ctx,
    );
    test_scenario::return_shared(system_state);
    scenario.next_epoch(@0x0);
    storage_rebate
}

/// Advances the epoch with the given storage charge and setting validator_subsidy, computation charge and computation charge burned all equal to the specified amount.
public fun advance_epoch_with_balanced_reward_amounts(
    storage_charge: u64,
    computation_charge_and_subsidy_amount: u64,
    scenario: &mut Scenario,
) {
    advance_epoch_with_amounts(
        computation_charge_and_subsidy_amount,
        storage_charge,
        computation_charge_and_subsidy_amount,
        computation_charge_and_subsidy_amount,
        scenario,
    )
}

/// Advances the epoch with the given storage charge and setting validator_subsidy, computation charge and computation charge burned all equal to the specified amount.
public fun advance_epoch_with_balanced_reward_amounts_and_max_committee_size(
    storage_charge: u64,
    computation_charge_and_subsidy_amount: u64,
    max_committee_members_count: u64,
    scenario: &mut Scenario,
) {
    let storage_rebate = advance_epoch_with_reward_amounts_return_rebate_and_max_committee_members_count(
        computation_charge_and_subsidy_amount * NANOS_PER_IOTA,
        storage_charge * NANOS_PER_IOTA,
        computation_charge_and_subsidy_amount * NANOS_PER_IOTA,
        computation_charge_and_subsidy_amount * NANOS_PER_IOTA,
        0,
        0,
        max_committee_members_count,
        scenario,
    );

    test_utils::destroy(storage_rebate);
}

/// Advances the epoch with the given validator subsidy, storage charge, computation charge and computation charge burned amounts.
public fun advance_epoch_with_amounts(
    validator_subsidy: u64,
    storage_charge: u64,
    computation_charge: u64,
    computation_charge_burned: u64,
    scenario: &mut Scenario,
) {
    let storage_rebate = advance_epoch_with_reward_amounts_return_rebate(
        validator_subsidy * NANOS_PER_IOTA,
        storage_charge * NANOS_PER_IOTA,
        computation_charge * NANOS_PER_IOTA,
        computation_charge_burned * NANOS_PER_IOTA,
        0,
        0,
        scenario,
    );
    test_utils::destroy(storage_rebate)
}

public fun advance_epoch_with_reward_amounts_and_slashing_rates(
    storage_charge: u64,
    computation_charge: u64,
    reward_slashing_rate: u64,
    scenario: &mut Scenario,
) {
    scenario.next_tx(@0x0);
    let new_epoch = scenario.ctx().epoch() + 1;
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();

    let validator_subsidy = computation_charge;

    // Use the same value as the default value of max_active_validators.
    let max_committee_members_count = 150;

    let storage_rebate = system_state.advance_epoch_for_testing(
        new_epoch,
        1,
        validator_subsidy * NANOS_PER_IOTA,
        storage_charge * NANOS_PER_IOTA,
        computation_charge * NANOS_PER_IOTA,
        computation_charge * NANOS_PER_IOTA,
        0,
        0,
        reward_slashing_rate,
        0,
        max_committee_members_count,
        ctx,
    );
    test_utils::destroy(storage_rebate);
    test_scenario::return_shared(system_state);
    scenario.next_epoch(@0x0);
}

public fun stake_with(staker: address, validator: address, amount: u64, scenario: &mut Scenario) {
    scenario.next_tx(staker);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();

    system_state.request_add_stake(
        coin::mint_for_testing(amount * NANOS_PER_IOTA, ctx),
        validator,
        ctx,
    );
    test_scenario::return_shared(system_state);
}

public fun unstake(staker: address, staked_iota_idx: u64, scenario: &mut Scenario) {
    scenario.next_tx(staker);
    let stake_iota_ids = scenario.ids_for_sender<StakedIota>();
    let staked_iota = scenario.take_from_sender_by_id(stake_iota_ids[staked_iota_idx]);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();
    system_state.request_withdraw_stake(staked_iota, ctx);
    test_scenario::return_shared(system_state);
}

public fun add_validator_full_flow(
    validator: address,
    name: vector<u8>,
    net_addr: vector<u8>,
    init_stake_amount: u64,
    pubkey: vector<u8>,
    pop: vector<u8>,
    scenario: &mut Scenario,
) {
    scenario.next_tx(validator);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();

    system_state.request_add_validator_candidate(
        pubkey,
        vector[
            171,
            2,
            39,
            3,
            139,
            105,
            166,
            171,
            153,
            151,
            102,
            197,
            151,
            186,
            140,
            116,
            114,
            90,
            213,
            225,
            20,
            167,
            60,
            69,
            203,
            12,
            180,
            198,
            9,
            217,
            117,
            38,
        ],
        vector[
            171,
            3,
            39,
            3,
            139,
            105,
            166,
            171,
            153,
            151,
            102,
            197,
            151,
            186,
            140,
            116,
            114,
            90,
            213,
            225,
            20,
            167,
            60,
            69,
            203,
            12,
            180,
            198,
            9,
            217,
            117,
            38,
        ],
        pop,
        name,
        b"description",
        b"image_url",
        b"project_url",
        net_addr,
        net_addr,
        net_addr,
        1,
        0,
        ctx,
    );
    system_state.request_add_stake(
        coin::mint_for_testing<IOTA>(init_stake_amount * NANOS_PER_IOTA, ctx),
        validator,
        ctx,
    );
    system_state.request_add_validator_for_testing(0, ctx);
    test_scenario::return_shared(system_state);
}

public fun add_validator_candidate(
    validator: address,
    name: vector<u8>,
    net_addr: vector<u8>,
    pubkey: vector<u8>,
    pop: vector<u8>,
    scenario: &mut Scenario,
) {
    scenario.next_tx(validator);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();

    system_state.request_add_validator_candidate(
        pubkey,
        vector[
            171,
            2,
            39,
            3,
            139,
            105,
            166,
            171,
            153,
            151,
            102,
            197,
            151,
            186,
            140,
            116,
            114,
            90,
            213,
            225,
            20,
            167,
            60,
            69,
            203,
            12,
            180,
            198,
            9,
            217,
            117,
            38,
        ],
        vector[
            171,
            3,
            39,
            3,
            139,
            105,
            166,
            171,
            153,
            151,
            102,
            197,
            151,
            186,
            140,
            116,
            114,
            90,
            213,
            225,
            20,
            167,
            60,
            69,
            203,
            12,
            180,
            198,
            9,
            217,
            117,
            38,
        ],
        pop,
        name,
        b"description",
        b"image_url",
        b"project_url",
        net_addr,
        net_addr,
        net_addr,
        1,
        0,
        ctx,
    );
    test_scenario::return_shared(system_state);
}

public fun remove_validator_candidate(validator: address, scenario: &mut Scenario) {
    scenario.next_tx(validator);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();

    system_state.request_remove_validator_candidate(ctx);
    test_scenario::return_shared(system_state);
}

public fun add_validator(validator: address, scenario: &mut Scenario) {
    scenario.next_tx(validator);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();

    system_state.request_add_validator_for_testing(0, ctx);
    test_scenario::return_shared(system_state);
}

public fun remove_validator(validator: address, scenario: &mut Scenario) {
    scenario.next_tx(validator);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();

    system_state.request_remove_validator(ctx);
    test_scenario::return_shared(system_state);
}

public fun assert_validator_self_stake_amounts(
    validator_addrs: vector<address>,
    stake_amounts: vector<u64>,
    scenario: &mut Scenario,
) {
    let mut i = 0;
    while (i < validator_addrs.length()) {
        let validator_addr = validator_addrs[i];
        let amount = stake_amounts[i];
        scenario.next_tx(validator_addr);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let stake_plus_rewards = stake_plus_current_rewards_for_validator(
            validator_addr,
            &mut system_state,
            scenario,
        );
        assert_eq(stake_plus_rewards, amount);
        test_scenario::return_shared(system_state);
        i = i + 1;
    };
}

public fun assert_validator_total_stake_amounts(
    validator_addrs: vector<address>,
    stake_amounts: vector<u64>,
    scenario: &mut Scenario,
) {
    let mut i = 0;
    while (i < validator_addrs.length()) {
        let validator_addr = validator_addrs[i];
        let amount = stake_amounts[i];
        scenario.next_tx(validator_addr);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let validator_amount = system_state.validator_stake_amount(validator_addr);
        assert_eq(validator_amount, amount);
        test_scenario::return_shared(system_state);
        i = i + 1;
    };
}

public fun assert_validator_non_self_stake_amounts(
    validator_addrs: vector<address>,
    stake_amounts: vector<u64>,
    scenario: &mut Scenario,
) {
    let mut i = 0;
    while (i < validator_addrs.length()) {
        let validator_addr = validator_addrs[i];
        let amount = stake_amounts[i];
        scenario.next_tx(validator_addr);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let non_self_stake_amount =
            system_state.validator_stake_amount(validator_addr) - stake_plus_current_rewards_for_validator(validator_addr, &mut system_state, scenario);
        assert_eq(non_self_stake_amount, amount);
        test_scenario::return_shared(system_state);
        i = i + 1;
    };
}

public fun assert_validator_candidate_total_stake_amounts(
    validator_addrs: vector<address>,
    stake_amounts: vector<u64>,
    scenario: &mut Scenario,
) {
    let mut i = 0;
    while (i < validator_addrs.length()) {
        let validator_addr = validator_addrs[i];
        let amount = stake_amounts[i];
        scenario.next_tx(validator_addr);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let validator_amount = system_state
            .validators()
            .get_candidate_validator_ref(validator_addr)
            .total_stake_amount();
        assert_eq(validator_amount, amount);
        test_scenario::return_shared(system_state);
        i = i + 1;
    };
}

/// Return the rewards for the validator at `addr` in terms of IOTA.
public fun stake_plus_current_rewards_for_validator(
    addr: address,
    system_state: &mut IotaSystemState,
    scenario: &mut Scenario,
): u64 {
    let validator_ref = system_state.validators().get_active_validator_ref_inner(addr);
    let amount = stake_plus_current_rewards(addr, validator_ref.get_staking_pool_ref(), scenario);
    amount
}

public fun stake_plus_current_rewards(
    addr: address,
    staking_pool: &StakingPoolV1,
    scenario: &mut Scenario,
): u64 {
    let mut sum = 0;
    scenario.next_tx(addr);
    let mut stake_ids = scenario.ids_for_sender<StakedIota>();
    let current_epoch = scenario.ctx().epoch();

    while (!stake_ids.is_empty()) {
        let staked_iota_id = stake_ids.pop_back();
        let staked_iota = scenario.take_from_sender_by_id<StakedIota>(staked_iota_id);
        sum = sum + staking_pool.calculate_rewards(&staked_iota, current_epoch);
        scenario.return_to_sender(staked_iota);
    };
    sum
}

public fun total_iota_balance(addr: address, scenario: &mut Scenario): u64 {
    let mut sum = 0;
    scenario.next_tx(addr);
    let coin_ids = scenario.ids_for_sender<Coin<IOTA>>();
    let mut i = 0;
    while (i < coin_ids.length()) {
        let coin = scenario.take_from_sender_by_id<Coin<IOTA>>(coin_ids[i]);
        sum = sum + coin.value();
        scenario.return_to_sender(coin);
        i = i + 1;
    };
    sum
}

/// Returns the total IOTA supply in the system state.
public fun total_supply(scenario: &mut Scenario): u64 {
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let total_supply = system_state.get_total_iota_supply();
    test_scenario::return_shared(system_state);
    total_supply
}
