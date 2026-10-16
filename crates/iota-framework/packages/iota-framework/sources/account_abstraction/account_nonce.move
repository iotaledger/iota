// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Counters on an account that make approvals for that account single-use.
///
/// The account keeps one counter per check, in a dynamic field named by the check's type. An
/// approval names the account and the counter's next value, and taking the value moves the counter
/// on. The counter is created at 0 on first use and is never removed: if it went away, it would
/// start again at 0 and old approvals would match again.
///
/// Each counter is one sequence, so an account has one pending approval per check at a time.
/// Approvals for different checks don't affect each other.
module iota::account_nonce;

use iota::dynamic_field as df;

// === Errors ===

#[error(code = 0)]
const EUnexpectedNonce: vector<u8> = b"The nonce is not the next value of the account's counter.";

// === Structs ===

/// Dynamic field name of the counter for approvals checked by `Check`.
public struct AccountNonceKey<phantom Check> has copy, drop, store {}

// === View Functions ===

/// Returns the value the next approval checked by `Check` for the account must name: 0 if none
/// was taken yet.
public fun next_value<Check>(account_id: &UID): u64 {
    let key = AccountNonceKey<Check> {};
    if (df::exists_(account_id, key)) *df::borrow(account_id, key) else 0
}

// === Package Functions ===

/// Checks that `expected` is the next value of the account's counter for `Check` and moves the
/// counter on.
///
/// Aborts if `expected` is not the next value.
public(package) fun use_nonce<Check>(account_id: &mut UID, expected: u64) {
    let key = AccountNonceKey<Check> {};
    if (df::exists_(account_id, key)) {
        let counter: &mut u64 = df::borrow_mut(account_id, key);
        assert!(*counter == expected, EUnexpectedNonce);
        *counter = expected + 1;
    } else {
        assert!(expected == 0, EUnexpectedNonce);
        df::add(account_id, key, 1u64);
    }
}
