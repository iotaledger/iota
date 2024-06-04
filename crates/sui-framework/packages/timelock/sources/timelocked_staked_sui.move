// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module timelock::timelocked_staked_sui {

    use sui_system::staking_pool::StakedSui;

    const EIncompatibleTimelockedStakedSui: u64 = 0;

    /// A self-custodial object holding the timelocked staked SUI tokens.
    public struct TimelockedStakedSui has key {
        id: UID,
        /// A self-custodial object holding the staked SUI tokens.
        staked_sui: StakedSui,
        /// This is the epoch time stamp of when the lock expires.
        expiration_timestamp_ms: u64,
    }

    /// Create a new instance of `TimelockedStakedSui`.
    public(package) fun create(
        staked_sui: StakedSui,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ): TimelockedStakedSui {
        TimelockedStakedSui {
            id: object::new(ctx),
            staked_sui,
            expiration_timestamp_ms
        }
    }

    /// Function to get the pool id of a `TimelockedStakedSui`.
    public fun pool_id(self: &TimelockedStakedSui): ID { self.staked_sui.pool_id() }

    /// Function to get the staked sui amount of a `TimelockedStakedSui`.
    public fun staked_sui_amount(self: &TimelockedStakedSui): u64 { self.staked_sui.staked_sui_amount() }

    /// Allows calling `.amount()` on `TimelockedStakedSui` to invoke `staked_sui_amount`
    public use fun staked_sui_amount as TimelockedStakedSui.amount;

    /// Function to get the stake activation epoch of a `TimelockedStakedSui`.
    public fun stake_activation_epoch(self: &TimelockedStakedSui): u64 {
        self.staked_sui.stake_activation_epoch()
    }

    /// Function to get the expiration timestamp of a `TimelockedStakedSui`.
    public fun expiration_timestamp_ms(self: &TimelockedStakedSui): u64 {
        self.expiration_timestamp_ms
    }

    /// Split `TimelockedStakedSui` into two parts, one with principal `split_amount`,
    /// and the remaining principal is left in `self`.
    /// All the other parameters of the `TimelockedStakedSui` like `stake_activation_epoch` or `pool_id` remain the same.
    public fun split(self: &mut TimelockedStakedSui, split_amount: u64, ctx: &mut TxContext): TimelockedStakedSui {
        let splitted_stake = self.staked_sui.split(split_amount, ctx);

        TimelockedStakedSui {
            id: object::new(ctx),
            staked_sui: splitted_stake,
            expiration_timestamp_ms: self.expiration_timestamp_ms,
        }
    }

    /// Split the given `TimelockedStakedSui` to the two parts, one with principal `split_amount`,
    /// transfer the newly split part to the sender address.
    public entry fun split_staked_sui(stake: &mut TimelockedStakedSui, split_amount: u64, ctx: &mut TxContext) {
        transfer::transfer(split(stake, split_amount, ctx), ctx.sender());
    }

    /// Allows calling `.split_to_sender()` on `TimelockedStakedSui` to invoke `split_staked_sui`
    public use fun split_staked_sui as TimelockedStakedSui.split_to_sender;

    /// Consume the staked sui `other` and add its value to `self`.
    /// Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)
    public entry fun join_staked_sui(self: &mut TimelockedStakedSui, other: TimelockedStakedSui) {
        assert!(self.is_equal_staking_metadata(&other), EIncompatibleTimelockedStakedSui);

        let TimelockedStakedSui {
            id,
            staked_sui,
            expiration_timestamp_ms: _,
        } = other;

        id.delete();

        self.staked_sui.join(staked_sui);
    }

    /// Allows calling `.join()` on `TimelockedStakedSui` to invoke `join_staked_sui`
    public use fun join_staked_sui as TimelockedStakedSui.join;

    /// Returns true if all the staking parameters of the staked sui except the principal are identical
    public fun is_equal_staking_metadata(self: &TimelockedStakedSui, other: &TimelockedStakedSui): bool {
        self.staked_sui.is_equal_staking_metadata(&other.staked_sui) &&
        (self.expiration_timestamp_ms == other.expiration_timestamp_ms)
    }

    /// An utility function to destroy a `TimelockedStakedSui`.
    public(package) fun unpack(self: TimelockedStakedSui): (StakedSui, u64) {
        let TimelockedStakedSui {
            id,
            staked_sui,
            expiration_timestamp_ms,
        } = self;

        object::delete(id);

        (staked_sui, expiration_timestamp_ms)
    }

    /// An utility function to transfer a `TimelockedStakedSui`.
    public(package) fun transfer(stake: TimelockedStakedSui, recipient: address) {
        transfer::transfer(stake, recipient);
    }
}
