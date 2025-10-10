// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module time_locked::unlock_time;

use iota::clock::Clock;
use iota::dynamic_field;

// === Errors ===

#[error(code = 0)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";
#[error(code = 1)]
const EUnlockTimeAttached: vector<u8> = b"Unlock time already attached.";
#[error(code = 2)]
const EUnlockTimeMissing: vector<u8> = b"Unlock time missing.";

// === Constants ===

// === Structs ===

public struct UnlockTime has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

public fun attach(account_id: &mut UID, unlock_time: u64) {
    assert!(!has(account_id), EUnlockTimeAttached);
    dynamic_field::add(account_id, UnlockTime {}, unlock_time)
}

public fun detach(account_id: &mut UID): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    dynamic_field::remove(account_id, UnlockTime {})
}

public fun rotate(account_id: &mut UID, unlock_time: u64): u64 {
    assert!(has(account_id), EUnlockTimeMissing);

    let prev_unlock_time = dynamic_field::remove(account_id, UnlockTime {});
    dynamic_field::add(account_id, UnlockTime {}, unlock_time);
    prev_unlock_time
}

public fun authenticate_with_epoch_timestamp(account_id: &UID, ctx: &TxContext) {
    authenticate_unlock_time(account_id, ctx.epoch_timestamp_ms())
}

public fun authenticate_with_clock(account_id: &UID, clock: &Clock) {
    authenticate_unlock_time(account_id, clock.timestamp_ms())
}

public fun authenticate_unlock_time(account_id: &UID, current_time: u64) {
    assert!(has(account_id), EUnlockTimeMissing);

    let unlock_time: &u64 = borrow(account_id);

    // Enforce the time lock
    assert!(current_time >= *unlock_time, EAccountStillLocked);
}

// === View Functions ===

public fun has(account_id: &UID): bool {
    dynamic_field::exists_(account_id, UnlockTime {})
}

public fun borrow(account_id: &UID): &u64 {
    dynamic_field::borrow(account_id, UnlockTime {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
