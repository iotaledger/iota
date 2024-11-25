// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module locked_stake::locked_stake_tests {

    use iota_system::governance_test_utils::{advance_epoch, set_up_iota_system_state};
    use iota_system::iota_system::{Self, IotaSystemState};
    use iota::coin;
    use iota::test_scenario;
    use iota::test_utils::{assert_eq, destroy};
    use iota::vec_map;
    use iota::balance;
    use locked_stake::locked_stake as ls;
    use locked_stake::epoch_time_lock;

    const NANOS_PER_IOTA: u64 = 1_000_000_000;

    #[test]
    #[expected_failure(abort_code = epoch_time_lock::EEpochAlreadyPassed)]
    fun test_incorrect_creation() {
        let mut scenario_val = test_scenario::begin(@0x0);
        let scenario = &mut scenario_val;

        set_up_iota_system_state(vector[@0x1, @0x2, @0x3]);

        // Advance epoch twice so we are now at epoch 2.
        advance_epoch(scenario);
        advance_epoch(scenario);
        let ctx = test_scenario::ctx(scenario);
        assert_eq(tx_context::epoch(ctx), 2);

        // Create a locked stake with epoch 1. Should fail here.
        let ls = ls::new(1, ctx);

        destroy(ls);
        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_deposit_stake_unstake() {
        let mut scenario_val = test_scenario::begin(@0x0);
        let scenario = &mut scenario_val;

        set_up_iota_system_state(vector[@0x1, @0x2, @0x3]);

        let mut ls = ls::new(10, test_scenario::ctx(scenario));

        // Deposit 100 IOTA.
        ls::deposit_iota(&mut ls, balance::create_for_testing(100 * NANOS_PER_IOTA));

        assert_eq(ls::iota_balance(&ls), 100 * NANOS_PER_IOTA);

        test_scenario::next_tx(scenario, @0x1);
        let mut system_state = test_scenario::take_shared<IotaSystemState>(scenario);

        // Stake 10 of the 100 IOTA.
        ls::stake(&mut ls, &mut system_state, 10 * NANOS_PER_IOTA, @0x1, test_scenario::ctx(scenario));
        test_scenario::return_shared(system_state);

        assert_eq(ls::iota_balance(&ls), 90 * NANOS_PER_IOTA);
        assert_eq(vec_map::size(ls::staked_iota(&ls)), 1);

        test_scenario::next_tx(scenario, @0x1);
        let mut system_state = test_scenario::take_shared<IotaSystemState>(scenario);
        let ctx = test_scenario::ctx(scenario);

        // Create a StakedIota object and add it to the LockedStake object.
        let staked_iota = iota_system::request_add_stake_non_entry(
            &mut system_state, coin::mint_for_testing(20 * NANOS_PER_IOTA, ctx), @0x2, ctx);
        test_scenario::return_shared(system_state);

        ls::deposit_staked_iota(&mut ls, staked_iota);
        assert_eq(ls::iota_balance(&ls), 90 * NANOS_PER_IOTA);
        assert_eq(vec_map::size(ls::staked_iota(&ls)), 2);
        advance_epoch(scenario);

        test_scenario::next_tx(scenario, @0x1);
        let (staked_iota_id, _) = vec_map::get_entry_by_idx(ls::staked_iota(&ls), 0);
        let mut system_state = test_scenario::take_shared<IotaSystemState>(scenario);

        // Unstake both stake objects
        ls::unstake(&mut ls, &mut system_state, *staked_iota_id, test_scenario::ctx(scenario));
        test_scenario::return_shared(system_state);
        assert_eq(ls::iota_balance(&ls), 100 * NANOS_PER_IOTA);
        assert_eq(vec_map::size(ls::staked_iota(&ls)), 1);

        test_scenario::next_tx(scenario, @0x1);
        let (staked_iota_id, _) = vec_map::get_entry_by_idx(ls::staked_iota(&ls), 0);
        let mut system_state = test_scenario::take_shared<IotaSystemState>(scenario);
        ls::unstake(&mut ls, &mut system_state, *staked_iota_id, test_scenario::ctx(scenario));
        test_scenario::return_shared(system_state);
        assert_eq(ls::iota_balance(&ls), 120 * NANOS_PER_IOTA);
        assert_eq(vec_map::size(ls::staked_iota(&ls)), 0);

        destroy(ls);
        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_unlock_correct_epoch() {
        let mut scenario_val = test_scenario::begin(@0x0);
        let scenario = &mut scenario_val;

        set_up_iota_system_state(vector[@0x1, @0x2, @0x3]);

        let mut ls = ls::new(2, test_scenario::ctx(scenario));

        ls::deposit_iota(&mut ls, balance::create_for_testing(100 * NANOS_PER_IOTA));

        assert_eq(ls::iota_balance(&ls), 100 * NANOS_PER_IOTA);

        test_scenario::next_tx(scenario, @0x1);
        let mut system_state = test_scenario::take_shared<IotaSystemState>(scenario);
        ls::stake(&mut ls, &mut system_state, 10 * NANOS_PER_IOTA, @0x1, test_scenario::ctx(scenario));
        test_scenario::return_shared(system_state);

        advance_epoch(scenario);
        advance_epoch(scenario);
        advance_epoch(scenario);
        advance_epoch(scenario);

        let (staked_iota, iota_balance) = ls::unlock(ls, test_scenario::ctx(scenario));
        assert_eq(balance::value(&iota_balance), 90 * NANOS_PER_IOTA);
        assert_eq(vec_map::size(&staked_iota), 1);

        destroy(staked_iota);
        destroy(iota_balance);
        test_scenario::end(scenario_val);
    }

    #[test]
    #[expected_failure(abort_code = epoch_time_lock::EEpochNotYetEnded)]
    fun test_unlock_incorrect_epoch() {
        let mut scenario_val = test_scenario::begin(@0x0);
        let scenario = &mut scenario_val;

        set_up_iota_system_state(vector[@0x1, @0x2, @0x3]);

        let ls = ls::new(2, test_scenario::ctx(scenario));
        let (staked_iota, iota_balance) = ls::unlock(ls, test_scenario::ctx(scenario));
        destroy(staked_iota);
        destroy(iota_balance);
        test_scenario::end(scenario_val);
    }
}
