// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::delayed_abstract_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::coin::Coin;
use iota::dynamic_field;
use iota::iota::IOTA;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

// === Constants ===

// === Structs ===

/// This struct represents an abstract account that is firstly created as a shared object and then converted into account.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// Arbitrary dynamic fields may be added and removed as necessary.
///
/// An `DelayedAbstractAccount` cannot be constructed directly. To create an `DelayedAbstractAccount` use `DelayedAbstractAccountBuilder`.
public struct DelayedAbstractAccount has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Construct an empty DelayedAbstractAccount
public fun create(ctx: &mut TxContext) {
    transfer::share_object(DelayedAbstractAccount {
        id: object::new(ctx),
    });
}

/// Finish building the `DelayedAbstractAccount` and share the object.
public fun build(
    self: DelayedAbstractAccount,
    authenticator: AuthenticatorFunctionRefV1<DelayedAbstractAccount>,
) {
    account::create_account_v1(self, authenticator);
}

/// Returns `true` if and only if the account has been initialized with an authenticator.
public fun is_initialized(self: &DelayedAbstractAccount): bool {
    account::has_auth_function_ref_v1(&self.id)
}

/// Adds a new dynamic field to the account.
///
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut DelayedAbstractAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
///
/// Only the account itself can call this function.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut DelayedAbstractAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
///
/// Only the account itself can call this function.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut DelayedAbstractAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Replace a dynamic field.
///
/// Only the account itself can call this function.
/// This function cannot change the type of the stored `Value`.
public fun replace_field<Name: copy + drop + store, Value: store>(
    self: &mut DelayedAbstractAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);

    let account_id = &mut self.id;
    let previous_value = dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}

/// Rotate the attached authenticator.
///
/// Only the account itself can call this function.
public fun rotate_auth_function_ref_v1(
    self: &mut DelayedAbstractAccount,
    authenticator: AuthenticatorFunctionRefV1<DelayedAbstractAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<DelayedAbstractAccount> {
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

// === Public-View Functions ===

/// Return the account's address.
public fun account_address(self: &DelayedAbstractAccount): address {
    self.id.to_address()
}

/// Borrows a reference to a dynamic field from the account.
///
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &DelayedAbstractAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &DelayedAbstractAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the attached authenticator.
public fun borrow_auth_function_ref_v1(
    self: &DelayedAbstractAccount,
): &AuthenticatorFunctionRefV1<DelayedAbstractAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

/// Receive an object that was previously sent to this DelayedAbstractAccount.
/// Gated so only the account itself can do it.
public fun receive_object(
    self: &mut DelayedAbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account_only_if_initialized(self, ctx);
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.account_address());
}

/// Receive an object that was previously sent to this DelayedAbstractAccount.
/// This variant does not check the transaction sender.
public fun receive_object_without_sender_check(
    self: &mut DelayedAbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    _ctx: &TxContext,
) {
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.account_address());
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account itself, but only if the account has been initialized.
fun ensure_tx_sender_is_account_only_if_initialized(
    self: &DelayedAbstractAccount,
    ctx: &TxContext,
) {
    if (is_initialized(self)) {
        assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
    }
}

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
