// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iotaccount::iotaccount;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

// === Constants ===

// === Structs ===

/// Safely construct an IOTAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator function ref and dynamic fields.
public struct IOTAccountBuilder {
    account: IOTAccount,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
}

/// This struct represents an abstract IOTA account.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// Arbitrary dynamic fields may be added and removed as necessary.
///
/// An `IOTAccount` cannot be constructed directly. To create an `IOTAccount` use `IOTAccountBuilder`.
public struct IOTAccount has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Construct an IOTAccountBuilder and set the Authenticator.
///
/// The `AuthenticatorFunctionRef` will be attached to the account being built.
public fun builder(
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
): IOTAccountBuilder {
    IOTAccountBuilder {
        account: IOTAccount { id: object::new(ctx) },
        authenticator,
    }
}

/// Attach a `Value` as a dynamic field to the account being built.
public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);
    self
}

/// Finish building a shared `IOTAccount` instance.
public fun build(self: IOTAccountBuilder): address {
    let IOTAccountBuilder { account, authenticator } = self;

    let account_address = account.account_address();

    account::create_account_v1(account, authenticator);

    account_address
}

/// Adds a new dynamic field to the account.
///
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Add a new field.
    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
///
/// Only the account itself can call this function.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
///
/// Only the account itself can call this function.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Rotate a dynamic field.
///
/// Only the account itself can call this function.
/// This function cannot change the type of the stored `Value`.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_account(self, ctx);

    let account_id = &mut self.id;
    let previous_value = dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}

/// Rotate the attached authenticator.
///
/// Only the account itself can call this function.
public fun rotate_auth_function_ref_v1(
    self: &mut IOTAccount,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<IOTAccount> {
    ensure_tx_sender_is_account(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

// === Public-View Functions ===

/// Return the account's address.
public fun account_address(self: &IOTAccount): address {
    self.id.to_address()
}

/// Borrows a reference to a dynamic field from the account.
///
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the attached authenticator.
public fun borrow_auth_function_ref_v1(self: &IOTAccount): &AuthenticatorFunctionRefV1<IOTAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

public fun ensure_tx_sender_is_account_id(account: &UID, ctx: &TxContext) {
    assert!(account.to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
