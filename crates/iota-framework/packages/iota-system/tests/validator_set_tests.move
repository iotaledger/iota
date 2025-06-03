// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::validator_set_tests;

use iota::balance;
use iota::coin;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{Self, assert_eq, assert_same_elems};
use iota::vec_map;
use iota_system::staking_pool::StakedIota;
use iota_system::validator::{Self, ValidatorV1, staking_pool_id};
use iota_system::validator_set::{
    Self,
    ValidatorSetV2,
    active_validator_addresses,
    committee_validator_addresses
};

const NANOS_PER_IOTA: u64 = 1_000_000_000; // used internally for stakes.

#[test]
fun test_validator_set_flow() {
    // Create 4 validators, with stake 100, 200, 300, 400. Only the first validator is an initial validator.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let validator2 = create_validator(@0x2, 3, 1, false, ctx);
    let validator3 = create_validator(@0x3, 4, 1, false, ctx);
    let validator4 = create_validator(@0x4, 5, 1, false, ctx);
    let validator5 = create_validator(@0x5, 6, 1, false, ctx);
    let validator6 = create_validator(@0x6, 2, 1, false, ctx);

    let committee_size = 4;

    // Create a validator set with only the first validator in it.
    let mut validator_set = validator_set::new_v2(vector[validator1], 4, ctx);
    assert!(validator_set.total_stake_inner() == 100 * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);

    // Add the other 3 validators one by one.
    add_and_activate_validator(
        &mut validator_set,
        validator2,
        scenario,
    );
    // Adding validator during the epoch should not affect stake and quorum threshold.
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);

    add_and_activate_validator(
        &mut validator_set,
        validator3,
        scenario,
    );

    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    {
        let ctx1 = scenario.ctx();
        let stake = validator_set.request_add_stake(
            @0x1,
            coin::mint_for_testing(500 * NANOS_PER_IOTA, ctx1).into_balance(),
            ctx1,
        );
        transfer::public_transfer(stake, @0x1);
        // Adding stake to existing active validator during the epoch
        // should not change total stake.
        assert!(validator_set.total_stake_inner() == 100 * NANOS_PER_IOTA);
    };

    add_and_activate_validator(
        &mut validator_set,
        validator4,
        scenario,
    );
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);

    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // Total stake for these should be the starting stake + the 500 staked with validator 1 in addition to the starting stake.
    assert_eq(validator_set.total_stake_inner(), ((100 + 500) + 300 + 400 + 500) * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4],
    );

    scenario.next_tx(@0x1);
    {
        let ctx1 = scenario.ctx();

        validator_set.request_remove_validator(ctx1);
    };

    // Total validator candidate count changes, but total stake remains during epoch.
    assert_eq(validator_set.total_stake_inner(), ((100 + 500) + 300 + 400 + 500) * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4],
    );

    add_and_activate_validator(
        &mut validator_set,
        validator5,
        scenario,
    );
    add_and_activate_validator(
        &mut validator_set,
        validator6,
        scenario,
    );
    assert_eq(validator_set.total_stake_inner(), ((100 + 500) + 300 + 400 + 500) * NANOS_PER_IOTA);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4],
    );

    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // Validator1 is gone. This removes its stake (100) + the 500 staked with it.
    // Validator5 and Validator6 join the active validator set.
    // Validator5 joins the committee and brings 600 worth of stake.
    assert_eq(validator_set.total_stake_inner(), (300 + 400 + 500 + 600) * NANOS_PER_IOTA);
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4],
    );

    scenario.next_tx(@0x6);
    {
        let ctx1 = scenario.ctx();
        let stake = validator_set.request_add_stake(
            @0x6,
            coin::mint_for_testing(1000 * NANOS_PER_IOTA, ctx1).into_balance(),
            ctx1,
        );
        transfer::public_transfer(stake, @0x6);
        // Adding stake to existing active validator during the epoch
        // should not change total stake.
    };
    assert_eq(validator_set.total_stake_inner(), (300 + 400 + 500 + 600) * NANOS_PER_IOTA);
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4],
    );

    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Validator6 joins the committee and brings 1200 worth of stake after its stake increased, replacing Validator2
    // who has the lowest stake (300). Total stake increases by 900 (1200-300).
    assert_eq(validator_set.total_stake_inner(), (400 + 500 + 600 + 1200) * NANOS_PER_IOTA);
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x6, @0x5, @0x3, @0x4],
    );

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection_equal_stakes() {
    // Create 9 validators with different stakes and initialize committee in some random order.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator_with_stake(@0x1, 2, 1, 28 * NANOS_PER_IOTA, true, ctx);
    let v2 = create_validator_with_stake(@0x2, 4, 1, 4 * NANOS_PER_IOTA, true, ctx);
    let v3 = create_validator_with_stake(@0x3, 6, 1, 22 * NANOS_PER_IOTA, true, ctx);
    let v4 = create_validator_with_stake(@0x4, 8, 1, 8 * NANOS_PER_IOTA, true, ctx);
    let v5 = create_validator_with_stake(@0x5, 20, 1, 24 * NANOS_PER_IOTA, false, ctx);
    let v6 = create_validator_with_stake(@0x6, 22, 1, 22 * NANOS_PER_IOTA, false, ctx);
    let v7 = create_validator_with_stake(@0x7, 24, 1, 24 * NANOS_PER_IOTA, false, ctx);
    let v8 = create_validator_with_stake(@0x8, 3, 2, 3 * NANOS_PER_IOTA, false, ctx);
    let v9 = create_validator_with_stake(@0x9, 28, 1, 28 * NANOS_PER_IOTA, false, ctx);

    let committee_size = 5;

    // Create a validator set with all validators in it, to check that regardless of the active_validators order, top stakers are selected correctly.
    let validator_set_instance = validator_set::new_v2(
        vector[v1, v2, v3, v4, v5, v6, v7, v8, v9],
        committee_size,
        ctx,
    );

    assert_same_elems(
        validator_set_instance.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set_instance.committee_validator_addresses(),
        vector[@0x9, @0x1, @0x7, @0x5, @0x6],
    );
    assert_eq(
        validator_set_instance.total_stake_inner(),
        (28 + 28 + 24 + 24 + 22) * NANOS_PER_IOTA,
    );

    test_utils::destroy(validator_set_instance);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection_random_order_1() {
    // Create 9 validators with different stakes and initialize committee in some random order.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx);
    let v2 = create_validator(@0x2, 4, 1, true, ctx);
    let v3 = create_validator(@0x3, 6, 1, true, ctx);
    let v4 = create_validator(@0x4, 8, 1, true, ctx);
    let v5 = create_validator(@0x5, 20, 1, false, ctx);
    let v6 = create_validator(@0x6, 22, 1, false, ctx);
    let v7 = create_validator(@0x7, 24, 1, false, ctx);
    let v8 = create_validator(@0x8, 3, 2, false, ctx);
    let v9 = create_validator(@0x9, 28, 1, false, ctx);

    let committee_size = 5;

    // Create a validator set with all validators in it, to check that regardless of the active_validators order, top stakers are selected correctly.
    let validator_set_instance = validator_set::new_v2(
        vector[v4, v9, v1, v7, v5, v6, v3, v8, v2],
        committee_size,
        ctx,
    );

    assert_same_elems(
        validator_set_instance.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set_instance.committee_validator_addresses(),
        vector[@0x9, @0x7, @0x6, @0x5, @0x4],
    );
    assert_eq(
        validator_set_instance.total_stake_inner(),
        (28 + 24 + 22 + 20 + 8) * 100 * NANOS_PER_IOTA,
    );

    test_utils::destroy(validator_set_instance);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection_random_order_2() {
    // Create 9 validators with different stakes and initialize committee in some random order.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx);
    let v2 = create_validator(@0x2, 4, 1, true, ctx);
    let v3 = create_validator(@0x3, 6, 1, true, ctx);
    let v4 = create_validator(@0x4, 8, 1, true, ctx);
    let v5 = create_validator(@0x5, 20, 1, false, ctx);
    let v6 = create_validator(@0x6, 22, 1, false, ctx);
    let v7 = create_validator(@0x7, 24, 1, false, ctx);
    let v8 = create_validator(@0x8, 3, 2, false, ctx);
    let v9 = create_validator(@0x9, 28, 1, false, ctx);

    let committee_size = 5;

    // Create a validator set with all validators in it, to check that regardless of the active_validators order, top stakers are selected correctly.
    let validator_set_instance = validator_set::new_v2(
        vector[v5, v2, v8, v3, v6, v9, v1, v7, v4],
        committee_size,
        ctx,
    );

    assert_same_elems(
        validator_set_instance.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set_instance.committee_validator_addresses(),
        vector[@0x9, @0x7, @0x6, @0x5, @0x4],
    );
    assert_eq(
        validator_set_instance.total_stake_inner(),
        (28 + 24 + 22 + 20 + 8) * 100 * NANOS_PER_IOTA,
    );

    test_utils::destroy(validator_set_instance);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection_ascending_order() {
    // Create 9 validators with different stakes and initialize committee in ascending order.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx);
    let v2 = create_validator(@0x2, 4, 1, true, ctx);
    let v3 = create_validator(@0x3, 6, 1, true, ctx);
    let v4 = create_validator(@0x4, 8, 1, true, ctx);
    let v5 = create_validator(@0x5, 20, 1, false, ctx);
    let v6 = create_validator(@0x6, 22, 1, false, ctx);
    let v7 = create_validator(@0x7, 24, 1, false, ctx);
    let v8 = create_validator(@0x8, 3, 2, false, ctx);
    let v9 = create_validator(@0x9, 28, 1, false, ctx);

    let committee_size = 5;

    // Create a validator set with all validators in it in ascending order, to check that regardless of the active_validators order, top stakers are selected correctly.
    let validator_set_instance = validator_set::new_v2(
        vector[v9, v7, v6, v5, v4, v3, v2, v8, v1],
        committee_size,
        ctx,
    );

    assert_same_elems(
        validator_set_instance.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set_instance.committee_validator_addresses(),
        vector[@0x9, @0x7, @0x6, @0x5, @0x4],
    );
    assert_eq(
        validator_set_instance.total_stake_inner(),
        (28 + 24 + 22 + 20 + 8) * 100 * NANOS_PER_IOTA,
    );

    test_utils::destroy(validator_set_instance);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection_descending_order() {
    // Create 9 validators with different stakes and initialize committee in some descending order.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx);
    let v2 = create_validator(@0x2, 4, 1, true, ctx);
    let v3 = create_validator(@0x3, 6, 1, true, ctx);
    let v4 = create_validator(@0x4, 8, 1, true, ctx);
    let v5 = create_validator(@0x5, 20, 1, false, ctx);
    let v6 = create_validator(@0x6, 22, 1, false, ctx);
    let v7 = create_validator(@0x7, 24, 1, false, ctx);
    let v8 = create_validator(@0x8, 3, 2, false, ctx);
    let v9 = create_validator(@0x9, 28, 1, false, ctx);

    let committee_size = 5;

    // Create a validator set with all validators in it in descending order, to check that regardless of the active_validators order, top stakers are selected correctly.
    let validator_set_instance = validator_set::new_v2(
        vector[v1, v8, v2, v3, v4, v5, v6, v7, v9],
        committee_size,
        ctx,
    );

    assert_same_elems(
        validator_set_instance.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set_instance.committee_validator_addresses(),
        vector[@0x9, @0x7, @0x6, @0x5, @0x4],
    );
    assert_eq(
        validator_set_instance.total_stake_inner(),
        (28 + 24 + 22 + 20 + 8) * 100 * NANOS_PER_IOTA,
    );

    test_utils::destroy(validator_set_instance);
    scenario_val.end();
}

#[test]
fun test_top_stakers_committee_selection() {
    // Create 9 validators with different stakes.
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx);
    let v2 = create_validator(@0x2, 4, 1, true, ctx);
    let v3 = create_validator(@0x3, 6, 1, true, ctx);
    let v4 = create_validator(@0x4, 8, 1, true, ctx);
    let v5 = create_validator(@0x5, 20, 1, false, ctx);
    let v6 = create_validator(@0x6, 22, 1, false, ctx);
    let v7 = create_validator(@0x7, 24, 1, false, ctx);
    let v8 = create_validator(@0x8, 3, 2, false, ctx);
    let v9 = create_validator(@0x9, 28, 1, false, ctx);

    let committee_size = 5;

    // Create a validator set with only the first 4 validators in it.
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4], committee_size, ctx);

    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    // Assert same elems instead of equality because if there is less validators than max_committee_members_count then sorting is not performed.
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x4, @0x3, @0x2, @0x1],
    );

    assert_eq(validator_set.total_stake_inner(), 20 * 100 * NANOS_PER_IOTA);

    // Add 5th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v5, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that 5th validator is in the committee. Validator 5 brings 20*100 stake to the committee.
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5],
    );
    // Assert same elems instead of equality because if there is less validators than max_committee_members_count then sorting is not performed.
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x4, @0x3, @0x2, @0x1],
    );

    assert_eq(validator_set.total_stake_inner(), 40 * 100 * NANOS_PER_IOTA);

    // Add 6th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v6, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that validator 6 becomes committee member and replaces another validator, because committee is full.
    // Validator 6 brings 22 * 100 stake, which replaces 2 * 100 stake from validator 1 which left the committee.
    // Total stake increases by 20 * 100 [(22 - 2) * 100]
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x6, @0x5, @0x4, @0x3, @0x2]);

    assert_eq(validator_set.total_stake_inner(), 60 * 100 * NANOS_PER_IOTA);

    // Add 7th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v7, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that validator 7 becomes committee member and replaces another validator, because committee is full.
    // Validator 7 brings 24 * 100 stake, which replaces 4 * 100 stake from validator 2 which left the committee.
    // Total stake increases by 20 * 100 [(24 - 4) * 100]
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x7, @0x6, @0x5, @0x4, @0x3]);
    assert_eq(validator_set.total_stake_inner(), 80 * 100 * NANOS_PER_IOTA);

    // Add 8th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v8, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that validator 8 does not become a committee member and the committee stays the same.
    // Validator has less stake than the lowest committee member (2 * 100 for validator 8 vs 3 * 100 for validator 3).
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x7, @0x6, @0x5, @0x4, @0x3]);

    assert_eq(validator_set.total_stake_inner(), 80 * 100 * NANOS_PER_IOTA);

    // Add 9th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v9, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that validator 9 becomes committee member and replaces another validator, because committee is full.
    // Validator 9 brings 28 * 100 stake, which replaces 6 * 100 stake from validator 3 which left the committee.
    // Total stake increases by 22 * 100 [(26 - 6) * 100]
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x9, @0x7, @0x6, @0x5, @0x4]);
    assert_eq(validator_set.total_stake_inner(), 102 * 100 * NANOS_PER_IOTA);

    // Advance epoch with larger committee
    advance_epoch_with_dummy_rewards(&mut validator_set, 7, scenario);

    // Make sure that validator 9 becomes committee member and replaces another validator, because committee is full.
    // Validator 9 brings 28 * 100 stake, which replaces 6 * 100 stake from validator 3 which left the committee.
    // Total stake increases by 22 * 100 [(26 - 6) * 100]
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x9, @0x7, @0x6, @0x5, @0x4, @0x3, @0x2],
    );
    assert_eq(validator_set.total_stake_inner(), (102 + 6 + 4) * 100 * NANOS_PER_IOTA);

    // Advance epoch with smaller committee
    advance_epoch_with_dummy_rewards(&mut validator_set, 3, scenario);

    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x9, @0x7, @0x6]);
    assert_eq(validator_set.total_stake_inner(), (28 + 24 + 22) * 100 * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = validator_set::EStakingBelowThreshold)]
