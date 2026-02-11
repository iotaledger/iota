// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module for validator set functionality.
///
/// This module tests the core validator set operations including:
/// - Validator lifecycle (joining, leaving, staking changes)
/// - Committee selection based on stake ranking
/// - Authority capability notification delays
/// - Low stake departure mechanisms
/// - Eligible validator scenarios with complex index mapping
/// - Edge cases and error conditions
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

/// Standard IOTA denomination conversion factor
const NANOS_PER_IOTA: u64 = 1_000_000_000;

/// Tests the complete validator set lifecycle including joining, staking, committee transitions,
/// and the authority capability notification delay mechanism.
///
/// This test demonstrates:
/// - How validators join and become active over multiple epochs
/// - The two-epoch delay for new validators to join the committee (authority capability notification)
/// - Dynamic stake changes and their effect on committee composition
/// - Validator removal and replacement in the committee
#[test]
fun test_validator_set_flow() {
    // Create validators with varying stakes (100, 300, 400, 500, 600, 200 IOTA)
    // Only validator1 starts as an initial validator
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let validator1 = create_validator(@0x1, 1, 1, true, ctx); // 100 IOTA initial validator
    let validator2 = create_validator(@0x2, 3, 1, false, ctx); // 300 IOTA
    let validator3 = create_validator(@0x3, 4, 1, false, ctx); // 400 IOTA
    let validator4 = create_validator(@0x4, 5, 1, false, ctx); // 500 IOTA
    let validator5 = create_validator(@0x5, 6, 1, false, ctx); // 600 IOTA
    let validator6 = create_validator(@0x6, 2, 1, false, ctx); // 200 IOTA

    let committee_size = 4;

    // Create a validator set with only the initial validator
    let mut validator_set = validator_set::new_v2(vector[validator1], committee_size, ctx);
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);
    assert!(validator_set.total_stake_inner() == 100 * NANOS_PER_IOTA);

    // Add validators as candidates during the current epoch
    // Note: Adding validators mid-epoch doesn't immediately change active set or committee
    add_and_activate_validator(
        &mut validator_set,
        validator2,
        scenario,
    );
    // Validator2 is added as pre-active but won't be active until next epoch
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);

    add_and_activate_validator(
        &mut validator_set,
        validator3,
        scenario,
    );

    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);

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
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_same_elems(validator_set.committee_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.total_stake_inner(), 100 * NANOS_PER_IOTA);

    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // EPOCH 1 TRANSITION: New validators become active but cannot join committee immediately
    //
    // Key concept: Authority Capability Notification Delay
    // - Newly activated validators must wait one additional epoch before joining committee
    // - This allows time for them to notify their AuthorityCapabilities to the network
    // - Only existing committee members (validator1) continue in committee
    // - Validator1's added stake (500 IOTA) is now reflected in total
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x1],
    );
    assert_eq(validator_set.total_stake_inner(), (100 + 500) * NANOS_PER_IOTA);

    // EPOCH 2 TRANSITION: Previously activated validators can now join committee
    // After the one-epoch delay, new validators can join the committee
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // All validators are now eligible for committee membership
    // Committee includes all 4 validators since committee_size = 4
    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4],
    );
    assert_eq(validator_set.total_stake_inner(), ((100 + 500) + 300 + 400 + 500) * NANOS_PER_IOTA);

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
    // VALIDATOR REMOVAL AND NEW ADDITIONS:
    // - Validator1 is removed (losing 100 + 500 = 600 IOTA stake)
    // - Validator5 (600 IOTA) and Validator6 (200 IOTA) become active
    // - New validators cannot join committee immediately (authority capability delay)
    // - Committee now has only 3 members instead of 4
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x2, @0x3, @0x4],
    );
    assert_eq(validator_set.total_stake_inner(), (300 + 400 + 500) * NANOS_PER_IOTA);

    // COMMITTEE REBALANCING: New validators can now join after one-epoch delay
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // Validator5 (600 IOTA) joins committee, pushing out Validator6 (200 IOTA lowest stake)
    // Committee selection is based on highest stake when committee_size < active validators
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4],
    );
    assert_eq(validator_set.total_stake_inner(), (300 + 400 + 500 + 600) * NANOS_PER_IOTA);

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

    // Advance epoch again to allow Validator6 to join committee
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);
    // Validator6 joins the committee and brings 1200 worth of stake after its stake increased, replacing Validator2
    // who has the lowest stake (300). Total stake increases by 900 (1200-300).
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x5, @0x2, @0x3, @0x4, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x6, @0x5, @0x3, @0x4],
    );
    assert_eq(validator_set.total_stake_inner(), (400 + 500 + 600 + 1200) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

/// Tests committee selection when validators have equal stakes.
///
/// This test verifies that when validators have identical stakes, the committee selection
/// algorithm produces deterministic results based on the tie-breaking mechanism.
/// Tests the ordering and selection logic when stake amounts are equal.
#[test]
fun test_top_stakers_committee_selection_equal_stakes() {
    // Create 9 validators with mixed stakes including some equal values
    // Tests committee selection determinism when stakes are identical
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

    // Initialize all validators in the set to test stake-based committee selection
    // Order of initialization should not affect final committee composition
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

/// Tests that validator initialization order doesn't affect committee selection.
///
/// This comprehensive test verifies that regardless of the order validators are added
/// to the validator set, the final committee selection is based purely on stake ranking.
/// Tests multiple different initialization orders to ensure deterministic behavior.
#[test]
fun test_top_stakers_committee_selection_various_orders() {
    // Verify committee selection is deterministic regardless of validator initialization order
    // Tests the robustness of the stake-based ranking algorithm
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let committee_size = 5;
    // Pre-calculated expected results based on stake ranking (highest to lowest)
    let expected_committee = vector[@0x9, @0x7, @0x6, @0x5, @0x4]; // Stakes: 2800, 2400, 2200, 2000, 800 IOTA
    let expected_stake = (28 + 24 + 22 + 20 + 8) * 100 * NANOS_PER_IOTA;
    let all_validators = vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9];

    // Test Case 1: Random initialization order
    // Validator set created in non-stake order: 4,9,1,7,5,6,3,8,2
    {
        let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA
        let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA
        let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
        let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA
        let v5 = create_validator(@0x5, 20, 1, false, ctx); // 2000 IOTA
        let v6 = create_validator(@0x6, 22, 1, false, ctx); // 2200 IOTA
        let v7 = create_validator(@0x7, 24, 1, false, ctx); // 2400 IOTA
        let v8 = create_validator(@0x8, 3, 2, false, ctx); // 300 IOTA
        let v9 = create_validator(@0x9, 28, 1, false, ctx); // 2800 IOTA

        let validator_set_instance = validator_set::new_v2(
            vector[v4, v9, v1, v7, v5, v6, v3, v8, v2],
            committee_size,
            ctx,
        );

        assert_same_elems(validator_set_instance.active_validator_addresses(), all_validators);
        assert_eq(validator_set_instance.committee_validator_addresses(), expected_committee);
        assert_eq(validator_set_instance.total_stake_inner(), expected_stake);
        test_utils::destroy(validator_set_instance);
    };

    // Test Case 2: Alternative random initialization order
    // Validators created in different non-stake order: 5,2,8,3,6,9,1,7,4
    {
        let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA
        let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA
        let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
        let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA
        let v5 = create_validator(@0x5, 20, 1, false, ctx); // 2000 IOTA
        let v6 = create_validator(@0x6, 22, 1, false, ctx); // 2200 IOTA
        let v7 = create_validator(@0x7, 24, 1, false, ctx); // 2400 IOTA
        let v8 = create_validator(@0x8, 3, 2, false, ctx); // 300 IOTA
        let v9 = create_validator(@0x9, 28, 1, false, ctx); // 2800 IOTA

        let validator_set_instance = validator_set::new_v2(
            vector[v5, v2, v8, v3, v6, v9, v1, v7, v4],
            committee_size,
            ctx,
        );

        assert_same_elems(validator_set_instance.active_validator_addresses(), all_validators);
        assert_eq(validator_set_instance.committee_validator_addresses(), expected_committee);
        assert_eq(validator_set_instance.total_stake_inner(), expected_stake);
        test_utils::destroy(validator_set_instance);
    };

    // Test Case 3: Ascending stake order initialization
    // Validators created in ascending stake order: 1,8,2,3,4,5,6,7,9
    {
        let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA
        let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA
        let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
        let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA
        let v5 = create_validator(@0x5, 20, 1, false, ctx); // 2000 IOTA
        let v6 = create_validator(@0x6, 22, 1, false, ctx); // 2200 IOTA
        let v7 = create_validator(@0x7, 24, 1, false, ctx); // 2400 IOTA
        let v8 = create_validator(@0x8, 3, 2, false, ctx); // 300 IOTA
        let v9 = create_validator(@0x9, 28, 1, false, ctx); // 2800 IOTA

        let validator_set_instance = validator_set::new_v2(
            vector[v1, v8, v2, v3, v4, v5, v6, v7, v9],
            committee_size,
            ctx,
        );

        assert_same_elems(validator_set_instance.active_validator_addresses(), all_validators);
        assert_eq(validator_set_instance.committee_validator_addresses(), expected_committee);
        assert_eq(validator_set_instance.total_stake_inner(), expected_stake);
        test_utils::destroy(validator_set_instance);
    };

    scenario_val.end();
}

/// Tests comprehensive committee selection based on stake ranking.
///
/// This test demonstrates the complete validator lifecycle and committee dynamics:
/// - Initial committee formation with fewer validators than committee size
/// - Sequential validator additions with authority capability delays
/// - Committee rebalancing as new validators become eligible
/// - Dynamic committee size adjustments
#[test]
fun test_top_stakers_committee_selection() {
    // Create 9 validators with varied stakes to test comprehensive committee dynamics
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
    let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA
    let v5 = create_validator(@0x5, 20, 1, false, ctx); // 2000 IOTA
    let v6 = create_validator(@0x6, 22, 1, false, ctx); // 2200 IOTA
    let v7 = create_validator(@0x7, 24, 1, false, ctx); // 2400 IOTA
    let v8 = create_validator(@0x8, 3, 2, false, ctx); // 300 IOTA (high gas price)
    let v9 = create_validator(@0x9, 28, 1, false, ctx); // 2800 IOTA

    let committee_size = 5;

    // Initialize with first 4 validators (all are initial committee validators)
    // Committee size = 5, but only 4 validators available initially
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4], committee_size, ctx);

    assert_same_elems(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4]);
    // When validators < committee_size, all validators join committee (no stake-based sorting needed)
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x4, @0x3, @0x2, @0x1], // All 4 validators in committee
    );

    assert_eq(validator_set.total_stake_inner(), 20 * 100 * NANOS_PER_IOTA);

    // Add validator 5 (2000 IOTA) - should eventually join committee due to high stake
    add_and_activate_validator(&mut validator_set, v5, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // AUTHORITY CAPABILITY DELAY: Validator5 is active but cannot join committee yet
    // Must wait one additional epoch to notify AuthorityCapabilities
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x4, @0x3, @0x2, @0x1], // Committee unchanged during capability delay
    );

    assert_eq(validator_set.total_stake_inner(), 20 * 100 * NANOS_PER_IOTA);

    // COMMITTEE EXPANSION: Validator5 can now join after authority capability delay
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Validator5 (2000 IOTA) joins committee, filling the 5th slot
    // No validator displacement needed since committee_size = 5 and we have exactly 5 validators
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5],
    );
    // All 5 validators now in committee (no need for stake-based sorting yet)
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x4, @0x3, @0x2, @0x1],
    );

    assert_eq(validator_set.total_stake_inner(), 40 * 100 * NANOS_PER_IOTA);

    // Add 6th validator and advance to new epoch.
    add_and_activate_validator(&mut validator_set, v6, scenario);
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Validator 6 is now active but can't join committee yet - needs one more epoch
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6],
    );
    assert_same_elems(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x4, @0x3, @0x2, @0x1],
    );

    assert_eq(validator_set.total_stake_inner(), 40 * 100 * NANOS_PER_IOTA);

    // Advance epoch again to allow validator 6 to join committee
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

    // Validator 7 is now active but can't join committee yet - needs one more epoch
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x6, @0x5, @0x4, @0x3, @0x2]);
    assert_eq(validator_set.total_stake_inner(), 60 * 100 * NANOS_PER_IOTA);

    // Advance epoch again to allow validator 7 to join committee
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

    // Validator 8 is now active and even after the epoch delay still won't join committee due to low stake
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x7, @0x6, @0x5, @0x4, @0x3]);

    assert_eq(validator_set.total_stake_inner(), 80 * 100 * NANOS_PER_IOTA);

    // Advance epoch again - validator 8 still doesn't join committee (stake too low)
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

    // Validator 9 is now active but can't join committee yet - needs one more epoch
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5, @0x6, @0x7, @0x8, @0x9],
    );
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x7, @0x6, @0x5, @0x4, @0x3]);
    assert_eq(validator_set.total_stake_inner(), 80 * 100 * NANOS_PER_IOTA);

    // Advance epoch again to allow validator 9 to join committee
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Make sure that validator 9 becomes committee member and replaces another validator, because committee is full.
    // Validator 9 brings 28 * 100 stake, which replaces 6 * 100 stake from validator 3 which left the committee.
    // Total stake increases by 22 * 100 [(28 - 6) * 100]
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

    // Default: all validators are eligible (indices 0 to n-1)
    let eligible_validators = vector::tabulate!(
        validator_set.active_validators_inner().length(),
        |i| i,
    );

   let scores = vector::tabulate!(
        validator_set.committee_validator_addresses().length(),
        |_| 65536u64,    
    );

    validator_set.advance_epoch(
        &mut dummy_computation_charge,
        &mut vec_map::empty(),
        0, // reward_slashing_rate
        0, // low_stake_threshold
        0, // very_low_stake_threshold
        0, // low_stake_grace_period
        committee_size,
        eligible_validators,
        scores,
        true,
        scenario.ctx(),
    );

    dummy_computation_charge.destroy_zero();
}

