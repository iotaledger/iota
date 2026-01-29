// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::abstract_account;

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

/// Safely construct an AbstractAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator and dynamic fields.
public struct AbstractAccountBuilder {
    account: AbstractAccount,
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
}

/// This struct represents an abstract account.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// Arbitrary dynamic fields may be added and removed as necessary.
///
/// An `AbstractAccount` cannot be constructed directly. To create an `AbstractAccount` use `AbstractAccountBuilder`.
public struct AbstractAccount has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Construct an AbstractAccountBuilder and set the Authenticator.
///
/// The `AuthenticatorFunctionRef` will be attached to the account being built.
public fun builder(
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &mut TxContext,
): AbstractAccountBuilder {
    AbstractAccountBuilder {
        account: AbstractAccount { id: object::new(ctx) },
        authenticator,
    }
}

/// Attach a `Value` as a dynamic field to the account being built.
public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: AbstractAccountBuilder,
    name: Name,
    value: Value,
): AbstractAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);
    self
}

/// Finish building the `AbstractAccount` and share the object.
public fun build(self: AbstractAccountBuilder) {
    let AbstractAccountBuilder { account, authenticator } = self;
    account::create_account_v1(account, authenticator);
}

/// Adds a new dynamic field to the account.
///
/// Only the account itself can call this function.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut AbstractAccount,
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
    self: &mut AbstractAccount,
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
    self: &mut AbstractAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Replace a dynamic field.
///
/// Only the account itself can call this function.
/// This function cannot change the type of the stored `Value`.
public fun replace_field<Name: copy + drop + store, Value: store>(
    self: &mut AbstractAccount,
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
    self: &mut AbstractAccount,
    authenticator: AuthenticatorFunctionRefV1<AbstractAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<AbstractAccount> {
    ensure_tx_sender_is_account(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

// === Public-View Functions ===

/// Return the account's address.
public fun account_address(self: &AbstractAccount): address {
    self.id.to_address()
}

/// Borrows a reference to a dynamic field from the account.
///
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &AbstractAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &AbstractAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the attached authenticator.
public fun borrow_auth_function_ref_v1(
    self: &AbstractAccount,
): &AuthenticatorFunctionRefV1<AbstractAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

/// Receive an object that was previously sent to this AbstractAccount.
/// Gated so only the account itself can do it.
public fun receive_object(
    self: &mut AbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(self, ctx);
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.account_address());
}

/// Receive an object that was previously sent to this AbstractAccount.
/// This variant does not check the transaction sender.
public fun receive_object_without_sender_check(
    self: &mut AbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    _ctx: &TxContext,
) {
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.account_address());
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &AbstractAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
