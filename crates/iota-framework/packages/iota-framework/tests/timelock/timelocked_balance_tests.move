// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::timelocked_balance_tests;

use iota::balance::{Self, Balance};
use iota::iota::IOTA;
use iota::labeler::LabelerCap;
use iota::test_label_one::{Self, TEST_LABEL_ONE};
use iota::test_label_two::{Self, TEST_LABEL_TWO};
use iota::test_scenario;
use iota::test_utils::{Self, assert_eq};
use iota::timelock::{Self, TimeLock};

#[test]
fun test_join_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);

    // Lock the IOTA balances.
    let mut timelock1 = timelock::lock(iota1, 100, scenario.ctx());
    let timelock2 = timelock::lock(iota2, 100, scenario.ctx());

    // Join the timelocks.
    timelock::join(&mut timelock1, timelock2);

    // Check the joined timelock.
    assert_eq(timelock1.expiration_timestamp_ms(), 100);
    assert_eq(timelock1.locked().value(), 25);
    assert_eq(timelock1.label().is_none(), true);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.end();
}

#[test]
fun test_join_labeled_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Initialize LabelerCap instances.
    test_label_one::assign_labeler_cap(sender, scenario.ctx());

    // Advance the scenario to a new transaction.
    scenario.next_tx(sender);

    // Take the capabilities.
    let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);

    // Lock the IOTA balances.
    let mut timelock1 = timelock::lock_with_label(&labeler_one, iota1, 100, scenario.ctx());
    let timelock2 = timelock::lock_with_label(&labeler_one, iota2, 100, scenario.ctx());

    // Join the timelocks.
    timelock::join(&mut timelock1, timelock2);

    // Check the joined timelock.
    assert_eq(timelock1.locked().value(), 25);
    assert_eq(timelock1.expiration_timestamp_ms(), 100);
    assert_eq(timelock1.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.return_to_sender(labeler_one);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = timelock::EDifferentExpirationTime)]
fun test_join_timelocked_balances_with_different_exp_time() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);

    // Lock the IOTA balances.
    let mut timelock1 = timelock::lock(iota1, 100, scenario.ctx());
    let timelock2 = timelock::lock(iota2, 200, scenario.ctx());

    // Join the timelocks.
    timelock::join(&mut timelock1, timelock2);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = timelock::EDifferentLabels)]
fun test_join_labeled_timelocked_balances_with_different_labels() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Initialize LabelerCap instances.
    test_label_one::assign_labeler_cap(sender, scenario.ctx());
    test_label_two::assign_labeler_cap(sender, scenario.ctx());

    // Advance the scenario to a new transaction.
    scenario.next_tx(sender);

    // Take the capabilities.
    let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();
    let labeler_two = scenario.take_from_sender<LabelerCap<TEST_LABEL_TWO>>();

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);

    // Lock the IOTA balance.
    let mut timelock1 = timelock::lock_with_label(&labeler_one, iota1, 100, scenario.ctx());
    let timelock2 = timelock::lock_with_label(&labeler_two, iota2, 100, scenario.ctx());

    // Join the timelocks.
    timelock::join(&mut timelock1, timelock2);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.return_to_sender(labeler_one);
    scenario.return_to_sender(labeler_two);

    scenario.end();
}

#[test]
fun test_join_vec_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);
    let iota3 = balance::create_for_testing<IOTA>(20);
    let iota4 = balance::create_for_testing<IOTA>(25);

    // Lock the IOTA balances.
    let mut timelock1 = timelock::lock(iota1, 100, scenario.ctx());

    let mut others = vector[];

    others.push_back(timelock::lock(iota2, 100, scenario.ctx()));
    others.push_back(timelock::lock(iota3, 100, scenario.ctx()));
    others.push_back(timelock::lock(iota4, 100, scenario.ctx()));

    // Join the timelocks.
    timelock::join_vec(&mut timelock1, others);

    // Check the joined timelock.
    assert_eq(timelock1.expiration_timestamp_ms(), 100);
    assert_eq(timelock1.locked().value(), 70);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.end();
}

#[test]
fun test_join_empty_vec_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut timelock = timelock::lock(iota, 100, scenario.ctx());
    let others = vector[];

    // Join the timelocks.
    timelock::join_vec(&mut timelock, others);

    // Check the joined timelock.
    assert_eq(timelock.expiration_timestamp_ms(), 100);
    assert_eq(timelock.locked().value(), 10);

    // Cleanup.
    test_utils::destroy(timelock);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = timelock::EDifferentExpirationTime)]
