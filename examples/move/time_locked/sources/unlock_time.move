// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Common functionality for constructing signature based authentication logic for abstract accounts.
// These tools have protection for the values they manage, but impose no other access restrictions.
// It is the sole responsibility of the account developer to ensure that only the right sender has
// access to any logic provided by these functions.
module time_locked::unlock_time;

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

// A dynamic field key used for storing the "unlock time" for an account.
public struct UnlockTimeField has copy, drop, store {}

// Attach unlock time data to the account with the provided `unlock_time`.
// `unlock_time` is the unix timestamp in millisecond.
public fun attach(account_id: &mut UID, unlock_time: u64) {
    assert!(!has(account_id), EUnlockTimeAttached);
    df::add(account_id, UnlockTimeField {}, unlock_time)
}

// Detach unlock time data from the account, disabling unlock time based authentication
// for the account.
public fun detach(account_id: &mut UID): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    df::remove(account_id, UnlockTimeField {})
}

// Update the unlock time after which the account will unlock.
public fun rotate(account_id: &mut UID, unlock_time: u64): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    let prev_unlock_time = df::remove(account_id, UnlockTimeField {});
    df::add(account_id, UnlockTimeField {}, unlock_time);
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
    assert!(has(account_id), EUnlockTimeMissing);

    let unlock_time: &u64 = borrow(account_id);

    // Enforce the time lock
    assert!(current_time >= *unlock_time, EAccountStillLocked);
}

// === View Functions ===

// Check if the account has an unlock time set.
public fun has(account_id: &UID): bool {
    df::exists_(account_id, UnlockTimeField {})
}

// Borrow the unix timestamp in milliseconds after which (including) the account
// will be accessible.
public fun borrow(account_id: &UID): &u64 {
    df::borrow(account_id, UnlockTimeField {})
}

// === Package Functions ===

public(package) fun unlock_time_field(): UnlockTimeField {
    UnlockTimeField {}
}
