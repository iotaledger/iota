// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module time_locked::time_locked;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::clock::Clock;
use iota::dynamic_field;
use iotaccount::basic_keyed_account;
use iotaccount::iotaccount;

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

public struct TimeLocked has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

public fun attach_unlock_time(account_id: &mut UID, unlock_time: u64) {
    assert!(!has_unlock_time(account_id), EUnlockTimeAttached);
    dynamic_field::add(account_id, UnlockTime {}, unlock_time)
}

public fun detach_unlock_time(account_id: &mut UID): u64 {
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    dynamic_field::remove(account_id, UnlockTime {})
}

public fun rotate_unlock_time(account_id: &mut UID, unlock_time: u64): u64 {
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    let prev_unlock_time = dynamic_field::remove(account_id, UnlockTime {});
    dynamic_field::add(account_id, UnlockTime {}, unlock_time);
    prev_unlock_time
}

public fun create(
    public_key: vector<u8>,
    unlock_time: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
) {
    let mut id = object::new(ctx);

    account::attach_auth_info_v1(&mut id, authenticator);

    basic_keyed_account::attach_public_key(&mut id, public_key);
    attach_unlock_time(&mut id, unlock_time);

    let account = TimeLocked { id };
    iota::transfer::share_object(account);
}

/// Authenticate access for the `Time locked account`.
public fun authenticate(
    id: &UID,
    clock: &Clock,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    iotaccount::ensure_tx_sender_is_account_id(&id.to_address(), ctx);

    basic_keyed_account::authenticate_ed25519_signature(id, signature, ctx.digest());
    let now = clock.timestamp_ms();
    authenticate_unlock_time(id, now);
}

public fun authenticate_unlock_time(account_id: &UID, current_time: u64) {
    assert!(has_unlock_time(account_id), EUnlockTimeMissing);

    let unlock_time: &u64 = borrow_unlock_time(account_id);

    // Enforce the time lock
    assert!(current_time >= *unlock_time, EAccountStillLocked);
}

// === View Functions ===

public fun account_address(self: &TimeLocked): address {
    self.id.to_address()
}

public fun borrow_uid(self: &TimeLocked): &UID {
    &self.id
}

public fun has_unlock_time(account_id: &UID): bool {
    dynamic_field::exists_(account_id, UnlockTime {})
}

public fun borrow_unlock_time(account_id: &UID): &u64 {
    dynamic_field::borrow(account_id, UnlockTime {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
