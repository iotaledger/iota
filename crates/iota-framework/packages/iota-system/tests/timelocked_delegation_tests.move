// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::timelocked_stake_tests;

use iota::balance::{Self, Balance};
use iota::clock;
use iota::coin::Coin;
use iota::iota::IOTA;
use iota::labeler::LabelerCap;
use iota::table::Table;
use iota::test_label_one::{Self, TEST_LABEL_ONE};
use iota::test_label_two::{Self, TEST_LABEL_TWO};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{Self, assert_eq};
use iota::timelock::{Self, TimeLock};
use iota_system::governance_test_utils::{
    add_validator,
    add_validator_candidate,
    advance_epoch,
    advance_epoch_with_balanced_reward_amounts,
    advance_epoch_with_amounts,
    assert_validator_total_stake_amounts,
    create_iota_system_state_for_testing,
    create_validators_with_stakes_and_commission_rates,
    remove_validator,
    remove_validator_candidate,
    total_iota_balance,
    unstake
};
use iota_system::iota_system::IotaSystemState;
use iota_system::staking_pool::{Self, PoolTokenExchangeRate};
use iota_system::timelocked_staking::{Self, TimelockedStakedIota};
use iota_system::validator_set;

const VALIDATOR_ADDR_1: address = @0x1;
const VALIDATOR_ADDR_2: address = @0x2;

const STAKER_ADDR_1: address = @0x42;
const STAKER_ADDR_2: address = @0x43;
const STAKER_ADDR_3: address = @0x44;

const NEW_VALIDATOR_ADDR: address =
    @0x1a4623343cd42be47d67314fce0ad042f3c82685544bc91d8c11d24e74ba7357;
// Generated with seed [0;32]
const NEW_VALIDATOR_PUBKEY: vector<u8> =
    x"99f25ef61f8032b914636460982c5cc6f134ef1ddae76657f2cbfec1ebfc8d097374080df6fcf0dcb8bc4b0d8e0af5d80ebbff2b4c599f54f42d6312dfc314276078c1cc347ebbbec5198be258513f386b930d02c2749a803e2330955ebd1a10";
// Generated using [fn test_proof_of_possession]
const NEW_VALIDATOR_POP: vector<u8> =
    x"8b93fc1b33379e2796d361c4056f0f04ad5aea7f4a8c02eaac57340ff09b6dc158eb1945eece103319167f420daf0cb3";

const NANOS_PER_IOTA: u64 = 1_000_000_000;

#[test]
fun test_split_join_staked_iota() {
    // All this is just to generate a dummy StakedIota object to split and join later
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 10, scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        let ctx = scenario.ctx();
        staked_iota.split_to_sender(20 * NANOS_PER_IOTA, ctx);
        scenario.return_to_sender(staked_iota);
    };

    // Verify the correctness of the split and send the join txn
    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
        assert!(staked_iota_ids.length() == 2, 101); // staked iota split to 2 coins

        let mut part1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[0]);
        let part2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[1]);

        let amount1 = part1.amount();
        let amount2 = part2.amount();
        assert!(amount1 == 20 * NANOS_PER_IOTA || amount1 == 40 * NANOS_PER_IOTA, 102);
        assert!(amount2 == 20 * NANOS_PER_IOTA || amount2 == 40 * NANOS_PER_IOTA, 103);
        assert!(amount1 + amount2 == 60 * NANOS_PER_IOTA, 104);

        part1.join(part2);
        assert!(part1.amount() == 60 * NANOS_PER_IOTA, 105);
        scenario.return_to_sender(part1);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = timelocked_staking::EIncompatibleTimelockedStakedIota)]
fun test_join_different_epochs() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Create two instances of staked iota w/ different epoch activations
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 10, scenario);
    advance_epoch(scenario);
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 10, scenario);

    // Verify that these cannot be merged
    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
        let mut part1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[0]);
        let part2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[1]);

        part1.join(part2);

        scenario.return_to_sender(part1);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = timelocked_staking::EIncompatibleTimelockedStakedIota)]
fun test_join_different_timestamps() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Create two instances of staked iota w/ different epoch activations
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 10, scenario);
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 20, scenario);

    // Verify that these cannot be merged
    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
        let mut part1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[0]);
        let part2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[1]);

        part1.join(part2);

        scenario.return_to_sender(part1);
    };
    scenario_val.end();
}

