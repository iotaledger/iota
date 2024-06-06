// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Utility functions for time-locked balance.
module timelock::timelocked_balance {

    use iota::balance::Balance;

    use timelock::timelock::{Self, TimeLock};

    /// For when trying to join two time-locked balances with different expiration time.
    const EDifferentExpirationTime: u64 = 0;
    /// For when trying to join two time-locked balances with different labels.
    const EDifferentLabels: u64 = 1;

    /// Join two `TimeLock<Balance<T>>` together.
    public fun join<T>(self: &mut TimeLock<Balance<T>>, other: TimeLock<Balance<T>>) {
        // Check the preconditions.
        assert!(self.expiration_timestamp_ms() == other.expiration_timestamp_ms(), EDifferentExpirationTime);
        assert!(self.label() == other.label(), EDifferentLabels);

        // Unpack the time-locked balance.
        let (value, _, _) = timelock::unpack(other);

        // Join the balances.
        self.locked_mut().join(value);
    }

    /// Join everything in `others` with `self`.
    public fun join_vec<T>(self: &mut TimeLock<Balance<T>>, mut others: vector<TimeLock<Balance<T>>>) {
        // Create useful variables.
        let (mut i, len) = (0, others.length());

        // Join all the balances.
        while (i < len) {
            let other = others.pop_back();
            Self::join(self, other);
            i = i + 1
        };

        // Destroy the empty vector.
        vector::destroy_empty(others)
    }

    /// Split a `TimeLock<Balance<T>>` and take a sub balance from it.
    public fun split<T>(self: &mut TimeLock<Balance<T>>, value: u64, ctx: &mut TxContext): TimeLock<Balance<T>> {
        // Split the locked balance.
        let value = self.locked_mut().split(value);

        // Pack the splitted balance into a timelock.
        timelock::pack(value, self.expiration_timestamp_ms(), self.label(), ctx)
    }
}
