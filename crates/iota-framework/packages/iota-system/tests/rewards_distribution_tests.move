// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::rewards_distribution_tests;

use iota::address;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, destroy};
use iota_system::governance_test_utils::{
    advance_epoch,
    advance_epoch_with_balanced_reward_amounts,
    advance_epoch_with_balanced_reward_amounts_and_max_committee_size,
    advance_epoch_with_max_committee_members_count,
    advance_epoch_with_reward_amounts_return_rebate,
    advance_epoch_with_reward_amounts_and_slashing_rates,
    advance_epoch_with_amounts,
    advance_epoch_with_subsidy_and_scores,
    assert_validator_total_stake_amounts,
    assert_validator_non_self_stake_amounts,
    assert_validator_self_stake_amounts,
    create_validator_for_testing,
    create_validators_with_stakes_and_commission_rates,
    create_iota_system_state_for_testing,
    stake_with,
    total_iota_balance,
    total_supply,
    unstake,
    assert_equal_approx
};
use iota_system::iota_system::IotaSystemState;
use iota_system::staking_pool::StakedIota;
use iota_system::validator_cap::UnverifiedValidatorOperationCap;

const VALIDATOR_ADDR_1: address = @0x1;
const VALIDATOR_ADDR_2: address = @0x2;
const VALIDATOR_ADDR_3: address = @0x3;
const VALIDATOR_ADDR_4: address = @0x4;
const VALIDATOR_ADDR_5: address = @0x5;

const STAKER_ADDR_1: address = @0x42;
const STAKER_ADDR_2: address = @0x43;
const STAKER_ADDR_3: address = @0x44;
const STAKER_ADDR_4: address = @0x45;

const NANOS_PER_IOTA: u64 = 1_000_000_000;

#[test]
fun test_validator_rewards() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting..
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    advance_epoch_with_balanced_reward_amounts(0, 100, scenario);

    // Rewards of 100 IOTA are split evenly between the validators.
    // => +25 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 25) * NANOS_PER_IOTA,
            (200 + 25) * NANOS_PER_IOTA,
            (300 + 25) * NANOS_PER_IOTA,
            (400 + 25) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    stake_with(VALIDATOR_ADDR_2, VALIDATOR_ADDR_2, 720, scenario);

    advance_epoch(scenario);

    advance_epoch_with_balanced_reward_amounts(0, 100, scenario);

    // Even though validator 2 has a lot more stake now, it should not get more rewards because
    // the voting power is capped at 10%.
    // Rewards of 100 IOTA are split evenly between the validators.
    // => +25 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (125 + 25) * NANOS_PER_IOTA,
            (225 + 720 + 25) * NANOS_PER_IOTA,
            (325 + 25) * NANOS_PER_IOTA,
            (425 + 25) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_rewards_with_big_amounts() {
    set_up_iota_system_state_with_big_amounts();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100_000_000 * NANOS_PER_IOTA,
            200_000_000 * NANOS_PER_IOTA,
            300_000_000 * NANOS_PER_IOTA,
            400_000_000 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    advance_epoch_with_balanced_reward_amounts(0, 100, scenario);

    // Rewards of 100 IOTA are split evenly between the validators.
    // => +25 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100_000_000 + 25) * NANOS_PER_IOTA,
            (200_000_000 + 25) * NANOS_PER_IOTA,
            (300_000_000 + 25) * NANOS_PER_IOTA,
            (400_000_000 + 25) * NANOS_PER_IOTA,
        ],
        scenario,
    );
    scenario_val.end();
}