#[test]
fun test_join_same_labels() {
    set_up_iota_system_state();

    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;

    set_up_timelock_labeler_caps(STAKER_ADDR_1, scenario);

    // Create two instances of labeled staked iota w/ different amounts
    scenario.next_tx(STAKER_ADDR_1);
    {
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

        stake_labeled_timelocked_with(
            &labeler_one,
            STAKER_ADDR_1,
            VALIDATOR_ADDR_1,
            60,
            10,
            scenario,
        );
        stake_labeled_timelocked_with(
            &labeler_one,
            STAKER_ADDR_1,
            VALIDATOR_ADDR_1,
            50,
            10,
            scenario,
        );

        scenario.return_to_sender(labeler_one);
    };

    // Verify that these can be merged
    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
        let mut part1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[0]);
        let part2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[1]);

        part1.join(part2);

        assert_eq(part1.staked_iota_amount(), 110 * NANOS_PER_IOTA);
        assert_eq(part1.expiration_timestamp_ms(), 10);
        assert_eq(part1.is_labeled_with<TEST_LABEL_ONE>(), true);

        scenario.return_to_sender(part1);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = timelocked_staking::EIncompatibleTimelockedStakedIota)]
fun test_join_different_labels() {
    set_up_iota_system_state();

    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;

    set_up_timelock_labeler_caps(STAKER_ADDR_1, scenario);

    // Create two instances of labeled staked iota w/ different labels
    scenario.next_tx(STAKER_ADDR_1);
    {
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();
        let labeler_two = scenario.take_from_sender<LabelerCap<TEST_LABEL_TWO>>();

        stake_labeled_timelocked_with(
            &labeler_one,
            STAKER_ADDR_1,
            VALIDATOR_ADDR_1,
            60,
            10,
            scenario,
        );
        stake_labeled_timelocked_with(
            &labeler_two,
            STAKER_ADDR_1,
            VALIDATOR_ADDR_1,
            60,
            10,
            scenario,
        );

        scenario.return_to_sender(labeler_one);
        scenario.return_to_sender(labeler_two);
    };

    // Verify that these cannot be merged
    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
        let mut part1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[0]);
        let part2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(staked_iota_ids[1]);

        part1.join(part2);

        scenario.return_to_sender(part1);
    };
    scenario_val.end();
}

#[test]
fun test_split_with_labels() {
    set_up_iota_system_state();

    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;

    set_up_timelock_labeler_caps(STAKER_ADDR_1, scenario);

    // Create one instance of labeled staked iota
    scenario.next_tx(STAKER_ADDR_1);
    {
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

        stake_labeled_timelocked_with(
            &labeler_one,
            STAKER_ADDR_1,
            VALIDATOR_ADDR_1,
            60,
            10,
            scenario,
        );

        scenario.return_to_sender(labeler_one);

        advance_epoch(scenario);
    };

    // Verify that it can be split
    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut original = scenario.take_from_sender<TimelockedStakedIota>();
        let split = original.split(20 * NANOS_PER_IOTA, scenario.ctx());

        assert_eq(original.staked_iota_amount(), 40 * NANOS_PER_IOTA);
        assert_eq(original.expiration_timestamp_ms(), 10);
        assert_eq(original.is_labeled_with<TEST_LABEL_ONE>(), true);

        assert_eq(split.staked_iota_amount(), 20 * NANOS_PER_IOTA);
        assert_eq(split.expiration_timestamp_ms(), 10);
        assert_eq(split.is_labeled_with<TEST_LABEL_ONE>(), true);

        scenario.return_to_sender(original);
        test_utils::destroy(split);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = staking_pool::EStakedIotaBelowThreshold)]
fun test_split_below_threshold() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        let ctx = scenario.ctx();
        // The remaining amount after splitting is below the threshold so this should fail.
        staked_iota.split_to_sender(1 * NANOS_PER_IOTA + 1, ctx);
        scenario.return_to_sender(staked_iota);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = staking_pool::EStakedIotaBelowThreshold)]
fun test_split_nonentry_below_threshold() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        let ctx = scenario.ctx();
        // The remaining amount after splitting is below the threshold so this should fail.
        let stake = staked_iota.split(1 * NANOS_PER_IOTA + 1, ctx);
        test_utils::destroy(stake);
        scenario.return_to_sender(staked_iota);
    };
    scenario_val.end();
}

