// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_const)]
module iota_system::staking_pool;

use iota::bag::{Self, Bag};
use iota::balance::{Self, Balance};
use iota::iota::IOTA;
use iota::table::{Self, Table};

/// StakedIota objects cannot be split to below this amount.
const MIN_STAKING_THRESHOLD: u64 = 1_000_000_000; // 1 IOTA

const EInsufficientPoolTokenBalance: u64 = 0;
const EWrongPool: u64 = 1;
const EWithdrawAmountCannotBeZero: u64 = 2;
const EInsufficientIotaTokenBalance: u64 = 3;
const EInsufficientRewardsPoolBalance: u64 = 4;
const EDestroyNonzeroBalance: u64 = 5;
const ETokenTimeLockIsSome: u64 = 6;
const EWrongDelegation: u64 = 7;
const EPendingDelegationDoesNotExist: u64 = 8;
const ETokenBalancesDoNotMatchExchangeRate: u64 = 9;
const EDelegationToInactivePool: u64 = 10;
const EDeactivationOfInactivePool: u64 = 11;
const EIncompatibleStakedIota: u64 = 12;
const EWithdrawalInSameEpoch: u64 = 13;
const EPoolAlreadyActive: u64 = 14;
const EPoolNotPreactive: u64 = 15;
const EActivationOfInactivePool: u64 = 16;
const EDelegationOfZeroIota: u64 = 17;
const EStakedIotaBelowThreshold: u64 = 18;

/// A staking pool embedded in each validator struct in the system state object.
public struct StakingPoolV1 has key, store {
    id: UID,
    /// The epoch at which this pool became active.
    /// The value is `None` if the pool is pre-active and `Some(<epoch_number>)` if active or inactive.
    activation_epoch: Option<u64>,
    /// The epoch at which this staking pool ceased to be active. `None` = \{pre-active, active},
    /// `Some(<epoch_number>)` if in-active, and it was de-activated at epoch `<epoch_number>`.
    deactivation_epoch: Option<u64>,
    /// The total number of IOTA tokens in this pool, including the IOTA in the rewards_pool, as well as in all the principal
    /// in the `StakedIota` object, updated at epoch boundaries.
    iota_balance: u64,
    /// The epoch stake rewards will be added here at the end of each epoch.
    rewards_pool: Balance<IOTA>,
    /// Total number of pool tokens issued by the pool.
    pool_token_balance: u64,
    /// Exchange rate history of previous epochs. Key is the epoch number.
    /// The entries start from the `activation_epoch` of this pool and contains exchange rates at the beginning of each epoch,
    /// i.e., right after the rewards for the previous epoch have been deposited into the pool.
    exchange_rates: Table<u64, PoolTokenExchangeRate>,
    /// Pending stake amount for this epoch, emptied at epoch boundaries.
    pending_stake: u64,
    /// Pending stake withdrawn during the current epoch, emptied at epoch boundaries.
    /// This includes both the principal and rewards IOTA withdrawn.
    pending_total_iota_withdraw: u64,
    /// Pending pool token withdrawn during the current epoch, emptied at epoch boundaries.
    pending_pool_token_withdraw: u64,
    /// Any extra fields that's not defined statically.
    extra_fields: Bag,
}

/// Struct representing the exchange rate of the stake pool token to IOTA.
public struct PoolTokenExchangeRate has copy, drop, store {
    iota_amount: u64,
    pool_token_amount: u64,
}

/// A self-custodial object holding the staked IOTA tokens.
public struct StakedIota has key, store {
    id: UID,
    /// ID of the staking pool we are staking with.
    pool_id: ID,
    /// The epoch at which the stake becomes active.
    stake_activation_epoch: u64,
    /// The staked IOTA tokens.
    principal: Balance<IOTA>,
}

// ==== initializer ====

/// Create a new, empty staking pool.
public(package) fun new(ctx: &mut TxContext): StakingPoolV1 {
    let exchange_rates = table::new(ctx);
    StakingPoolV1 {
        id: object::new(ctx),
        activation_epoch: option::none(),
        deactivation_epoch: option::none(),
        iota_balance: 0,
        rewards_pool: balance::zero(),
        pool_token_balance: 0,
        exchange_rates,
        pending_stake: 0,
        pending_total_iota_withdraw: 0,
        pending_pool_token_withdraw: 0,
        extra_fields: bag::new(ctx),
    }
}