fun test_staking_below_threshold() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let mut validator_set = validator_set::new_v2(vector[validator1], 3, ctx);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    let ctx1 = scenario.ctx();

    let stake = validator_set.request_add_stake(
        @0x1,
        balance::create_for_testing(NANOS_PER_IOTA - 1), // 1 NANOS lower than the threshold
        ctx1,
    );
    transfer::public_transfer(stake, @0x1);
    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
fun test_staking_min_threshold() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let committee_size = 1;
    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let mut validator_set = validator_set::new_v2(vector[validator1], committee_size, ctx);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    let ctx1 = scenario.ctx();
    let stake = validator_set.request_add_stake(
        @0x1,
        balance::create_for_testing(NANOS_PER_IOTA), // min possible stake
        ctx1,
    );
    transfer::public_transfer(stake, @0x1);

    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    assert!(validator_set.total_stake_inner() == 101 * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
#[expected_failure(abort_code = validator_set::EMinJoiningStakeNotReached)]
fun test_add_validator_failure_below_min_stake() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create 2 validators, with stake 100 and 200.
    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let validator2 = create_validator(@0x2, 2, 1, false, ctx);

    // Create a validator set with only the first validator in it.
    let mut validator_set = validator_set::new_v2(vector[validator1], 2, ctx);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    let ctx1 = scenario.ctx();
    validator_set.request_add_validator_candidate(validator2, ctx1);

    scenario.next_tx(@0x42);
    {
        let ctx = scenario.ctx();
        let stake = validator_set.request_add_stake(
            @0x2,
            balance::create_for_testing(500 * NANOS_PER_IOTA),
            ctx,
        );
        transfer::public_transfer(stake, @0x42);
        // Adding stake to a preactive validator should not change total stake.
        assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    };

    scenario.next_tx(@0x2);
    // Validator 2 now has 700 IOTA in stake but that's not enough because we need 701.
    validator_set.request_add_validator(701 * NANOS_PER_IOTA, scenario.ctx());

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
fun test_add_validator_with_nonzero_min_stake() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create 2 validators, with stake 100 and 200.
    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let validator2 = create_validator(@0x2, 2, 1, false, ctx);

    // Create a validator set with only the first validator in it.
    let mut validator_set = validator_set::new_v2(vector[validator1], 2, ctx);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    let ctx1 = scenario.ctx();
    validator_set.request_add_validator_candidate(validator2, ctx1);

    scenario.next_tx(@0x42);
    {
        let ctx = scenario.ctx();
        let stake = validator_set.request_add_stake(
            @0x2,
            balance::create_for_testing(500 * NANOS_PER_IOTA),
            ctx,
        );
        transfer::public_transfer(stake, @0x42);
        // Adding stake to a preactive validator should not change total stake.
        assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    };

    scenario.next_tx(@0x2);
    // Validator 2 now has 700 IOTA in stake and that's just enough.
    validator_set.request_add_validator(700 * NANOS_PER_IOTA, scenario.ctx());

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
fun test_add_candidate_then_remove() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create 2 validators, with stake 100 and 200.
    let validator1 = create_validator(@0x1, 1, 1, true, ctx);
    let validator2 = create_validator(@0x2, 2, 1, false, ctx);

    let pool_id_2 = staking_pool_id(&validator2);

    // Create a validator set with only the first validator in it.
    let mut validator_set = validator_set::new_v2(vector[validator1], 2, ctx);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    let ctx1 = scenario.ctx();
    // Add the second one as a candidate.
    validator_set.request_add_validator_candidate(validator2, ctx1);
    assert!(validator_set.is_validator_candidate_inner(@0x2));
    assert_eq(validator_set.validator_address_by_pool_id_inner(&pool_id_2), @0x2);

    scenario.next_tx(@0x2);
    // Then remove its candidacy.
    validator_set.request_remove_validator_candidate(scenario.ctx());
    assert!(!validator_set.is_validator_candidate_inner(@0x2));
    assert!(validator_set.is_inactive_validator_inner(pool_id_2));
    assert_eq(validator_set.validator_address_by_pool_id_inner(&pool_id_2), @0x2);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

#[test]
fun test_low_stake_departure() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    // Create 4 validators.
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA of stake
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA of stake
    let v3 = create_validator(@0x3, 10, 1, true, ctx); // 1000 IOTA of stake
    let v4 = create_validator(@0x4, 4, 1, true, ctx); // 400 IOTA of stake
    // Create an additional validator that initially will not be part of the committee and will be kicked out of active validators as well.
    let v5 = create_validator(@0x5, 1, 1, true, ctx); // 100 IOTA of stake

    let committee_size = 4;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4, v5], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;
    assert_same_elems(
        active_validator_addresses(&validator_set),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5],
    );
    assert_same_elems(
        committee_validator_addresses(&validator_set),
        vector[@0x1, @0x2, @0x3, @0x4],
    );

    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        250,
        3,
        scenario,
    );

    // v1 is kicked out because their stake 100 is less than the very low stake threshold
    // which is 200.
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);

    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);

    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x2, @0x3, @0x4]);

    // Add some stake to @0x4 to get her out of the danger zone.
    scenario.next_tx(@0x42);
    {
        let ctx = scenario.ctx();
        let stake = validator_set.request_add_stake(
            @0x4,
            balance::create_for_testing(500 * NANOS_PER_IOTA),
            ctx,
        );
        transfer::public_transfer(stake, @0x42);
    };

    // So only @0x2 will be kicked out.
    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x3, @0x4]);

    // Withdraw the stake from @0x4.
    scenario.next_tx(@0x42);
    {
        let stake = scenario.take_from_sender<StakedIota>();
        let ctx = scenario.ctx();
        let withdrawn_balance = validator_set.request_withdraw_stake(
            stake,
            ctx,
        );
        transfer::public_transfer(withdrawn_balance.into_coin(ctx), @0x42);
    };

    // Now @0x4 gets kicked out after 3 grace days are used at the 4th epoch change.
    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_eq(active_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x3, @0x4]);

    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x3, @0x4]);
    advance_epoch_with_low_stake_params(
        &mut validator_set,
        committee_size,
        500,
        200,
        3,
        scenario,
    );
    // @0x4 was kicked out.
    assert_same_elems(active_validator_addresses(&validator_set), vector[@0x3]);
    assert_same_elems(committee_validator_addresses(&validator_set), vector[@0x3]);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

