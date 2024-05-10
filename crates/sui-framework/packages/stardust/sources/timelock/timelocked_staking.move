// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module stardust::timelocked_staking {

    use sui::balance::Balance;
    use sui::sui::SUI;

    use sui_system::sui_system::{SuiSystemState};
    use stardust::timelock::{Self, TimeLock};
    use stardust::timelocked_staked_sui::{Self, TimelockedStakedSui};

    const ETimeLockShouldNotBeExpired: u64 = 0;

    /// Add stake to a validator's staking pool.
    public entry fun request_add_stake(
        sui_system: &mut SuiSystemState,
        timelocked_stake: TimeLock<Balance<SUI>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) {
        let timelocked_staked_sui = request_add_stake_non_entry(sui_system, timelocked_stake, validator_address, ctx);

        timelocked_staked_sui::transfer(timelocked_staked_sui, ctx.sender());
    }

    /// The non-entry version of `request_add_stake`, which returns the timelocked staked SUI instead of transferring it to the sender.
    public fun request_add_stake_non_entry(
        sui_system: &mut SuiSystemState,
        timelocked_stake: TimeLock<Balance<SUI>>,
        validator_address: address,
        ctx: &mut TxContext,
    ) : TimelockedStakedSui {
        assert!(timelocked_stake.is_locked(ctx), ETimeLockShouldNotBeExpired);

        let (stake, expire_timestamp_ms) = timelock::unpack(timelocked_stake);

        let staked_sui = sui_system.request_add_stake_non_entry(
            stake.into_coin(ctx),
            validator_address,
            ctx,
        );

        timelocked_staked_sui::create(
            staked_sui,
            expire_timestamp_ms,
            ctx
        )
    }

    /// Withdraw a timelocked stake from a validator's staking pool.
    public entry fun request_withdraw_stake(
        sui_system: &mut SuiSystemState,
        timelocked_staked_sui: TimelockedStakedSui,
        ctx: &mut TxContext,
    ) {
        let (timelocked_sui, reward) = request_withdraw_stake_non_entry(sui_system, timelocked_staked_sui, ctx);

        timelock::transfer(timelocked_sui, ctx.sender());
        transfer::public_transfer(reward.into_coin(ctx), ctx.sender());
    }

    /// Non-entry version of `request_withdraw_stake` that returns the withdrawn timelocked SUI and reward
    /// instead of transferring it to the sender.
    public fun request_withdraw_stake_non_entry(
        sui_system: &mut SuiSystemState,
        timelocked_staked_sui: TimelockedStakedSui,
        ctx: &mut TxContext,
    ) : (TimeLock<Balance<SUI>>, Balance<SUI>) {
        let (staked_sui, expire_timestamp_ms) = timelocked_staked_sui.unpack();
        let principal = staked_sui.staked_sui_amount();

        let mut withdraw_stake = sui_system.request_withdraw_stake_non_entry(staked_sui, ctx);

        // The sui_system withdraw functions return a balance that consists of the original staked amount plus the reward amount;
        // In here, it splits the original staked balance to timelock it again.
        let principal = withdraw_stake.split(principal);

        (timelock::pack(principal, expire_timestamp_ms, ctx), withdraw_stake)
    }
}
