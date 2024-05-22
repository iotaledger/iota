// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module stardust::timelocked_staking {

    use sui::balance::{Self, Balance};
    use sui::sui::SUI;

    use sui_system::sui_system::{SuiSystemState};

    use stardust::timelock::{Self, TimeLock};
    use stardust::timelocked_staked_sui::{Self, TimelockedStakedSui};

    /// For when trying to stake an expired time-locked balance.
    const ETimeLockShouldNotBeExpired: u64 = 0;

    /// Add a time-locked stake to a validator's staking pool.
    public entry fun request_add_stake(
        sui_system: &mut SuiSystemState,
        timelocked_balance: TimeLock<Balance<SUI>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) {
        // Stake the time-locked balance.
        let timelocked_staked_sui = request_add_stake_non_entry(sui_system, timelocked_balance, validator_address, ctx);

        // Transfer the receipt to the sender.
        timelocked_staked_sui::transfer(timelocked_staked_sui, ctx.sender());
    }

    /// The non-entry version of `request_add_stake`, which returns the time-locked staked SUI instead of transferring it to the sender.
    public fun request_add_stake_non_entry(
        sui_system: &mut SuiSystemState,
        timelocked_balance: TimeLock<Balance<SUI>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) : TimelockedStakedSui {
        // Check the preconditions.
        assert!(timelocked_balance.is_locked(ctx), ETimeLockShouldNotBeExpired);

        // Unpack the time-locked balance.
        let (balance, expiration_timestamp_ms) = timelock::unpack(timelocked_balance);

        // Stake the time-locked balance.
        let staked_sui = sui_system.request_add_stake_non_entry(
            balance.into_coin(ctx),
            validator_address,
            ctx,
        );

        // Create and return a receipt.
        timelocked_staked_sui::create(
            staked_sui,
            expiration_timestamp_ms,
            ctx
        )
    }

    /// Add a time-locked stake to a validator's staking pool using multiple time-locked balances.
    public entry fun request_add_stake_mul_bal(
        sui_system: &mut SuiSystemState,
        timelocked_balances: vector<TimeLock<Balance<SUI>>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) {
        // Stake the time-locked balances.
        let mut receipts = request_add_stake_mul_bal_non_entry(sui_system, timelocked_balances, validator_address, ctx);

        // Create useful variables.
        let (mut i, len) = (0, receipts.length());

        // Send all the receipts to the sender.
        while (i < len) {
            // Take a receipt.
            let receipt = receipts.pop_back();

            // Transfer the receipt to the sender.
            timelocked_staked_sui::transfer(receipt, ctx.sender());

            i = i + 1
        };

        // Destroy the empty vector.
        vector::destroy_empty(receipts)
    }

    /// The non-entry version of `request_add_stake_mul_bal`,
    /// which returns a list of the time-locked staked SUIs instead of transferring them to the sender.
    public fun request_add_stake_mul_bal_non_entry(
        sui_system: &mut SuiSystemState,
        mut timelocked_balances: vector<TimeLock<Balance<SUI>>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) : vector<TimelockedStakedSui> {
        // Create a vector to store the results.
        let mut result = vector[];

        // Create useful variables.
        let (mut i, len) = (0, timelocked_balances.length());

        // Stake all the time-locked balances.
        while (i < len) {
            // Take a time-locked balance.
            let timelocked_balance = timelocked_balances.pop_back();

            // Stake the time-locked balance.
            let timelocked_staked_sui = request_add_stake_non_entry(sui_system, timelocked_balance, validator_address, ctx);

            // Store the created receipt.
            result.push_back(timelocked_staked_sui);

            i = i + 1
        };

        // Destroy the empty vector.
        vector::destroy_empty(timelocked_balances);

        result
    }

    /// Withdraw a time-locked stake from a validator's staking pool.
    public entry fun request_withdraw_stake(
        sui_system: &mut SuiSystemState,
        timelocked_staked_sui: TimelockedStakedSui,
        ctx: &mut TxContext,
    ) {
        // Withdraw the time-locked balance.
        let (timelocked_balance, reward) = request_withdraw_stake_non_entry(sui_system, timelocked_staked_sui, ctx);

        // Transfer the withdrawn time-locked balance to the sender.
        timelock::transfer(timelocked_balance, ctx.sender());

        // Send coins only if the reward is not zero.
        if (reward.value() > 0) {
            transfer::public_transfer(reward.into_coin(ctx), ctx.sender());
        }
        else {
            balance::destroy_zero(reward);
        }
    }

    /// Non-entry version of `request_withdraw_stake` that returns the withdrawn time-locked SUI and reward
    /// instead of transferring it to the sender.
    public fun request_withdraw_stake_non_entry(
        sui_system: &mut SuiSystemState,
        timelocked_staked_sui: TimelockedStakedSui,
        ctx: &mut TxContext,
    ) : (TimeLock<Balance<SUI>>, Balance<SUI>) {
        // Unpack the `TimelockedStakedSui` instance.
        let (staked_sui, expiration_timestamp_ms) = timelocked_staked_sui.unpack();

        // Store the original stake amount.
        let principal = staked_sui.staked_sui_amount();

        // Withdraw the balance.
        let mut withdraw_stake = sui_system.request_withdraw_stake_non_entry(staked_sui, ctx);

        // The sui_system withdraw functions return a balance that consists of the original staked amount plus the reward amount;
        // In here, it splits the original staked balance to timelock it again.
        let principal = withdraw_stake.split(principal);

        // Pack and return a time-locked balance, and the reward.
        (timelock::pack(principal, expiration_timestamp_ms, ctx), withdraw_stake)
    }
}