#[test]
fun test_validator_subsidy_no_supply_change() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let prev_supply = total_supply(scenario);

    let validator_subsidy = 100;
    let computation_charge = 100;
    let computation_charge_burned = 100;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    let new_supply = total_supply(scenario);

    // Since the validator subsidy and computation charge are the same, no new tokens should
    // have been minted, so the supply should stay constant.
    assert!(prev_supply == new_supply, 0);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_deflation() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let prev_supply = total_supply(scenario);

    let validator_subsidy = 60;
    let computation_charge = 100;
    let computation_charge_burned = 100;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    let new_supply = total_supply(scenario);

    // The difference between computation charge burned and validator subsidy should have been burned.
    assert_eq(
        prev_supply - (computation_charge_burned - validator_subsidy) * NANOS_PER_IOTA,
        new_supply,
    );

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_inflation() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let prev_supply = total_supply(scenario);

    let validator_subsidy = 100;
    let computation_charge = 60;
    let computation_charge_burned = 60;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    let new_supply = total_supply(scenario);

    // The difference between validator subsidy and computation charge burned should have been minted.
    assert_eq(
        prev_supply + (validator_subsidy - computation_charge_burned) * NANOS_PER_IOTA,
        new_supply,
    );

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_higher_than_computation_charge() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
    advance_epoch_with_amounts(800, 0, 400, 400, scenario);

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // => +200 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 200) * NANOS_PER_IOTA,
            (200 + 200) * NANOS_PER_IOTA,
            (300 + 200) * NANOS_PER_IOTA,
            (400 + 200) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The validator's commission (25% according to IIP-8) is received as a separate StakedIota.
    // Need to unstake both the original stake and the commission to get the full amount.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Validator 1 should get the entire reward of 200 plus its initially staked 100 IOTA.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+200) * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_lower_than_computation_charge() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The computation charge is higher than the validator subsidy, so 200 IOTA should be burned.
    advance_epoch_with_amounts(800, 0, 1000, 1000, scenario);

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // => +200 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 200) * NANOS_PER_IOTA,
            (200 + 200) * NANOS_PER_IOTA,
            (300 + 200) * NANOS_PER_IOTA,
            (400 + 200) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The validator's commission (25% according to IIP-8) is received as a separate StakedIota.
    // Need to unstake both the original stake and the commission to get the full amount.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Validator 1 should get the entire reward of 200 plus its initially staked 100 IOTA.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+200) * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_higher_than_computation_charge_with_commission() {
    // Use 25 equal-stake validators (1000 IOTA each) so each has VP ≈ 400 bp ≈ 4%.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let stakes = vector::tabulate!(25, |_| 1000);
    let commission_rates = vector::tabulate!(25, |_| 0);
    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        stakes,
        commission_rates,
        ctx,
    );
    create_iota_system_state_for_testing(validators, 100000, 0, ctx);

    scenario_val.next_tx(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // With staker stakes added, V1 will have VP ≈ 437 bp, V2 ≈ 417 bp.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 50, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[(1000 + 100) * NANOS_PER_IOTA, (1000 + 50) * NANOS_PER_IOTA],
        scenario,
    );

    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 500, scenario); // 5% commission

    // 800 IOTA total reward, distributed proportionally to VP across 25 validators.
    // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
    // V1 effective commission with IIP-8 = max(5%, ~4.37%) = 5%.
    // V2 effective commission with IIP-8 = max(0%, ~4.17%) = ~4.17%.
    advance_epoch_with_amounts(800, 0, 400, 400, scenario);

    // V1: 1100 + 35.04 reward = 1135.04 IOTA.
    // V2: 1050 + 33.44 reward = 1083.44 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1_135_040_000_000, 1_083_440_000_000],
        scenario,
    );

    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // S1 gets its share of V1's pool reward after 5% commission (≈ 3.03 IOTA reward).
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 103_026_181_818);
    // S2 gets its share of V2's pool reward after ~4.17% IIP-8 commission (≈ 1.53 IOTA reward).
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 51_525_819_428);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_higher_than_computation_charge_with_tips() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // need to advance epoch so validator's staking starts counting
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    let validator_subsidy = 800;
    let computation_charge = 500; // 100 IOTA tips
    let computation_charge_burned = 400;

    // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // total reward is validator subsidy (800) + tips (100) = 900
    // => +225 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 225) * NANOS_PER_IOTA,
            (200 + 225) * NANOS_PER_IOTA,
            (300 + 225) * NANOS_PER_IOTA,
            (400 + 225) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The validator's commission (25% according to IIP-8) is received as a separate StakedIota.
    // Need to unstake both the original stake and the commission to get the full amount.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Validator 1 should get the entire reward of 225 plus its initially staked 100 IOTA.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+225) * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_lower_than_computation_charge_with_tips() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // need to advance epoch so validator's staking starts counting
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    let validator_subsidy = 800;
    let computation_charge = 1100; // 100 IOTA tips
    let computation_charge_burned = 1000;

    // The computation charge is higher than the validator subsidy, so 200 IOTA should be burned.
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // total reward is validator subsidy (800) + tips (100) = 900
    // => +225 IOTA for each validator
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 225) * NANOS_PER_IOTA,
            (200 + 225) * NANOS_PER_IOTA,
            (300 + 225) * NANOS_PER_IOTA,
            (400 + 225) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // The validator's commission (25% according to IIP-8) is received as a separate StakedIota.
    // Need to unstake both the original stake and the commission to get the full amount.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Validator 1 should get the entire reward of 225 plus its initially staked 100 IOTA.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+225) * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_validator_subsidy_higher_than_computation_charge_with_commission_and_tips() {
    // Use 25 equal-stake validators (1000 IOTA each) so each has VP ≈ 400 bp ≈ 4%.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let stakes = vector::tabulate!(25, |_| 1000);
    let commission_rates = vector::tabulate!(25, |_| 0);
    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        stakes,
        commission_rates,
        ctx,
    );
    create_iota_system_state_for_testing(validators, 100000, 0, ctx);

    scenario_val.next_tx(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // With staker stakes added, V1 will have VP ≈ 437 bp, V2 ≈ 417 bp.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 50, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[(1000 + 100) * NANOS_PER_IOTA, (1000 + 50) * NANOS_PER_IOTA],
        scenario,
    );

    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 500, scenario); // 5% commission

    let validator_subsidy = 700;
    let computation_charge = 500; // 100 IOTA tips
    let computation_charge_burned = 400;

    // Total reward = validator_subsidy (700) + tips (100) = 800 IOTA,
    // distributed proportionally to VP across 25 validators.
    // V1 effective commission with IIP-8 = max(5%, ~4.37%) = 5%.
    // V2 effective commission with IIP-8 = max(0%, ~4.17%) = ~4.17%.
    advance_epoch_with_amounts(
        validator_subsidy,
        0,
        computation_charge,
        computation_charge_burned,
        scenario,
    );

    // V1: 1100 + 35.04 reward = 1135.04 IOTA.
    // V2: 1050 + 33.44 reward = 1083.44 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1_135_040_000_000, 1_083_440_000_000],
        scenario,
    );

    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // S1 gets its share of V1's pool reward after 5% commission (≈ 3.03 IOTA reward).
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 103_026_181_818);
    // S2 gets its share of V2's pool reward after ~4.17% IIP-8 commission (≈ 1.53 IOTA reward).
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 51_525_819_428);

    scenario_val.end();
}

