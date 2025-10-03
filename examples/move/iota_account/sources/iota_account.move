// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_account::iota_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"The user who signed the transaction is not the account.";
#[error(code = 1)]
const EOwnerPublicKeyCannotBeUsed: vector<u8> = b"The `OwnerPublicKey` type cannot be used as a name for user-defined dynamic fields.";

#[error(code = 10)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 11)]
const ESecp256k1VerificationFailed: vector<u8> = b"Secp256k1 authenticator verification failed.";
#[error(code = 12)]
const ESecp256r1VerificationFailed: vector<u8> = b"Secp256r1 authenticator verification failed.";

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
public struct IOTAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a new `IOTAccount`  as a shared object with the given authenticator.
/// 
/// `authenticator` is expected to have a signature like the following:
///
/// public fun authenticate(self: &IOTAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
/// 
/// to allow to verify the `signature` parameter against the public key stored in the account.
/// 
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    // Create a UID for an account object.
    let mut id = object::new(ctx);

    // Add the account owner public key as a dynamic field.
    dynamic_field::add(&mut id, OwnerPublicKey{}, public_key);

    // Add the authenticator info as a dynamic field.
    account::attach_auth_info_v1(&mut id, authenticator);

    // Create a mutable shared account object.
    iota::transfer::share_object(IOTAccount { id });
}

// --------------------------------------- Field Operations ---------------------------------------

/// Adds a new dynamic field to the account.
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name<Name>();

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
/// Only the account itself can call this function.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name<Name>();

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a reference to a dynamic field from the account.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
/// Only the account itself can call this function.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name<Name>();

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorInfoV1` instance.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_auth_info_v1(self: &IOTAccount): &AuthenticatorInfoV1 {
    account::borrow_auth_info_v1(&self.id)
}

// --------------------------------------- Authentication ---------------------------------------

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    self: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    let account_id = &mut self.id;

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    let owner_public_key = OwnerPublicKey{};

    dynamic_field::remove<_, vector<u8>>(account_id, owner_public_key);
    dynamic_field::add(account_id, owner_public_key, public_key);

    // Update the authenticator info dynamic field. It is expected that the field already exists.
    account::rotate_auth_info_v1(account_id, authenticator);
}

// --------------------------------------- Authenticators ---------------------------------------

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(signature), self.borrow_public_key(), ctx.digest()),
        EEd25519VerificationFailed
    );
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ecdsa_k1::secp256k1_verify(&decode(signature), self.borrow_public_key(), ctx.digest(), 0),
        ESecp256k1VerificationFailed
    );
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ecdsa_r1::secp256r1_verify(&decode(signature), self.borrow_public_key(), ctx.digest(), 0),
        ESecp256r1VerificationFailed
    );
}

// --------------------------------------- Utilities ---------------------------------------

/// An utility function to borrow the account-related public key.
fun borrow_public_key(self: &IOTAccount): &vector<u8> {
    dynamic_field::borrow(&self.id, OwnerPublicKey{})
}

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks if `name` is allowed to be used for a user-defined dynamic field.
fun check_reserved_df_name<Name: copy + drop + store>() {
    // Check that `Name` is not `OwnerPublicKey`.
    assert!(std::type_name::get<Name>() != std::type_name::get<OwnerPublicKey>(), EOwnerPublicKeyCannotBeUsed);
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun create_owner_public_key_for_testing(): OwnerPublicKey {
    OwnerPublicKey{}
}

#[test_only]
public fun get_address(self: &IOTAccount): address {
    self.id.to_address()
}
