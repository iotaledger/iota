// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The TimeLocked module defines a `TimeLocked` IOTAccount, which is an account protected by both an unlock
/// time and a public key.
///
/// The unlock time data is stored as a dynamic field of the account and the public key is stored as a
/// dynamic field of the account as well, using the `public_key_iotaccount` module.
///
/// Authenticator functions are provided to authenticate the account by verifying the public key signature
/// and checking the unlock time against the current time. Current time can be defined through the usage of
/// the Clock shared object or by using the epoch timestamp provided in the transaction context.
module time_locked::time_locked_iotaccount;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::clock::Clock;
use iotaccount::iotaccount::{Self, IOTAccount, IOTAccountBuilder};
use public_key_authentication::public_key_authentication;
use public_key_authentication::public_key_iotaccount;
use time_locked::unlock_time_authentication::{Self, unlock_time_field_name};

/// Allows calling `.with_public_key` on an `IOTAccountBuilder` to set a `public_key`.
use fun public_key_iotaccount::with_public_key as IOTAccountBuilder.with_public_key;

/// Allows calling `.rotate_public_key` on an `IOTAccount` to rotate a `public_key`.
use fun public_key_iotaccount::rotate_public_key as IOTAccount.rotate_public_key;

// === Errors ===

// === Constants ===

// === Structs ===

// === TimeLocked account ===

// Create a TimeLocked IOTAccount.
//
// The generated TimeLocked account is first protected by an
// Ed25519 authentication and then by an unlock time point.
// The provided `public_key` will be used for Ed25519 authentication,
// while the `unlock_time` is the point in time after which (including) the account
// can be accessed. This time is expected to be a unix timestamp in milliseconds.
public fun create(
    public_key: vector<u8>,
    admin: Option<address>,
    unlock_time: u64,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    // Create builder and attach the public key and the unlock time field to the account.
    let builder = iotaccount::builder(authenticator, ctx)
        .with_public_key(public_key)
        .with_field(unlock_time_field_name(), unlock_time);
    // Optionally attach the admin
    let builder = if (admin.is_some()) {
        builder.with_admin(admin.destroy_some())
    } else {
        builder
    };

    // Finally, build the account and share it.
    builder.build();
}

/// Attach an unlock time as a dynamic field to the account being built.
public fun with_unlock_time(builder: IOTAccountBuilder, unlock_time: u64): IOTAccountBuilder {
    builder.with_field(unlock_time_field_name(), unlock_time)
}

/// Rotates the account unlock time to a new one as well as the authenticator. It rotates the account public key if
/// `public_key` is provided as well.
/// Once this function is called, the previous unlock time and authenticator are no longer valid.
public fun rotate_unlock_time(
    account: &mut IOTAccount,
    public_key: Option<vector<u8>>,
    unlock_time: u64,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &TxContext,
) {
    // Update the account owner unlock time dynamic field. It is expected that the field already exists.
    account.rotate_field(unlock_time_field_name(), unlock_time, ctx);

    if (public_key.is_some()) {
        // Optionally update the account owner public key dynamic field. It is expected that the field already exists
        //if `public_key` is provided.
        account.rotate_public_key(public_key.destroy_some(), authenticator, ctx);
    } else {
        // Update the account authenticator dynamic field. It is expected that the field already exists.
        account.rotate_auth_function_ref_v1(authenticator, ctx);
    }
}

// Attach unlock time data to the account with the provided `unlock_time`.
// `unlock_time` is the unix timestamp in millisecond.
public fun add_unlock_time(account: &mut IOTAccount, unlock_time: u64, ctx: &TxContext) {
    account.add_field(unlock_time_field_name(), unlock_time, ctx);
}

// Detach unlock time data from the account, disabling unlock time based authentication
// for the account.
public fun remove_unlock_time(account: &mut IOTAccount, ctx: &TxContext) {
    account.remove_field<_, u64>(unlock_time_field_name(), ctx);
}

// === Authenticators ===

/// Authenticate access for the `TimeLocked` IOTAccount.
///
/// Uses an Ed25519 signature for authentication and checks the unlock time against the Clock.
#[authenticator]
public fun unlock_time_clock_ed25519_authenticator(
    account: &IOTAccount,
    clock: &Clock,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    public_key_authentication::authenticate_ed25519(
        account.borrow_uid(),
        signature,
        ctx,
    );
    unlock_time_authentication::authenticate_with_clock(account.borrow_uid(), clock);
}

/// Authenticate access for the `TimeLocked` IOTAccount.
///
/// Uses an Ed25519 signature for authentication and checks the unlock time against the epoch timestamp.
#[authenticator]
public fun unlock_time_epoch_ed25519_authenticator(
    account: &IOTAccount,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    public_key_authentication::authenticate_ed25519(
        account.borrow_uid(),
        signature,
        ctx,
    );
    unlock_time_authentication::authenticate_with_epoch_timestamp(account.borrow_uid(), ctx);
}

// === View Functions ===

/// An utility function to check if the account has an unlock time set.
public fun has_unlock_time(account: &IOTAccount): bool {
    account.has_field(unlock_time_field_name())
}

/// An utility function to borrow the account-related unlock time.
public fun borrow_unlock_time(account: &IOTAccount): &u64 {
    account.borrow_field(unlock_time_field_name())
}
