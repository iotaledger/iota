// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticator_shared_object_swap::account;

use authenticator_shared_object_swap::blacklist::Blacklist;
use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;
use iota::ed25519;
use iota::hex::decode;

#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 1)]
const EAccountIsBlacklisted: vector<u8> = b"Account is blacklisted.";

/// A dynamic field key for storing the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

/// This struct represents an account protected by an Ed25519 signature.
public struct Account has key {
    id: UID,
}

/// Creates a new `Account` instance as a shared object with the given public key and authenticator.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<Account>,
    ctx: &mut TxContext,
) {
    let mut account = Account { id: object::new(ctx) };

    dynamic_field::add(&mut account.id, owner_public_key(), public_key);

    account::create_account_v1(account, authenticator);
}

/// Authenticates access for the `Account`.
/// Verifies the provided Ed25519 signature against the stored public key.
/// Additionally, checks if the sender's account is blacklisted using the provided `blacklist` shared object.
///
/// IMPORTANT: This authenticator uses a shared object for blacklist checking, which may introduce
/// potential vulnerabilities if the shared object is not properly managed.
#[authenticator]
public fun authenticate(
    account: &Account,
    signature: vector<u8>,
    blacklist: &Blacklist,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert!(!blacklist.is_blacklisted(ctx.sender()), EAccountIsBlacklisted);

    assert!(
        ed25519::ed25519_verify(&decode(signature), account.public_key(), ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Helper function to get the dynamic field key for owner public key.
fun owner_public_key(): OwnerPublicKey {
    OwnerPublicKey {}
}

/// Helper function to borrow the owner public key from the account.
fun public_key(self: &Account): &vector<u8> {
    dynamic_field::borrow(&self.id, owner_public_key())
}
