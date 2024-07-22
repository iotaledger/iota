// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A timelock implementation.
module iota::timelock {

    use std::string::{Self, String};

    use iota::balance::Balance;
    use iota::labeler::LabelerCap;

    /// The `new` function was called at a non-genesis epoch.
    const ENotCalledAtGenesis: u64 = 0;
    /// Sender is not @0x0 the system address.
    const ENotSystemAddress: u64 = 1;
    /// Expiration timestamp of the lock is in the past.
    const EExpireEpochIsPast: u64 = 2;
    /// The lock has not expired yet.
    const ENotExpiredYet: u64 = 3;
    /// For when trying to join two time-locked balances with different expiration time.
    const EDifferentExpirationTime: u64 = 4;
    /// For when trying to join two time-locked balances with different labels.
    const EDifferentLabels: u64 = 5;

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

    /// `SystemTimelockCap` allows to `pack` and `unpack` TimeLocks
    public struct SystemTimelockCap has store {}

    // === TimeLock lock and unlock ===

    /// Function to lock an object till a unix timestamp in milliseconds.
    public fun lock<T: store>(locked: T, expiration_timestamp_ms: u64, ctx: &mut TxContext): TimeLock<T> {
        // Check that `expiration_timestamp_ms` is valid.
        check_expiration_timestamp_ms(expiration_timestamp_ms, ctx);

        // Create a timelock.
        pack(locked, expiration_timestamp_ms, option::none(), ctx)
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
        let label = type_name<L>();

        // Create a labeled timelock.
        pack(locked, expiration_timestamp_ms, option::some(label), ctx)
    }

    /// Function to lock an object `obj` until `expiration_timestamp_ms` and transfer it to address `to`.
    /// Since `Timelock<T>` does not support public transfer, use this function to lock an object to an address.
    public fun lock_and_transfer<T: store>(
        obj: T,
        to: address,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ) {
        transfer(lock(obj, expiration_timestamp_ms, ctx), to);
    }

    /// Function to lock a labeled object `obj` until `expiration_timestamp_ms` and transfer it to address `to`.
    /// Since `Timelock<T>` does not support public transfer, use this function to lock a labeled object to an address.
    public fun lock_with_label_and_transfer<T: store, L>(
        labeler: &LabelerCap<L>,
        obj: T,
        to: address,
        expiration_timestamp_ms: u64,
        ctx: &mut TxContext
    ) {
        transfer(lock_with_label(labeler, obj, expiration_timestamp_ms, ctx), to);
    }

    /// Function to unlock the object from a `TimeLock`.
    public fun unlock<T: store>(self: TimeLock<T>, ctx: &TxContext): T {
        // Unpack the timelock. 
        let (locked, expiration_timestamp_ms, _) = unpack(self);

        // Check if the lock has expired.
        assert!(expiration_timestamp_ms <= ctx.epoch_timestamp_ms(), ENotExpiredYet);

        locked
    }

    // === TimeLock balance functions ===

    /// Join two `TimeLock<Balance<T>>` together.
    public fun join<T>(self: &mut TimeLock<Balance<T>>, other: TimeLock<Balance<T>>) {
        // Check the preconditions.
        assert!(self.expiration_timestamp_ms() == other.expiration_timestamp_ms(), EDifferentExpirationTime);
        assert!(self.label() == other.label(), EDifferentLabels);

        // Unpack the time-locked balance.
        let (value, _, _) = unpack(other);

        // Join the balances.
        self.locked.join(value);
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
        let value = self.locked.split(value);

        // Pack the split balance into a timelock.
        pack(value, self.expiration_timestamp_ms(), self.label(), ctx)
    }

    /// Split the given `TimeLock<Balance<T>>` into two parts, one with principal `value`,
    /// and transfer the newly split part to the sender address.
    public entry fun split_balance<T>(self: &mut TimeLock<Balance<T>>, value: u64, ctx: &mut TxContext) {
        split(self, value, ctx).transfer_to_sender(ctx)
    }

    // === TimeLock public utilities ===

    /// A utility function to transfer a `TimeLock` to the sender.
    public fun transfer_to_sender<T: store>(lock: TimeLock<T>, ctx: &TxContext) {
        transfer(lock, ctx.sender())
    }

    /// A utility function to pack a `TimeLock` that can be invoked only by a system package.
    public fun system_pack<T: store>(
        _: &SystemTimelockCap,
        locked: T,
        expiration_timestamp_ms: u64,
        label: Option<String>,
        ctx: &mut TxContext): TimeLock<T>
    {
        pack(locked, expiration_timestamp_ms, label, ctx)
    }

    /// An utility function to unpack a `TimeLock` that can be invoked only by a system package.
    public fun system_unpack<T: store>(_: &SystemTimelockCap, lock: TimeLock<T>): (T, u64, Option<String>) {
        unpack(lock)
    }

    /// Return a fully qualified type name with the original package IDs
    /// that is used as type related a label value.
    public fun type_name<L>(): String {
        string::from_ascii(std::type_name::get_with_original_ids<L>().into_string())
    }

    // === TimeLock getters ===

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

    /// Function to get the label of a `TimeLock`.
    public fun label<T: store>(self: &TimeLock<T>): Option<String> {
        self.label
    }

    /// Check if a `TimeLock` is labeled with the type `L`.
    public fun is_labeled_with<T: store, L>(self: &TimeLock<T>): bool {
        if (self.label.is_some()) {
            self.label.borrow() == type_name<L>()
        }
        else {
            false
        }
    }

    // === Internal ===

    /// A utility function to pack a `TimeLock`.
    fun pack<T: store>(
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
    fun unpack<T: store>(lock: TimeLock<T>): (T, u64, Option<String>) {
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

    /// A utility function to transfer a `TimeLock` to a receiver.
    fun transfer<T: store>(lock: TimeLock<T>, receiver: address) {
        transfer::transfer(lock, receiver);
    }

    /// An utility function to check that the `expiration_timestamp_ms` value is valid.
    fun check_expiration_timestamp_ms(expiration_timestamp_ms: u64, ctx: &TxContext) {
        // Get the epoch timestamp.
        let epoch_timestamp_ms = ctx.epoch_timestamp_ms();

        // Check that `expiration_timestamp_ms` is valid.
        assert!(expiration_timestamp_ms > epoch_timestamp_ms, EExpireEpochIsPast);
    }
        
    // === SystemTimelockCap ===

    #[allow(unused_function)]
    /// Create a `SystemTimelockCap`.
    /// This should be called only once during genesis creation.
    fun new_system_timelock_cap(ctx: &TxContext): SystemTimelockCap {
        assert!(ctx.sender() == @0x0, ENotSystemAddress);
        assert!(ctx.epoch() == 0, ENotCalledAtGenesis);

        SystemTimelockCap {}
    }

    #[test_only]
    /// Create a `SystemTimelockCap` for testing purposes.
    public fun new_system_timelock_cap_for_testing(): SystemTimelockCap {
        SystemTimelockCap { }
    }
}
