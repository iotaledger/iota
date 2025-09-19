// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_template::account_template;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::dynamic_field;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> =
    b"The user who signed the transaction is not the account.";
#[error(code = 1)]
const EAuthenticatorDynamicFieldNameCannotBeUsed: vector<u8> =
    b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";
#[error(code = 2)]
const EAuthenticatorDynamicFieldNameMustBeSystem: vector<u8> =
    b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";

// Can this cause problems when upgrading to a different version?
public struct ReservedDfNames has copy, drop, store {}

/// This struct represents an IOTA account on-chain.
/// It holds all the related data was dynamic fields to simplify updates, migrations and extensions.
public struct IOTAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

// Ideally this should take a macro parameter lambda which could add the necessary fields, as needed,
// but it seems that a macro has to use the macro $ based substitution for every parameter.
// Could pass it a Bag as an input for the fields, but then all values would have to be objects.
// In the end this function shouldn't exist
// public fun create(
//     reserved_df_names: vector<std::type_name::TypeName>,
//     authenticator: AuthenticatorInfoV1,
//     ctx: &mut TxContext,
// ) {
//     // Create a UID for an account object.
//     let mut id = object::new(ctx);

//     // Add the authenticator info as a dynamic field.
//     dynamic_field::add(&mut id, ReservedDfNames {}, reserved_df_names);
//     // Add the authenticator info as a dynamic field.
//     dynamic_field::add(&mut id, account::authenticator_df_name(), authenticator);

//     iota::transfer::share_object(IOTAccount { id });
// }

public fun shared_create(uid: UID) {
    iota::transfer::share_object(IOTAccount { id: uid });
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
    check_reserved_df_name<Name>(self, false);

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
    check_reserved_df_name<Name>(self, false);

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
    check_reserved_df_name<Name>(self, false);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

public fun rotate_reserved<Name: copy + drop + store, Value: drop + store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check if `name` is allowed to be used.
    check_reserved_df_name<Name>(self, true);

    let account_id = &mut self.id;

    dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
}

// --------------------------------------- Utilities ---------------------------------------

/// Checks that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Checks if `name` is allowed to be used for a user-defined dynamic field.
public fun check_reserved_df_name<Name: copy + drop + store>(
    self: &IOTAccount,
    has_to_be_reserved: bool,
) {
    // Check that `name` is not equal to `account::authenticator_df_name()`.
    let reserved_df_names: &vector<std::type_name::TypeName> = dynamic_field::borrow(
        &self.id,
        ReservedDfNames {},
    );

    let reserved_found = reserved_df_names.any!(|reserved| reserved == std::type_name::get<Name>());

    if (has_to_be_reserved) {
        assert!(reserved_found, EAuthenticatorDynamicFieldNameMustBeSystem);
    } else {
        assert!(!reserved_found, EAuthenticatorDynamicFieldNameCannotBeUsed);
    }
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun get_address(self: &IOTAccount): address {
    self.id.to_address()
}
