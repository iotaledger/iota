// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::timelocked_staking;

use iota::balance::{Self, Balance};
use iota::iota::IOTA;
use iota::timelock::{Self, TimeLock};
use iota_system::iota_system::IotaSystemState;
use iota_system::staking_pool::StakedIota;
use iota_system::validator::ValidatorV1;
use std::string::String;

/// For when trying to stake an expired time-locked balance.
const ETimeLockShouldNotBeExpired: u64 = 0;
/// Incompatible objects when joining TimelockedStakedIota
const EIncompatibleTimelockedStakedIota: u64 = 1;

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

// === Public entry staking methods ===

/// Add a time-locked stake to a validator's staking pool.
public entry fun request_add_stake(
    iota_system: &mut IotaSystemState,
    timelocked_balance: TimeLock<Balance<IOTA>>,
    validator_address: address,
    ctx: &mut TxContext,
) {
    // Stake the time-locked balance.
    let timelocked_staked_iota = request_add_stake_non_entry(
        iota_system,
        timelocked_balance,
        validator_address,
        ctx,
    );

    // Transfer the receipt to the sender.
    timelocked_staked_iota.transfer_to_sender(ctx);
}

/// Add a time-locked stake to a validator's staking pool using multiple time-locked balances.
public entry fun request_add_stake_mul_bal(
    iota_system: &mut IotaSystemState,
    timelocked_balances: vector<TimeLock<Balance<IOTA>>>,
    validator_address: address,
    ctx: &mut TxContext,
) {
    // Stake the time-locked balances.
    let mut receipts = request_add_stake_mul_bal_non_entry(
        iota_system,
        timelocked_balances,
        validator_address,
        ctx,
    );

    // Create useful variables.
    let (mut i, len) = (0, receipts.length());

    // Send all the receipts to the sender.
    while (i < len) {
        // Take a receipt.
        let receipt = receipts.pop_back();

        // Transfer the receipt to the sender.
        receipt.transfer_to_sender(ctx);

        i = i + 1
    };

    // Destroy the empty vector.
    vector::destroy_empty(receipts)
}

/// Withdraw a time-locked stake from a validator's staking pool.
public entry fun request_withdraw_stake(
    iota_system: &mut IotaSystemState,
    timelocked_staked_iota: TimelockedStakedIota,
    ctx: &mut TxContext,
) {
    // Withdraw the time-locked balance.
    let (timelocked_balance, reward) = request_withdraw_stake_non_entry(
        iota_system,
        timelocked_staked_iota,
        ctx,
    );

    // Transfer the withdrawn time-locked balance to the sender.
    timelocked_balance.transfer_to_sender(ctx);

    // Send coins only if the reward is not zero.
    if (reward.value() > 0) {
        transfer::public_transfer(reward.into_coin(ctx), ctx.sender());
    } else {
        balance::destroy_zero(reward);
    }
}

// === Public non-entry staking methods ===

/// The non-entry version of `request_add_stake`, which returns the time-locked staked IOTA instead of transferring it to the sender.
public fun request_add_stake_non_entry(
    iota_system: &mut IotaSystemState,
    timelocked_balance: TimeLock<Balance<IOTA>>,
    validator_address: address,
    ctx: &mut TxContext,
): TimelockedStakedIota {
    // Check the preconditions.
    assert!(timelocked_balance.is_locked(ctx), ETimeLockShouldNotBeExpired);

    // Unpack the time-locked balance.
    let sys_admin_cap = iota_system.load_iota_system_admin_cap();
    let (balance, expiration_timestamp_ms, label) = timelock::system_unpack(
        sys_admin_cap,
        timelocked_balance,
    );

    // Stake the time-locked balance.
    let staked_iota = iota_system.request_add_stake_non_entry(
        balance.into_coin(ctx),
        validator_address,
        ctx,
    );

    // Create and return a receipt.
    TimelockedStakedIota {
        id: object::new(ctx),
        staked_iota,
        expiration_timestamp_ms,
        label,
    }
}

/// The non-entry version of `request_add_stake_mul_bal`,
/// which returns a list of the time-locked staked IOTAs instead of transferring them to the sender.
public fun request_add_stake_mul_bal_non_entry(
    iota_system: &mut IotaSystemState,
    mut timelocked_balances: vector<TimeLock<Balance<IOTA>>>,
    validator_address: address,
    ctx: &mut TxContext,
): vector<TimelockedStakedIota> {
    // Create a vector to store the results.
    let mut result = vector[];

    // Create useful variables.
    let (mut i, len) = (0, timelocked_balances.length());

    // Stake all the time-locked balances.
    while (i < len) {
        // Take a time-locked balance.
        let timelocked_balance = timelocked_balances.pop_back();

        // Stake the time-locked balance.
        let timelocked_staked_iota = request_add_stake_non_entry(
            iota_system,
            timelocked_balance,
            validator_address,
            ctx,
        );

        // Store the created receipt.
        result.push_back(timelocked_staked_iota);

        i = i + 1
    };

    // Destroy the empty vector.
    vector::destroy_empty(timelocked_balances);

    result
}