// ==== stake requests ====

/// Request to stake to a staking pool. The stake starts counting at the beginning of the next epoch,
public(package) fun request_add_stake(
    pool: &mut StakingPoolV1,
    stake: Balance<IOTA>,
    stake_activation_epoch: u64,
    ctx: &mut TxContext,
): StakedIota {
    let iota_amount = stake.value();
    assert!(!is_inactive(pool), EDelegationToInactivePool);
    assert!(iota_amount > 0, EDelegationOfZeroIota);
    let staked_iota = StakedIota {
        id: object::new(ctx),
        pool_id: object::id(pool),
        stake_activation_epoch,
        principal: stake,
    };
    pool.pending_stake = pool.pending_stake + iota_amount;
    staked_iota
}

/// Request to withdraw the given stake plus rewards from a staking pool.
/// Both the principal and corresponding rewards in IOTA are withdrawn.
/// A proportional amount of pool token withdraw is recorded and processed at epoch change time.
public(package) fun request_withdraw_stake(
    pool: &mut StakingPoolV1,
    staked_iota: StakedIota,
    ctx: &TxContext,
): Balance<IOTA> {
    // stake is inactive
    if (staked_iota.stake_activation_epoch > ctx.epoch()) {
        let principal = unwrap_staked_iota(staked_iota);

        // Stake for preactive validators is always processed immediately and not added to the pending stake.
        if (!pool.is_preactive()) {
            pool.pending_stake = pool.pending_stake - principal.value();
        };

        return principal
    };

    let (pool_token_withdraw_amount, mut principal_withdraw) = withdraw_from_principal(
        pool,
        staked_iota,
    );
    let principal_withdraw_amount = principal_withdraw.value();

    let rewards_withdraw = withdraw_rewards(
        pool,
        principal_withdraw_amount,
        pool_token_withdraw_amount,
        ctx.epoch(),
    );
    let total_iota_withdraw_amount = principal_withdraw_amount + rewards_withdraw.value();

    pool.pending_total_iota_withdraw =
        pool.pending_total_iota_withdraw + total_iota_withdraw_amount;
    pool.pending_pool_token_withdraw =
        pool.pending_pool_token_withdraw + pool_token_withdraw_amount;

    // If the pool is inactive, we immediately process the withdrawal.
    if (is_inactive(pool)) process_pending_stake_withdraw(pool);

    // TODO: implement withdraw bonding period here.
    principal_withdraw.join(rewards_withdraw);
    principal_withdraw
}

/// Withdraw the principal IOTA stored in the StakedIota object, and calculate the corresponding amount of pool
/// tokens using exchange rate at staking epoch.
/// Returns values are amount of pool tokens withdrawn and withdrawn principal portion of IOTA.
public(package) fun withdraw_from_principal(
    pool: &StakingPoolV1,
    staked_iota: StakedIota,
): (u64, Balance<IOTA>) {
    // Check that the stake information matches the pool.
    assert!(staked_iota.pool_id == object::id(pool), EWrongPool);

    let exchange_rate_at_staking_epoch = pool_token_exchange_rate_at_epoch(
        pool,
        staked_iota.stake_activation_epoch,
    );
    let principal_withdraw = unwrap_staked_iota(staked_iota);
    let pool_token_withdraw_amount = get_token_amount(
        &exchange_rate_at_staking_epoch,
        principal_withdraw.value(),
    );

    (pool_token_withdraw_amount, principal_withdraw)
}

fun unwrap_staked_iota(staked_iota: StakedIota): Balance<IOTA> {
    let StakedIota {
        id,
        pool_id: _,
        stake_activation_epoch: _,
        principal,
    } = staked_iota;
    object::delete(id);
    principal
}

/// Allows calling `.into_balance()` on `StakedIota` to invoke `unwrap_staked_iota`
public use fun unwrap_staked_iota as StakedIota.into_balance;

// ==== functions called at epoch boundaries ===

