// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module timelock::timelocked_staking {

    use iota::balance::{Self, Balance};
    use iota::iota::IOTA;

    use iota_system::iota_system::{IotaSystemState};

    use timelock::timelock::{Self, TimeLock};
    use timelock::timelocked_staked_iota::{Self, TimelockedStakedIota};

    /// For when trying to stake an expired time-locked balance.
    const ETimeLockShouldNotBeExpired: u64 = 0;

    /// Add a time-locked stake to a validator's staking pool.
    public entry fun request_add_stake(
        iota_system: &mut IotaSystemState,
        timelocked_balance: TimeLock<Balance<IOTA>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) {
        // Stake the time-locked balance.
        let timelocked_staked_iota = request_add_stake_non_entry(iota_system, timelocked_balance, validator_address, ctx);

        // Transfer the receipt to the sender.
        timelocked_staked_iota.self_transfer(ctx);
    }

    /// The non-entry version of `request_add_stake`, which returns the time-locked staked IOTA instead of transferring it to the sender.
    public fun request_add_stake_non_entry(
        iota_system: &mut IotaSystemState,
        timelocked_balance: TimeLock<Balance<IOTA>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) : TimelockedStakedIota {
        // Check the preconditions.
        assert!(timelocked_balance.is_locked(ctx), ETimeLockShouldNotBeExpired);

        // Unpack the time-locked balance.
        let (balance, expiration_timestamp_ms, label) = timelock::unpack(timelocked_balance);

        // Stake the time-locked balance.
        let staked_iota = iota_system.request_add_stake_non_entry(
            balance.into_coin(ctx),
            validator_address,
            ctx,
        );

        // Create and return a receipt.
        timelocked_staked_iota::create(
            staked_iota,
            expiration_timestamp_ms,
            label,
            ctx,
        )
    }

    /// Add a time-locked stake to a validator's staking pool using multiple time-locked balances.
    public entry fun request_add_stake_mul_bal(
        iota_system: &mut IotaSystemState,
        timelocked_balances: vector<TimeLock<Balance<IOTA>>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) {
        // Stake the time-locked balances.
        let mut receipts = request_add_stake_mul_bal_non_entry(iota_system, timelocked_balances, validator_address, ctx);

        // Create useful variables.
        let (mut i, len) = (0, receipts.length());

        // Send all the receipts to the sender.
        while (i < len) {
            // Take a receipt.
            let receipt = receipts.pop_back();

            // Transfer the receipt to the sender.
            receipt.self_transfer(ctx);

            i = i + 1
        };

        // Destroy the empty vector.
        vector::destroy_empty(receipts)
    }

    /// The non-entry version of `request_add_stake_mul_bal`,
    /// which returns a list of the time-locked staked IOTAs instead of transferring them to the sender.
    public fun request_add_stake_mul_bal_non_entry(
        iota_system: &mut IotaSystemState,
        mut timelocked_balances: vector<TimeLock<Balance<IOTA>>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) : vector<TimelockedStakedIota> {
        // Create a vector to store the results.
        let mut result = vector[];

        // Create useful variables.
        let (mut i, len) = (0, timelocked_balances.length());

        // Stake all the time-locked balances.
        while (i < len) {
            // Take a time-locked balance.
            let timelocked_balance = timelocked_balances.pop_back();

            // Stake the time-locked balance.
            let timelocked_staked_iota = request_add_stake_non_entry(iota_system, timelocked_balance, validator_address, ctx);

            // Store the created receipt.
            result.push_back(timelocked_staked_iota);

            i = i + 1
        };

        // Destroy the empty vector.
        vector::destroy_empty(timelocked_balances);

        result
    }

    /// Withdraw a time-locked stake from a validator's staking pool.
    public entry fun request_withdraw_stake(
        iota_system: &mut IotaSystemState,
        timelocked_staked_iota: TimelockedStakedIota,
        ctx: &mut TxContext,
    ) {
        // Withdraw the time-locked balance.
        let (timelocked_balance, reward) = request_withdraw_stake_non_entry(iota_system, timelocked_staked_iota, ctx);

        // Transfer the withdrawn time-locked balance to the sender.
       timelocked_balance.self_transfer(ctx);

        // Send coins only if the reward is not zero.
        if (reward.value() > 0) {
            transfer::public_transfer(reward.into_coin(ctx), ctx.sender());
        }
        else {
            balance::destroy_zero(reward);
        }
    }

    /// Non-entry version of `request_withdraw_stake` that returns the withdrawn time-locked IOTA and reward
    /// instead of transferring it to the sender.
    public fun request_withdraw_stake_non_entry(
        iota_system: &mut IotaSystemState,
        timelocked_staked_iota: TimelockedStakedIota,
        ctx: &mut TxContext,
    ) : (TimeLock<Balance<IOTA>>, Balance<IOTA>) {
        // Unpack the `TimelockedStakedIota` instance.
        let (staked_iota, expiration_timestamp_ms, label) = timelocked_staked_iota.unpack();

        // Store the original stake amount.
        let principal = staked_iota.staked_iota_amount();

        // Withdraw the balance.
        let mut withdraw_stake = iota_system.request_withdraw_stake_non_entry(staked_iota, ctx);

        // The iota_system withdraw functions return a balance that consists of the original staked amount plus the reward amount;
        // In here, it splits the original staked balance to timelock it again.
        let principal = withdraw_stake.split(principal);

        // Pack and return a time-locked balance, and the reward.
        (timelock::pack(principal, expiration_timestamp_ms, label, ctx), withdraw_stake)
    }
}
