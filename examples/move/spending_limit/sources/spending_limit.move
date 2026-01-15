// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::spending_limit;

use iota::dynamic_field;

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

/// A dynamic field key for the spending limit.
public struct SpendingLimit has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Checks that the given amount is within the spending limit.
public fun check_amount_against_spending_limit(account_id: &UID, amount: u64) {
    assert!(exists(account_id), ESpendingLimitMissing);

    let spending_limit = borrow(account_id);
    assert!(amount <= *spending_limit, EOverspend);
}

// Attaches a spending limit to the given account ID.
public fun attach(account_id: &mut UID, amount: u64) {
    assert!(!exists(account_id), ESpendingLimitAlreadyAttached);
    assert!(amount > 0, EInvalidLimit);
    dynamic_field::add(account_id, SpendingLimit {}, amount)
}

// Detaches the spending limit from the given account ID and returns the previous limit.
public fun detach(account_id: &mut UID): u64 {
    assert!(exists(account_id), ESpendingLimitMissing);
    dynamic_field::remove(account_id, SpendingLimit {})
}

// Rotates the spending limit to a new amount, returning the previous limit.
public fun rotate(account_id: &mut UID, amount: u64): u64 {
    assert!(exists(account_id), ESpendingLimitMissing);
    assert!(amount > 0, EInvalidLimit);
    let prev_limit = dynamic_field::remove(account_id, SpendingLimit {});
    dynamic_field::add(account_id, SpendingLimit {}, amount);
    prev_limit
}

// Returns a mutable reference to the spending limit for the given account ID.
public(package) fun borrow_mut(account_id: &mut UID): &mut u64 {
    dynamic_field::borrow_mut(account_id, SpendingLimit {})
}

// === View Functions ===

public fun exists(account_id: &UID): bool {
    dynamic_field::exists_(account_id, SpendingLimit {})
}

public fun borrow(account_id: &UID): &u64 {
    dynamic_field::borrow(account_id, SpendingLimit {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