#[test]
fun test_validator_rewards_non_committee() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        vector[100, 200, 300, 400, 500],
        vector[0, 0, 0, 0, 0],
        ctx,
    );
    create_iota_system_state_for_testing(validators, 1500, 0, ctx);
    scenario_val.end();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting..
    // Advance epoch and select only 4 committee members out of 5 active validators.
    advance_epoch_with_max_committee_members_count(4, scenario);

    assert_validator_total_stake_amounts(
        vector[
            VALIDATOR_ADDR_1,
            VALIDATOR_ADDR_2,
            VALIDATOR_ADDR_3,
            VALIDATOR_ADDR_4,
            VALIDATOR_ADDR_5,
        ],
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
            500 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    advance_epoch_with_balanced_reward_amounts_and_max_committee_size(0, 100, 4, scenario);

    // Rewards of 100 IOTA are split evenly between the validators.
    assert_validator_total_stake_amounts(
        vector[
            VALIDATOR_ADDR_1,
            VALIDATOR_ADDR_2,
            VALIDATOR_ADDR_3,
            VALIDATOR_ADDR_4,
            VALIDATOR_ADDR_5,
        ],
        vector[
            (100) * NANOS_PER_IOTA,
            (200 + 25) * NANOS_PER_IOTA,
            (300 + 25) * NANOS_PER_IOTA,
            (400 + 25) * NANOS_PER_IOTA,
            (500 + 25) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    stake_with(VALIDATOR_ADDR_2, VALIDATOR_ADDR_2, 720, scenario);

    // Advance epoch with expanded committee and no rewards.
    // All active validators should be part of the committee from now on and share rewards.
    advance_epoch_with_max_committee_members_count(5, scenario);

    advance_epoch_with_balanced_reward_amounts(0, 100, scenario);

    // Even though validator 2 has a lot more stake now, it should not get more rewards because
    // the voting power is capped at 10%.
    // Rewards of 100 IOTA are split evenly between the validators.
    // => +25 IOTA for each validator
    assert_validator_total_stake_amounts(
        vector[
            VALIDATOR_ADDR_1,
            VALIDATOR_ADDR_2,
            VALIDATOR_ADDR_3,
            VALIDATOR_ADDR_4,
            VALIDATOR_ADDR_5,
        ],
        vector[
            (100 + 20) * NANOS_PER_IOTA,
            (225 + 720 + 20) * NANOS_PER_IOTA,
            (325 + 20) * NANOS_PER_IOTA,
            (425 + 20) * NANOS_PER_IOTA,
            (525 + 20) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_stake_rewards() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 200, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 200) * NANOS_PER_IOTA,
            (200 + 100) * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // => +30 IOTA for each pool
    // Due to the dynamic minimum commission (IIP-8) the validators have a minimum commission of 25% in this scenario.
    // Validator 1 gets:
    //     commission = 25/100 * 30 IOTA => +7.5 IOTA
    //     stake reward = 100/300 * (30 - 7.5) IOTA => +7.5 IOTA
    //     total = 15 IOTA of rewards.
    // Validator 2 gets:
    //     commission = 25/100 * 30 IOTA => +7.5 IOTA
    //     stake reward = 200/300 * (30 - 7.5) IOTA => +15 IOTA
    //     total = 22.5 IOTA of rewards.
    // Validators 3 and 4 have all the stake in the pool => +30 IOTA for validators 3 and 4
    advance_epoch_with_balanced_reward_amounts(0, 120, scenario);
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 15) * NANOS_PER_IOTA,
            (200) * NANOS_PER_IOTA + 22_500_000_000,
            (300 + 30) * NANOS_PER_IOTA,
            (400 + 30) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    unstake(STAKER_ADDR_1, 0, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 600, scenario);

    // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
    // => +30 IOTA for each pool
    advance_epoch_with_balanced_reward_amounts(0, 120, scenario);
    // staker 1 receives only 200/300*(30-7.5)=15 IOTA of rewards, since we are using pre-epoch exchange rate.
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (200 + 15) * NANOS_PER_IOTA);
    // The recent changes in stake are not valid for this epoch yet. Thus:
    // Validators 1, 3 and 4 have all the stake in the pool => +30 IOTA for validators 1, 3 and 4
    // Validator 2 gets:
    //     commission = 25/100 * 30 IOTA => +7.5 IOTA
    //     stake reward = 222.5/330 * (30 - 7.5) IOTA => +15.17045454 IOTA
    //     total = 22.670454545 IOTA of rewards.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (115 + 30) * NANOS_PER_IOTA,
            222_500_000_000 + 22_670_454_545,
            (330 + 30) * NANOS_PER_IOTA,
            (430 + 30) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    unstake(STAKER_ADDR_2, 0, scenario);
    // Staker 2 receives:
    // Principal: 100 IOTA
    // Epoch 1: (100/300 * (30 - 7.5)) = 7.5 IOTA
    // Epoch 2: (107.5/330 * (30 - 7.5)) ≈ 7.329545454 IOTA
    // The stake added in the last epoch (600 IOTA) is not unstaked yet.
    assert_equal_approx(
        total_iota_balance(STAKER_ADDR_2, scenario),
        100_000_000_000 + 7_500_000_000 + 7_329_545_454,
        1,
    );

    // +10 IOTA for each pool
    // Validator 1: staker2's 600 IOTA is now active in the pool (total ~745), so V1 only gets
    //   commission (25% × 10 = 2.5) + pool share (~145/745 × 7.5 ≈ 1.459731543) ≈ 3.96 IOTA
    // Validator 2: sole staker (staker2 withdrew their 100), gets all 10 IOTA
    // Validators 3 and 4: sole stakers, get all 10 IOTA each
    advance_epoch_with_balanced_reward_amounts(0, 40, scenario);
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (115 + 30) * NANOS_PER_IOTA + 2_500_000_000 + 1_459_731_543,
            222_500_000_000 + 22_670_454_545 + 10 * NANOS_PER_IOTA,
            (330 + 30 + 10) * NANOS_PER_IOTA,
            (430 + 30 + 10) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Unstake staker2's 600 IOTA from V1. The rewarded amount is ~600/745 × 7.5 ≈ 6.040268456 IOTA.
    // Staker 2's balance is then ~114.83 from previous V2 unstake + 600 + ~6.04
    unstake(STAKER_ADDR_2, 0, scenario);
    assert_equal_approx(
        total_iota_balance(STAKER_ADDR_2, scenario),
        100_000_000_000 + 7_500_000_000 + 7_329_545_454 + 600_000_000_000 + 6_040_268_456,
        1,
    );
    scenario_val.end();
}

#[test]
fun test_stake_tiny_rewards() {
    set_up_iota_system_state_with_big_amounts();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Stake a large amount.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 200000000, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    advance_epoch_with_balanced_reward_amounts(0, 150000, scenario);

    // Stake a small amount.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 10, scenario);
    advance_epoch_with_balanced_reward_amounts(0, 130, scenario);

    // Unstake the stakes.
    unstake(STAKER_ADDR_1, 1, scenario);

    // Advance epoch should succeed.
    advance_epoch_with_balanced_reward_amounts(0, 150, scenario);
    scenario_val.end();
}

#[test]
fun test_validator_commission() {
    // Use 25 equal-stake validators (1000 IOTA each) so each has VP ≈ 400 bp ≈ 4%.
    // This way, setting V2 to 20% and V1 to 10% commission actually has an effect
    // (both exceed the IIP-8 dynamic minimum of ~4.37%).
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let stakes = vector::tabulate!(25, |_| 1000);
    let commission_rates = vector::tabulate!(25, |_| 0);
    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        stakes,
        commission_rates,
        ctx,
    );
    create_iota_system_state_for_testing(validators, 100000, 0, ctx);

    scenario_val.next_tx(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);
    // V1: 1100 IOTA, V2: 1100 IOTA, 23 others: 1000 IOTA each. Total: 25200 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1100 * NANOS_PER_IOTA, 1100 * NANOS_PER_IOTA],
        scenario,
    );

    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_2, 2000, scenario); // 20% commission

    // 120 IOTA total reward, distributed proportionally to VP across 25 validators.
    // V1 and V2 each have VP = 437 bp, so each pool gets 437/10000 * 120 = 5.244 IOTA.
    // V1 effective commission = max(0%, 437 bp) = 437 bp ≈ 4.37% (IIP-8 minimum).
    // V2 effective commission = max(20%, 437 bp) = 20% (manual commission wins).
    advance_epoch_with_balanced_reward_amounts(0, 120, scenario);

    // V1 pool reward = 5.244 IOTA:
    //   V1 commission (4.37%) ≈ 0.229 IOTA → staked as validator self-stake.
    //   Pool deposit ≈ 5.015 IOTA, S1 gets 100/1100 share ≈ 0.456 IOTA.
    // V2 pool reward = 5.244 IOTA:
    //   V2 commission (20%) ≈ 1.049 IOTA → staked as validator self-stake.
    //   Pool deposit ≈ 4.195 IOTA, S2 gets 100/1100 share ≈ 0.381 IOTA.
    // S2 earns less than S1 because V2's 20% commission is higher than V1's 4.37%.
    assert_validator_non_self_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        // S1: 100 + 0.456 ≈ 100.456 IOTA. S2: 100 + 0.381 ≈ 100.381 IOTA.
        vector[100_455_894_291, 100_381_381_818],
        scenario,
    );

    assert_validator_self_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        // V1: 1000 + 1000/1100 * 5.015 (pool share) + 0.229 (commission) ≈ 1004.788 IOTA.
        // V2: 1000 + 1000/1100 * 4.195 (pool share) + 1.049 (commission) ≈ 1004.863 IOTA.
        // V2 self-stake is slightly higher because higher commission captures more reward.
        vector[1_004_788_105_709, 1_004_862_618_182],
        scenario,
    );

    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario); // 10% commission

    // 240 IOTA total reward. V1 and V2 each get 437/10000 * 240 = 10.488 IOTA.
    // V1 effective commission = max(10%, ~437 bp) = 10%.
    // V2 effective commission = max(20%, ~437 bp) = 20%.
    advance_epoch_with_balanced_reward_amounts(0, 240, scenario);

    // Each pool gets 10.488 IOTA total reward (same VP), so totals are equal.
    // Total = 1100 (initial) + 5.244 (epoch 1) + 10.488 (epoch 2) = 1115.732 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1_115_732_000_000, 1_115_732_000_000],
        scenario,
    );

    // V1 commission (10%) ≈ 1.049 IOTA, deposit ≈ 9.439 IOTA.
    //   S1 earns about 100.456/1105.244 * 9.439 ≈ 0.858 IOTA this epoch.
    // V2 commission (20%) ≈ 2.098 IOTA, deposit ≈ 8.390 IOTA.
    //   S2 earns about 100.381/1105.244 * 8.390 ≈ 0.762 IOTA this epoch.
    assert_validator_non_self_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        // S1: 100.456 + 0.858 ≈ 101.314 IOTA. S2: 100.381 + 0.762 ≈ 101.143 IOTA.
        vector[101_313_825_461, 101_143_421_646],
        scenario,
    );

    assert_validator_self_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        // self = total - non_self.
        // V1: 1115.732 - 101.314 ≈ 1014.418 IOTA.
        // V2: 1115.732 - 101.143 ≈ 1014.589 IOTA.
        vector[1_014_418_174_539, 1_014_588_578_354],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_validator_commission_with_unstaking() {
    // Use 25 equal-stake validators (1000 IOTA each) so each has VP ≈ 400 bp ≈ 4%.
    // This way, setting V1 to 10% commission actually has an effect
    // (exceeds the IIP-8 dynamic minimum of ~4.38%).
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let stakes = vector::tabulate!(25, |_| 1000);
    let commission_rates = vector::tabulate!(25, |_| 0);
    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        stakes,
        commission_rates,
        ctx,
    );
    create_iota_system_state_for_testing(validators, 100000, 0, ctx);

    scenario_val.next_tx(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);
    // V1: 1100 IOTA, V2: 1100 IOTA, 23 others: 1000 IOTA each. Total: 25200 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1100 * NANOS_PER_IOTA, 1100 * NANOS_PER_IOTA],
        scenario,
    );

    // Validator 1: 10% commission. V2 keeps 0% (IIP-8 minimum applies).
    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario);

    // 800 IOTA total reward, distributed proportionally to VP across 25 validators.
    // V1 and V2 each have VP = 437 bp, so each pool gets 437/10000 * 800 = 34.96 IOTA.
    // V1 effective commission = max(10%, ~4.37%) = 10%.
    // V2 effective commission = max(0%, ~4.37%) = ~4.37% (IIP-8 minimum).
    advance_epoch_with_balanced_reward_amounts(0, 800, scenario);

    // Both pools get the same total reward (same VP), so totals are equal.
    // Total = 1100 + 34.96 = 1134.96 IOTA.
    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[1_134_960_000_000, 1_134_960_000_000],
        scenario,
    );

    // Unstake V1's original 1000 IOTA self-stake.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    // V1 commission (10%) = 3.496 IOTA, staked as a new StakedIota. Unstake it.
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_1, 0, scenario);

    // Unstake V2's original 1000 IOTA self-stake.
    unstake(VALIDATOR_ADDR_2, 0, scenario);
    // V2 commission (IIP-8 minimum, 437 bp) ≈ 1.528 IOTA, staked as a new StakedIota. Unstake it.
    unstake(VALIDATOR_ADDR_2, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // V1 pool: commission (10%) = 3.496 IOTA, deposit = 31.464 IOTA.
    //   V1 gets 1000/1100 * 31.464 ≈ 28.604 (pool share) + 3.496 (commission) ≈ 32.100 IOTA.
    //   S1 gets 100/1100 * 31.464 ≈ 2.860 IOTA.
    // V2 pool: commission (4.37%) ≈ 1.528 IOTA, deposit ≈ 33.432 IOTA.
    //   V2 gets 1000/1100 * 33.432 ≈ 30.393 (pool share) + 1.528 (commission) ≈ 31.921 IOTA.
    //   S2 gets 100/1100 * 33.432 ≈ 3.039 IOTA.
    // S2 earns more than S1 (3.039 vs 2.860) because V2's commission is lower than V1's.
    assert_equal_approx(
        total_iota_balance(VALIDATOR_ADDR_1, scenario),
        // V1: 1000 + 32.100 ≈ 1032.100 IOTA.
        1_032_099_636_363,
        1,
    );
    assert_equal_approx(
        total_iota_balance(STAKER_ADDR_1, scenario),
        // S1: 100 + 2.860 ≈ 102.860 IOTA.
        102_860_363_636,
        1,
    );
    assert_equal_approx(
        total_iota_balance(VALIDATOR_ADDR_2, scenario),
        // V2: 1000 + 31.921 ≈ 1031.921 IOTA.
        1_031_920_704_727,
        1,
    );
    assert_equal_approx(
        total_iota_balance(STAKER_ADDR_2, scenario),
        // S2: 100 + 3.039 ≈ 103.039 IOTA.
        103_039_295_272,
        1,
    );

    scenario_val.end();
}