#[test]
fun test_add_remove_stake_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let ctx = scenario.ctx();

        // Create a stake to VALIDATOR_ADDR_1.
        timelocked_staking::request_add_stake(
            system_state_mut_ref,
            timelock::lock(balance::create_for_testing(60 * NANOS_PER_IOTA), 10, ctx),
            VALIDATOR_ADDR_1,
            ctx,
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        let ctx = scenario.ctx();

        // Unstake from VALIDATOR_ADDR_1
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota, ctx);

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 100 * NANOS_PER_IOTA);
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_add_remove_labeled_stake_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    set_up_timelock_labeler_caps(STAKER_ADDR_1, scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let ctx = scenario.ctx();

        // Create a stake to VALIDATOR_ADDR_1.
        timelocked_staking::request_add_stake(
            system_state_mut_ref,
            timelock::lock_with_label(
                &labeler_one,
                balance::create_for_testing(60 * NANOS_PER_IOTA),
                10,
                ctx,
            ),
            VALIDATOR_ADDR_1,
            ctx,
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);

        scenario.return_to_sender(labeler_one);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        // Unstake from VALIDATOR_ADDR_1
        timelocked_staking::request_withdraw_stake(
            system_state_mut_ref,
            staked_iota,
            scenario.ctx(),
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();

        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 100 * NANOS_PER_IOTA);

        // Check the time-locked balance.
        let timelock = scenario.take_from_sender<TimeLock<Balance<IOTA>>>();

        assert_eq(timelock.locked().value(), 60 * NANOS_PER_IOTA);
        assert_eq(timelock.expiration_timestamp_ms(), 10);
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);

        scenario.return_to_sender(timelock);

        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_add_remove_stake_mul_bal_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let ctx = scenario.ctx();

        let mut balances = vector[];

        balances.push_back(
            timelock::lock(balance::create_for_testing(30 * NANOS_PER_IOTA), 10, ctx),
        );
        balances.push_back(
            timelock::lock(balance::create_for_testing(60 * NANOS_PER_IOTA), 20, ctx),
        );

        // Create a stake to VALIDATOR_ADDR_1.
        timelocked_staking::request_add_stake_mul_bal(
            system_state_mut_ref,
            balances,
            VALIDATOR_ADDR_1,
            ctx,
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let stake_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();

        let staked_iota1 = scenario.take_from_sender_by_id<TimelockedStakedIota>(stake_iota_ids[0]);
        assert_eq(staked_iota1.amount(), 30 * NANOS_PER_IOTA);
        let staked_iota2 = scenario.take_from_sender_by_id<TimelockedStakedIota>(stake_iota_ids[1]);
        assert_eq(staked_iota2.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            190 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        let ctx = scenario.ctx();

        // First unstake from VALIDATOR_ADDR_1
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota1, ctx);

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            190 * NANOS_PER_IOTA,
        );

        scenario.return_to_sender(staked_iota2);
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        let ctx = scenario.ctx();

        // Second unstake from VALIDATOR_ADDR_1
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota, ctx);

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        assert_eq(scenario.has_most_recent_for_sender<TimelockedStakedIota>(), false);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);
    };

    scenario_val.end();
}

#[test]
fun test_unlock_expired() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    // Increment epoch timestamp.
    scenario.ctx().increment_epoch_timestamp(10);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        // The `timelocked_staked_iota` is expired so it should be unlocked.
        let staked_iota = timelocked_staked_iota.unlock(scenario.ctx());

        assert_eq(staked_iota.amount(), 2 * NANOS_PER_IOTA);

        test_utils::destroy(staked_iota);
    };
    scenario_val.end();
}

#[test]
fun test_unlock_with_clock_expired() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    // Create a clock object.
    let mut clock = clock::create_for_testing(scenario.ctx());

    // Increment the clock timestamp.
    clock.increment_for_testing(10);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        // The `timelocked_staked_iota` is expired so it should be unlocked.
        let staked_iota = timelocked_staked_iota.unlock_with_clock(&clock);

        assert_eq(staked_iota.amount(), 2 * NANOS_PER_IOTA);

        test_utils::destroy(staked_iota);
    };

    clock::destroy_for_testing(clock);

    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = timelocked_staking::ETimelockedStakedIotaShouldBeExpired)]
fun test_unlock_not_expired() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        // The `timelocked_staked_iota` is still not-expired so this should fail.
        let staked_iota = timelocked_staked_iota.unlock(scenario.ctx());
        test_utils::destroy(staked_iota);
    };
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = timelocked_staking::ETimelockedStakedIotaShouldBeExpired)]
fun test_unlock_with_clock_not_expired() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(STAKER_ADDR_1);
    let scenario = &mut scenario_val;
    // Stake 2 IOTA
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 2, 10, scenario);

    // Create a clock object.
    let clock = clock::create_for_testing(scenario.ctx());

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        // The `timelocked_staked_iota` is still not-expired so this should fail.
        let staked_iota = timelocked_staked_iota.unlock_with_clock(&clock);
        test_utils::destroy(staked_iota);
    };

    clock::destroy_for_testing(clock);

    scenario_val.end();
}

