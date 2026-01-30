// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module time_locked::account;

use generic_keyed_authentication::owner_public_key;
use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::clock::Clock;
use iotaccount::iotaccount;
use time_locked::unlock_time;

// === Errors ===

// === Constants ===

// === Structs ===

// A "time locked" abstract account.
public struct TimeLocked has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

// Create a `TimeLocked` account.
//
// The generated `TimeLocked` account is first protected by an
// Ed25519 authentication and then by an unlock time point.
// The provided `public_key` will be used for Ed25519 authentication,
// while the `unlock_time` is the point in time after which (including) the account
// can be accessed. This time is expected to be a unix timestamp in milliseconds.
public fun create(
    public_key: vector<u8>,
    unlock_time: u64,
    authenticator: AuthenticatorFunctionRefV1<TimeLocked>,
    ctx: &mut TxContext,
) {
    let mut id = object::new(ctx);

    owner_public_key::attach(&mut id, public_key);
    unlock_time::attach(&mut id, unlock_time);

    let account = TimeLocked { id };

    account::create_account_v1(account, authenticator);
}

/// Authenticate access for the `Time locked account`.
///
/// Specific authenticate function for the `TimeLocked` account, not
/// callable by general move code.
#[authenticator]
public fun authenticate(
    account: &TimeLocked,
    clock: &Clock,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    iotaccount::ensure_tx_sender_is_account_id(&account.id, ctx);

    owner_public_key::authenticate_ed25519(&account.id, signature, ctx.digest());
    unlock_time::authenticate_with_clock(&account.id, clock);
}

// === View Functions ===

// Query the address of the `TimeLocked` account.
public fun account_address(self: &TimeLocked): address {
    self.id.to_address()
}

// Borrow the unix timestamp in milliseconds after which (including) the account
// will be accessible.
public fun borrow_unlock_time(self: &TimeLocked): &u64 {
    unlock_time::borrow(&self.id)
}

// Borrow the public key used for Ed25519 authentication.
public fun borrow_public_key(self: &TimeLocked): &vector<u8> {
    owner_public_key::borrow(&self.id)
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