fun advance_epoch_with_eligible_validators(
    validator_set: &mut ValidatorSetV2,
    committee_size: u64,
    eligible_validators: vector<u64>,
    scenario: &mut Scenario,
) {
    scenario.next_epoch(@0x0);
    let mut dummy_computation_charge = balance::zero();



    let scores = vector::tabulate!(
        validator_set.committee_validator_addresses().length(),
        |_| 65536u64,    
    );

    validator_set.advance_epoch(
        &mut dummy_computation_charge,
        &mut vec_map::empty(),
        0, // reward_slashing_rate
        0, // low_stake_threshold
        0, // very_low_stake_threshold
        0, // low_stake_grace_period
        committee_size,
        eligible_validators,
        scores,
        true,
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

    // Default: all validators are eligible
    let eligible_validators = vector::tabulate!(
        validator_set.active_validators_inner().length(),
        |i| i,
    );

   let scores = vector::tabulate!(
        validator_set.committee_validator_addresses().length(),
        |_| 65536u64,    
    );

    validator_set.advance_epoch(
        &mut dummy_computation_charge,
        &mut vec_map::empty(),
        0, // reward_slashing_rate
        low_stake_threshold * NANOS_PER_IOTA,
        very_low_stake_threshold * NANOS_PER_IOTA,
        low_stake_grace_period,
        committee_size,
        eligible_validators,
        scores,
        true,
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

// Test case 1: Top validators ineligible and sparse eligible indices mapping
#[test]
fun test_eligible_committee_selection_ineligible_top_validators() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators with different stakes to test both ineligible top validators and sparse indices
    let v1 = create_validator(@0x1, 1, 1, true, ctx); // 100 IOTA
    let v2 = create_validator(@0x2, 3, 1, true, ctx); // 300 IOTA
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
    let v4 = create_validator(@0x4, 7, 1, false, ctx); // 700 IOTA
    let v5 = create_validator(@0x5, 9, 1, false, ctx); // 900 IOTA
    let v6 = create_validator(@0x6, 4, 1, true, ctx); // 400 IOTA

    let committee_size = 4;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v6], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Add the high-stake validators that won't be eligible
    add_and_activate_validator(&mut validator_set, v5, scenario);
    add_and_activate_validator(&mut validator_set, v4, scenario);

    // Advance epoch with all initial validators eligible (indices 0, 1, 2, 3)
    // This will form a 4-member committee: v3:600, v6:400, v2:300, v1:100
    // Top validators v5 (900) and v4 (700) are not in eligible list yet
    let eligible_validators = vector[0, 1, 2, 3];
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // After first epoch: v4 and v5 are active but can't join committee yet - need one more epoch
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x6, @0x4, @0x5],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x6], // All 4 initial validators in committee
    );

    // Second epoch: Test scenario where one committee member becomes ineligible
    // Active validators: [v1:100, v2:300, v3:600, v6:400, v4:700, v5:900]
    // Previous committee was [v1, v2, v3, v6] - make v2 ineligible but keep 3/4 eligible (≥2/3)
    // Also add v4 to eligible list to have enough validators for committee of 4
    // v5 remains ineligible despite highest stake to test top validator ineligibility
    let eligible_validators = vector[0, 2, 3, 4]; // v1:100, v3:600, v6:400, v4:700 eligible (v2 and v5 ineligible)
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Committee should contain all 4 eligible validators: v4:700, v3:600, v6:400, v1:100
    // 3 out of 4 original committee members (v1, v3, v6) stay eligible, satisfying ≥2/3 requirement
    // v2 becomes ineligible, v4 joins the committee
    // v5:900 is still not selected despite highest stake (ineligible)
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x6, @0x4, @0x5],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x3, @0x6, @0x4], // All 4 eligible validators by stake
    );
    assert_eq(validator_set.total_stake_inner(), (700 + 600 + 400 + 100) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 2: Eligible validator removed and replaced by another eligible validator
#[test]
fun test_eligible_committee_selection_eligible_removed_replaced() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators: 400, 600, 700, 800, 900, 1000 IOTA - all initially eligible
    let v1 = create_validator(@0x1, 4, 1, true, ctx); // 400 IOTA - index 0
    let v2 = create_validator(@0x2, 6, 1, true, ctx); // 600 IOTA - index 1
    let v3 = create_validator(@0x3, 7, 1, true, ctx); // 700 IOTA - index 2
    let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA - index 3
    let v5 = create_validator(@0x5, 9, 1, true, ctx); // 900 IOTA - index 4
    let v6 = create_validator(@0x6, 10, 1, true, ctx); // 1000 IOTA - index 5

    let committee_size = 4;
    let mut validator_set = validator_set::new_v2(
        vector[v1, v2, v3, v4, v5, v6],
        committee_size,
        ctx,
    );
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Initially: all 6 validators are eligible, committee should be v6, v5, v4, v3 (top 4 by stake)
    let eligible_validators = vector[0, 1, 2, 3, 4, 5];
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x6, @0x5, @0x4, @0x3], // Top 4 by stake
    );
    assert_eq(validator_set.total_stake_inner(), (1000 + 900 + 800 + 700) * NANOS_PER_IOTA);

    // Remove v6 (highest stake committee member) from eligible list
    // Keep v3, v4, v5 eligible (3/4 = 75% ≥ 2/3 of original committee stay eligible)
    // Add v1 and v2 to have enough validators for committee of 4
    // Previous committee was [v3, v4, v5, v6] - now [v3, v4, v5] stay eligible
    let eligible_validators = vector[0, 1, 2, 3, 4]; // v6 (index 5) is no longer eligible
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Committee should now be v5, v4, v3, v2 (top 4 eligible validators by stake)
    // 3 out of 4 original committee members (v3, v4, v5) stay eligible, satisfying ≥2/3 requirement
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x4, @0x3, @0x2], // Top 4 eligible validators by stake
    );
    assert_eq(validator_set.total_stake_inner(), (900 + 800 + 700 + 600) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 3: Eligible validator removed, ineligible top validator added but not selected
#[test]
fun test_eligible_committee_selection_complex_scenario() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators: 100, 300, 600, 500, 700, 400 IOTA
    let v1 = create_validator(@0x1, 1, 1, true, ctx); // 100 IOTA - index 0
    let v2 = create_validator(@0x2, 3, 1, true, ctx); // 300 IOTA - index 1
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA - index 2
    let v6 = create_validator(@0x6, 5, 1, true, ctx); // 500 IOTA - index 3 (new initial validator)
    let v4 = create_validator(@0x4, 7, 1, false, ctx); // 700 IOTA - will be added later
    let v5 = create_validator(@0x5, 4, 1, false, ctx); // 400 IOTA - will be added later

    let committee_size = 4;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v6], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Add validators v4 (700) and v5 (400)
    // These validators will be processed during the advance_epoch call and become active
    add_and_activate_validator(&mut validator_set, v4, scenario);
    add_and_activate_validator(&mut validator_set, v5, scenario);

    // Before advance_epoch: active validators = [v1:100, v2:300, v3:600, v6:500] at indices [0,1,2,3], pending = [v4:700, v5:400]
    // After advance_epoch: active validators will be [v1:100, v2:300, v3:600, v6:500, v4:700, v5:400] at indices [0,1,2,3,4,5]
    // eligible_validators indices refer to PRE-advance_epoch positions, so only [0,1,2,3] are valid
    // Make 3 out of 4 original committee members eligible (≥2/3) - remove v2 but keep v1, v3, v6 eligible
    let eligible_validators = vector[0, 2, 3]; // v1:100, v3:600, v6:500 are eligible from original set (3/4 = 75% ≥ 2/3)
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Despite v4 having the highest stake (700), it's not eligible because it wasn't in the original set
    // Despite v2 being in original committee, it's not eligible in this epoch
    // Committee should be the 3 eligible validators: v3:600, v6:500, v1:100 (from original set)
    // This maintains ≥2/3 of original committee members as eligible while testing ineligible top validator scenario
    assert_same_elems(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x6, @0x4, @0x5],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x3, @0x6], // Top 3 eligible validators by stake (only 3 instead of 4 due to insufficient eligible)
    );
    assert_eq(validator_set.total_stake_inner(), (600 + 500 + 100) * NANOS_PER_IOTA); // Only eligible committee stake

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 4: Committee size exceeds eligible validators - basic and index mapping scenarios
#[test]
fun test_eligible_committee_selection_insufficient_eligible_validators_and_index_mapping() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators to test both insufficient eligible validators and sparse index mapping
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA - index 0 (ineligible)
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA - index 1 (eligible)
    let v3 = create_validator(@0x3, 7, 1, true, ctx); // 700 IOTA - index 2 (eligible)
    let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA - index 3 (ineligible)
    let v5 = create_validator(@0x5, 12, 1, true, ctx); // 1200 IOTA - index 4 (eligible)

    let committee_size = 4; // Want 4 committee members but only 3 eligible
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4, v5], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Check initial committee and active validators
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x5, @0x4, @0x3, @0x2]);
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);

    // Test when committee_size > eligible validators count with sparse indices
    // Want 4 committee members but only make 3 validators eligible with sparse indices [1, 2, 4]
    // When n >= eligible.length(), take_top_n! returns [0, 1, 2] (sequential positions in eligible array)
    // These positions must be mapped to actual validator indices [eligible[0], eligible[1], eligible[2]] = [1, 2, 4] = v2:400, v3:700, v5:1200
    // Without proper mapping, wrong validators active_validators[0], active_validators[1], active_validators[2] = v1:200, v2:400, v3:700 would be selected
    let eligible_validators = vector[1, 2, 4]; // Only v2:400, v3:700, and v5:1200 are eligible (sparse indices)
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Committee should include all 3 eligible validators: v5:1200, v3:700, v2:400 (only 3 instead of committee size of 4)
    // This tests both insufficient eligible validators scenario and correct sparse index mapping
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x2, @0x3, @0x5], // All 3 eligible validators by stake (correct sparse index mapping)
    );
    assert_eq(validator_set.total_stake_inner(), (1200 + 700 + 400) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 5: Edge case - single eligible validator fallback to all active validators
#[test]
fun test_eligible_committee_selection_single_eligible_validator_scenarios() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators to test single eligible validator fallback scenario
    let v1 = create_validator(@0x1, 1, 1, true, ctx); // 100 IOTA - index 0
    let v2 = create_validator(@0x2, 2, 1, true, ctx); // 200 IOTA - index 1
    let v3 = create_validator(@0x3, 15, 1, true, ctx); // 1500 IOTA - index 2
    let v4 = create_validator(@0x4, 4, 1, true, ctx); // 400 IOTA - index 3
    let v5 = create_validator(@0x5, 5, 1, true, ctx); // 500 IOTA - index 4

    let committee_size = 3;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4, v5], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Check initial committee and active validators
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x3, @0x5, @0x4]);
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);

    // Test with single eligible validator - system should fall back to all active validators
    // When only one validator is eligible, the system cannot maintain committee diversity/security
    // So it falls back to using all active validators for committee selection
    // This ensures committee has sufficient validators for consensus and security
    let eligible_validators = vector[2]; // Only v3:1500 at index 2 is eligible
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // With single eligible validator, system falls back to all active validators
    // Committee should contain top 3 validators by stake from all active validators: v3:1500, v5:500, v4:400
    // This fallback behavior ensures committee has enough validators for proper consensus
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x3, @0x5, @0x4], // Top 3 validators by stake (fallback to all active validators)
    );
    assert_eq(validator_set.total_stake_inner(), (1500 + 500 + 400) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 6: Test with validator additions and removals affecting eligibility indices
#[test]
fun test_eligible_committee_selection_with_validator_changes() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA
    let v4 = create_validator(@0x4, 8, 1, false, ctx); // 800 IOTA
    let v5 = create_validator(@0x5, 10, 1, false, ctx); // 1000 IOTA
    let v6 = create_validator(@0x6, 12, 1, false, ctx); // 1200 IOTA
    let v7 = create_validator(@0x7, 14, 1, false, ctx); // 1400 IOTA
    let committee_size = 4;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Add validators v4 and v5 first to have more validators
    add_and_activate_validator(&mut validator_set, v5, scenario);
    add_and_activate_validator(&mut validator_set, v4, scenario);

    // Advance epoch to activate them
    advance_epoch_with_dummy_rewards(&mut validator_set, committee_size, scenario);

    // Now we have active validators: v1:200, v2:400, v3:600, v4:800, v5:1000
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x2, @0x3, @0x4, @0x5],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x2, @0x3], // Top 3 eligible validators did not change.
    );

    // Remove validator v2 (index 1) - this validator will be removed from active_validators
    scenario.next_tx(@0x2);
    validator_set.request_remove_validator(scenario.ctx());

    // Test key scenario: v2 is removed from active_validators but we can still include it in eligible list
    // Before epoch change: active_validators = [v1:200, v2:400, v3:600, v4:800, v5:1000] (indices 0,1,2,3,4)
    // After processing removals: active_validators = [v1:200, v3:600, v4:800, v5:1000]
    // But we can still make the removed validator v2 (index 1) eligible
    let eligible_validators = vector[0, 1, 2, 4]; // Include removed validator v2 (index 1) and others
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // v2 was removed from active_validators but was eligible, so should not be in committee
    // Committee should include v5:1000, v4:800, v3:600, v1:200 (top eligible validators still active)
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x3, @0x4, @0x5], // v2 was removed
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x3, @0x5], // Top 3 eligible validators by stake (v2 was removed, v4 ineligible)
    );

    // Test scenario where eligible validators count > committee_size
    // Now test with 6 validators but committee_size = 4
    add_and_activate_validator(&mut validator_set, v7, scenario);
    add_and_activate_validator(&mut validator_set, v6, scenario);
    let eligible_validators = vector[0, 1, 2, 3]; // All active validators eligible

    // Should select top 3 eligible validators by stake
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x3, @0x4, @0x5, @0x6, @0x7],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x1, @0x3, @0x4, @0x5], // Top 4 eligible validators by stake (all active validators from the previous epoch)
    );

    // Now we have 6 active validators with 4 committee slots
    // Make 5 validators eligible (more than committee_size)
    let eligible_validators = vector[0, 1, 2, 3, 5]; // 5 eligible validators, committee_size = 4
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Should select top 3 eligible validators by stake
    assert_eq(
        validator_set.active_validator_addresses(),
        vector[@0x1, @0x3, @0x4, @0x5, @0x6, @0x7],
    );
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x7, @0x5, @0x4, @0x3], // Top 4 eligible validators by stake
    );

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 7: Empty eligible validators list - fallback to all prev validators
#[test]
fun test_empty_eligible_validators_fallback_to_all_prev_validators() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators with different stakes to test comprehensive fallback scenarios
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA - index 0
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA - index 1
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA - index 2
    let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA - index 3
    let v5 = create_validator(@0x5, 10, 1, true, ctx); // 1000 IOTA - index 4

    let committee_size = 3;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4, v5], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Check initial committee (top 3 by stake)
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x5, @0x4, @0x3]); // Top 3 by stake

    // Test Case 1: Empty eligible validators with committee_size < active_validators
    // Should fall back to all prev validators and select top committee_size by stake
    let eligible_validators = vector[]; // Empty list
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Should select top 3 validators by stake from all prev validators (fallback behavior)
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x5, @0x4, @0x3]); // Top 3 by stake
    assert_eq(validator_set.total_stake_inner(), (1000 + 800 + 600) * NANOS_PER_IOTA);

    // Test Case 2: Empty eligible validators with committee_size > active_validators
    // Should include all available validators
    let eligible_validators = vector[]; // Empty list
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        7, // Committee size larger than validator count (5)
        eligible_validators,
        scenario,
    );

    // Should include all 5 validators since committee_size > validator count
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]); // All validators by stake
    assert_eq(validator_set.total_stake_inner(), (1000 + 800 + 600 + 400 + 200) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 8: Empty eligible validators list with single validator
#[test]
fun test_empty_eligible_validators_single_validator_fallback() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create single validator to test edge case
    let v1 = create_validator(@0x1, 5, 1, true, ctx); // 500 IOTA

    let committee_size = 3;
    let mut validator_set = validator_set::new_v2(vector[v1], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Check initial state
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x1]);

    // Test empty eligible validators with single validator
    let eligible_validators = vector[]; // Empty list
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Should fallback to the single available validator
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x1]);
    assert_eq(validator_set.total_stake_inner(), 500 * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 9: Invalid/out-of-bounds indices in eligible list
#[test]
#[expected_failure(abort_code = validator_set::EInvalidEligibleValidatorIndex)]
fun test_eligible_committee_selection_invalid_indices() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create only 3 validators (indices 0, 1, 2)
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA - index 0
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA - index 1
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA - index 2

    let committee_size = 2;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Test with out-of-bounds index (index 5 doesn't exist, only 0,1,2 are valid)
    let eligible_validators = vector[0, 2, 5]; // Index 5 is out of bounds // TODO: this should return an error
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Should fail with INDEX_OUT_OF_BOUNDS before reaching this point
    test_utils::destroy(validator_set);
    scenario_val.end();
}

// Test case 10: Duplicate indices in eligible validators list
#[test]
fun test_eligible_committee_selection_duplicate_indices() {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();

    // Create validators with different stakes
    let v1 = create_validator(@0x1, 2, 1, true, ctx); // 200 IOTA - index 0
    let v2 = create_validator(@0x2, 4, 1, true, ctx); // 400 IOTA - index 1
    let v3 = create_validator(@0x3, 6, 1, true, ctx); // 600 IOTA - index 2
    let v4 = create_validator(@0x4, 8, 1, true, ctx); // 800 IOTA - index 3
    let v5 = create_validator(@0x5, 10, 1, true, ctx); // 1000 IOTA - index 4

    let committee_size = 3;
    let mut validator_set = validator_set::new_v2(vector[v1, v2, v3, v4, v5], committee_size, ctx);
    scenario_val.end();

    let mut scenario_val = test_scenario::begin(@0x1);
    let scenario = &mut scenario_val;

    // Check initial committee
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(validator_set.committee_validator_addresses(), vector[@0x5, @0x4, @0x3]);

    // Test with duplicate indices in eligible validators list
    // Duplicates: index 1 appears twice, index 3 appears twice
    let eligible_validators = vector[1, 2, 1, 3, 4, 3]; // v2:400, v3:600, v2:400, v4:800, v5:1000, v4:800
    advance_epoch_with_eligible_validators(
        &mut validator_set,
        committee_size,
        eligible_validators,
        scenario,
    );

    // Committee should handle duplicates correctly by treating them as unique validators
    // Should select top 3 by stake from the unique eligible validators: v5:1000, v4:800, v3:600
    // Duplicates should not affect the selection logic
    assert_eq(validator_set.active_validator_addresses(), vector[@0x1, @0x2, @0x3, @0x4, @0x5]);
    assert_eq(
        validator_set.committee_validator_addresses(),
        vector[@0x5, @0x4, @0x3], // Top 3 eligible validators by stake (v5:1000, v4:800, v3:600 but ordered differently)
    );
    assert_eq(validator_set.total_stake_inner(), (1000 + 800 + 600) * NANOS_PER_IOTA);

    test_utils::destroy(validator_set);
    scenario_val.end();
}