#[test]
fun test_add_unlock_remove_stake_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let ctx = scenario.ctx();

        // Create a stake to VALIDATOR_ADDR_1.
        timelocked_staking::request_add_stake(
            system_state_mut_ref,
            timelock::lock(balance::create_for_testing(60 * NANOS_PER_IOTA), 10, ctx),
            VALIDATOR_ADDR_1,
            ctx,
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);
    };

    // Increment epoch timestamp.
    scenario.ctx().increment_epoch_timestamp(10);

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(timelocked_staked_iota.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        let ctx = scenario.ctx();

        // Unlock the staked iota.
        let staked_iota = timelocked_staked_iota.unlock(ctx);

        // Unstake from VALIDATOR_ADDR_1
        system_state_mut_ref.request_withdraw_stake(staked_iota, ctx);

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 100 * NANOS_PER_IOTA);
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_add_unlock_with_clock_remove_stake_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let ctx = scenario.ctx();

        // Create a stake to VALIDATOR_ADDR_1.
        timelocked_staking::request_add_stake(
            system_state_mut_ref,
            timelock::lock(balance::create_for_testing(60 * NANOS_PER_IOTA), 10, ctx),
            VALIDATOR_ADDR_1,
            ctx,
        );

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            100 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        test_scenario::return_shared(system_state);
    };

    // Create a clock object.
    let mut clock = clock::create_for_testing(scenario.ctx());

    // Increment the clock timestamp.
    clock.increment_for_testing(10);

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let timelocked_staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(timelocked_staked_iota.amount(), 60 * NANOS_PER_IOTA);

        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_2),
            100 * NANOS_PER_IOTA,
        );

        let ctx = scenario.ctx();

        // Unlock the staked iota.
        let staked_iota = timelocked_staked_iota.unlock_with_clock(&clock);

        // Unstake from VALIDATOR_ADDR_1
        system_state_mut_ref.request_withdraw_stake(staked_iota, ctx);

        assert_eq(
            system_state_mut_ref.validator_stake_amount(VALIDATOR_ADDR_1),
            160 * NANOS_PER_IOTA,
        );
        test_scenario::return_shared(system_state);
    };

    advance_epoch(scenario);

    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert_eq(system_state.validator_stake_amount(VALIDATOR_ADDR_1), 100 * NANOS_PER_IOTA);
        test_scenario::return_shared(system_state);
    };
    clock::destroy_for_testing(clock);
    scenario_val.end();
}

