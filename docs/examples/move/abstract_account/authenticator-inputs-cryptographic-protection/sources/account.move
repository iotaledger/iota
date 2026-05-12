// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticator_inputs_cryptographic_protection::account;

use authenticator_inputs_cryptographic_protection::blacklist::Blacklist;
use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;
use iota::ed25519;
use iota::hex::decode;
use std::bcs;
use std::hash;

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
/// The authenticator computes a message hash that includes the transaction digest, the blacklist object ID, and a raw value argument.
/// This provides cryptographic protection for the authenticator inputs, ensuring that the signature is bound to specific transaction data
/// and the inputs.
#[authenticator]
public fun authenticate(
    account: &Account,
    blacklist: &Blacklist,
    raw_value: u64,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    assert!(
        ed25519::ed25519_verify(
            &decode(signature),
            account.public_key(),
            &compute_message(blacklist, raw_value, ctx),
        ),
        EEd25519VerificationFailed,
    );

    assert!(!blacklist.is_blacklisted(ctx.sender()), EAccountIsBlacklisted);
}

/// Helper function to get the dynamic field key for owner public key.
fun owner_public_key(): OwnerPublicKey {
    OwnerPublicKey {}
}

/// Helper function to borrow the owner public key from the account.
fun public_key(self: &Account): &vector<u8> {
    dynamic_field::borrow(&self.id, owner_public_key())
}

/// Helper function to compute the message that should be signed for authentication.
fun compute_message(blacklist: &Blacklist, raw_value: u64, ctx: &TxContext): vector<u8> {
    let mut message = vector::empty();

    message.append(*ctx.digest());
    message.append(object::id(blacklist).id_to_bytes());
    message.append(bcs::to_bytes(&raw_value));

    hash::sha2_256(message)
}