#[test]
fun test_rewards_slashing() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let initial_supply = total_supply(scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

    advance_epoch(scenario);

    // Validator_2 is reported by 3 other validators, so 75% of total stake, since the voting power is capped at 10%.
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);

    // Validator_1 is reported by only 1 other validator, which is 25% of total stake.
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_1, scenario);

    // 3600 IOTA of total rewards, 50% threshold and 10% reward slashing.
    // So validator_2 is the only one whose rewards should get slashed.
    // Each pool would get +900 IOTA, disregarding the slashing rate.
    // Validator 1 gets 100/200*900 = +450 IOTA.
    // Validator 2 would get 200/300*900 = +600 IOTA (disregarding slashing).
    // Validators 3 and 4 have all the pool stake, so they get +900 IOTA each.
    advance_epoch_with_reward_amounts_and_slashing_rates(
        0,
        3600,
        1000,
        scenario,
    );

    // Due to IIP-8, all validators have 25% effective commission (= voting power).
    // V1 (reward=900): commission = 225, deposit = 675, pool share = 100/200*675 = 337.5
    //     V1 self = 100 + 225 + 337.5 = 662.5
    // V2 (reward=810 after 10% slash): commission = 202.5, deposit = 607.5
    //     V2 pool share = 200/300*607.5 = 405, V2 self = 200 + 202.5 + 405 = 807.5
    // V3/V4 are sole stakers.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA + 225 * NANOS_PER_IOTA + 337_500_000_000,
            200 * NANOS_PER_IOTA + 202_500_000_000 + 405 * NANOS_PER_IOTA,
            (300 + 900) * NANOS_PER_IOTA,
            (400 + 900) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Unstake so we can check the stake rewards as well.
    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // Staker 1 gets pool share: 100/200 * (900 - 225 commission) = 337.5 IOTA
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA + 337_500_000_000);
    // Staker 2 gets pool share: 100/300 * (810 - 202.5 commission) = 202.5 IOTA
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 100 * NANOS_PER_IOTA + 202_500_000_000);

    // Ensure that the slashed rewards are burned.
    assert_eq(total_supply(scenario), initial_supply - 90 * NANOS_PER_IOTA);
    scenario_val.end();
}

