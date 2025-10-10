// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module time_locked::account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::clock::Clock;
use iotaccount::basic_keyed_account;
use iotaccount::iotaccount;
use time_locked::utils;

// === Errors ===

// === Constants ===

// === Structs ===

public struct TimeLocked has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

public fun create(
    public_key: vector<u8>,
    unlock_time: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
) {
    let mut id = object::new(ctx);

    account::attach_auth_info_v1(&mut id, authenticator);

    basic_keyed_account::attach_public_key(&mut id, public_key);
    utils::attach_unlock_time(&mut id, unlock_time);

    let account = TimeLocked { id };
    iota::transfer::share_object(account);
}

/// Authenticate access for the `Time locked account`.
public fun authenticate(
    account: &TimeLocked,
    clock: &Clock,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    iotaccount::ensure_tx_sender_is_account_id(&account.id, ctx);

    basic_keyed_account::authenticate_ed25519_signature(&account.id, signature, ctx.digest());
    let now = clock.timestamp_ms();
    utils::authenticate_unlock_time(&account.id, now);
}

// === View Functions ===

public fun account_address(self: &TimeLocked): address {
    self.id.to_address()
}

public fun borrow_uid(self: &TimeLocked): &UID {
    &self.id
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
