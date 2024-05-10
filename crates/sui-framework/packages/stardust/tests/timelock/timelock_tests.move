// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module stardust::timelock_tests {

    use sui::balance;
    use sui::sui::SUI;
    use sui::test_scenario;

    use stardust::timelock;

    #[test]
    fun test_lock_unlock_flow() {
        // Set up a test enviroment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Minting some IOTA.
        let iota = balance::create_for_testing<SUI>(10);

        // Lock the IOTA balance.
        let timelock = timelock::lock(iota, 100, scenario.ctx());

        // Check the locked IOTA.
        assert!(timelock.locked().value() == 10, 0);

        // Check if the timelock is locked.
        assert!(timelock.is_locked(scenario.ctx()), 1);
        assert!(timelock.remaining_time(scenario.ctx()) == 100, 2);

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(10);

        // Check if the timelock is still locked.
        assert!(timelock.is_locked(scenario.ctx()), 3);
        assert!(timelock.remaining_time(scenario.ctx()) == 90, 4);

        // Increment epoch timestamp again.
        scenario.ctx().increment_epoch_timestamp(90);

        // Check if the timelock is unlocked.
        assert!(!timelock.is_locked(scenario.ctx()), 5);
        assert!(timelock.remaining_time(scenario.ctx()) == 0, 6);

        // Unlock the IOTA balance.
        let balance = timelock::unlock(timelock, scenario.ctx());

        // Check the unlocked IOTA balance.
        assert!(balance.value() == 10, 7);

        // Cleanup.
        balance::destroy_for_testing(balance);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = timelock::EExpireEpochIsPast)]
    fun test_expiration_time_is_passed() {
        // Set up a test enviroment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Increment epoch timestamp.
        scenario.ctx().increment_epoch_timestamp(100);

        // Minting some IOTA.
        let iota = balance::create_for_testing<SUI>(10);

        // Lock the IOTA balance with a wrong expiration time.
        let timelock = timelock::lock(iota, 10, scenario.ctx());

        // Cleanup.
        let (balance, _) = timelock::unpack(timelock);

        balance::destroy_for_testing(balance);

        scenario.end();
    }

    #[test]
    #[expected_failure(abort_code = timelock::ENotExpiredYet)]
    fun test_unlock_not_expired_object() {
        // Set up a test enviroment.
        let sender = @0xA;
        let mut scenario = test_scenario::begin(sender);

        // Minting some IOTA.
        let iota = balance::create_for_testing<SUI>(10);

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