/// Called at epoch advancement times to add rewards (in IOTA) to the staking pool.
public(package) fun deposit_rewards(pool: &mut StakingPoolV1, rewards: Balance<IOTA>) {
    pool.iota_balance = pool.iota_balance + rewards.value();
    pool.rewards_pool.join(rewards);
}

public(package) fun process_pending_stakes_and_withdraws(
    pool: &mut StakingPoolV1,
    ctx: &TxContext,
) {
    let new_epoch = ctx.epoch() + 1;
    process_pending_stake_withdraw(pool);
    process_pending_stake(pool);
    pool
        .exchange_rates
        .add(
            new_epoch,
            PoolTokenExchangeRate {
                iota_amount: pool.iota_balance,
                pool_token_amount: pool.pool_token_balance,
            },
        );
    check_balance_invariants(pool, new_epoch);
}

/// Called at epoch boundaries to process pending stake withdraws requested during the epoch.
/// Also called immediately upon withdrawal if the pool is inactive.
fun process_pending_stake_withdraw(pool: &mut StakingPoolV1) {
    pool.iota_balance = pool.iota_balance - pool.pending_total_iota_withdraw;
    pool.pool_token_balance = pool.pool_token_balance - pool.pending_pool_token_withdraw;
    pool.pending_total_iota_withdraw = 0;
    pool.pending_pool_token_withdraw = 0;
}

/// Called at epoch boundaries to process the pending stake.
public(package) fun process_pending_stake(pool: &mut StakingPoolV1) {
    // Use the most up to date exchange rate with the rewards deposited and withdraws effectuated.
    let latest_exchange_rate = PoolTokenExchangeRate {
        iota_amount: pool.iota_balance,
        pool_token_amount: pool.pool_token_balance,
    };
    pool.iota_balance = pool.iota_balance + pool.pending_stake;
    pool.pool_token_balance = get_token_amount(&latest_exchange_rate, pool.iota_balance);
    pool.pending_stake = 0;
}

/// This function does the following:
///     1. Calculates the total amount of IOTA (including principal and rewards) that the provided pool tokens represent
///        at the current exchange rate.
///     2. Using the above number and the given `principal_withdraw_amount`, calculates the rewards portion of the
///        stake we should withdraw.
///     3. Withdraws the rewards portion from the rewards pool at the current exchange rate. We only withdraw the rewards
///        portion because the principal portion was already taken out of the staker's self custodied StakedIota.
fun withdraw_rewards(
    pool: &mut StakingPoolV1,
    principal_withdraw_amount: u64,
    pool_token_withdraw_amount: u64,
    epoch: u64,
): Balance<IOTA> {
    let exchange_rate = pool_token_exchange_rate_at_epoch(pool, epoch);
    let total_iota_withdraw_amount = get_iota_amount(&exchange_rate, pool_token_withdraw_amount);
    let mut reward_withdraw_amount = if (total_iota_withdraw_amount >= principal_withdraw_amount)
        total_iota_withdraw_amount - principal_withdraw_amount else 0;
    // This may happen when we are withdrawing everything from the pool and
    // the rewards pool balance may be less than reward_withdraw_amount.
    // TODO: FIGURE OUT EXACTLY WHY THIS CAN HAPPEN.
    reward_withdraw_amount = reward_withdraw_amount.min(pool.rewards_pool.value());
    pool.rewards_pool.split(reward_withdraw_amount)
}

// ==== preactive pool related ====

/// Called by `validator` module to activate a staking pool.
public(package) fun activate_staking_pool(pool: &mut StakingPoolV1, activation_epoch: u64) {
    // Add the initial exchange rate to the table.
    pool
        .exchange_rates
        .add(
            activation_epoch,
            initial_exchange_rate(),
        );
    // Check that the pool is preactive and not inactive.
    assert!(is_preactive(pool), EPoolAlreadyActive);
    assert!(!is_inactive(pool), EActivationOfInactivePool);
    // Fill in the active epoch.
    pool.activation_epoch.fill(activation_epoch);
}

// ==== inactive pool related ====