#[test]
fun test_entire_rewards_slashing() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let initial_supply = total_supply(scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

    advance_epoch(scenario);

    // Validator_2 is reported by 3 other validators, so 75% of total stake, since the voting power is capped at 10%.
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);

    // 3600 IOTA of total rewards, 100% reward slashing.
    // So validator_2 is the only one whose rewards should get slashed.
    // Each pool would get +900 IOTA, disregarding the slashing rate.
    // Due to IIP-8 dynamic minimum commission, all validators have 25% effective commission (= voting power).
    // Validator 1 gets:
    //     commission = 25/100 * 900 IOTA => +225 IOTA
    //     pool share = 100/200 * (900 - 225) = +337.5 IOTA
    //     total = +562.5 IOTA
    // Validator 2 would get 200/300*900 = +600 IOTA (disregarding slashing).
    // Validators 3 and 4 have all the pool stake, so they get +900 IOTA each.
    advance_epoch_with_reward_amounts_and_slashing_rates(
        0,
        3600,
        10_000,
        scenario,
    );

    // The entire rewards of validator 2's staking pool are slashed, which is 900 IOTA.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA + 225 * NANOS_PER_IOTA + 337_500_000_000,
            (200 + 600 - 600) * NANOS_PER_IOTA,
            (300 + 900) * NANOS_PER_IOTA,
            (400 + 900) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Unstake so we can check the stake rewards as well.
    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // Staker 1 gets 100/200*(900 - 225) = 337.5 IOTA as pool share rewards.
    // All of staker 2's rewards are slashed so she only gets back her principal.
    assert!(total_iota_balance(STAKER_ADDR_1, scenario) == 100 * NANOS_PER_IOTA + 337_500_000_000);
    assert!(total_iota_balance(STAKER_ADDR_2, scenario) == (100 + 300 - 300) * NANOS_PER_IOTA);

    // Ensure that the slashed rewards are burned.
    assert_eq(total_supply(scenario), initial_supply - 900 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_rewards_slashing_with_storage_fund() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let initial_supply = total_supply(scenario);

    // Put 300 IOTA into the storage fund. This should not change the pools' stake or give rewards.
    advance_epoch_with_balanced_reward_amounts(300, 0, scenario);
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Add a few stakes.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_3, 200, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_4, 100, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Validator_4 is reported by 3 other validators, so 75% of total stake, since the voting power is capped at 10%.
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, scenario);

    // 1000 IOTA of storage charges, 1500 IOTA of computation charges, 50% slashing threshold
    // and 20% slashing rate.
    // Because of the voting power cap, each pool gets +375 IOTA. V4 is slashed 20%: 375 - 75 = 300.
    // Due to IIP-8, all validators have 25% effective commission (= voting power).
    advance_epoch_with_reward_amounts_and_slashing_rates(
        1000,
        1500,
        2000,
        scenario,
    );

    // Validators 1 and 2 are sole stakers, so they get all 375 IOTA each.
    // Validator 3 gets:
    //     commission = 25/100 * 375 = 93.75 IOTA
    //     pool share = 300/500 * (375 - 93.75) = 168.75 IOTA
    //     total = 262.5 IOTA
    // Validator 4 gets (after 20% slashing, reward = 300):
    //     commission = 25/100 * 300 = 75 IOTA
    //     pool share = 400/500 * (300 - 75) = 180 IOTA
    //     total = 255 IOTA
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 375) * NANOS_PER_IOTA,
            (200 + 375) * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA + 93_750_000_000 + 168_750_000_000,
            400 * NANOS_PER_IOTA + 75 * NANOS_PER_IOTA + 180 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    // Unstake so we can check the stake rewards as well.
    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);

    // Staker 1 gets 200/500 * (375 - 93.75) = 112.5 IOTA of pool share rewards.
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 200 * NANOS_PER_IOTA + 112_500_000_000);
    // Staker 2 gets 100/500 * (300 - 75) = 45 IOTA of pool share rewards.
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (100 + 45) * NANOS_PER_IOTA);

    // Ensure that the slashed rewards are burned.
    assert_eq(total_supply(scenario), initial_supply - 75 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_everyone_slashed() {
    // This test is to make sure that if everyone is slashed, our protocol works as expected without aborting
    // and rewards are burned, and no tokens go to the storage fund.
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    let initial_supply = total_supply(scenario);

    slash_all_validators(scenario);

    advance_epoch_with_reward_amounts_and_slashing_rates(
        1000,
        500,
        10_000,
        scenario,
    );

    // All validators should have 0 rewards added so their stake stays the same.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            100 * NANOS_PER_IOTA,
            200 * NANOS_PER_IOTA,
            300 * NANOS_PER_IOTA,
            400 * NANOS_PER_IOTA,
        ],
        scenario,
    );

    scenario.next_tx(@0x0);
    // Storage fund balance should be the same as before.
    let mut system_state = scenario.take_shared<IotaSystemState>();
    assert_eq(system_state.get_storage_fund_total_balance(), 1000 * NANOS_PER_IOTA);

    // The entire 1000 IOTA of storage charges should go to the object rebate portion of the storage fund.
    assert_eq(system_state.get_storage_fund_object_rebates(), 1000 * NANOS_PER_IOTA);

    // Ensure that the slashed rewards are burned.
    assert_eq(system_state.get_total_iota_supply(), initial_supply - 500 * NANOS_PER_IOTA);

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_mul_rewards_withdraws_at_same_epoch() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 220, scenario);

    // Due to IIP-8, all validators have 25% effective commission.
    // Each pool gets +10 IOTA. Commission = 2.5 IOTA, deposit = 7.5 IOTA per validator.
    // S1 earns nothing this epoch since the stake is still pending.
    // Pools' total stake after this are
    // P1: 100 + 220 + 10 = 330; P2: 200 + 10 = 210; P3: 300 + 10 = 310; P4: 400 + 10 = 410
    advance_epoch_with_balanced_reward_amounts(0, 40, scenario);

    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 480, scenario);

    // Each pool gets +30 IOTA. Commission = 7.5, deposit = 22.5 per validator.
    // S1 gets 220/330 * 22.5 = +15 IOTA, totalling ~235 IOTA of stake.
    // Pools' total stake after this are
    // P1: 330 + 480 + 30 = 840; P2: 210 + 30 = 240; P3: 310 + 30 = 340; P4: 410 + 30 = 440
    advance_epoch_with_balanced_reward_amounts(0, 120, scenario);

    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 130, scenario);
    stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 390, scenario);

    // Each pool gets +70 IOTA. Commission = 17.5, deposit = 52.5 per validator.
    // S1 gets ~235/840 * 52.5 ≈ +14.69 IOTA, totalling ~250 IOTA of stake.
    // S2 gets 480/840 * 52.5 = +30 IOTA, totalling ~510 IOTA of stake.
    // Pools' total stake after this are
    // P1: 840 + 130 + 390 + 70 = 1430; P2: 240 + 70 = 310; P3: 340 + 70 = 410; P4: 440 + 70 = 510
    advance_epoch_with_balanced_reward_amounts(0, 280, scenario);

    stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 280, scenario);
    stake_with(STAKER_ADDR_4, VALIDATOR_ADDR_1, 1400, scenario);

    // Each pool gets +110 IOTA. Commission = 27.5, deposit = 82.5 per validator.
    // S1 gets ~(250+130)/1430 * 82.5 ≈ +21.92 IOTA, totalling ~402 IOTA of stake.
    // S2 gets ~510/1430 * 82.5 ≈ +29.42 IOTA, totalling ~539 IOTA of stake.
    // S3 gets 390/1430 * 82.5 ≈ +22.50 IOTA, totalling ~692 IOTA of stake.
    // Pools' total stake after this are
    // P1: 1430 + 280 + 1400 + 110 = 3220; P2: 310 + 110 = 420
    // P3: 410 + 110 = 520; P4: 510 + 110 = 620
    advance_epoch_with_balanced_reward_amounts(0, 440, scenario);

    scenario.next_tx(@0x0);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    // Check that we have the right amount of IOTA in the staking pool.
    assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 3220 * NANOS_PER_IOTA);
    test_scenario::return_shared(system_state);

    // Withdraw all stakes at once.
    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_1, 0, scenario);
    unstake(STAKER_ADDR_2, 0, scenario);
    unstake(STAKER_ADDR_3, 0, scenario);
    unstake(STAKER_ADDR_3, 0, scenario);
    unstake(STAKER_ADDR_4, 0, scenario);

    // With IIP-8 (25% commission for 4 equal-VP validators), stakers receive less.
    // S1: staked 220 (epoch 0) + 130 (epoch 2), earned rewards for 3 and 1 epochs.
    // Total ≈ 350 + 51.59 = 401.59 IOTA.
    // S1: staked 220 (epoch 0) + 130 (epoch 2), earned rewards for 3 and 1 epochs.
    // Total ≈ 350 + 51.59 = 401.59 IOTA.
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 401_592_548_075);
    // S2: staked 480 (epoch 1), earned rewards for 2 epochs.
    // Total ≈ 480 + 59.42 = 539.42 IOTA.
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 539_423_076_923);
    // S3: staked 390 (epoch 2) + 280 (epoch 3), earned rewards for 1 and 0 epochs.
    // Total ≈ 670 + 22.50 = 692.50 IOTA.
    assert_eq(total_iota_balance(STAKER_ADDR_3, scenario), 692_499_999_999);
    // S4: staked 1400 (epoch 3), staked and withdrawn in same epoch, so no rewards.
    assert_eq(total_iota_balance(STAKER_ADDR_4, scenario), 1400 * NANOS_PER_IOTA);

    advance_epoch_with_balanced_reward_amounts(0, 0, scenario);

    scenario.next_tx(@0x0);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    // Since all staker stakes are gone, the pool contains only the validator's stake:
    // 100 IOTA self-stake + accumulated commissions (2.5 + 7.5 + 17.5 + 27.5 = 55 IOTA)
    // plus compound growth on those commissions ≈ 186.48 IOTA.
    assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 186_484_375_003);
    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_uncapped_rewards() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;

    let ctx = scenario.ctx();
    let mut validators = vector[];

    let num_validators = 20;
    let mut i = 0;
    // Create a set of 20 validators, each with 481 + i * 2 IOTA of stake.
    // The stake total sums up to be 481 + 483 + ... + 517 + 519 = 1000 IOTA.
    while (i < num_validators) {
        let validator = create_validator_for_testing(
            address::from_u256(i as u256),
            (481 + i * 2),
            0,
            ctx,
        );
        validators.push_back(validator);
        i = i + 1;
    };

    create_iota_system_state_for_testing(validators, 0, 0, ctx);
    // Each validator's stake gets doubled.
    advance_epoch_with_balanced_reward_amounts(0, 10000, scenario);

    let mut i = 0;
    scenario.next_tx(@0x0);
    // Check that each validator has the correct amount of IOTA in their stake pool.
    let mut system_state = scenario.take_shared<IotaSystemState>();
    while (i < num_validators) {
        let addr = address::from_u256(i as u256);
        assert_eq(system_state.validator_stake_amount(addr), (962 + i * 4) * NANOS_PER_IOTA);
        i = i + 1;
    };
    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_slashed_validators_leftover_burning() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // To get the leftover, we have to slash every validator. This way, the computation charge will remain as leftover.
    slash_all_validators(scenario);

    // Pass 700 IOTA as computation charge(for an instance).
    advance_epoch_with_reward_amounts_and_slashing_rates(
        1000,
        700,
        10_000,
        scenario,
    );

    scenario.next_tx(@0x0);
    // The total supply of 1000 IOTA should be reduced by 700 IOTA because the 700 IOTA becomes leftover and should be burned.
    assert_eq(total_supply(scenario), 300 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = iota::balance::EOverflow)]
