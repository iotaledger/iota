// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iotaccount::iotaccount;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::bcs;
use iota::dynamic_field;

// === Imports ===

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
#[error(code = 1)]
const EAuthenticatorDynamicFieldNameCannotBeUsed: vector<u8> =
    b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";

// === Constants ===

// === Structs ===

/// Safely construct an IOTAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator info and dynamic fields.
public struct IOTAccountBuilder {
    account: IOTAccount,
}

/// This struct represents an abstract IOTA account.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// It distinguishes between two classes of dynamic fields.
/// Reserved ones, used for managing the account's internal state, such as unlock times and public keys
/// and regular ones which can be used for general data storage.
///
/// The list of reserved fields is stored as a dynamic field under `ReservedDynamicFields`.
///
/// As regular data, dynamic fields may be added and removed as necessary, but reserved ones cannot.
/// Reserved fields are part of the authentication logic so they should not be removed only rotated.
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
/// The `AuthenticatorInfo` will be attached as a dynamic field under key provided by:
/// `account::authenticator_df_name()`.
public fun builder(authenticator: AuthenticatorInfoV1, ctx: &mut TxContext): IOTAccountBuilder {
    // Builder should be mutable, but that triggers a compiler warning and it works
    // without for some reason, so it has been removed.
    let builder = IOTAccountBuilder {
        account: IOTAccount { id: object::new(ctx) },
    };
    builder.add_dynamic_field(account::authenticator_df_name(), authenticator)
}

/// Attach a `Value` as a regular dynamic field to the builder.
public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);

    self
}

/// Finish building the `IOTAccount` and share the object.
public fun finish(self: IOTAccountBuilder): IOTAccount {
    let IOTAccountBuilder { account } = self;
    account
}

/// Share IOTAccount.
public fun share(self: IOTAccount) {
    iota::transfer::share_object(self);
}

/// Adds a new dynamic field to the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones.
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
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    ensure_authenticator_not_modified(&name);

    // Remove a new field and return it.
    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    ensure_authenticator_not_modified(&name);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Rotate a dynamic field.
///
/// Only the account itself can call this function.
/// This function cannot change the type of the stored `Value`.
public fun rotate<Name: copy + drop + store, Value: store>(
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

// === Admin Functions ===

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Public-Package Functions ===

// === Private Functions ===

/// Checks if `name` does not equal `account::authenticator_df_name()`.
fun ensure_authenticator_not_modified<Name: copy + drop + store>(name: &Name) {
    // Check that `name` is not equal to `account::authenticator_df_name()`.
    assert!(
        (std::type_name::get<Name>() != std::type_name::get<vector<u8>>()) ||
        (bcs::to_bytes(name) != bcs::to_bytes(&account::authenticator_df_name())),
        EAuthenticatorDynamicFieldNameCannotBeUsed,
    );
}

// === Test Functions ===
