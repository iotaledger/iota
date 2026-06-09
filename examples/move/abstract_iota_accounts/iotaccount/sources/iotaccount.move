// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The IOTAccount module defines a generic account struct that can be used as a base for different
/// types of accounts in the IOTA ecosystem.
///
/// The account data is stored as dynamic fields, which allows for flexible updates and extensions
/// without needing to change the underlying struct definition. The module also defines a builder
/// for safely constructing accounts with the necessary authenticator function reference and dynamic
/// fields.
///
/// The module includes functions for modifying the account (adding/removing/rotating fields and
/// admins) as well as public-view functions for reading the account's address, fields and attached
/// authenticator.
///
/// Authenticator functions are expected to be defined separately and passed as a reference when
/// creating an account. Whilst, rotating the authenticator function reference is handled within
/// this module. An admin can be optionally set for an account, in order to enable a more complex
/// rotation of the authenticator function reference. This can be useful in the case in which the
/// main authenticator function cannot be invoked to rotate itself, for example, because of a key
/// loss. The admin account is not necessarily expected to be owned by a different entity; it can be
/// used as another way to authenticate the account, in addition to the main authenticator function,
/// e.g., an admin account using a social recovery mechanism.
module iotaccount::iotaccount;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field as df;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
#[error(code = 1)]
const ETransactionSenderIsNotTheAccountOrAdmin: vector<u8> =
    b"Transaction must be signed by the account or the admin.";

// === Constants ===

// === Structs ===

/// This struct represents an IOTAccount.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// Arbitrary dynamic fields may be added and removed as necessary.
///
/// An `IOTAccount` cannot be constructed directly. To create an `IOTAccount` use `IOTAccountBuilder`.
public struct IOTAccount has key {
    id: UID,
}

/// A builder struct used to safely construct an IOTAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
/// Its main usage is to add fields to the account being built, and then to finish the building
/// process by calling `build()`. The most important field to add is the `AuthenticatorFunctionRefV1`,
/// which will be checked for validity in `build()`.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator function ref and dynamic fields.
public struct IOTAccountBuilder {
    account: IOTAccount,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
}

/// A dynamic field name for the account admin address.
public struct AdminFieldName has copy, drop, store {}

// === IOTAccountBuilder ===

/// Construct an IOTAccountBuilder and set the AuthenticatorFunctionRef.
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
public fun with_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    df::add(&mut self.account.id, name, value);
    self
}

/// Attach an Admin as a dynamic field to the account being built.
public fun with_admin(self: IOTAccountBuilder, admin: address): IOTAccountBuilder {
    self.with_field(AdminFieldName {}, admin)
}

/// Finish building an `IOTAccount` instance. This will check the validity of the attached authenticator
/// and then share the account object.
public fun build(self: IOTAccountBuilder): address {
    // Unpack the builder to get the built account and the attached authenticator.
    let IOTAccountBuilder { account, authenticator } = self;

    // Store the account's address, as the account will be passed by value later.
    let account_address = account.account_address();

    // Use the main API to create an account; this function will check the validity of the attached
    // authenticator against the IOTAccount type and then share the account object.
    account::create_account_v1(account, authenticator);

    account_address
}

// === IOTAccount Modification Functions ===

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
    df::add(&mut self.id, name, value);
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
    df::remove(&mut self.id, name)
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
    df::borrow_mut(&mut self.id, name)
}

/// Rotate a dynamic field.
///
/// Either the account or the admin can call this function.
/// This function cannot change the type of the stored `Value`.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_account_or_admin(self, ctx);

    let account_id = &mut self.id;
    let previous_value = df::remove<_, Value>(account_id, name);
    df::add(account_id, name, value);
    previous_value
}

/// Rotate the attached authenticator.
///
/// Only the account itself or the admin can call this function.
public fun rotate_auth_function_ref_v1(
    self: &mut IOTAccount,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<IOTAccount> {
    // Check that the sender of this transaction is the account or the admin.
    ensure_tx_sender_is_account_or_admin(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

/// Adds a new admin to the account.
///
/// Either the account or the admin can call this function.
public fun add_admin(self: &mut IOTAccount, admin: address, ctx: &TxContext) {
    // Check that the sender of this transaction is the account or the admin.
    ensure_tx_sender_is_account_or_admin(self, ctx);

    // Add a new admin.
    df::add(&mut self.id, AdminFieldName {}, admin);
}

/// Rotate an admin.
///
/// Either the account or the admin can call this function.
public fun rotate_admin(self: &mut IOTAccount, admin: address, ctx: &TxContext): address {
    // Check that the sender of this transaction is the account or the admin.
    ensure_tx_sender_is_account_or_admin(self, ctx);

    let account_id = &mut self.id;
    let previous_admin = df::remove<_, address>(account_id, AdminFieldName {});
    df::add(account_id, AdminFieldName {}, admin);
    previous_admin
}

// === IOTAccount Public-View Functions ===

/// Return the account's address.
public fun account_address(self: &IOTAccount): address {
    self.id.to_address()
}

/// Return the account's uid.
public fun borrow_uid(self: &IOTAccount): &UID {
    &self.id
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    df::exists_(&self.id, name)
}

/// Borrows a reference to a dynamic field from the account.
///
/// This function is not gated to be called only by the account,
/// anybody can call it to read the account dynamic fields.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
): &Value {
    df::borrow(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
/// This function is not gated to be called only by the account,
/// anybody can call it to read the attached authenticator.
public fun borrow_auth_function_ref_v1(self: &IOTAccount): &AuthenticatorFunctionRefV1<IOTAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

/// Borrows the admin of the account.
public fun borrow_admin(self: &IOTAccount): Option<address> {
    if (df::exists_(&self.id, AdminFieldName {})) {
        option::some(*df::borrow(&self.id, AdminFieldName {}))
    } else {
        option::none()
    }
}

// === Public-Package Functions ===

// === Private Functions ===

/// Check that the sender of this transaction is the account itself.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Check that the sender of this transaction is the account or the admin.
fun ensure_tx_sender_is_account_or_admin(self: &IOTAccount, ctx: &TxContext) {
    assert!(
        self.id.uid_to_address() == ctx.sender() || self.borrow_admin() == option::some(ctx.sender()),
        ETransactionSenderIsNotTheAccountOrAdmin,
    );
}

// === Test Functions ===