fun test_leftover_is_larger_than_supply() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // To get the leftover, we have to slash every validator. This way, the computation charge will remain as leftover.
    slash_all_validators(scenario);

    // Pass 1700 IOTA as computation charge which is larger than the total supply of 1000 IOTA.
    advance_epoch_with_reward_amounts_and_slashing_rates(
        1000,
        1700,
        10_000,
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_leftover_burning_after_reward_distribution() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // The leftover comes from the unequal distribution of rewards to validators.
    // As example 1_000_000_000_1 cannot be split into equal parts, so it cause leftover.
    let storage_rebate = advance_epoch_with_reward_amounts_return_rebate(
        1_000_000_000_1,
        1_000_000_000_000,
        1_000_000_000_1,
        1_000_000_000_1,
        0,
        0,
        scenario,
    );
    destroy(storage_rebate);

    scenario.next_tx(@0x0);

    // Total supply after leftover has burned.
    // The 999,999,999,999 is obtained by subtracting the leftover from the total supply: 1,000,000,000,000 - 1 = 999,999,999,999.
    assert_eq(total_supply(scenario), 999_999_999_999);

    scenario_val.end();
}

#[test]
fun test_constant_exchange_rates_with_no_rewards() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Add different stake to different pools.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 1, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 50, scenario);
    stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_3, 3, scenario);

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Advance a couple of epochs to make the exchange rate different than 1.
    // Epoch change from 1 to 2 - Validator 1: 10% commission.
    set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario);
    // Epoch change from 2 to 3.
    advance_epoch_with_balanced_reward_amounts(0, 1, scenario);

    // Stake 150 IOTA to validator 3 from a new staker address, then unstake the 3 IOTA previously staked.
    stake_with(STAKER_ADDR_4, VALIDATOR_ADDR_3, 150, scenario);
    unstake(STAKER_ADDR_3, 0, scenario);

    // Advance from epoch 3 to 4 without rewards.
    advance_epoch(scenario);

    // Get exchange rates for all pools and check that their rates are constant during the last epoch change (3 to 4).
    // Validators 1 and 2 should have no changes in stake, so the test should pass exactly (meaning without accounting
    // for fixed point arithmetic errors) for them.
    // Validator 3 should have a change in stake, so the test should account for some error.

    scenario.next_tx(VALIDATOR_ADDR_1);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let staked_iota_1 = scenario.take_from_address<StakedIota>(STAKER_ADDR_1);
    let pool_id_1 = staked_iota_1.pool_id();
    let rates1 = system_state.pool_exchange_rates(&pool_id_1);
    let pool_token_amount_epoch_3_pool_1 = rates1[3].pool_token_amount() as u128;
    let pool_token_amount_epoch_4_pool_1 = rates1[4].pool_token_amount() as u128;
    let iota_amount_epoch_3_pool_1 = rates1[3].iota_amount() as u128;
    let iota_amount_epoch_4_pool_1 = rates1[4].iota_amount() as u128;
    test_scenario::return_to_address(STAKER_ADDR_1, staked_iota_1);
    assert!(
        iota_amount_epoch_4_pool_1  * pool_token_amount_epoch_3_pool_1 == pool_token_amount_epoch_4_pool_1 * iota_amount_epoch_3_pool_1,
        0,
    );

    let staked_iota_2 = scenario.take_from_address<StakedIota>(STAKER_ADDR_2);
    let pool_id_2 = staked_iota_2.pool_id();
    let rates2 = system_state.pool_exchange_rates(&pool_id_2);
    let pool_token_amount_epoch_3_pool_2 = rates2[3].pool_token_amount() as u128;
    let pool_token_amount_epoch_4_pool_2 = rates2[4].pool_token_amount() as u128;
    let iota_amount_epoch_3_pool_2 = rates2[3].iota_amount() as u128;
    let iota_amount_epoch_4_pool_2 = rates2[4].iota_amount() as u128;
    test_scenario::return_to_address(STAKER_ADDR_2, staked_iota_2);
    assert!(
        iota_amount_epoch_4_pool_2 * pool_token_amount_epoch_3_pool_2 == pool_token_amount_epoch_4_pool_2 * iota_amount_epoch_3_pool_2,
        0,
    );

    let staked_iota_3 = scenario.take_from_address<StakedIota>(STAKER_ADDR_4);
    let pool_id_3 = staked_iota_3.pool_id();
    let rates3 = system_state.pool_exchange_rates(&pool_id_3);
    let pool_token_amount_epoch_3_pool_3 = rates3[3].pool_token_amount() as u128;
    let pool_token_amount_epoch_4_pool_3 = rates3[4].pool_token_amount() as u128;
    let iota_amount_epoch_3_pool_3 = rates3[3].iota_amount() as u128;
    let iota_amount_epoch_4_pool_3 = rates3[4].iota_amount() as u128;
    test_scenario::return_to_address(STAKER_ADDR_4, staked_iota_3);
    assert!(
        iota_amount_epoch_4_pool_3 * pool_token_amount_epoch_3_pool_3 > pool_token_amount_epoch_4_pool_3 * (iota_amount_epoch_3_pool_3 - 1000),
        0,
    );
    assert!(
        iota_amount_epoch_4_pool_3 * pool_token_amount_epoch_3_pool_3 < pool_token_amount_epoch_4_pool_3 * (iota_amount_epoch_3_pool_3 + 1000),
        0,
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_pool_tokens_minted() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Add some stake to the pool.
    stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 1, scenario);
    stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 50, scenario);

    // Need to advance epoch so staking starts counting.
    advance_epoch(scenario);

    // Advance a couple of epochs to make the exchange rate different than 1.
    // Epoch 1 to 2
    advance_epoch_with_balanced_reward_amounts(0, 3, scenario);
    // Epoch 2 to 3
    advance_epoch_with_balanced_reward_amounts(0, 1, scenario);

    // Epoch change from 3 to 4, with addition of stake and no rewards.
    stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 3, scenario);
    advance_epoch(scenario);

    // Epoch change from 4 to 5, with removal of stake and no rewards.
    unstake(STAKER_ADDR_2, 0, scenario);
    advance_epoch(scenario);

    // Epoch change from 5 to 6, with rewards.
    advance_epoch_with_balanced_reward_amounts(0, 100, scenario);

    // Get exchange rates for the pool to check its pool token supply during those epoch changes.
    scenario.next_tx(VALIDATOR_ADDR_1);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let staked_iota = scenario.take_from_address<StakedIota>(STAKER_ADDR_1);
    let pool_id = staked_iota.pool_id();
    let rates = system_state.pool_exchange_rates(&pool_id);
    let pool_token_amount_epoch_3 = rates[3].pool_token_amount() as u128;
    let pool_token_amount_epoch_4 = rates[4].pool_token_amount() as u128;
    let iota_amount_epoch_4 = rates[4].iota_amount() as u128;
    let pool_token_amount_epoch_5 = rates[5].pool_token_amount() as u128;
    let pool_token_amount_epoch_6 = rates[6].pool_token_amount() as u128;
    test_scenario::return_to_address(STAKER_ADDR_1, staked_iota);

    // Test 1: from epoch 3 to 4, 3_000_000_000 NANOs were added to the pool. The number of pool tokens
    // minted should be equal to 3_000_000_000 * pool_token_amount_epoch_4 / iota_amount_epoch_4, in theory.
    // Because of the fixed point arithmetic, the result is not exact, so we accept a small error.
    assert!(
        iota_amount_epoch_4 * (pool_token_amount_epoch_4 - pool_token_amount_epoch_3) > (3_000_000_000 - 1_000) * pool_token_amount_epoch_4,
        0,
    );
    assert!(
        iota_amount_epoch_4 * (pool_token_amount_epoch_4 - pool_token_amount_epoch_3) < (3_000_000_000 + 1_000) * pool_token_amount_epoch_4,
        0,
    );

    // Test 2: from epoch 4 to 5, 50_000_000_000 NANOs plus its rewards were removed from the pool.
    // The number of burned pool tokens should be equal to 50_000_000_000, since this stake was added
    // in the first epoch, when we had 1 pool token per NANO.
    assert!(pool_token_amount_epoch_4  - pool_token_amount_epoch_5 == 50_000_000_000, 0);

    // Test 3: from epoch 5 to 6, no IOTAs were explicitly added or removed from the pool by stakers.
    // However, commission creates new tokens for the validator's commission StakedIota which mints new pool tokens,
    // so the number of pool tokens should increase.
    assert!(pool_token_amount_epoch_6 > pool_token_amount_epoch_5, 0);

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_rewards_with_scores_no_adjustment() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Set validator scores.
    let system_state = scenario.take_shared<IotaSystemState>();
    let max_score = 65_536u64;
    let scores = vector[
        max_score / 4, // Validator 1
        max_score / 2, // Validator 2
        (max_score * 3) / 4, // Validator 3
        max_score, // Validator 4
    ];
    test_scenario::return_shared(system_state);

    // Advance epoch with 800 IOTA of subsidy and the above scores, but don't adjust rewards by score.
    advance_epoch_with_subsidy_and_scores(800, scores, false, scenario);

    // Check that the rewards were distributed and were unaffected by the scores.
    // Each pool gets +200 IOTA.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 200) * NANOS_PER_IOTA,
            (200 + 200) * NANOS_PER_IOTA,
            (300 + 200) * NANOS_PER_IOTA,
            (400 + 200) * NANOS_PER_IOTA,
        ],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_rewards_with_scores_and_adjustment() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Set validator scores.
    let system_state = scenario.take_shared<IotaSystemState>();
    let max_score = 65_536u64;
    let scores = vector[
        max_score / 4, // Validator 1
        max_score / 2, // Validator 2
        (max_score * 3) / 4, // Validator 3
        max_score, // Validator 4
    ];
    test_scenario::return_shared(system_state);

    // Advance epoch with 800 IOTA of subsidy and the above scores, and adjust rewards by score.
    advance_epoch_with_subsidy_and_scores(800, scores, true, scenario);

    // Check that the rewards were distributed according to the scores.
    // Each pool gets +200 IOTA and adjusted by score.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 50) * NANOS_PER_IOTA, // 200 * 1/4 = 50
            (200 + 100) * NANOS_PER_IOTA, // 200 * 1/2 = 100
            (300 + 150) * NANOS_PER_IOTA, // 200 * 3/4 = 150
            (400 + 200) * NANOS_PER_IOTA, // 200 * 1 = 200
        ],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_rewards_with_scores_and_slashing_no_adjustment() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Set validator scores.
    let system_state = scenario.take_shared<IotaSystemState>();
    let max_score = 65_536u64;
    let scores = vector[
        max_score / 4, // Validator 1
        max_score / 2, // Validator 2
        (max_score * 3) / 4, // Validator 3
        max_score, // Validator 4
    ];
    test_scenario::return_shared(system_state);

    // validators 2 and 4 reported by all others, so they get slashed.
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, scenario);

    // Advance epoch with 800 IOTA of subsidy and the above scores.
    advance_epoch_with_subsidy_and_scores(800, scores, false, scenario);

    // Check that the rewards were distributed according to slashing only, without score adjustment.
    // Each pool gets +200 IOTA.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 200) * NANOS_PER_IOTA, // full rewards
            (200) * NANOS_PER_IOTA, // slashed
            (300 + 200) * NANOS_PER_IOTA, // full rewards
            (400) * NANOS_PER_IOTA, // slashed
        ],
        scenario,
    );

    scenario_val.end();
}