#[test]
fun test_remove_stake_post_active_flow_no_rewards() {
    set_up_iota_system_state_with_storage_fund();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 10, scenario);

    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[200 * NANOS_PER_IOTA, 100 * NANOS_PER_IOTA],
        scenario,
    );

    advance_epoch(scenario);

    remove_validator(VALIDATOR_ADDR_1, scenario);

    advance_epoch(scenario);

    // Make sure stake withdrawal happens
    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert!(
            !system_state_mut_ref
                .validators()
                .is_active_validator_by_iota_address(VALIDATOR_ADDR_1),
            0,
        );

        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 100 * NANOS_PER_IOTA);

        // Unstake from VALIDATOR_ADDR_1
        assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 1);
        let ctx = scenario.ctx();
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota, ctx);

        // Make sure they have all of their stake.
        assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
        assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 2);

        test_scenario::return_shared(system_state);
    };

    // Validator unstakes now.
    assert!(!has_iota_coins(VALIDATOR_ADDR_1, scenario), 3);
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Make sure have all of their stake. NB there is no epoch change. This is immediate.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), 100 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_remove_stake_post_active_flow_with_rewards() {
    set_up_iota_system_state_with_storage_fund();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 10, scenario);

    advance_epoch(scenario);

    assert_validator_total_stake_amounts(
        vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
        vector[200 * NANOS_PER_IOTA, 100 * NANOS_PER_IOTA],
        scenario,
    );

    // Each validator pool gets 40 IOTA.
    advance_epoch_with_balanced_reward_amounts(0, 80, scenario);

    remove_validator(VALIDATOR_ADDR_1, scenario);

    advance_epoch(scenario);

    // With IIP-8 dynamic commission (50% for 2 validators), staker gets half the pool reward.
    let staker_reward_amt = 10 * NANOS_PER_IOTA;

    // Make sure stake withdrawal happens
    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert!(
            !system_state_mut_ref
                .validators()
                .is_active_validator_by_iota_address(VALIDATOR_ADDR_1),
            0,
        );

        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 100 * NANOS_PER_IOTA);

        // Unstake from VALIDATOR_ADDR_1
        assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 1);
        let ctx = scenario.ctx();
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota, ctx);

        // Make sure they have all of their stake.
        assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), staker_reward_amt);

        test_scenario::return_shared(system_state);
    };

    // Validator unstakes now.
    assert!(!has_iota_coins(VALIDATOR_ADDR_1, scenario), 2);
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    // Unstake commission StakedIota created by IIP-8 dynamic commission.
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Make sure have all of their stake. NB there is no epoch change. This is immediate.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), 130 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_earns_rewards_at_last_epoch() {
    set_up_iota_system_state_with_storage_fund();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 10, scenario);

    advance_epoch(scenario);

    remove_validator(VALIDATOR_ADDR_1, scenario);

    // Add some rewards after the validator requests to leave. Since the validator is still active
    // this epoch, they should get the rewards from this epoch.
    advance_epoch_with_balanced_reward_amounts(0, 80, scenario);

    // Each validator pool gets 40 IOTA. With IIP-8 dynamic commission (50% for 2 validators),
    // staker gets half the pool reward, validator gets its half plus commission.
    let staker_reward_amt = 10 * NANOS_PER_IOTA;

    // Make sure stake withdrawal happens
    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        let staked_iota = scenario.take_from_sender<TimelockedStakedIota>();
        assert_eq(staked_iota.amount(), 100 * NANOS_PER_IOTA);

        // Unstake from VALIDATOR_ADDR_1
        assert!(!has_timelocked_iota_balance(STAKER_ADDR_1, scenario), 0);
        assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 1);
        let ctx = scenario.ctx();
        timelocked_staking::request_withdraw_stake(system_state_mut_ref, staked_iota, ctx);

        // Make sure they have all of their stake.
        assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
        assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), staker_reward_amt);

        test_scenario::return_shared(system_state);
    };

    // Validator unstakes now.
    assert!(!has_iota_coins(VALIDATOR_ADDR_1, scenario), 2);
    unstake(VALIDATOR_ADDR_1, 0, scenario);
    // Unstake commission StakedIota created by IIP-8 dynamic commission.
    unstake(VALIDATOR_ADDR_1, 0, scenario);

    // Make sure have all of their stake. NB there is no epoch change. This is immediate.
    assert_eq(total_iota_balance(VALIDATOR_ADDR_1, scenario), 130 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = validator_set::ENotAValidator)]
fun test_add_stake_post_active_flow() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 10, scenario);

    advance_epoch(scenario);

    remove_validator(VALIDATOR_ADDR_1, scenario);

    advance_epoch(scenario);

    // Make sure the validator is no longer active.
    scenario.next_tx(STAKER_ADDR_1);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let system_state_mut_ref = &mut system_state;

        assert!(
            !system_state_mut_ref
                .validators()
                .is_active_validator_by_iota_address(VALIDATOR_ADDR_1),
            0,
        );

        test_scenario::return_shared(system_state);
    };

    // Now try and stake to the old validator/staking pool. This should fail!
    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 60, 10, scenario);

    scenario_val.end();
}

#[test]
fun test_add_preactive_remove_preactive() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name5",
        b"/ip4/127.0.0.1/udp/85",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 NANOS to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Advance epoch twice with some rewards
    advance_epoch_with_balanced_reward_amounts(0, 400, scenario);
    advance_epoch_with_balanced_reward_amounts(0, 900, scenario);

    // Unstake from the preactive validator. There should be no rewards earned.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 0);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_add_preactive_remove_preactive_same_epoch() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name5",
        b"/ip4/127.0.0.1/udp/85",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 NANOS to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Unstake from the preactive validator. There should be no rewards earned.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 0);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = validator_set::ENotAValidator)]
fun test_add_preactive_remove_pending_failure() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name4",
        b"/ip4/127.0.0.1/udp/84",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    add_validator(NEW_VALIDATOR_ADDR, scenario);

    // Unstake from the now pending validator. This should fail because pending active validators don't accept withdraws.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);

    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = validator_set::ENotAValidator)]
fun test_add_pending_failure() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name4",
        b"/ip4/127.0.0.1/udp/84",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    add_validator(NEW_VALIDATOR_ADDR, scenario);

    // Delegate 100 IOTA to the pending validator. This should fail because pending active validators don't accept
    // new stakes or withdraws.
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    scenario_val.end();
}

