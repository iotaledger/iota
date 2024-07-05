// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module timelock::timelocked_staked_iota {

    use std::string::String;

    use iota_system::staking_pool::StakedIota;

    use timelock::labeler;

    const EIncompatibleTimelockedStakedIota: u64 = 0;

    /// A self-custodial object holding the timelocked staked IOTA tokens.
    public struct TimelockedStakedIota has key {
        id: UID,
        /// A self-custodial object holding the staked IOTA tokens.
        staked_iota: StakedIota,
        /// This is the epoch time stamp of when the lock expires.
        expiration_timestamp_ms: u64,
        /// Timelock related label.
        label: Option<String>,
    }

    /// Create a new instance of `TimelockedStakedIota`.
    public(package) fun create(
        staked_iota: StakedIota,
        expiration_timestamp_ms: u64,
        label: Option<String>,
        ctx: &mut TxContext
    ): TimelockedStakedIota {
        TimelockedStakedIota {
            id: object::new(ctx),
            staked_iota,
            expiration_timestamp_ms,
            label,
        }
    }

    /// Function to get the pool id of a `TimelockedStakedIota`.
    public fun pool_id(self: &TimelockedStakedIota): ID { self.staked_iota.pool_id() }

    /// Function to get the staked iota amount of a `TimelockedStakedIota`.
    public fun staked_iota_amount(self: &TimelockedStakedIota): u64 { self.staked_iota.staked_iota_amount() }

    /// Allows calling `.amount()` on `TimelockedStakedIota` to invoke `staked_iota_amount`
    public use fun staked_iota_amount as TimelockedStakedIota.amount;

    /// Function to get the stake activation epoch of a `TimelockedStakedIota`.
    public fun stake_activation_epoch(self: &TimelockedStakedIota): u64 {
        self.staked_iota.stake_activation_epoch()
    }

    /// Function to get the expiration timestamp of a `TimelockedStakedIota`.
    public fun expiration_timestamp_ms(self: &TimelockedStakedIota): u64 {
        self.expiration_timestamp_ms
    }

    /// Function to get the label of a `TimelockedStakedIota`.
    public fun label(self: &TimelockedStakedIota): Option<String> {
        self.label
    }

    /// Check if a `TimelockedStakedIota` is labeled with the type `L`.
    public fun is_labeled_with<L>(self: &TimelockedStakedIota): bool {
        if (self.label.is_some()) {
            self.label.borrow() == labeler::type_name<L>()
        }
        else {
            false
        }
    }

    /// Split `TimelockedStakedIota` into two parts, one with principal `split_amount`,
    /// and the remaining principal is left in `self`.
    /// All the other parameters of the `TimelockedStakedIota` like `stake_activation_epoch` or `pool_id` remain the same.
    public fun split(self: &mut TimelockedStakedIota, split_amount: u64, ctx: &mut TxContext): TimelockedStakedIota {
        let splitted_stake = self.staked_iota.split(split_amount, ctx);

        TimelockedStakedIota {
            id: object::new(ctx),
            staked_iota: splitted_stake,
            expiration_timestamp_ms: self.expiration_timestamp_ms,
            label: self.label,
        }
    }

    /// Split the given `TimelockedStakedIota` to the two parts, one with principal `split_amount`,
    /// transfer the newly split part to the sender address.
    public entry fun split_staked_iota(stake: &mut TimelockedStakedIota, split_amount: u64, ctx: &mut TxContext) {
        split(stake, split_amount, ctx).self_transfer(ctx);
    }

    /// Allows calling `.split_to_sender()` on `TimelockedStakedIota` to invoke `split_staked_iota`
    public use fun split_staked_iota as TimelockedStakedIota.split_to_sender;

    /// Consume the staked iota `other` and add its value to `self`.
    /// Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)
    public entry fun join_staked_iota(self: &mut TimelockedStakedIota, other: TimelockedStakedIota) {
        assert!(self.is_equal_staking_metadata(&other), EIncompatibleTimelockedStakedIota);

        let TimelockedStakedIota {
            id,
            staked_iota,
            expiration_timestamp_ms: _,
            label: _,
        } = other;

        id.delete();

        self.staked_iota.join(staked_iota);
    }

    /// Allows calling `.join()` on `TimelockedStakedIota` to invoke `join_staked_iota`
    public use fun join_staked_iota as TimelockedStakedIota.join;

    /// Returns true if all the staking parameters of the staked iota except the principal are identical
    public fun is_equal_staking_metadata(self: &TimelockedStakedIota, other: &TimelockedStakedIota): bool {
        self.staked_iota.is_equal_staking_metadata(&other.staked_iota) &&
        (self.expiration_timestamp_ms == other.expiration_timestamp_ms) &&
        (self.label() == other.label())
    }

    /// A utility function to destroy a `TimelockedStakedIota`.
    public(package) fun unpack(self: TimelockedStakedIota): (StakedIota, u64, Option<String>) {
        let TimelockedStakedIota {
            id,
            staked_iota,
            expiration_timestamp_ms,
            label,
        } = self;

        object::delete(id);

        (staked_iota, expiration_timestamp_ms, label)
    }

    /// A utility function to transfer a `TimelockedStakedIota`.
    public fun self_transfer(stake: TimelockedStakedIota, ctx: &TxContext) {
        transfer::transfer(stake, ctx.sender())
    }

    /// A utility function to transfer multiple `TimelockedStakedIota`.
    public fun self_transfer_multiple(mut stakes: vector<TimelockedStakedIota>, ctx: &TxContext) {
        // Transfer all the time-locked stakes to the recipient.
        while (!stakes.is_empty()) {
           let stake = stakes.pop_back();
           self_transfer(stake, ctx);
        };

        // Destroy the empty vector.
        vector::destroy_empty(stakes);
    }
}