fun test_join_vec_timelocked_balances_with_different_exp_time() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota1 = balance::create_for_testing<IOTA>(10);
    let iota2 = balance::create_for_testing<IOTA>(15);
    let iota3 = balance::create_for_testing<IOTA>(20);
    let iota4 = balance::create_for_testing<IOTA>(25);

    // Lock the IOTA balances.
    let mut timelock1 = timelock::lock(iota1, 100, scenario.ctx());

    let mut others = vector[];

    others.push_back(timelock::lock(iota2, 100, scenario.ctx()));
    others.push_back(timelock::lock(iota3, 200, scenario.ctx()));
    others.push_back(timelock::lock(iota4, 100, scenario.ctx()));

    // Join the timelocks.
    timelock::join_vec(&mut timelock1, others);

    // Cleanup.
    test_utils::destroy(timelock1);

    scenario.end();
}

#[test]
fun test_split_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock(iota, 100, scenario.ctx());

    // Split the timelock.
    let split = timelock::split(&mut original, 3, scenario.ctx());

    // Check the original timelock.
    assert_eq(original.expiration_timestamp_ms(), 100);
    assert_eq(original.locked().value(), 7);

    // Check the split timelock.
    assert_eq(split.expiration_timestamp_ms(), 100);
    assert_eq(split.locked().value(), 3);

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.end();
}

#[test]
fun test_split_and_transfer_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock(iota, 100, scenario.ctx());

    // Split and transfer the timelock.
    let split = timelock::split(&mut original, 3, scenario.ctx());
    split.transfer_to_sender(scenario.ctx());
    scenario.next_tx(sender);

    // Check the original timelock.
    assert_eq(original.expiration_timestamp_ms(), 100);
    assert_eq(original.locked().value(), 7);

    // Check the split timelock.
    let split = scenario.take_from_address<TimeLock<Balance<IOTA>>>(sender);
    assert_eq(split.expiration_timestamp_ms(), 100);
    assert_eq(split.locked().value(), 3);

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.end();
}

#[test]
fun test_split_zero_value_from_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock(iota, 100, scenario.ctx());

    // Split the timelock.
    let split = timelock::split(&mut original, 0, scenario.ctx());

    // Check the original timelock.
    assert_eq(original.expiration_timestamp_ms(), 100);
    assert_eq(original.locked().value(), 10);

    // Check the split timelock.
    assert_eq(split.expiration_timestamp_ms(), 100);
    assert_eq(split.locked().value(), 0);

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.end();
}

#[test]
fun test_split_same_value_from_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock(iota, 100, scenario.ctx());

    // Split the timelock.
    let split = timelock::split(&mut original, 10, scenario.ctx());

    // Check the original timelock.
    assert_eq(original.expiration_timestamp_ms(), 100);
    assert_eq(original.locked().value(), 0);

    // Check the split timelock.
    assert_eq(split.expiration_timestamp_ms(), 100);
    assert_eq(split.locked().value(), 10);

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.end();
}

#[test]
fun test_split_labeled_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Initialize LabelerCap instances.
    test_label_one::assign_labeler_cap(sender, scenario.ctx());

    // Advance the scenario to a new transaction.
    scenario.next_tx(sender);

    // Take the capabilities.
    let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock_with_label(&labeler_one, iota, 100, scenario.ctx());

    // Split the timelock.
    let split = timelock::split(&mut original, 3, scenario.ctx());

    // Check the original timelock.
    assert_eq(original.locked().value(), 7);
    assert_eq(original.expiration_timestamp_ms(), 100);
    assert_eq(original.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);

    // Check the split timelock.
    assert_eq(split.locked().value(), 3);
    assert_eq(split.expiration_timestamp_ms(), 100);
    assert_eq(split.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.return_to_sender(labeler_one);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = balance::ENotEnough)]
fun test_split_bigger_value_from_timelocked_balances() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Minting some IOTA.
    let iota = balance::create_for_testing<IOTA>(10);

    // Lock the IOTA balance.
    let mut original = timelock::lock(iota, 100, scenario.ctx());

    // Split the timelock.
    let split = timelock::split(&mut original, 11, scenario.ctx());

    // Cleanup.
    test_utils::destroy(original);
    test_utils::destroy(split);

    scenario.end();
}