#[test]
fun test_add_preactive_remove_active() {
    set_up_iota_system_state_with_storage_fund();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;
    // At this point we got the following distribution of stake:
    // V1: 100, V2: 100, storage fund: 100

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name3",
        b"/ip4/127.0.0.1/udp/83",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 IOTA to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);
    // At this point we got the following distribution of stake:
    // V1: 100, V2: 100, V3: 100, storage fund: 100

    advance_epoch_with_balanced_reward_amounts(0, 300, scenario);
    // At this point we got the following distribution of stake:
    // V1: 250, V2: 250, V3: 100, storage fund: 100

    stake_timelocked_with(STAKER_ADDR_2, NEW_VALIDATOR_ADDR, 50, 10, scenario);
    stake_timelocked_with(STAKER_ADDR_3, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Now the preactive becomes active
    add_validator(NEW_VALIDATOR_ADDR, scenario);
    // Advance one more epoch because a validator can't immediately become a committee member,
    // but first needs to announce its AuthorityCapabilities to other validators,
    // showing that it supports the correct ProtocolVersion.
    advance_epoch(scenario);
    // Now it becomes eligible to be selected to the committee.
    advance_epoch(scenario);
    // At this point we got the following distribution of stake:
    // V1: 250, V2: 250, V3: 250, storage fund: 100

    advance_epoch_with_balanced_reward_amounts(0, 85, scenario);
    // At this point we got the following distribution of stake:
    // V1: 278_330_500_000, V2: 278_330_500_000, V3: 278_339_000_000, storage fund: 100

    // staker 1 and 3 unstake from the validator and earn about
    // 2/5 * (1 - 0.3334) * 85/3 = 7.56 IOTA each (with IIP-8 ~33.34% commission on V3).
    // Although they stake in different epochs, they earn the same rewards as long as they unstake
    // in the same epoch because the validator was preactive when they staked.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 7_556_310_960);

    unstake_timelocked(STAKER_ADDR_3, 0, scenario);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_3, scenario), 100 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_3, scenario), 7_556_310_960);

    advance_epoch_with_balanced_reward_amounts(0, 85, scenario);

    unstake_timelocked(STAKER_ADDR_2, 0, scenario);
    // With IIP-8 (~33.34% commission on V3), staker 2 earns about
    // 1/5 * (1 - 0.3334) * 85/3 = 3.78 IOTA from the previous epoch
    // and about 16.07 IOTA from this one,
    // so in total she earns about 3.78 + 16.07 = 19.85 IOTA in rewards.
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_2, scenario), 50 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 19_845_992_909);

    scenario_val.end();
}

#[test]
fun test_add_preactive_remove_post_active() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name1",
        b"/ip4/127.0.0.1/udp/81",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 IOTA to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Now the preactive becomes active
    add_validator(NEW_VALIDATOR_ADDR, scenario);
    // Advance one more epoch because a validator can't immediately become a committee member,
    // but first needs to announce its AuthorityCapabilities to other validators,
    // showing that it supports the correct ProtocolVersion.
    advance_epoch(scenario);
    // Now it becomes eligible to be selected to the committee.
    advance_epoch(scenario);

    // The new validator's voting power is slightly greater than 1/3 (3334 bp out of 10000).
    // With IIP-8, effective commission = 33.34%, so staker 1 earns about 20 IOTA
    // (pool reward ~30 IOTA minus ~10 IOTA commission).
    advance_epoch_with_balanced_reward_amounts(0, 90, scenario);

    // And now the validator leaves the validator set.
    remove_validator(NEW_VALIDATOR_ADDR, scenario);

    advance_epoch(scenario);

    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    // With IIP-8, V3's commission (33.34%) reduces staker reward.
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 20_001_999_600);

    scenario_val.end();
}

#[test]
fun test_add_preactive_candidate_drop_out() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name2",
        b"/ip4/127.0.0.1/udp/82",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 NANOS to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Advance epoch and give out some rewards. The candidate should get nothing, of course.
    advance_epoch_with_balanced_reward_amounts(0, 800, scenario);

    // Now the candidate leaves.
    remove_validator_candidate(NEW_VALIDATOR_ADDR, scenario);

    // Advance epoch a few times.
    advance_epoch(scenario);
    advance_epoch(scenario);
    advance_epoch(scenario);

    // Unstake now and the staker should get no rewards.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 0);

    scenario_val.end();
}

