// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/// A timelock implementation.
module stardust::timelock {
    /// Error code for when the expire timestamp of the lock is in the past.
    const EExpireEpochIsPast: u64 = 0;

    /// Error code for when the lock has not expired yet.
    const ENotExpiredYet: u64 = 1;

    /// `TimeLock` struct that holds a locked object.
    public struct TimeLock<T: store> has key {
        id: UID,
        /// The locked object.
        locked: T,
        /// This is the epoch time stamp of when the lock expires.
        expire_timestamp_ms: u64,
    }

    /// Function to lock an object till a unix timestamp in milliseconds.
    public fun lock<T: store>(locked: T, expire_timestamp_ms: u64, ctx: &mut TxContext): TimeLock<T> {
        // Get the epoch timestamp.
        let epoch_timestamp_ms = ctx.epoch_timestamp_ms();

        // Check that `expire_timestamp_ms` is valid.
        assert!(expire_timestamp_ms > epoch_timestamp_ms, EExpireEpochIsPast);

        // Create a timelock.
        pack(locked, expire_timestamp_ms, ctx)
    }

    /// Function to unlock the object from a `TimeLock`.
    public fun unlock<T: store>(self: TimeLock<T>, ctx: &mut TxContext): T {
        // Unpack the timelock. 
        let (locked, expire_timestamp_ms) = unpack(self);

        // Check if the lock has expired.
        assert!(expire_timestamp_ms <= ctx.epoch_timestamp_ms(), ENotExpiredYet);

        locked
    }

    /// Function to check if a `TimeLock` is locked.
    public fun is_locked<T: store>(self: &TimeLock<T>, ctx: &mut TxContext): bool {
        self.remaining_time(ctx) > 0
    }

    /// Function to get the remaining time of a `TimeLock`.
    /// Returns 0 if the lock has expired.
    public fun remaining_time<T: store>(self: &TimeLock<T>, ctx: &mut TxContext): u64 {
        // Get the epoch timestamp.
        let current_timestamp_ms = ctx.epoch_timestamp_ms();

        // Check if the lock has expired.
        if (self.expire_timestamp_ms < current_timestamp_ms) {
            return 0
        };

        // Calculate the remaining time.
        self.expire_timestamp_ms - current_timestamp_ms
    }

    /// Function to get the locked object of a `TimeLock`.
    public fun locked<T: store>(self: &TimeLock<T>): &T {
        &self.locked
    }

    /// An utility function to pack a `TimeLock`.
    public(package) fun pack<T: store>(locked: T, expire_timestamp_ms: u64, ctx: &mut TxContext): TimeLock<T> {
        // Create a timelock.
        TimeLock {
            id: object::new(ctx),
            locked,
            expire_timestamp_ms
        }
    }

    /// An utility function to unpack a `TimeLock`.
    public(package) fun unpack<T: store>(lock: TimeLock<T>): (T, u64) {
        // Unpack the timelock.
        let TimeLock {
            id,
            locked,
            expire_timestamp_ms
        } = lock;

        // Delete the timelock. 
        object::delete(id);

        (locked, expire_timestamp_ms)
    }

    /// An utility function to transfer a `TimeLock`.
    public(package) fun transfer<T: store>(lock: TimeLock<T>, recipient: address) {
        transfer::transfer(lock, recipient);
    }
}
