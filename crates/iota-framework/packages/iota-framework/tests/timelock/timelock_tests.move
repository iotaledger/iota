// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::timelock_tests {

    use std::string;

    use iota::balance::{Self, Balance};
    use iota::iota::IOTA;
    use iota::test_scenario;
    use iota::test_utils::{Self, assert_eq};

    use iota::labeler::LabelerCap;
    use iota::timelock::{Self, TimeLock};

    use iota::test_label_one::{Self, TEST_LABEL_ONE};
    use iota::test_label_two::TEST_LABEL_TWO;

    #[test]
    fun test_lock_unlock_flow() {
        // Set up a test environment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance.
        let timelock = timelock::lock(iota, 100, scenario.ctx());

        // Check the locked IOTA.
        assert_eq(timelock.locked().value(), 10);

        // Check if the timelock is locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 100);

        // Check the label.
        assert_eq(timelock.label().is_none(), true);
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), false);

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Check if the timelock is still locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 90);

        // Increment epoch timestamp again.
        scenario.ctx().increment_epoch_timestamp(90);

        // Check if the timelock is unlocked.
        assert_eq(timelock.is_locked(scenario.ctx()), false);
        assert_eq(timelock.remaining_time(scenario.ctx()), 0);

        // Unlock the IOTA balance.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Check the unlocked IOTA balance.
        assert_eq(balance.value(), 10);

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.end();
    }

    #[test]
    fun test_lock_unlock_labeled_flow() {
        // Set up a test environment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Initialize a LabelerCap instance.
        test_label_one::assign_labeler_cap(sender, scenario.ctx());

        // Advance the scenario to a new transaction.
        scenario.next_tx(sender);

        // Take the capability.
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance.
        let timelock = timelock::lock_with_label(&labeler_one, iota, 100, scenario.ctx());

        // Check the locked IOTA.
        assert_eq(timelock.locked().value(), 10);

        // Check if the timelock is locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 100);

        // Check the labels.
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_TWO>(), false);

        assert_eq(*timelock.label().borrow(), string::utf8(b"0000000000000000000000000000000000000000000000000000000000000002::test_label_one::TEST_LABEL_ONE"));

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Check if the timelock is still locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 90);

        // Increment epoch timestamp again.
        scenario.ctx().increment_epoch_timestamp(90);

        // Check if the timelock is unlocked.
        assert_eq(timelock.is_locked(scenario.ctx()), false);
        assert_eq(timelock.remaining_time(scenario.ctx()), 0);

        // Unlock the IOTA balance.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Check the unlocked IOTA balance.
        assert_eq(balance.value(), 10);

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.return_to_sender(labeler_one);

        scenario.end();
    }

    #[test]
    fun test_lock_to_unlock_flow() {
        // Set up a test environment.
        let sender = @0xA;
        let recipient = @0xB;
        let mut scenario = test_scenario::begin(sender);

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance to recipient.
        timelock::lock_and_transfer(iota, recipient, 100, scenario.ctx());

        // Advance the scenario to a new transaction.
        scenario.next_tx(recipient);

        // Was it transferred correctly?
        let timelock = scenario.take_from_sender<TimeLock<Balance<IOTA>>>();

        // Check the locked IOTA.
        assert_eq(timelock.locked().value(), 10);

        // Check if the timelock is locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 100);

        // Check the label.
        assert_eq(timelock.label().is_none(), true);
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), false);

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Check if the timelock is still locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 90);

        // Increment epoch timestamp again.
        scenario.ctx().increment_epoch_timestamp(90);

        // Check if the timelock is unlocked.
        assert_eq(timelock.is_locked(scenario.ctx()), false);
        assert_eq(timelock.remaining_time(scenario.ctx()), 0);

        scenario.return_to_sender(timelock);

        scenario.next_tx(recipient);

        let timelock = scenario.take_from_sender<TimeLock<Balance<IOTA>>>();

        // Unlock the IOTA balance.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Check the unlocked IOTA balance.
        assert_eq(balance.value(), 10);

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.end();
    }

    #[test]
    fun test_lock_unlock_to_labeled_flow() {
        // Set up a test environment.
        let sender = @0xA;
        let recipient = @0xB;
        let mut scenario = test_scenario::begin(sender);

        // Initialize a LabelerCap instance.
        test_label_one::assign_labeler_cap(sender, scenario.ctx());

        // Advance the scenario to a new transaction.
        scenario.next_tx(sender);

        // Take the capability.
        let labeler_one = scenario.take_from_sender<LabelerCap<TEST_LABEL_ONE>>();

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance.
        timelock::lock_with_label_and_transfer(&labeler_one, iota, recipient, 100, scenario.ctx());
        scenario.return_to_sender(labeler_one);

        scenario.next_tx(recipient);

        let timelock = scenario.take_from_sender<TimeLock<Balance<IOTA>>>();

        // Check the locked IOTA.
        assert_eq(timelock.locked().value(), 10);

        // Check if the timelock is locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 100);

        // Check the labels.
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_ONE>(), true);
        assert_eq(timelock.is_labeled_with<Balance<IOTA>, TEST_LABEL_TWO>(), false);

        assert_eq(*timelock.label().borrow(), string::utf8(b"0000000000000000000000000000000000000000000000000000000000000002::test_label_one::TEST_LABEL_ONE"));

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Check if the timelock is still locked.
        assert_eq(timelock.is_locked(scenario.ctx()), true);
        assert_eq(timelock.remaining_time(scenario.ctx()), 90);

        // Increment epoch timestamp again.
        scenario.ctx().increment_epoch_timestamp(90);

        // Check if the timelock is unlocked.
        assert_eq(timelock.is_locked(scenario.ctx()), false);
        assert_eq(timelock.remaining_time(scenario.ctx()), 0);

        scenario.return_to_sender(timelock);

        scenario.next_tx(recipient);

        let timelock = scenario.take_from_sender<TimeLock<Balance<IOTA>>>();

        // Unlock the IOTA balance.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Check the unlocked IOTA balance.
        assert_eq(balance.value(), 10);

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = timelock::EExpireEpochIsPast)]
    fun test_expiration_time_is_passed() {
        // Set up a test environment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(100);

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance with a wrong expiration time.
        let timelock = timelock::lock(iota, 10, scenario.ctx());

        // Cleanup.
        test_utils::destroy(timelock);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = timelock::ENotExpiredYet)]
    fun test_unlock_not_expired_object() {
        // Set up a test environment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Minting some IOTA.
        let iota = balance::create_for_testing<IOTA>(10);

        // Lock the IOTA balance.
        let timelock = timelock::lock(iota, 100, scenario.ctx());

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Unlock the IOTA balance which is not expired.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.end();
    }
}
