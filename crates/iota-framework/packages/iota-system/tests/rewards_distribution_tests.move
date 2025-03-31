// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::rewards_distribution_tests {
    use iota::test_scenario::{Self, Scenario};
    use iota_system::iota_system::IotaSystemState;
    use iota_system::validator_cap::UnverifiedValidatorOperationCap;
    use iota_system::governance_test_utils::{
        advance_epoch,
        advance_epoch_with_balanced_reward_amounts,
        advance_epoch_with_balanced_reward_amounts_and_max_committee_size,
        advance_epoch_with_max_committee_members_count,
        advance_epoch_with_reward_amounts_return_rebate,
        advance_epoch_with_reward_amounts_and_slashing_rates,
        advance_epoch_with_amounts,
        assert_validator_total_stake_amounts,
        assert_validator_non_self_stake_amounts,
        assert_validator_self_stake_amounts,
        create_validator_for_testing,
        create_iota_system_state_for_testing,
        stake_with,
        total_iota_balance, total_supply,
        unstake
    };
    use iota_system::staking_pool::StakedIota;

    use iota::test_utils::{assert_eq, destroy};

    use iota::address;

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
            scenario
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
            scenario
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
            scenario
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
            scenario
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
            scenario);
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
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);

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
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);

        let new_supply = total_supply(scenario);

        // The difference between computation charge burned and validator subsidy should have been burned.
        assert_eq(prev_supply - (computation_charge_burned - validator_subsidy) * NANOS_PER_IOTA, new_supply);

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
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);

        let new_supply = total_supply(scenario);

        // The difference between validator subsidy and computation charge burned should have been minted.
        assert_eq(prev_supply + (validator_subsidy - computation_charge_burned) * NANOS_PER_IOTA, new_supply);

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
            scenario
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
            scenario
        );

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
            scenario
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
            scenario
        );

        unstake(VALIDATOR_ADDR_1, 0, scenario);

        // Validator 1 should get the entire reward of 200 plus its initially staked 100 IOTA.
        assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+200) * NANOS_PER_IOTA);

        scenario_val.end();
    }

    #[test]
    fun test_validator_subsidy_higher_than_computation_charge_with_commission() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 50, scenario);
        
        // Need to advance epoch so validator's staking starts counting.
        advance_epoch(scenario);

        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 100) * NANOS_PER_IOTA,
                (200 + 50) * NANOS_PER_IOTA,
                300 * NANOS_PER_IOTA,
                400 * NANOS_PER_IOTA,
            ],
            scenario
        );

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 500, scenario); // 5% commission

        // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
        advance_epoch_with_amounts(800, 0, 400, 400, scenario);

        // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
        // => +200 IOTA for each validator
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
                (200 + 200) * NANOS_PER_IOTA,
                (250 + 200) * NANOS_PER_IOTA,
                (300 + 200) * NANOS_PER_IOTA,
                (400 + 200) * NANOS_PER_IOTA,
            ],
            scenario
        );

        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Staker 1 should have its original 100 staked IOTA and get half the pool reward (100)
        // minus the validator's commission (100 * 0.05 = 5), so 95.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (100 + 95) * NANOS_PER_IOTA);

        // Staker 2 should get 50/250 = 1/5 of the pool reward, which is 40.
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (50 + 40) * NANOS_PER_IOTA);

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
            scenario
        );

        let validator_subsidy = 800;
        let computation_charge = 500; // 100 IOTA tips
        let computation_charge_burned = 400;

        // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);
        
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
            scenario
        );

        unstake(VALIDATOR_ADDR_1, 0, scenario);

        // Validator 1 should get the entire reward of 200 plus its initially staked 100 IOTA.
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
            scenario
        );

        let validator_subsidy = 800;
        let computation_charge = 1100; // 100 IOTA tips
        let computation_charge_burned = 1000;

        // The computation charge is higher than the validator subsidy, so 200 IOTA should be burned.
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);

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
            scenario
        );

        unstake(VALIDATOR_ADDR_1, 0, scenario);

        // Validator 1 should get the entire reward of 200 plus its initially staked 100 IOTA.
        assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+225) * NANOS_PER_IOTA);

        scenario_val.end();
    }

    #[test]
    fun test_validator_subsidy_higher_than_computation_charge_with_commission_and_tips() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 50, scenario);
        
        // need to advance epoch so validator's staking starts counting
        advance_epoch(scenario);

        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 100) * NANOS_PER_IOTA,
                (200 + 50) * NANOS_PER_IOTA,
                300 * NANOS_PER_IOTA,
                400 * NANOS_PER_IOTA,
            ],
            scenario
        );

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 500, scenario); // 5% commission


        let validator_subsidy = 700;
        let computation_charge = 500; // 100 IOTA tips
        let computation_charge_burned = 400;

        // The computation charge is lower than the validator subsidy, so 400 IOTA should be minted.
        advance_epoch_with_amounts(validator_subsidy, 0, computation_charge, computation_charge_burned, scenario);

        // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
        // total reward is validator subsidy (700) + tips (100) = 900
        // => +200 IOTA for each validator
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
                (200 + 200) * NANOS_PER_IOTA,
                (250 + 200) * NANOS_PER_IOTA,
                (300 + 200) * NANOS_PER_IOTA,
                (400 + 200) * NANOS_PER_IOTA,
            ],
            scenario
        );

        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Staker 1 should have its original 100 staked IOTA and get half the pool reward (100)
        // minus the validator's commission (100 * 0.05 = 5), so 95.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (100 + 95) * NANOS_PER_IOTA);

        // Staker 2 should get 50/250 = 1/5 of the pool reward, which is 40.
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (50 + 40) * NANOS_PER_IOTA);

        scenario_val.end();
    }

    #[test]
    fun test_validator_rewards_non_committee() {
        let mut scenario_val = test_scenario::begin(@0x0);
        let scenario = &mut scenario_val;
        let ctx = scenario.ctx();

        let validators = vector[
            create_validator_for_testing(VALIDATOR_ADDR_1, 100, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_2, 200, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_3, 300, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_4, 400, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_5, 500, ctx),

        ];
        create_iota_system_state_for_testing(validators, 1500, 0, ctx);
        scenario_val.end();
                let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        // Need to advance epoch so validator's staking starts counting..
        // Advance epoch and select only 4 committee members out of 5 active validators.
        advance_epoch_with_max_committee_members_count(4, scenario);

        assert_validator_total_stake_amounts(
            vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, VALIDATOR_ADDR_5],
            vector[
                100 * NANOS_PER_IOTA,
                200 * NANOS_PER_IOTA,
                300 * NANOS_PER_IOTA,
                400 * NANOS_PER_IOTA,
                500 * NANOS_PER_IOTA,

            ],
            scenario
        );

        advance_epoch_with_balanced_reward_amounts_and_max_committee_size(0, 100, 4, scenario);
        
        // Rewards of 100 IOTA are split evenly between the validators.
        assert_validator_total_stake_amounts(
            vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, VALIDATOR_ADDR_5],
            vector[
                (100) * NANOS_PER_IOTA,
                (200 + 25) * NANOS_PER_IOTA,
                (300 + 25) * NANOS_PER_IOTA,
                (400 + 25) * NANOS_PER_IOTA,
                (500 + 25) * NANOS_PER_IOTA,
            ],
            scenario
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
            vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, VALIDATOR_ADDR_5],
            vector[
                (100 + 20) * NANOS_PER_IOTA,
                (225 + 720 + 20) * NANOS_PER_IOTA,
                (325 + 20) * NANOS_PER_IOTA,
                (425 + 20) * NANOS_PER_IOTA,
                (525 + 20) * NANOS_PER_IOTA,

            ],
            scenario
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
            scenario);
        
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                100 * NANOS_PER_IOTA,
                200 * NANOS_PER_IOTA,
                300 * NANOS_PER_IOTA,
                400 * NANOS_PER_IOTA,
            ], scenario);

        // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
        // => +30 IOTA for each pool
        // Validator 1 gets 100/300 * 30 IOTA => +10 IOTA for validator 1
        // Validator 2 gets 200/300 * 30 IOTA => +20 IOTA for validator 2
        // Validators 3 and 4 have all the stake in the pool => +30 IOTA for validators 3 and 4
        advance_epoch_with_balanced_reward_amounts(0, 120, scenario);
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 10) * NANOS_PER_IOTA,
                (200 + 20) * NANOS_PER_IOTA,
                (300 + 30) * NANOS_PER_IOTA,
                (400 + 30) * NANOS_PER_IOTA,
            ],
            scenario);
        
        unstake(STAKER_ADDR_1, 0, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 600, scenario);
        
        // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
        // => +30 IOTA for each pool
        advance_epoch_with_balanced_reward_amounts(0, 120, scenario);
        // staker 1 receives only 200/300*30=20 IOTA of rewards, (not 40) since we are using pre-epoch exchange rate.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (200 + 20) * NANOS_PER_IOTA);
        // The recent changes in stake are not valid for this epoch yet. Thus:
        // Validators 1, 3 and 4 have all the stake in the pool => +30 IOTA for validators 1, 3 and 4
        // Validator 2 gets 200/300 * 30 IOTA => +20 IOTA for validator 
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                (110 + 30) * NANOS_PER_IOTA,
                (220 + 20) * NANOS_PER_IOTA,
                (330 + 30) * NANOS_PER_IOTA,
                (430 + 30) * NANOS_PER_IOTA,
            ],
            scenario);
        
        unstake(STAKER_ADDR_2, 0, scenario);
        // Staker 2 receives its available principal (100 IOTA) + 100/300*60=20 IOTA of rewards, relative to 2 epochs.
        // The stake added in the last epoch (600 IOTA) is not unstaked yet.
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (100 + 20) * NANOS_PER_IOTA);

        // +10 IOTA for each pool
        advance_epoch_with_balanced_reward_amounts(0, 40, scenario);

        // Unstakes the additional 600 IOTA of principal + rewards relative to past epoch.
        // The rewarded amount is 600/740*10 IOTA.
        // Staker 2's balance is then ~120 + 600 + 8.1081081
        unstake(STAKER_ADDR_2, 0, scenario); 
        // TODO: Come up with better numbers and clean it up!
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 728108108107);
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
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

        // Need to advance epoch so validator's staking starts counting.
        advance_epoch(scenario);
        // Pool's stake:
        // V1: 200, V2: 300, V3: 300, V4: 400

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_2, 2000, scenario); // 20% commission

        // Each validator pool has 25% of the voting power and thus gets 25% of the reward.
        // => +30 IOTA for each pool
        advance_epoch_with_balanced_reward_amounts(0, 120, scenario);

        // Staker 1 gets 100/200*30 IOTA => +15 IOTA
        // Staker 2 would get 100/300*30 IOTA = 10 IOTA. However, since the commission rate is 20% for this pool, they get 8 IOTA.
        assert_validator_non_self_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 15) * NANOS_PER_IOTA,
                (100 + 8) * NANOS_PER_IOTA,
                0,
                0,
            ],
            scenario);
        
        // Validator 1 gets 100/200*30 => +15 IOTA
        // Validator 2 gets 200/300*30 + 2 (from the commission) => +22 IOTA
        // Validators 3 and 4 have all the pool stake and get +30 IOTA each.
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 15) * NANOS_PER_IOTA,
                (200 + 22) * NANOS_PER_IOTA,
                (300 + 30) * NANOS_PER_IOTA,
                (400 + 30) * NANOS_PER_IOTA,
            ],
            scenario);

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario); // 10% commission

        // +60 IOTA for each pool
        advance_epoch_with_balanced_reward_amounts(0, 240, scenario);
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
                (230 + 60) * NANOS_PER_IOTA,
                (330 + 60) * NANOS_PER_IOTA,
                (330 + 60) * NANOS_PER_IOTA,
                (430 + 60) * NANOS_PER_IOTA,
            ], scenario);

        // Staker 1 rewards in the recent distribution is 0.9 x 30 = 27 IOTA.
        // Validator 1 rewards in the recent distribution is 60 - 27 = 33 IOTA.

        // Staker 2 amounts for 0.8 * 60 * (108 / 330) + 108 = 123.709 IOTA.
        // Validator 2 amounts for 390 - 123.709 = 266.291 IOTA.
        assert_validator_non_self_stake_amounts(
            validator_addrs(),
            vector[
                142 * NANOS_PER_IOTA,
                123709090909,
                0, 
                0,
            ], 
            scenario);
        
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                148 * NANOS_PER_IOTA,
                266290909091,
                390 * NANOS_PER_IOTA, 
                490 * NANOS_PER_IOTA,
            ],
            scenario);

        scenario_val.end();
    }

    #[test]
    fun test_validator_commission_with_staking() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);

        // Need to advance epoch so validator's staking starts counting.
        advance_epoch(scenario);
        // V1: 200, V2: 200, V3: 300, V4: 400

        // Validator 1: 10% commission.
        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario);

        advance_epoch_with_balanced_reward_amounts(0, 800, scenario);

        // Each validator pool gets 25% of the voting power and thus gets 25% of the reward (200 IOTA).
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[
              (200 + 200) * NANOS_PER_IOTA,
              (200 + 200) * NANOS_PER_IOTA,
              (300 + 200) * NANOS_PER_IOTA,
              (400 + 200) * NANOS_PER_IOTA,
            ],
            scenario
        );

        // Unstakes the initially created StakedIota of value 100 IOTA.
        unstake(VALIDATOR_ADDR_1, 0, scenario);
        // The validator should have received a 10% commission on the reward of 200 IOTA (= 20 IOTA)
        // in the form of a StakedIota.
        unstake(VALIDATOR_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_1, 0, scenario);

        // The remaining 200 - 20 = 180 should be distributed equally between validator
        // and staker since both have equivalent stake.
        assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), (100+90+20) * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (100+90) * NANOS_PER_IOTA);

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
            0, 3600, 1000, scenario
        );

        // Without reward slashing, the validator's stakes should be [100+450, 200+600, 300+900, 400+900]
        // after the last epoch advancement.
        // Since 60 IOTA, or 10% of validator_2's rewards (600) are slashed, she only has 200 + 600 - 60 = 740 now.
        // Note that the slashed rewards are not distributed to the other validators.
        assert_validator_self_stake_amounts(
            validator_addrs(),
            vector[
                (100 + 450) * NANOS_PER_IOTA,
                (200 + 600 - 60) * NANOS_PER_IOTA,
                (300 + 900) * NANOS_PER_IOTA,
                (400 + 900) * NANOS_PER_IOTA,
            ],
            scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Same analysis as above. Delegator 1 gets 450 IOTA, and 10% of staker 2's rewards (30 IOTA) are slashed.
        assert!(total_iota_balance(STAKER_ADDR_1, scenario) == (100 + 450) * NANOS_PER_IOTA);
        assert!(total_iota_balance(STAKER_ADDR_2, scenario) == (100 + 300 - 30) * NANOS_PER_IOTA);

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
        // Validator 1 gets 100/200*900 = +450 IOTA.
        // Validator 2 would get 200/300*900 = +600 IOTA (disregarding slashing).
        // Validators 3 and 4 have all the pool stake, so they get +900 IOTA each.
        advance_epoch_with_reward_amounts_and_slashing_rates(
            0, 3600, 10_000, scenario
        );

        // Without reward slashing, the validator's stakes should be [100+450, 200+600, 300+900, 400+900]
        // after the last epoch advancement.
        // The entire rewards of validator 2's staking pool are slashed, which is 900 IOTA.
        assert_validator_self_stake_amounts(
            validator_addrs(), 
            vector[
                (100 + 450) * NANOS_PER_IOTA,
                (200 + 600 - 600) * NANOS_PER_IOTA,
                (300 + 900) * NANOS_PER_IOTA,
                (400 + 900) * NANOS_PER_IOTA,
            ],
            scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Same analysis as above. Staker 1 gets 450 IOTA as rewards, and since all of staker 2's rewards are slashed she only gets back her principal.
        assert!(total_iota_balance(STAKER_ADDR_1, scenario) == (100 + 450) * NANOS_PER_IOTA);
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
            ], scenario);

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
        // Because of the voting power cap, each pool gets +375 IOTA.
        advance_epoch_with_reward_amounts_and_slashing_rates(
            1000, 1500, 2000, scenario
        );

        // Validator 1 should get 375 * 1 = 375 in rewards.
        // Validator 2 should get 375 * 1 = 375 in rewards.
        // Validator 3 should get 375 * 3/5 = 225 in rewards.
        // Validator 4 should get (375 - 75) * 4/5 = 240 in rewards.
        assert_validator_self_stake_amounts(
            validator_addrs(), 
            vector[
                (100 + 375) * NANOS_PER_IOTA,
                (200 + 375) * NANOS_PER_IOTA,
                (300 + 225) * NANOS_PER_IOTA,
                (400 + 240) * NANOS_PER_IOTA,
            ],
            scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Staker 1 gets 375 * 2/5 = 150 IOTA of rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (200 + 150) * NANOS_PER_IOTA);
        // Staker 2 gets (375 - 75) * 1/5 = 60 IOTA of rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (100 + 60) * NANOS_PER_IOTA);

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
            1000, 500, 10_000, scenario
        );

        // All validators should have 0 rewards added so their stake stays the same.
        assert_validator_self_stake_amounts(
            validator_addrs(), 
            vector[
                100 * NANOS_PER_IOTA,
                200 * NANOS_PER_IOTA,
                300 * NANOS_PER_IOTA,
                400 * NANOS_PER_IOTA,
            ], scenario);

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

        // Since the voting power is capped at 10%, each pool gets +10 IOTA.
        // Pools' stake after this are 
        // P1: 100 + 220 + 10 = 330; P2: 200 + 10 = 210; P3: 300 + 10 = 310; P4: 400 + 10 = 410 
        advance_epoch_with_balanced_reward_amounts(0, 40, scenario);

        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 480, scenario);

        // Here, each pool gets +30 IOTA.
        // Staker 1 gets 220/330 * 30 = +20 IOTA, totalling 240 IOTA of stake.
        // Pools' stake after this are 
        // P1: 330 + 480 + 30 = 840; P2: 210 + 30 = 240; P3: 310 + 30 = 340; P4: 410 + 30 = 440
        advance_epoch_with_balanced_reward_amounts(0, 120, scenario);

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 130, scenario);
        stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 390, scenario);

        // Here, each pool gets +70 IOTA.
        // Staker 1 gets 240/840*70 = +20 IOTA, totalling 390 IOTA of stake.
        // Staker 2 gets 480/840*70 = +40 IOTA, totalling 520 IOTA of stake.
        // Pools' stake after this are 
        // P1: 840 + 130 + 390 + 70 = 1430; P2: 240 + 70 = 310; P3: 340 + 70 = 410; P4: 440 + 70 = 510
        advance_epoch_with_balanced_reward_amounts(0, 280, scenario);

        stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 280, scenario);
        stake_with(STAKER_ADDR_4, VALIDATOR_ADDR_1, 1400, scenario);

        // Here, each pool gets +110 IOTA.
        // Staker 1 gets 390/1430*110 = +30 IOTA, totalling 420 IOTA of stake.
        // Staker 2 gets 520/1430*110 = +40 IOTA, totalling 560 IOTA of stake.
        // Staker 3 gets 390/1430*110 = +30 IOTA, totalling 700 IOTA of stake.
        // Pools' stake after this are 
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

        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 420 * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 560 * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_3, scenario), 700 * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_4, scenario), 1400 * NANOS_PER_IOTA);

        advance_epoch_with_balanced_reward_amounts(0, 0, scenario);

        scenario.next_tx(@0x0);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        // Since all the stakes are gone the pool is empty except for the validator's stake.
        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 140 * NANOS_PER_IOTA);
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
            let validator = create_validator_for_testing(address::from_u256(i as u256), (481 + i * 2), ctx);
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
            1000, 700, 10_000, scenario
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
            1000, 1700, 10_000, scenario
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
        let storage_rebate = advance_epoch_with_reward_amounts_return_rebate(1_000_000_000_1, 1_000_000_000_000, 1_000_000_000_1, 1_000_000_000_1, 0, 0, scenario);
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
        let pool_token_amount_epoch_3_pool_1  = rates1[3].pool_token_amount() as u128;
        let pool_token_amount_epoch_4_pool_1 = rates1[4].pool_token_amount() as u128;
        let iota_amount_epoch_3_pool_1 = rates1[3].iota_amount() as u128;
        let iota_amount_epoch_4_pool_1 = rates1[4].iota_amount() as u128;
        test_scenario::return_to_address(STAKER_ADDR_1, staked_iota_1);
        assert!(iota_amount_epoch_4_pool_1  * pool_token_amount_epoch_3_pool_1 == pool_token_amount_epoch_4_pool_1 * iota_amount_epoch_3_pool_1, 0);

        let staked_iota_2 = scenario.take_from_address<StakedIota>(STAKER_ADDR_2);
        let pool_id_2 = staked_iota_2.pool_id();
        let rates2 = system_state.pool_exchange_rates(&pool_id_2);
        let pool_token_amount_epoch_3_pool_2 = rates2[3].pool_token_amount() as u128;
        let pool_token_amount_epoch_4_pool_2 = rates2[4].pool_token_amount() as u128;
        let iota_amount_epoch_3_pool_2 = rates2[3].iota_amount() as u128;
        let iota_amount_epoch_4_pool_2 = rates2[4].iota_amount() as u128;
        test_scenario::return_to_address(STAKER_ADDR_2, staked_iota_2);
        assert!(iota_amount_epoch_4_pool_2 * pool_token_amount_epoch_3_pool_2 == pool_token_amount_epoch_4_pool_2 * iota_amount_epoch_3_pool_2, 0);

        let staked_iota_3 = scenario.take_from_address<StakedIota>(STAKER_ADDR_4);
        let pool_id_3 = staked_iota_3.pool_id();
        let rates3 = system_state.pool_exchange_rates(&pool_id_3);
        let pool_token_amount_epoch_3_pool_3 = rates3[3].pool_token_amount() as u128;
        let pool_token_amount_epoch_4_pool_3 = rates3[4].pool_token_amount() as u128;
        let iota_amount_epoch_3_pool_3 = rates3[3].iota_amount() as u128;
        let iota_amount_epoch_4_pool_3 = rates3[4].iota_amount() as u128;
        test_scenario::return_to_address(STAKER_ADDR_4, staked_iota_3);
        assert!(iota_amount_epoch_4_pool_3 * pool_token_amount_epoch_3_pool_3 > pool_token_amount_epoch_4_pool_3 * (iota_amount_epoch_3_pool_3 - 1000) , 0);
        assert!(iota_amount_epoch_4_pool_3 * pool_token_amount_epoch_3_pool_3 < pool_token_amount_epoch_4_pool_3 * (iota_amount_epoch_3_pool_3 + 1000), 0);

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
        let pool_token_amount_epoch_3  = rates[3].pool_token_amount() as u128;
        let pool_token_amount_epoch_4  = rates[4].pool_token_amount() as u128;
        let iota_amount_epoch_4  = rates[4].iota_amount() as u128;
        let pool_token_amount_epoch_5  = rates[5].pool_token_amount() as u128;
        let pool_token_amount_epoch_6  = rates[6].pool_token_amount() as u128;
        test_scenario::return_to_address(STAKER_ADDR_1, staked_iota);

        // Test 1: from epoch 3 to 4, 3_000_000_000 NANOs were added to the pool. The number of pool tokens
        // minted should be equal to 3_000_000_000 * pool_token_amount_epoch_4 / iota_amount_epoch_4, in theory.
        // Because of the fixed point arithmetic, the result is not exact, so we accept a small error.
        assert!(iota_amount_epoch_4 * (pool_token_amount_epoch_4 - pool_token_amount_epoch_3) > (3_000_000_000 - 1_000) * pool_token_amount_epoch_4, 0);
        assert!(iota_amount_epoch_4 * (pool_token_amount_epoch_4 - pool_token_amount_epoch_3) < (3_000_000_000 + 1_000) * pool_token_amount_epoch_4, 0);

        // Test 2: from epoch 4 to 5, 50_000_000_000 NANOs plus its rewards were removed from the pool. 
        // The number of burned pool tokens should be equal to 50_000_000_000, since this stake was added 
        // in the first epoch, when we had 1 pool token per NANO.
        assert!(pool_token_amount_epoch_4  - pool_token_amount_epoch_5 == 50_000_000_000, 0);

        // Test 3: from epoch 5 to 6, no IOTAs were added or removed from the pool. 
        // The number of pool tokens should be constant even with the distribution of rewards.
        assert!(pool_token_amount_epoch_5 == pool_token_amount_epoch_6, 0);

        test_scenario::return_shared(system_state);
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
            create_validator_for_testing(VALIDATOR_ADDR_1, 100, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_2, 200, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_3, 300, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_4, 400, ctx),
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

        let validators = vector[
            create_validator_for_testing(VALIDATOR_ADDR_1, 100000000, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_2, 200000000, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_3, 300000000, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_4, 400000000, ctx),
        ];
        create_iota_system_state_for_testing(validators, 1000000000, 0, ctx);
        scenario_val.end();
    }

    fun validator_addrs() : vector<address> {
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, VALIDATOR_ADDR_3, VALIDATOR_ADDR_4]
    }

    fun set_commission_rate_and_advance_epoch(addr: address, commission_rate: u64, scenario: &mut Scenario) {
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

}