fun create_validator(
    addr: address,
    hint: u8,
    gas_price: u64,
    is_initial_validator: bool,
    ctx: &mut TxContext,
): ValidatorV1 {
    let stake_value = hint as u64 * 100 * NANOS_PER_IOTA;
    create_validator_with_stake(addr, hint, gas_price, stake_value, is_initial_validator, ctx)
}

fun create_validator_with_stake(
    addr: address,
    hint: u8,
    gas_price: u64,
    stake_value: u64,
    is_initial_validator: bool,
    ctx: &mut TxContext,
): ValidatorV1 {
    let name = hint_to_ascii(hint);
    let validator = validator::new_for_testing(
        addr,
        vector[hint],
        vector[hint],
        vector[hint],
        vector[hint],
        copy name,
        copy name,
        copy name,
        name,
        vector[hint],
        vector[hint],
        vector[hint],
        option::some(balance::create_for_testing(stake_value)),
        gas_price,
        0,
        is_initial_validator,
        ctx,
    );
    validator
}

fun hint_to_ascii(hint: u8): vector<u8> {
    let ascii_bytes = vector[hint / 100 + 65, hint % 100 / 10 + 65, hint % 10 + 65];
    ascii_bytes.to_ascii_string().into_bytes()
}

fun advance_epoch_with_dummy_rewards(
    validator_set: &mut ValidatorSetV2,
    committee_size: u64,
    scenario: &mut Scenario,
) {
    scenario.next_epoch(@0x0);
    let mut dummy_computation_charge = balance::zero();

    validator_set.advance_epoch(
        &mut dummy_computation_charge,
        &mut vec_map::empty(),
        0, // reward_slashing_rate
        0, // low_stake_threshold
        0, // very_low_stake_threshold
        0, // low_stake_grace_period
        committee_size,
        scenario.ctx(),
    );

    dummy_computation_charge.destroy_zero();
}

fun advance_epoch_with_low_stake_params(
    validator_set: &mut ValidatorSetV2,
    committee_size: u64,
    low_stake_threshold: u64,
    very_low_stake_threshold: u64,
    low_stake_grace_period: u64,
    scenario: &mut Scenario,
) {
    scenario.next_epoch(@0x0);
    let mut dummy_computation_charge = balance::zero();
    validator_set.advance_epoch(
        &mut dummy_computation_charge,
        &mut vec_map::empty(),
        0, // reward_slashing_rate
        low_stake_threshold * NANOS_PER_IOTA,
        very_low_stake_threshold * NANOS_PER_IOTA,
        low_stake_grace_period,
        committee_size,
        scenario.ctx(),
    );

    dummy_computation_charge.destroy_zero();
}

fun add_and_activate_validator(
    validator_set: &mut ValidatorSetV2,
    validator: ValidatorV1,
    scenario: &mut Scenario,
) {
    scenario.next_tx(validator.iota_address());
    let ctx = scenario.ctx();
    validator_set.request_add_validator_candidate(validator, ctx);
    validator_set.request_add_validator(0, ctx);
}