/// Deactivate a staking pool by setting the `deactivation_epoch`. After
/// this pool deactivation, the pool stops earning rewards. Only stake
/// withdraws can be made to the pool.
public(package) fun deactivate_staking_pool(pool: &mut StakingPoolV1, deactivation_epoch: u64) {
    // We can't deactivate an already deactivated pool.
    assert!(!is_inactive(pool), EDeactivationOfInactivePool);
    pool.deactivation_epoch = option::some(deactivation_epoch);
}

// ==== getters and misc utility functions ====

public fun iota_balance(pool: &StakingPoolV1): u64 { pool.iota_balance }

public fun pool_id(staked_iota: &StakedIota): ID { staked_iota.pool_id }

public fun staked_iota_amount(staked_iota: &StakedIota): u64 { staked_iota.principal.value() }

/// Allows calling `.amount()` on `StakedIota` to invoke `staked_iota_amount`
public use fun staked_iota_amount as StakedIota.amount;

public fun stake_activation_epoch(staked_iota: &StakedIota): u64 {
    staked_iota.stake_activation_epoch
}

/// Returns true if the input staking pool is preactive.
public fun is_preactive(pool: &StakingPoolV1): bool {
    pool.activation_epoch.is_none()
}

/// Returns true if the input staking pool is inactive.
public fun is_inactive(pool: &StakingPoolV1): bool {
    pool.deactivation_epoch.is_some()
}

/// Split StakedIota `self` to two parts, one with principal `split_amount`,
/// and the remaining principal is left in `self`.
/// All the other parameters of the StakedIota like `stake_activation_epoch` or `pool_id` remain the same.
public fun split(self: &mut StakedIota, split_amount: u64, ctx: &mut TxContext): StakedIota {
    let original_amount = self.principal.value();
    assert!(split_amount <= original_amount, EInsufficientIotaTokenBalance);
    let remaining_amount = original_amount - split_amount;
    // Both resulting parts should have at least MIN_STAKING_THRESHOLD.
    assert!(remaining_amount >= MIN_STAKING_THRESHOLD, EStakedIotaBelowThreshold);
    assert!(split_amount >= MIN_STAKING_THRESHOLD, EStakedIotaBelowThreshold);
    StakedIota {
        id: object::new(ctx),
        pool_id: self.pool_id,
        stake_activation_epoch: self.stake_activation_epoch,
        principal: self.principal.split(split_amount),
    }
}

/// Split the given StakedIota to the two parts, one with principal `split_amount`,
/// transfer the newly split part to the sender address.
public entry fun split_staked_iota(stake: &mut StakedIota, split_amount: u64, ctx: &mut TxContext) {
    transfer::transfer(split(stake, split_amount, ctx), ctx.sender());
}

/// Allows calling `.split_to_sender()` on `StakedIota` to invoke `split_staked_iota`
public use fun split_staked_iota as StakedIota.split_to_sender;

/// Consume the staked iota `other` and add its value to `self`.
/// Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)
public entry fun join_staked_iota(self: &mut StakedIota, other: StakedIota) {
    assert!(is_equal_staking_metadata(self, &other), EIncompatibleStakedIota);
    let StakedIota {
        id,
        pool_id: _,
        stake_activation_epoch: _,
        principal,
    } = other;

    id.delete();
    self.principal.join(principal);
}

/// Allows calling `.join()` on `StakedIota` to invoke `join_staked_iota`
public use fun join_staked_iota as StakedIota.join;

/// Returns true if all the staking parameters of the staked iota except the principal are identical
public fun is_equal_staking_metadata(self: &StakedIota, other: &StakedIota): bool {
    (self.pool_id == other.pool_id) &&
        (self.stake_activation_epoch == other.stake_activation_epoch)
}

public fun pool_token_exchange_rate_at_epoch(
    pool: &StakingPoolV1,
    epoch: u64,
): PoolTokenExchangeRate {
    // If the pool is preactive then the exchange rate is always 1:1.
    if (is_preactive_at_epoch(pool, epoch)) {
        return initial_exchange_rate()
    };
    let clamped_epoch = pool.deactivation_epoch.get_with_default(epoch);
    let mut epoch = clamped_epoch.min(epoch);
    let activation_epoch = *pool.activation_epoch.borrow();

    // Find the latest epoch that's earlier than the given epoch with an entry in the table
    while (epoch >= activation_epoch) {
        if (pool.exchange_rates.contains(epoch)) {
            return pool.exchange_rates[epoch]
        };
        epoch = epoch - 1;
    };
    // This line really should be unreachable. Do we want an assert false here?
    initial_exchange_rate()
}