#[test]
fun test_rewards_with_scores_and_slashing_and_adjustment() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    // Need to advance epoch so validator's staking starts counting.
    advance_epoch(scenario);

    // Set validator scores.
    let system_state = scenario.take_shared<IotaSystemState>();
    let max_score = 65_536u64;
    let scores = vector[
        max_score / 4, // Validator 1
        max_score / 2, // Validator 2
        (max_score * 3) / 4, // Validator 3
        max_score, // Validator 4
    ];
    test_scenario::return_shared(system_state);

    // validators 2 and 4 reported by all others, so they get slashed.
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, scenario);

    // Advance epoch with 800 IOTA of subsidy and the above scores.
    advance_epoch_with_subsidy_and_scores(800, scores, true, scenario);

    // Check that the rewards were distributed according to slashing only, without score adjustment.
    // Each pool gets +200 IOTA.
    assert_validator_self_stake_amounts(
        validator_addrs(),
        vector[
            (100 + 50) * NANOS_PER_IOTA, // adjusted rewards = 200 * 1/4 = 50
            (200) * NANOS_PER_IOTA, // slashed
            (300 + 150) * NANOS_PER_IOTA, // adjusted rewards = 200 * 3/4 = 150
            (400) * NANOS_PER_IOTA, // slashed
        ],
        scenario,
    );

    scenario_val.end();
}

