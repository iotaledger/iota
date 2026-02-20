// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Common functionality for constructing signature based authentication logic for abstract accounts.
// These tools have protection for the values they manage, but impose no other access restrictions.
// It is the sole responsibility of the account developer to ensure that only the right sender has
// access to any logic provided by these functions.
module time_locked::unlock_time_authentication;

use iota::clock::Clock;
use iota::dynamic_field as df;

// === Errors ===

#[error(code = 0)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";
#[error(code = 1)]
const EUnlockTimeAttached: vector<u8> = b"Unlock time already attached.";
#[error(code = 2)]
const EUnlockTimeMissing: vector<u8> = b"Unlock time missing.";

// === Constants ===

// === Structs ===

// A dynamic field name used for storing the "unlock time" for an account.
public struct UnlockTimeFieldName has copy, drop, store {}

// === Public Functions ===

// Attach unlock time data to the account with the provided `unlock_time`.
// `unlock_time` is the unix timestamp in millisecond.
public fun attach_unlock_time(account_id: &mut UID, unlock_time: u64) {
    assert!(!has_unlock_time(account_id), EUnlockTimeAttached);
    df::add(account_id, UnlockTimeFieldName {}, unlock_time)
}

// Detach unlock time data from the account, disabling unlock time based authentication
// for the account.
public fun detach_unlock_time(account_id: &mut UID): u64 {
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    df::remove(account_id, UnlockTimeFieldName {})
}

// Update the unlock time after which the account will unlock.
public fun rotate_unlock_time(account_id: &mut UID, unlock_time: u64): u64 {
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    let prev_unlock_time = df::remove(account_id, UnlockTimeFieldName {});
    df::add(account_id, UnlockTimeFieldName {}, unlock_time);
    prev_unlock_time
}

// === Public Authenticators Helpers ===

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
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    let unlock_time: &u64 = borrow_unlock_time(account_id);

    // Enforce the time lock
    assert!(current_time >= *unlock_time, EAccountStillLocked);
}

// === View Functions ===

// Check if the account has an unlock time set.
public fun has_unlock_time(account_id: &UID): bool {
    df::exists_(account_id, UnlockTimeFieldName {})
}

// Borrow the unix timestamp in milliseconds after which (including) the account
// will be accessible.
public fun borrow_unlock_time(account_id: &UID): &u64 {
    df::borrow(account_id, UnlockTimeFieldName {})
}

// === Package Functions ===

// An utility function to construct the dynamic field name for the unlock time field.
public(package) fun unlock_time_field_name(): UnlockTimeFieldName {
    UnlockTimeFieldName {}
}
