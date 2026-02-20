// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::spending_limit_authentication;

use iota::dynamic_field as df;

// === Errors ===

#[error(code = 0)]
const EOverspend: vector<u8> = b"Spending limit exceeded.";

#[error(code = 1)]
const ESpendingLimitAlreadyAttached: vector<u8> = b"Spending limit already attached.";

#[error(code = 2)]
const ESpendingLimitMissing: vector<u8> = b"Spending limit is missing.";

#[error(code = 3)]
const EInvalidLimit: vector<u8> = b"Invalid spending limit.";

// === Constants ===

// === Structs ===

/// A dynamic field name for the spending limit.
public struct SpendingLimitFieldName has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Spending Limit ===

/// Attaches a spending limit to the given account ID.
public fun attach_spending_limit(account_id: &mut UID, amount: u64) {
    assert!(!has_spending_limit(account_id), ESpendingLimitAlreadyAttached);
    assert!(amount > 0, EInvalidLimit);
    df::add(account_id, SpendingLimitFieldName {}, amount)
}

/// Detaches the spending limit from the given account ID and returns the previous limit.
public fun detach_spending_limit(account_id: &mut UID): u64 {
    assert!(has_spending_limit(account_id), ESpendingLimitMissing);
    df::remove(account_id, SpendingLimitFieldName {})
}

/// Rotates the spending limit to a new amount, returning the previous limit.
public fun rotate_spending_limit(account_id: &mut UID, amount: u64): u64 {
    assert!(has_spending_limit(account_id), ESpendingLimitMissing);
    assert!(amount > 0, EInvalidLimit);
    let prev_limit = df::remove(account_id, SpendingLimitFieldName {});
    df::add(account_id, SpendingLimitFieldName {}, amount);
    prev_limit
}

// === Public Authenticators Helpers ===

/// Checks that the given amount is within the spending limit.
public fun authenticate_spending_limit(account_id: &UID, amount: u64) {
    assert!(has_spending_limit(account_id), ESpendingLimitMissing);

    let spending_limit = borrow_spending_limit(account_id);
    assert!(amount <= *spending_limit, EOverspend);
}

// === View Functions ===

/// An utility function to check if the account has a spending limit set.
public fun has_spending_limit(account_id: &UID): bool {
    df::exists_(account_id, SpendingLimitFieldName {})
}

/// An utility function to borrow the spending limit value for the given account ID.
public fun borrow_spending_limit(account_id: &UID): &u64 {
    df::borrow(account_id, SpendingLimitFieldName {})
}

// === Admin Functions ===

// === Package Functions ===

/// Returns a mutable reference to the spending limit for the given account ID.
public(package) fun borrow_mut_spending_limit(account_id: &mut UID): &mut u64 {
    df::borrow_mut(account_id, SpendingLimitFieldName {})
}

// An utility function to construct the dynamic field name for the spending limit field.
public(package) fun spending_limit_field_name(): SpendingLimitFieldName {
    SpendingLimitFieldName {}
}

// === Private Functions ===

// === Test Functions ===
