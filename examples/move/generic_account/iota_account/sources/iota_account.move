// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_account::iota_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::bcs;
use iota::dynamic_field;

// --------------------------------------- Errors ---------------------------------------

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"The user who signed the transaction is not the account.";
#[error(code = 1)]
const EAuthenticatorDynamicFieldNameCannotBeUsed: vector<u8> = b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";

// ---------------------------------- IOTAccountBuilder ----------------------------------

public struct IOTAccountBuilder{
    account: IOTAccount,
}

public fun builder(ctx: &mut TxContext) : IOTAccountBuilder {
    IOTAccountBuilder{
        account: IOTAccount { id: object::new(ctx), reserved_df_names: vector::empty()}
    }
}

public fun add_authenticator(mut self: IOTAccountBuilder, authenticator: AuthenticatorInfoV1): IOTAccountBuilder {
    dynamic_field::add(&mut self.account.id, account::authenticator_df_name(), authenticator);
    self
}

public fun add_reserved_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    self.account.reserved_df_names.push_back(ReserveDFName{
        type_name: std::type_name::get<Name>(),
        bytes: bcs::to_bytes(&name),
    });
    dynamic_field::add(&mut self.account.id, name, value);
    self
}

public fun add_dynamic_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);
    self
}

public fun share(self: IOTAccountBuilder) {
    let IOTAccountBuilder { account } = self;

    iota::transfer::share_object(account);
}

// ---------------------------------- IOTAccount ----------------------------------

public struct ReserveDFName has store {
    type_name: std::type_name::TypeName,
    bytes: vector<u8>,
}

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
public struct IOTAccount has key {
    id: UID,
    // TODO: move to dynamic field.
    reserved_df_names: vector<ReserveDFName>,
}

public fun addr(self: &IOTAccount,): address {
    self.id.uid_to_address()
}

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
    self.check_reserved_df_name(&name);

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
    self. check_reserved_df_name(&name);

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
    self.check_reserved_df_name(&name);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Updates a dynamic field value and returns the previous one.
/// It is supposed that the dynamic field with the given name already exists.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    let previous_value = dynamic_field::remove(&mut self.id, name);
    dynamic_field::add(&mut self.id, name, value);
    previous_value
}

public fun rotate_reserved_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    assert!(self.is_reserved_df_name(&name));

    let previous_value = dynamic_field::remove(&mut self.id, name);
    dynamic_field::add(&mut self.id, name, value);
    previous_value
}

// --------------------------------------- Utilities ---------------------------------------

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks if `name` is allowed to be used for a user-defined dynamic field.
fun check_reserved_df_name<Name: copy + drop + store>(self: &IOTAccount, name: &Name) {
    // Check that `name` is not reserved.
    self.reserved_df_names.do_ref!(|item| {
        assert!(
            (std::type_name::get<Name>() != item.type_name) ||
            (bcs::to_bytes(name) != item.bytes),
            EAuthenticatorDynamicFieldNameCannotBeUsed
        );
    });
}

fun is_reserved_df_name<Name: copy + drop + store>(self: &IOTAccount, name: &Name): bool {
    self.reserved_df_names.find_index!(|n| std::type_name::get<Name>() == n.type_name && bcs::to_bytes(name) == n.bytes).is_some()
}