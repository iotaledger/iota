// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A timelock implementation.
module timelock::timelock {

    use std::string::String;

    use timelock::labeler::{Self, LabelerCap};

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
        expiration_timestamp_ms: u64,
        /// Timelock related label.
        label: Option<String>,
    }

    /// Function to lock an object till a unix timestamp in milliseconds.
    public fun lock<T: store>(locked: T, expiration_timestamp_ms: u64, ctx: &mut TxContext): TimeLock<T> {
        // Check that `expiration_timestamp_ms` is valid.
        check_expiration_timestamp_ms(expiration_timestamp_ms, ctx);

        // Create a timelock.
        pack(locked, expiration_timestamp_ms, option::none(), ctx)
    }

    /// Function to lock an object `obj` until `expiration_timestamp_ms` and transfer it to address `to`.
    /// Since `Timelock<T>` does not support public transfer, use this function to lock an object to an address.
    public fun lock_to<T: store>(
        obj: T,
        to: address,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ) {
        transfer::transfer(lock(obj, expiration_timestamp_ms, ctx), to);
    }

    /// Function to lock a labeled object till a unix timestamp in milliseconds.
    public fun lock_with_label<T: store, L>(
        _: &LabelerCap<L>,
        locked: T,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ): TimeLock<T> {
        // Check that `expiration_timestamp_ms` is valid.
        check_expiration_timestamp_ms(expiration_timestamp_ms, ctx);

        // Calculate a label value.
        let label = labeler::type_name<L>();

        // Create a labeled timelock.
        pack(locked, expiration_timestamp_ms, option::some(label), ctx)
    }

    /// Function to lock a labeled object `obj` until `expiration_timestamp_ms` and transfer it to address `to`.
    /// Since `Timelock<T>` does not support public transfer, use this function to lock a labeled object to an address.
    public fun lock_with_label_to<T: store, L>(
        labeler: &LabelerCap<L>,
        obj: T,
        to: address,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ) {
        transfer::transfer(lock_with_label(labeler, obj, expiration_timestamp_ms, ctx), to);
    }

    /// Function to unlock the object from a `TimeLock`.
    public fun unlock<T: store>(self: TimeLock<T>, ctx: &TxContext): T {
        // Unpack the timelock. 
        let (locked, expiration_timestamp_ms, _) = unpack(self);

        // Check if the lock has expired.
        assert!(expiration_timestamp_ms <= ctx.epoch_timestamp_ms(), ENotExpiredYet);

        locked
    }

    /// Function to get the expiration timestamp of a `TimeLock`.
    public fun expiration_timestamp_ms<T: store>(self: &TimeLock<T>): u64 {
        self.expiration_timestamp_ms
    }

    /// Function to check if a `TimeLock` is locked.
    public fun is_locked<T: store>(self: &TimeLock<T>, ctx: &TxContext): bool {
        self.remaining_time(ctx) > 0
    }

    /// Function to get the remaining time of a `TimeLock`.
    /// Returns 0 if the lock has expired.
    public fun remaining_time<T: store>(self: &TimeLock<T>, ctx: &TxContext): u64 {
        // Get the epoch timestamp.
        let current_timestamp_ms = ctx.epoch_timestamp_ms();

        // Check if the lock has expired.
        if (self.expiration_timestamp_ms < current_timestamp_ms) {
            return 0
        };

        // Calculate the remaining time.
        self.expiration_timestamp_ms - current_timestamp_ms
    }

    /// Function to get the locked object of a `TimeLock`.
    public fun locked<T: store>(self: &TimeLock<T>): &T {
        &self.locked
    }

    /// Function to get a mutable reference to the locked object of a `TimeLock`.
    /// Must not be callable from the outside, as one could modify the locked object.
    public(package) fun locked_mut<T: store>(self: &mut TimeLock<T>): &mut T {
        &mut self.locked
    }

    /// Function to get the label of a `TimeLock`.
    public fun label<T: store>(self: &TimeLock<T>): Option<String> {
        self.label
    }

    /// Check if a `TimeLock` is labeled with the type `L`.
    public fun is_labeled_with<T: store, L>(self: &TimeLock<T>): bool {
        if (self.label.is_some()) {
            self.label.borrow() == labeler::type_name<L>()
        }
        else {
            false
        }
    }

    /// A utility function to pack a `TimeLock`.
    public(package) fun pack<T: store>(
        locked: T,
        expiration_timestamp_ms: u64,
        label: Option<String>,
        ctx: &mut TxContext): TimeLock<T>
    {
        // Create a timelock.
        TimeLock {
            id: object::new(ctx),
            locked,
            expiration_timestamp_ms,
            label,
        }
    }

    /// An utility function to unpack a `TimeLock`.
    public(package) fun unpack<T: store>(lock: TimeLock<T>): (T, u64, Option<String>) {
        // Unpack the timelock.
        let TimeLock {
            id,
            locked,
            expiration_timestamp_ms,
            label,
        } = lock;

        // Delete the timelock. 
        object::delete(id);

        (locked, expiration_timestamp_ms, label)
    }

    /// A utility function to transfer a `TimeLock` to its original owner.
    public fun self_transfer<T: store>(lock: TimeLock<T>, ctx: &TxContext) {
        transfer::transfer(lock, ctx.sender())
    }

    /// An utility function to check that the `expiration_timestamp_ms` value is valid.
    fun check_expiration_timestamp_ms(expiration_timestamp_ms: u64, ctx: &TxContext) {
        // Get the epoch timestamp.
        let epoch_timestamp_ms = ctx.epoch_timestamp_ms();

        // Check that `expiration_timestamp_ms` is valid.
        assert!(expiration_timestamp_ms > epoch_timestamp_ms, EExpireEpochIsPast);
    }
}