/// Non-entry version of `request_withdraw_stake` that returns the withdrawn time-locked IOTA and reward
/// instead of transferring it to the sender.
public fun request_withdraw_stake_non_entry(
    iota_system: &mut IotaSystemState,
    timelocked_staked_iota: TimelockedStakedIota,
    ctx: &mut TxContext,
): (TimeLock<Balance<IOTA>>, Balance<IOTA>) {
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
    let sys_admin_cap = iota_system.load_iota_system_admin_cap();
    (
        timelock::system_pack(sys_admin_cap, principal, expiration_timestamp_ms, label, ctx),
        withdraw_stake,
    )
}

// === TimelockedStakedIota balance functions ===

/// Split `TimelockedStakedIota` into two parts, one with principal `split_amount`,
/// and the remaining principal is left in `self`.
/// All the other parameters of the `TimelockedStakedIota` like `stake_activation_epoch` or `pool_id` remain the same.
public fun split(
    self: &mut TimelockedStakedIota,
    split_amount: u64,
    ctx: &mut TxContext,
): TimelockedStakedIota {
    let split_stake = self.staked_iota.split(split_amount, ctx);

    TimelockedStakedIota {
        id: object::new(ctx),
        staked_iota: split_stake,
        expiration_timestamp_ms: self.expiration_timestamp_ms,
        label: self.label,
    }
}

/// Split the given `TimelockedStakedIota` to the two parts, one with principal `split_amount`,
/// transfer the newly split part to the sender address.
public entry fun split_staked_iota(
    stake: &mut TimelockedStakedIota,
    split_amount: u64,
    ctx: &mut TxContext,
) {
    split(stake, split_amount, ctx).transfer_to_sender(ctx);
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

// === TimelockedStakedIota public utilities ===

/// A utility function to transfer a `TimelockedStakedIota`.
public fun transfer_to_sender(stake: TimelockedStakedIota, ctx: &TxContext) {
    transfer(stake, ctx.sender())
}

/// A utility function to transfer multiple `TimelockedStakedIota`.
public fun transfer_to_sender_multiple(stakes: vector<TimelockedStakedIota>, ctx: &TxContext) {
    transfer_multiple(stakes, ctx.sender())
}

/// A utility function that returns true if all the staking parameters
/// of the staked iota except the principal are identical
public fun is_equal_staking_metadata(
    self: &TimelockedStakedIota,
    other: &TimelockedStakedIota,
): bool {
    self.staked_iota.is_equal_staking_metadata(&other.staked_iota) &&
        (self.expiration_timestamp_ms == other.expiration_timestamp_ms) &&
        (self.label() == other.label())
}

// === TimelockedStakedIota getters ===

/// Function to get the pool id of a `TimelockedStakedIota`.
public fun pool_id(self: &TimelockedStakedIota): ID { self.staked_iota.pool_id() }

/// Function to get the staked iota amount of a `TimelockedStakedIota`.
public fun staked_iota_amount(self: &TimelockedStakedIota): u64 {
    self.staked_iota.staked_iota_amount()
}

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
        self.label.borrow() == timelock::type_name<L>()
    } else {
        false
    }
}

// === Internal ===

/// A utility function to destroy a `TimelockedStakedIota`.
fun unpack(self: TimelockedStakedIota): (StakedIota, u64, Option<String>) {
    let TimelockedStakedIota {
        id,
        staked_iota,
        expiration_timestamp_ms,
        label,
    } = self;

    object::delete(id);

    (staked_iota, expiration_timestamp_ms, label)
}

/// A utility function to transfer a `TimelockedStakedIota` to a receiver.
fun transfer(stake: TimelockedStakedIota, receiver: address) {
    transfer::transfer(stake, receiver);
}

/// A utility function to transfer a vector of `TimelockedStakedIota` to a receiver.
fun transfer_multiple(mut stakes: vector<TimelockedStakedIota>, receiver: address) {
    // Transfer all the time-locked stakes to the recipient.
    while (!stakes.is_empty()) {
        let stake = stakes.pop_back();
        transfer::transfer(stake, receiver);
    };

    // Destroy the empty vector.
    vector::destroy_empty(stakes);
}

// == Genesis ==

/// Request to add timelocked stake to the validator's staking pool at genesis
public(package) fun request_add_stake_at_genesis(
    validator: &mut ValidatorV1,
    stake: Balance<IOTA>,
    staker_address: address,
    expiration_timestamp_ms: u64,
    label: Option<String>,
    ctx: &mut TxContext,
) {
    let staked_iota = validator.request_add_stake_at_genesis_with_receipt(stake, ctx);
    let timelocked_staked_iota = TimelockedStakedIota {
        id: object::new(ctx),
        staked_iota,
        expiration_timestamp_ms,
        label,
    };
    transfer(timelocked_staked_iota, staker_address);
}