/// Returns the total value of the pending staking requests for this staking pool.
public fun pending_stake_amount(staking_pool: &StakingPoolV1): u64 {
    staking_pool.pending_stake
}

/// Returns the total withdrawal from the staking pool this epoch.
public fun pending_stake_withdraw_amount(staking_pool: &StakingPoolV1): u64 {
    staking_pool.pending_total_iota_withdraw
}

public(package) fun exchange_rates(pool: &StakingPoolV1): &Table<u64, PoolTokenExchangeRate> {
    &pool.exchange_rates
}

public fun iota_amount(exchange_rate: &PoolTokenExchangeRate): u64 {
    exchange_rate.iota_amount
}

public fun pool_token_amount(exchange_rate: &PoolTokenExchangeRate): u64 {
    exchange_rate.pool_token_amount
}

/// Returns true if the provided staking pool is preactive at the provided epoch.
fun is_preactive_at_epoch(pool: &StakingPoolV1, epoch: u64): bool {
    // Either the pool is currently preactive or the pool's starting epoch is later than the provided epoch.
    is_preactive(pool) || (*pool.activation_epoch.borrow() > epoch)
}

fun get_iota_amount(exchange_rate: &PoolTokenExchangeRate, token_amount: u64): u64 {
    // When either amount is 0, that means we have no stakes with this pool.
    // The other amount might be non-zero when there's dust left in the pool.
    if (exchange_rate.iota_amount == 0 || exchange_rate.pool_token_amount == 0) {
        return token_amount
    };
    let res =
        exchange_rate.iota_amount as u128
                * (token_amount as u128)
                / (exchange_rate.pool_token_amount as u128);
    res as u64
}

fun get_token_amount(exchange_rate: &PoolTokenExchangeRate, iota_amount: u64): u64 {
    // When either amount is 0, that means we have no stakes with this pool.
    // The other amount might be non-zero when there's dust left in the pool.
    if (exchange_rate.iota_amount == 0 || exchange_rate.pool_token_amount == 0) {
        return iota_amount
    };
    let res =
        exchange_rate.pool_token_amount as u128
                * (iota_amount as u128)
                / (exchange_rate.iota_amount as u128);
    res as u64
}

fun initial_exchange_rate(): PoolTokenExchangeRate {
    PoolTokenExchangeRate { iota_amount: 0, pool_token_amount: 0 }
}

fun check_balance_invariants(pool: &StakingPoolV1, epoch: u64) {
    let exchange_rate = pool_token_exchange_rate_at_epoch(pool, epoch);
    // check that the pool token balance and iota balance ratio matches the exchange rate stored.
    let expected = get_token_amount(&exchange_rate, pool.iota_balance);
    let actual = pool.pool_token_balance;
    assert!(expected == actual, ETokenBalancesDoNotMatchExchangeRate)
}

// ==== test-related functions ====

// Given the `staked_iota` receipt calculate the current rewards (in terms of IOTA) for it.
#[test_only]
public fun calculate_rewards(
    pool: &StakingPoolV1,
    staked_iota: &StakedIota,
    current_epoch: u64,
): u64 {
    let staked_amount = staked_iota_amount(staked_iota);
    let pool_token_withdraw_amount = {
        let exchange_rate_at_staking_epoch = pool_token_exchange_rate_at_epoch(
            pool,
            staked_iota.stake_activation_epoch,
        );
        get_token_amount(&exchange_rate_at_staking_epoch, staked_amount)
    };

    let new_epoch_exchange_rate = pool_token_exchange_rate_at_epoch(pool, current_epoch);
    let total_iota_withdraw_amount = get_iota_amount(
        &new_epoch_exchange_rate,
        pool_token_withdraw_amount,
    );

    let mut reward_withdraw_amount = if (total_iota_withdraw_amount >= staked_amount)
        total_iota_withdraw_amount - staked_amount else 0;
    reward_withdraw_amount = reward_withdraw_amount.min(pool.rewards_pool.value());

    staked_amount + reward_withdraw_amount
}