#[test]
fun test_add_preactive_candidate_drop_out_same_epoch() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    add_validator_candidate(
        NEW_VALIDATOR_ADDR,
        b"name2",
        b"/ip4/127.0.0.1/udp/82",
        NEW_VALIDATOR_PUBKEY,
        NEW_VALIDATOR_POP,
        scenario,
    );

    // Delegate 100 NANOS to the preactive validator
    stake_timelocked_with(STAKER_ADDR_1, NEW_VALIDATOR_ADDR, 100, 10, scenario);

    // Now the candidate leaves.
    remove_validator_candidate(NEW_VALIDATOR_ADDR, scenario);

    // Unstake now and the staker should get no rewards.
    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    assert!(!has_iota_coins(STAKER_ADDR_1, scenario), 0);

    scenario_val.end();
}

#[test]
fun test_staking_pool_exchange_rate_getter() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    stake_timelocked_with(@0x42, @0x2, 100, 10, scenario); // stakes 100 IOTA with 0x2
    scenario.next_tx(@0x42);
    let staked_iota = scenario.take_from_address<TimelockedStakedIota>(@0x42);
    let pool_id = staked_iota.pool_id();
    test_scenario::return_to_address(@0x42, staked_iota);
    advance_epoch(scenario); // advances epoch to effectuate the stake
    // Each staking pool gets 10 IOTA of rewards.
    advance_epoch_with_balanced_reward_amounts(0, 20, scenario);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let rates = system_state.pool_exchange_rates(&pool_id);
    assert_eq(rates.length(), 3);
    assert_exchange_rate_eq(rates, 0, 0, 0); // no tokens at epoch 0
    assert_exchange_rate_eq(rates, 1, 200 * NANOS_PER_IOTA, 200 * NANOS_PER_IOTA); // 200 IOTA of self + delegate stake at epoch 1
    // With IIP-8 dynamic commission (50% for 2 equal validators with 5000 bp voting power
    // each), 5 out of 10 IOTA rewards are taken as commission. The commission is staked back
    // into the pool via `request_add_stake`, minting new pool tokens at the current exchange
    // rate: 5 * NANOS_PER_IOTA * 200 / 210 = 4_878_048_780 new tokens. So the pool_token_amount
    // becomes 200 * NANOS_PER_IOTA + 4_878_048_780 = 204_878_048_780, while iota_amount stays at 210.
    assert_exchange_rate_eq(rates, 2, 210 * NANOS_PER_IOTA, 204_878_048_780);
    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_timelock_validator_subsidy_higher_than_computation_charge() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 60, scenario);
    stake_timelocked_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 100, 60, scenario);
    advance_epoch(scenario);
    // V1: 200, V2: 200

    advance_epoch_with_amounts(800, 0, 400, 400, scenario);

    // The computation charge burned is lower than the validator subsidy, so 400 IOTA should be minted.
    // Each validator pool has 50% of the voting power and thus gets 50% of the reward (400 IOTA).
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[(200 + 400) * NANOS_PER_IOTA, (200 + 400) * NANOS_PER_IOTA],
        scenario,
    );

    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    unstake_timelocked(STAKER_ADDR_2, 0, scenario);

    // With IIP-8 dynamic commission (50% for 2 validators), stakers get half of the pool reward.
    // Both should still have their original timelocked 100 IOTA that they staked.
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);

    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_2, scenario), 100 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 100 * NANOS_PER_IOTA);

    scenario_val.end();
}

#[test]
fun test_timelock_validator_subsidy_lower_than_computation_charge() {
    set_up_iota_system_state();
    let mut scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
    let scenario = &mut scenario_val;

    stake_timelocked_with(STAKER_ADDR_1, VALIDATOR_ADDR_1, 100, 60, scenario);
    stake_timelocked_with(STAKER_ADDR_2, VALIDATOR_ADDR_2, 150, 60, scenario);
    advance_epoch(scenario);
    // V1: 200, V2: 250

    advance_epoch_with_amounts(800, 0, 1000, 1000, scenario);

    // The computation charge burned is higher than the validator subsidy, so 200 IOTA should be burned.
    // Each validator pool has 50% of the voting power and thus gets 50% of the reward (400 IOTA).
    assert_validator_total_stake_amounts(
        validator_addrs(),
        vector[(200 + 400) * NANOS_PER_IOTA, (250 + 400) * NANOS_PER_IOTA],
        scenario,
    );

    unstake_timelocked(STAKER_ADDR_1, 0, scenario);
    unstake_timelocked(STAKER_ADDR_2, 0, scenario);

    // With IIP-8 dynamic commission (50% for 2 validators), stakers get half of the pool reward.
    // Staker 1 gets 100/200 * 200 = 100 IOTA reward from V1's pool.
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_1, scenario), 100 * NANOS_PER_IOTA);

    // Staker 2 gets 150/250 * 200 = 120 IOTA reward from V2's pool.
    assert_eq(total_timelocked_iota_balance(STAKER_ADDR_2, scenario), 150 * NANOS_PER_IOTA);
    assert_eq(total_iota_balance(STAKER_ADDR_2, scenario), 120 * NANOS_PER_IOTA);

    scenario_val.end();
}

