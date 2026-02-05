// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module time_locked::unlock_time;

use iota::clock::Clock;
use iota::dynamic_field;

// Common functionality for constructing signature based authentication logic for abstract accounts.
// These tools have protection for the values they manage, but impose no other access restrictions.
// It is the sole responsibility of the account developer to ensure that only the right sender has
// access to any logic provided by these functions.

// === Errors ===

#[error(code = 0)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";
#[error(code = 1)]
const EUnlockTimeAttached: vector<u8> = b"Unlock time already attached.";
#[error(code = 2)]
const EUnlockTimeMissing: vector<u8> = b"Unlock time missing.";

// === Constants ===

// === Structs ===

// A dynamic field key used for storing the "unlock time" for an account.
public struct UnlockTime has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

// Attach unlock time data to the account with the provided `unlock_time`.
// `unlock_time` is the unix timestamp in millisecond.
public fun attach(account_id: &mut UID, unlock_time: u64) {
    assert!(!has(account_id), EUnlockTimeAttached);
    dynamic_field::add(account_id, UnlockTime {}, unlock_time)
}

// Detach unlock time data from the account, disabling unlock time based authentication
// for the account.
public fun detach(account_id: &mut UID): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    dynamic_field::remove(account_id, UnlockTime {})
}

// Update the unlock time after which the account will unlock.
public fun rotate(account_id: &mut UID, unlock_time: u64): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    let prev_unlock_time = dynamic_field::remove(account_id, UnlockTime {});
    dynamic_field::add(account_id, UnlockTime {}, unlock_time);
    prev_unlock_time
}

// Check if epoch's unix timestamp has passed the unlock time stored in
// the account.
public fun authenticate_with_epoch_timestamp(account_id: &UID, ctx: &TxContext) {
    authenticate_unlock_time(account_id, ctx.epoch_timestamp_ms())
}

// Check if current clock time has passed the unlock time stored in
// the account.
public fun authenticate_with_clock(account_id: &UID, clock: &Clock) {
    authenticate_unlock_time(account_id, clock.timestamp_ms())
}

// Check if `current_time` unix timestamp has passed the unlock time stored in
// the account.
public fun authenticate_unlock_time(account_id: &UID, current_time: u64) {
    assert!(has(account_id), EUnlockTimeMissing);

    let unlock_time: &u64 = borrow(account_id);

    // Enforce the time lock
    assert!(current_time >= *unlock_time, EAccountStillLocked);
}

// === View Functions ===

// Check if the account has an unlock time set.
public fun has(account_id: &UID): bool {
    dynamic_field::exists_(account_id, UnlockTime {})
}

// Borrow the unix timestamp in milliseconds after which (including) the account
// will be accessible.
public fun borrow(account_id: &UID): &u64 {
    dynamic_field::borrow(account_id, UnlockTime {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
