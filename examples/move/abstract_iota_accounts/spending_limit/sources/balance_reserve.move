// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::balance_reserve;

use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::dynamic_field as df;

// === Errors ===
#[error(code = 0)]
const EBalanceReserveAlreadyAttached: vector<u8> = b"Balance reserve already attached.";
#[error(code = 1)]
const EBalanceReserveMissing: vector<u8> = b"Balance reserve is missing.";
#[error(code = 2)]
const EInsufficientBalanceReserve: vector<u8> = b"Insufficient balance reserve.";

// === Constants ===

// === Structs ===

/// Struct for the balance reserve to keep in the account.
public struct BalanceReserve<phantom T> has store {
    balance: Balance<T>,
}

/// Marker for the gas reserve balance (outside balance reserve).
public struct BalanceReserveFieldName has copy, drop, store {}

// === Public Functions ===

public fun new_empty_balance_reserve<T>(): BalanceReserve<T> {
    BalanceReserve<T> {
        balance: balance::zero<T>(),
    }
}

/// Withdraws the specified amount from the balance reserve and returns it as a Coin<T>.
public fun withdraw_from_balance_reserve<T>(
    self: &mut BalanceReserve<T>,
    amount: u64,
    ctx: &mut TxContext,
): Coin<T> {
    assert!(balance::value(&self.balance) >= amount, EInsufficientBalanceReserve);

    coin::from_balance(self.balance.split(amount), ctx)
}

/// Deposit coins into the balance reserve.
public fun deposit_to_balance_reserve<T>(self: &mut BalanceReserve<T>, balance: Balance<T>) {
    self.balance.join(balance);
}

/// Attaches a balance reserve to the given account ID.
public fun attach_balance_reserve<T>(account_id: &mut UID, reserve: BalanceReserve<T>) {
    assert!(!has_balance_reserve(account_id), EBalanceReserveAlreadyAttached);

    df::add(account_id, BalanceReserveFieldName {}, reserve)
}

/// Detaches the balance reserve from the given account ID and returns the previous reserve.
public fun detach_balance_reserve<T>(account_id: &mut UID): BalanceReserve<T> {
    assert!(has_balance_reserve(account_id), EBalanceReserveMissing);

    df::remove(account_id, BalanceReserveFieldName {})
}

/// Rotates the balance reserve to a new amount, returning the previous reserve.
public fun rotate_balance_reserve<T>(
    account_id: &mut UID,
    reserve: BalanceReserve<T>,
): BalanceReserve<T> {
    assert!(has_balance_reserve(account_id), EBalanceReserveMissing);

    let prev_reserve = df::remove(account_id, BalanceReserveFieldName {});
    df::add(
        account_id,
        BalanceReserveFieldName {},
        reserve,
    );
    prev_reserve
}

// === View Functions ===

/// An utility function to check if the account has a balance reserve set.
public fun has_balance_reserve(account_id: &UID): bool {
    df::exists_(account_id, BalanceReserveFieldName {})
}

/// An utility function to borrow the balance reserve value for the given account ID.
public fun borrow_balance_reserve<T>(account_id: &UID): &BalanceReserve<T> {
    df::borrow(account_id, BalanceReserveFieldName {})
}

// === Admin Functions ===

// === Package Functions ===

/// Returns a mutable reference to the balance reserve for the given account ID.
public(package) fun borrow_mut_balance_reserve<T>(account_id: &mut UID): &mut BalanceReserve<T> {
    df::borrow_mut(account_id, BalanceReserveFieldName {})
}

/// An utility function to construct the dynamic field name for the balance reserve field.
public(package) fun balance_reserve_field_name(): BalanceReserveFieldName {
    BalanceReserveFieldName {}
}

// === Private Functions ===

// === Test Functions ===
