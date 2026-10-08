// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A time-locked Ed25519 authenticator for `SmartAccount`, adapted from
/// `time_locked_iotaccount::unlock_time_epoch_ed25519_authenticator` in the `time_locked` example.
///
/// The unlock time is stored on the account with `smart_account::add_field`, under a field key
/// type only this module can build.
module smart_account_allowed_authenticators::time_locked_authenticator;

use iota::smart_account::SmartAccount;
use smart_account_allowed_authenticators::ed25519_authenticator;

// === Errors ===

#[error(code = 0)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";
#[error(code = 1)]
const EUnlockTimeMissing: vector<u8> = b"Unlock time missing.";

// === Structs ===

/// Field key of the account's unlock time.
public struct UnlockTimeFieldName has copy, drop, store {}

// === Authenticators ===

/// Authenticates a transaction with an Ed25519 signature over its digest, once the epoch
/// timestamp has reached the account's unlock time.
#[authenticator]
public fun authenticate(
    account: &SmartAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert!(account.has_field(UnlockTimeFieldName {}), EUnlockTimeMissing);
    let unlock_time: u64 = *account.borrow_field(UnlockTimeFieldName {});
    assert!(ctx.epoch_timestamp_ms() >= unlock_time, EAccountStillLocked);

    ed25519_authenticator::verify_signature(account, &signature, ctx);
}

// === Admin Functions ===

/// Sets the unix timestamp, in milliseconds, from which the account unlocks.
///
/// Aborts if the transaction sender is not the account.
public fun set_unlock_time(account: &mut SmartAccount, unlock_time: u64, ctx: &TxContext) {
    if (account.has_field(UnlockTimeFieldName {})) {
        account.rotate_field<_, u64>(UnlockTimeFieldName {}, unlock_time, ctx);
    } else {
        account.add_field(UnlockTimeFieldName {}, unlock_time, ctx);
    }
}

// === View Functions ===

/// Returns the account's unlock time, if one is set.
public fun unlock_time(account: &SmartAccount): Option<u64> {
    if (account.has_field(UnlockTimeFieldName {})) {
        option::some(*account.borrow_field(UnlockTimeFieldName {}))
    } else {
        option::none()
    }
}
