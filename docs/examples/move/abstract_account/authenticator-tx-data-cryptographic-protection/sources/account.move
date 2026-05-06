// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticator_tx_data_cryptographic_protection::account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;

#[error(code = 0)]
const EAccountStillLocked: vector<u8> = b"The account is still locked.";

/// A dynamic field key used for storing the "unlock time" for an account.
public struct UnlockTime has copy, drop, store {}

/// This struct represents a time-locked account.
public struct Account has key {
    id: UID,
}

/// Creates a new `Account` instance as a shared object with the given unlock time and authenticator.
public fun create(
    unlock_time: u64,
    authenticator: AuthenticatorFunctionRefV1<Account>,
    ctx: &mut TxContext,
) {
    let mut account = Account { id: object::new(ctx) };

    dynamic_field::add(&mut account.id, unlock_time_key(), unlock_time);

    account::create_account_v1(account, authenticator);
}

/// Authenticates access for the `Account`.
/// Checks if current clock time has passed the unlock time stored in the account.
///
/// IMPORTANT: This authenticator misses cryptographic protection for the transaction being authenticated,
/// that could allow an attacker to execute unauthorized actions or manipulate the transaction data.
#[authenticator]
public fun vulnerable_authenticate(account: &Account, _: &AuthContext, ctx: &TxContext) {
    assert!(ctx.epoch_timestamp_ms() >= account.unlock_time(), EAccountStillLocked);
}

/// Helper function to get the dynamic field key for unlock time.
fun unlock_time_key(): UnlockTime {
    UnlockTime {}
}

/// Helper function to get the unlock time from the account.
fun unlock_time(self: &Account): u64 {
    *dynamic_field::borrow(&self.id, unlock_time_key())
}
