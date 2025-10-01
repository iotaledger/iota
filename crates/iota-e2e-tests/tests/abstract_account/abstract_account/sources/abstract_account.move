// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::bcs;
use iota::dynamic_field;
use iota::ed25519;
use iota::hex::decode;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> =
    b"The user who signed the transaction is not the account.";
#[error(code = 1)]
const EOwnerPublicKeyCannotBeUsed: vector<u8> =
    b"The `OwnerPublicKey` type cannot be used as a name for user-defined dynamic fields.";
#[error(code = 2)]
const EAuthenticatorDynamicFieldNameCannotBeUsed: vector<u8> =
    b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";

#[error(code = 10)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

/// This struct represents an abstract account on-chain.
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
public struct AbstractAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

/// Creates a new `AbstractAccount`  as a shared object with the given authenticator.
///
/// It uses the `authenticate_ed25519` to allow to verify the `signature` parameter against the public key stored in the account.
public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    // Create a UID for an account object.
    let mut id = object::new(ctx);

    // Add the account owner public key as a dynamic field.
    dynamic_field::add(&mut id, OwnerPublicKey {}, public_key);

    // Add the authenticator info as a dynamic field.
    dynamic_field::add(&mut id, account::authenticator_df_name(), authenticator);

    // Create a mutable shared account object.
    iota::transfer::share_object(AbstractAccount { id });
}

// --------------------------------------- Field Operations ---------------------------------------

/// Adds a new dynamic field to the account.
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut AbstractAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name(&name);

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
/// Only the account itself can call this function.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut AbstractAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name(&name);

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a reference to a dynamic field from the account.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &AbstractAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
/// Only the account itself can call this function.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut AbstractAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name(&name);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &AbstractAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

// --------------------------------------- Authenticators ---------------------------------------

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    self: &AbstractAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(signature), self.borrow_public_key(), ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Free access authenticator.
public fun authenticate_free_access(self: &AbstractAccount, _: &AuthContext, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Free access, do nothing.
}

// --------------------------------------- Utilities ---------------------------------------

/// An utility function to borrow the account-related public key.
fun borrow_public_key(self: &AbstractAccount): &vector<u8> {
    dynamic_field::borrow(&self.id, OwnerPublicKey {})
}

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks if `name` is allowed to be used for a user-defined dynamic field.
fun check_reserved_df_name<Name: copy + drop + store>(name: &Name) {
    // Check that `Name` is not `OwnerPublicKey`.
    assert!(
        std::type_name::get<Name>() != std::type_name::get<OwnerPublicKey>(),
        EOwnerPublicKeyCannotBeUsed,
    );

    // Check that `name` is not equal to `account::authenticator_df_name()`.
    assert!(
        (std::type_name::get<Name>() != std::type_name::get<vector<u8>>()) ||
        (bcs::to_bytes(name) != bcs::to_bytes(&account::authenticator_df_name())),
        EAuthenticatorDynamicFieldNameCannotBeUsed,
    );
}
