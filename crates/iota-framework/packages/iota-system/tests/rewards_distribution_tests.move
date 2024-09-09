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
        advance_epoch_with_reward_amounts,
        advance_epoch_with_reward_amounts_and_slashing_rates,
        assert_validator_total_stake_amounts,
        assert_validator_non_self_stake_amounts,
        assert_validator_self_stake_amounts,
        create_validator_for_testing,
        create_iota_system_state_for_testing,
        stake_with,
        total_iota_balance, unstake
    };
    use iota::test_utils::assert_eq;
    use iota::address;

    const VALIDATOR_ADDR_1: address = @0x1;
    const VALIDATOR_ADDR_2: address = @0x2;
    const VALIDATOR_ADDR_3: address = @0x3;
    const VALIDATOR_ADDR_4: address = @0x4;

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

        // need to advance epoch so validator's staking starts counting
        advance_epoch(scenario);

        advance_epoch_with_reward_amounts(0, 100, scenario);
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[125 * NANOS_PER_IOTA, 225 * NANOS_PER_IOTA, 325 * NANOS_PER_IOTA, 425 * NANOS_PER_IOTA],
            scenario
        );

        stake_with(VALIDATOR_ADDR_2, VALIDATOR_ADDR_2, 720, scenario);

        advance_epoch(scenario);
        advance_epoch_with_reward_amounts(0, 100, scenario);
        // Even though validator 2 has a lot more stake now, it should not get more rewards because
        // the voting power is capped at 10%.
        assert_validator_total_stake_amounts(
            validator_addrs(),
            vector[150 * NANOS_PER_IOTA, 970 * NANOS_PER_IOTA, 350 * NANOS_PER_IOTA, 450 * NANOS_PER_IOTA],
            scenario
        );
        scenario_val.end();
    }

    #[test]
    fun test_stake_subsidy() {
        set_up_iota_system_state_with_big_amounts();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        // need to advance epoch so validator's staking starts counting
        advance_epoch(scenario);

        advance_epoch_with_reward_amounts(0, 100, scenario);
        assert_validator_total_stake_amounts(validator_addrs(), vector[100_000_025 * NANOS_PER_IOTA, 200_000_025 * NANOS_PER_IOTA, 300_000_025 * NANOS_PER_IOTA, 400_000_025 * NANOS_PER_IOTA], scenario);
        scenario_val.end();
    }

    #[test]
    fun test_stake_rewards() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 200, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);
        advance_epoch(scenario);

        assert_validator_total_stake_amounts(validator_addrs(), vector[300 * NANOS_PER_IOTA, 300 * NANOS_PER_IOTA, 300 * NANOS_PER_IOTA, 400 * NANOS_PER_IOTA], scenario);
        assert_validator_self_stake_amounts(validator_addrs(), vector[100 * NANOS_PER_IOTA, 200 * NANOS_PER_IOTA, 300 * NANOS_PER_IOTA, 400 * NANOS_PER_IOTA], scenario);

        // Each pool gets 30 IOTA.
        advance_epoch_with_reward_amounts(0, 120, scenario);
        assert_validator_self_stake_amounts(validator_addrs(), vector[110 * NANOS_PER_IOTA, 220 * NANOS_PER_IOTA, 330 * NANOS_PER_IOTA, 430 * NANOS_PER_IOTA], scenario);
        unstake(STAKER_ADDR_1, 0, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 600, scenario);
        // Each pool gets 30 IOTA.
        advance_epoch_with_reward_amounts(0, 120, scenario);
        // staker 1 receives only 20 IOTA of rewards, not 40 since we are using pre-epoch exchange rate.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 220 * NANOS_PER_IOTA);
        assert_validator_self_stake_amounts(validator_addrs(), vector[140 * NANOS_PER_IOTA, 240 * NANOS_PER_IOTA, 360 * NANOS_PER_IOTA, 460 * NANOS_PER_IOTA], scenario);
        unstake(STAKER_ADDR_2, 0, scenario);
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 120 * NANOS_PER_IOTA); // 20 IOTA of rewards received

        advance_epoch_with_reward_amounts(0, 40, scenario);

        unstake(STAKER_ADDR_2, 0, scenario); // unstake 600 principal IOTA
        // additional 600 IOTA of principal and 46 IOTA of rewards withdrawn to Coin<IOTA>
        // For this stake, the staking exchange rate is 100 : 140 and the unstaking
        // exchange rate is 528 : 750 -ish so the total iota withdraw will be:
        // (600 * 100 / 140) * 750 / 528 = ~608. Together with the 120 IOTA we already have,
        // that would be about 728 IOTA.
        // TODO: Come up with better numbers and clean it up!
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 728108108107);
        scenario_val.end();
    }

    #[test]
    fun test_stake_tiny_rewards() {
        set_up_iota_system_state_with_big_amounts();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        // stake a large amount
        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 200000000, scenario);

        advance_epoch(scenario);

        advance_epoch_with_reward_amounts(0, 150000, scenario);

        // stake a small amount
        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 10, scenario);
        advance_epoch_with_reward_amounts(0, 130, scenario);

        // unstake the stakes
        unstake(STAKER_ADDR_1, 1, scenario);

        // and advance epoch should succeed
        advance_epoch_with_reward_amounts(0, 150, scenario);
        scenario_val.end();
    }

    #[test]
    fun test_validator_commission() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);
        advance_epoch(scenario);
        // V1: 200, V2: 300, V3: 300, V4: 400

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_2, 2000, scenario); // 50% commission
        advance_epoch_with_reward_amounts(0, 120, scenario);
        // V1: 230, V2: 330, V3: 330, V4: 430
        // 2 IOTA, or 20 % of staker_2's rewards, goes to validator_2
        assert_validator_non_self_stake_amounts(validator_addrs(), vector[115 * NANOS_PER_IOTA, 108 * NANOS_PER_IOTA, 0, 0], scenario);
        assert_validator_self_stake_amounts(validator_addrs(), vector[115 * NANOS_PER_IOTA, 222 * NANOS_PER_IOTA, 330 * NANOS_PER_IOTA, 430 * NANOS_PER_IOTA], scenario);

        set_commission_rate_and_advance_epoch(VALIDATOR_ADDR_1, 1000, scenario); // 10% commission

        advance_epoch_with_reward_amounts(0, 240, scenario);
        assert_validator_total_stake_amounts(validator_addrs(), vector[290 * NANOS_PER_IOTA, 390 * NANOS_PER_IOTA, 390 * NANOS_PER_IOTA, 490 * NANOS_PER_IOTA], scenario);

        // Staker 1 rewards in the recent distribution is 0.9 x 30 = 27 IOTA
        // Validator 1 rewards in the recent distribution is 60 - 27 = 33 IOTA

        // Staker 2 amounts for 0.8 * 60 * (108 / 330) + 108 = 123.709 IOTA
        // Validator 2 amounts for 390 - 123.709 = 266.291 IOTA
        assert_validator_non_self_stake_amounts(validator_addrs(), vector[142 * NANOS_PER_IOTA, 123709090909, 0, 0], scenario);
        assert_validator_self_stake_amounts(validator_addrs(), vector[148 * NANOS_PER_IOTA, 266290909091, 390 * NANOS_PER_IOTA, 490 * NANOS_PER_IOTA], scenario);

        scenario_val.end();
    }

    #[test]
    fun test_rewards_slashing() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        advance_epoch(scenario);

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

        advance_epoch(scenario);

        // validator_2 is reported by 3 other validators, so 75% of total stake.
        report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
        report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
        report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);

        // validator_1 is reported by only 1 other validator, which is 25% of total stake.
        report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_1, scenario);

        // 3600 IOTA of total rewards, 50% threshold and 10% reward slashing.
        // So validator_2 is the only one whose rewards should get slashed.
        advance_epoch_with_reward_amounts_and_slashing_rates(
            0, 3600, 1000, scenario
        );

        // Without reward slashing, the validator's stakes should be [100+450, 200+600, 300+900, 400+900]
        // after the last epoch advancement.
        // Since 60 IOTA, or 10% of validator_2's rewards (600) are slashed, she only has 800 - 60 = 740 now.
        // There are in total 90 IOTA of rewards slashed (60 from the validator, and 30 from her staker)
        // so the unslashed validators each get their share of additional rewards, which is 30.
        assert_validator_self_stake_amounts(validator_addrs(), vector[565 * NANOS_PER_IOTA, 740 * NANOS_PER_IOTA, 1230 * NANOS_PER_IOTA, 1330 * NANOS_PER_IOTA], scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Same analysis as above. Delegator 1 has 3 additional IOTA, and 10% of staker 2's rewards are slashed.
        assert!(total_iota_balance(STAKER_ADDR_1, scenario) == 565 * NANOS_PER_IOTA);
        assert!(total_iota_balance(STAKER_ADDR_2, scenario) == 370 * NANOS_PER_IOTA);
        scenario_val.end();
    }

    #[test]
    fun test_entire_rewards_slashing() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        advance_epoch(scenario);

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, scenario);

        advance_epoch(scenario);

        // validator_2 is reported by 3 other validators, so 75% of total stake.
        report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_2, scenario);
        report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_2, scenario);
        report_validator(VALIDATOR_ADDR_4, VALIDATOR_ADDR_2, scenario);


        // 3600 IOTA of total rewards, 100% reward slashing.
        // So validator_2 is the only one whose rewards should get slashed.
        advance_epoch_with_reward_amounts_and_slashing_rates(
            0, 3600, 10_000, scenario
        );

        // Without reward slashing, the validator's stakes should be [100+450, 200+600, 300+900, 400+900]
        // after the last epoch advancement.
        // The entire rewards of validator 2's staking pool are slashed, which is 900 IOTA.
        // so the unslashed validators each get their share of additional rewards, which is 300.
        assert_validator_self_stake_amounts(validator_addrs(), vector[(550 + 150) * NANOS_PER_IOTA, 200 * NANOS_PER_IOTA, (1200 + 300) * NANOS_PER_IOTA, (1300 + 300) * NANOS_PER_IOTA], scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Same analysis as above. Staker 1 has 150 additional IOTA, and since all of staker 2's rewards are slashed she only gets back her principal.
        assert!(total_iota_balance(STAKER_ADDR_1, scenario) == (550 + 150) * NANOS_PER_IOTA);
        assert!(total_iota_balance(STAKER_ADDR_2, scenario) == 100 * NANOS_PER_IOTA);
        scenario_val.end();
    }

    #[test]
    fun test_rewards_slashing_with_storage_fund() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        // Put 300 IOTA into the storage fund.
        advance_epoch_with_reward_amounts(300, 0, scenario);

        // Add a few stakes.
        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_3, 100, scenario);
        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_4, 100, scenario);
        advance_epoch(scenario);

        // validator_4 is reported by 3 other validators, so 75% of total stake.
        report_validator(VALIDATOR_ADDR_1, VALIDATOR_ADDR_4, scenario);
        report_validator(VALIDATOR_ADDR_2, VALIDATOR_ADDR_4, scenario);
        report_validator(VALIDATOR_ADDR_3, VALIDATOR_ADDR_4, scenario);

        // 1000 IOTA of storage rewards, 1500 IOTA of computation rewards, 50% slashing threshold
        // and 20% slashing rate
        advance_epoch_with_reward_amounts_and_slashing_rates(
            1000, 1500, 2000, scenario
        );

        // Each unslashed validator staking pool gets 300 IOTA of computation rewards + 75 IOTA of storage fund rewards +
        // 20 IOTA (1/3) of validator 4's slashed computation reward and 5 IOTA (1/3) of validator 4's slashed
        // storage fund reward, so in total it gets 400 IOTA of rewards.
        // Validator 3 has a delegator with her so she gets 320 * 3/4 + 75 + 5 = 320 IOTA of rewards.
        // Validator 4's should get 300 * 4/5 * (1 - 20%) = 192 in computation rewards and 75 * (1 - 20%) = 60 in storage rewards.
        assert_validator_self_stake_amounts(validator_addrs(), vector[500 * NANOS_PER_IOTA, 600 * NANOS_PER_IOTA, 620 * NANOS_PER_IOTA, 652 * NANOS_PER_IOTA], scenario);

        // Unstake so we can check the stake rewards as well.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);

        // Staker 1 gets 320 * 1/4 = 80 IOTA of rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (100 + 80) * NANOS_PER_IOTA);
        // Staker 2 gets 300 * 1/5 * (1 - 20%) = 48 IOTA of rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (100 + 48) * NANOS_PER_IOTA);

        scenario_val.end();
    }

    #[test]
    fun test_everyone_slashed() {
        // This test is to make sure that if everyone is slashed, our protocol works as expected without aborting
        // and all rewards go to the storage fund.
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

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

        advance_epoch_with_reward_amounts_and_slashing_rates(
            1000, 3000, 10_000, scenario
        );

        // All validators should have 0 rewards added so their stake stays the same.
        assert_validator_self_stake_amounts(validator_addrs(), vector[100 * NANOS_PER_IOTA, 200 * NANOS_PER_IOTA, 300 * NANOS_PER_IOTA, 400 * NANOS_PER_IOTA], scenario);

        scenario.next_tx(@0x0);
        // Storage fund balance should increase by 4000 IOTA.
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert_eq(system_state.get_storage_fund_total_balance(), 4000 * NANOS_PER_IOTA);

        // The entire 1000 IOTA of storage rewards should go to the object rebate portion of the storage fund.
        assert_eq(system_state.get_storage_fund_object_rebates(), 1000 * NANOS_PER_IOTA);

        test_scenario::return_shared(system_state);
        scenario_val.end();
    }

    #[test]
    fun test_mul_rewards_withdraws_at_same_epoch() {
        set_up_iota_system_state();
        let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 220, scenario);

        advance_epoch_with_reward_amounts(0, 40, scenario);

        stake_with(STAKER_ADDR_2, VALIDATOR_ADDR_1, 480, scenario);

        // Staker 1 gets 2/3 * 1/4 * 120 = 20 IOTA here.
        advance_epoch_with_reward_amounts(0, 120, scenario);

        stake_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 130, scenario);
        stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 390, scenario);

        // Staker 1 gets 20 IOTA here and staker 2 gets 40 IOTA here.
        advance_epoch_with_reward_amounts(0, 280, scenario);
        stake_with(STAKER_ADDR_3, VALIDATOR_ADDR_1, 280, scenario);
        stake_with(STAKER_ADDR_4, VALIDATOR_ADDR_1, 1400, scenario);

        // Staker 1 gets 30 IOTA, staker 2 gets 40 IOTA and staker 3 gets 30 IOTA.
        advance_epoch_with_reward_amounts(0, 440, scenario);

        scenario.next_tx(@0x0);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        // Check that we have the right amount of IOTA in the staking pool.
        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 140 * 23 * NANOS_PER_IOTA);
        test_scenario::return_shared(system_state);

        // Withdraw all stakes at once.
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_1, 0, scenario);
        unstake(STAKER_ADDR_2, 0, scenario);
        unstake(STAKER_ADDR_3, 0, scenario);
        unstake(STAKER_ADDR_3, 0, scenario);
        unstake(STAKER_ADDR_4, 0, scenario);

        // staker 1's first stake was active for 3 epochs so got 20 * 3 = 60 IOTA of rewards
        // and her second stake was active for only one epoch and got 10 IOTA of rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), (220 + 130 + 20 * 3 + 10) * NANOS_PER_IOTA);
        // staker 2's stake was active for 2 epochs so got 40 * 2 = 80 IOTA of rewards
        assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), (480 + 40 * 2) * NANOS_PER_IOTA);
        // staker 3's first stake was active for 1 epoch and got 30 IOTA of rewards
        // and her second stake didn't get any rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_3, scenario), (390 + 280 + 30) * NANOS_PER_IOTA);
        // staker 4 joined and left in an epoch where no rewards were earned so she got no rewards.
        assert_eq(total_iota_balance(STAKER_ADDR_4, scenario), 1400 * NANOS_PER_IOTA);

        advance_epoch_with_reward_amounts(0, 0, scenario);

        scenario.next_tx(@0x0);
        let mut system_state = scenario.take_shared<IotaSystemState>();
        // Since all the stakes are gone the pool is empty except for the validator's original stake.
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
        advance_epoch_with_reward_amounts(0, 10000, scenario);

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
}