// This will set up the IOTA system state with the following validator stakes:
// Validator 1 => 100
// Validator 2 => 200
// Validator 3 => 300
// Validator 4 => 400
fun set_up_iota_system_state() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let validators = vector[
        create_validator_for_testing(VALIDATOR_ADDR_1, 100, 0, ctx),
        create_validator_for_testing(VALIDATOR_ADDR_2, 200, 0, ctx),
        create_validator_for_testing(VALIDATOR_ADDR_3, 300, 0, ctx),
        create_validator_for_testing(VALIDATOR_ADDR_4, 400, 0, ctx),
    ];

    create_iota_system_state_for_testing(validators, 1000, 0, ctx);
    scenario_val.end();
}

// This will set up the IOTA system state with the following validator stakes:
// Validator 1 => 100000000
// Validator 2 => 200000000
// Validator 3 => 300000000
// Validator 4 => 400000000
fun set_up_iota_system_state_with_big_amounts() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        vector[100000000, 200000000, 300000000, 400000000],
        vector[0, 0, 0, 0],
        ctx,
    );
    create_iota_system_state_for_testing(validators, 1000000000, 0, ctx);
    scenario_val.end();
}

fun validator_addrs(): vector<address> {
    vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, VALIDATOR_ADDR_4]
}

fun set_commission_rate_and_advance_epoch(
    addr: address,
    commission_rate: u64,
    scenario: &mut Scenario,
) {
    scenario.next_tx(addr);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();
    system_state.request_set_commission_rate(commission_rate, ctx);
    test_scenario::return_shared(system_state);
    advance_epoch(scenario);
}

fun report_validator(reporter: address, reportee: address, scenario: &mut Scenario) {
    scenario.next_tx(reporter);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let cap = scenario.take_from_sender<UnverifiedValidatorOperationCap>();
    system_state.report_validator(&cap, reportee);
    scenario.return_to_sender(cap);
    test_scenario::return_shared(system_state);
}

fun slash_all_validators(scenario: &mut Scenario) {
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, scenario);
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_3, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_3, scenario);
    report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);
    report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_1, scenario);
    report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_1, scenario);
    report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_1, scenario);
}