fun assert_exchange_rate_eq(
    rates: &Table<u64, PoolTokenExchangeRate>,
    epoch: u64,
    iota_amount: u64,
    pool_token_amount: u64,
) {
    let rate = &rates[epoch];
    assert_eq(rate.iota_amount(), iota_amount);
    assert_eq(rate.pool_token_amount(), pool_token_amount);
}

fun validator_addrs(): vector<address> {
    vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2]
}

fun set_up_iota_system_state() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        vector[100, 100],
        vector[0, 0],
        ctx,
    );
    create_iota_system_state_for_testing(validators, 500, 0, ctx);
    scenario_val.end();
}

fun set_up_timelock_labeler_caps(to: address, scenario: &mut Scenario) {
    scenario.next_tx(to);

    test_label_one::assign_labeler_cap(to, scenario.ctx());
    test_label_two::assign_labeler_cap(to, scenario.ctx());
}

fun set_up_iota_system_state_with_storage_fund() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let (_, validators) = create_validators_with_stakes_and_commission_rates(
        vector[100, 100],
        vector[0, 0],
        ctx,
    );
    create_iota_system_state_for_testing(validators, 300, 100, ctx);
    scenario_val.end();
}

fun stake_timelocked_with(
    staker: address,
    validator: address,
    amount: u64,
    expiration_timestamp_ms: u64,
    scenario: &mut Scenario,
) {
    scenario.next_tx(staker);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();

    timelocked_staking::request_add_stake(
        &mut system_state,
        timelock::lock(
            balance::create_for_testing(amount * NANOS_PER_IOTA),
            expiration_timestamp_ms,
            ctx,
        ),
        validator,
        ctx,
    );
    test_scenario::return_shared(system_state);
}

fun stake_labeled_timelocked_with<L>(
    cap: &LabelerCap<L>,
    staker: address,
    validator: address,
    amount: u64,
    expiration_timestamp_ms: u64,
    scenario: &mut Scenario,
) {
    scenario.next_tx(staker);

    let mut system_state = scenario.take_shared<IotaSystemState>();
    let ctx = scenario.ctx();

    timelocked_staking::request_add_stake(
        &mut system_state,
        timelock::lock_with_label(
            cap,
            balance::create_for_testing(amount * NANOS_PER_IOTA),
            expiration_timestamp_ms,
            ctx,
        ),
        validator,
        ctx,
    );

    test_scenario::return_shared(system_state);
}

fun unstake_timelocked(staker: address, staked_iota_idx: u64, scenario: &mut Scenario) {
    scenario.next_tx(staker);
    let stake_iota_ids = scenario.ids_for_sender<TimelockedStakedIota>();
    let staked_iota = scenario.take_from_sender_by_id(stake_iota_ids[staked_iota_idx]);
    let mut system_state = scenario.take_shared<IotaSystemState>();

    let ctx = scenario.ctx();
    timelocked_staking::request_withdraw_stake(&mut system_state, staked_iota, ctx);
    test_scenario::return_shared(system_state);
}

fun total_timelocked_iota_balance(addr: address, scenario: &mut Scenario): u64 {
    let mut sum = 0;
    scenario.next_tx(addr);
    let lock_ids = scenario.ids_for_sender<TimeLock<Balance<IOTA>>>();
    let mut i = 0;
    while (i < lock_ids.length()) {
        let coin = scenario.take_from_sender_by_id<TimeLock<Balance<IOTA>>>(lock_ids[i]);
        sum = sum + coin.locked().value();
        scenario.return_to_sender(coin);
        i = i + 1;
    };
    sum
}

fun has_timelocked_iota_balance(addr: address, scenario: &mut Scenario): bool {
    scenario.next_tx(addr);
    scenario.has_most_recent_for_sender<TimeLock<Balance<IOTA>>>()
}

fun has_iota_coins(addr: address, scenario: &mut Scenario): bool {
    scenario.next_tx(addr);
    scenario.has_most_recent_for_sender<Coin<IOTA>>()
}
