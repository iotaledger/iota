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

// === Constants ===

// === Structs ===

public struct SpendLimit has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

public fun authenticate_with_amount(account_id: &UID, amount: u64) {
    assert!(has(account_id), ESpendingLimitMissing);

    let spending_limit: &u64 = borrow(account_id);
    assert!(amount <= *spending_limit, EOverspend);
}

public fun attach(account_id: &mut UID, amount: u64) {
    assert!(!has(account_id), ESpendingLimitAlreadyAttached);
    dynamic_field::add(account_id, SpendLimit {}, amount)
}

// === View Functions ===

public fun has(account_id: &UID): bool {
    dynamic_field::exists_(account_id, SpendLimit {})
}

public fun borrow(account_id: &UID): &u64 {
    dynamic_field::borrow(account_id, SpendLimit {})
}

public fun borrow_mut(account_id: &mut UID): &mut u64 {
    dynamic_field::borrow_mut(account_id, SpendLimit {})
}
// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
