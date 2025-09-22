// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_template::account_template;

use iota::account;
use iota::bcs;
use iota::dynamic_field;

// The following template provides the basic tools for a developer for implementing an abstract account,
// with the necessary authentication functions, error codes and expectations.

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
/// It should be emitted every time when an attempt at modifying a restricted dynamic field was made in
/// an inappropriate scope scope. This scope will generally be defined by the account implementers.
#[error(code = 1)]
const ERestrictedDynamicField: vector<u8> =
    b"Restricted dynamic fields cannot be modified directly.";
/// It should be emitted when only restricted dynamic fields may be modified. For example rotating authenticator
/// keys.
#[error(code = 2)]
const EInternalRestrictedDynamicField: vector<u8> =
    b"Internal configuration changes can only modify the restricted dynamic fields.";

public struct ReservedDfNames has copy, drop, store {}

/// This struct represents an IOTA account on-chain.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// It distinguishes between two classes of dynamic fields.
/// Reserved ones, used for managing the accounts internal state, such as unlock times and public keys.
/// Regular ones which can be used for data storage.
///
/// Reserved fields are expected to be keyed by their type and listed under ReservedDfNames.
/// The only exception to this is the dynamic field containing the authenticator function, which
/// has to use the key returned by "iota::account::authenticator_df_name()".
///
/// As regular data regular dynamic fields may be added and removed as necessary, but restricted ones cannot.
/// Since they are part of the authentication logic, in general they should not be removed only rotated.
public struct IOTAccount has key {
    id: UID,
}

// --------------------------------------- Creation ---------------------------------------

/// Create a shared IOTAccount.
public fun create_shared(uid: UID) {
    iota::transfer::share_object(IOTAccount { id: uid });
}

/// Create the key for accessing reserved dynamic fields.
public fun create_reserved_df_names(): ReservedDfNames {
    ReservedDfNames {}
}

// --------------------------------------- Field Operations ---------------------------------------

/// Add a new regular dynamic field to the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// restricted ones.
/// In case of violations: ETransactionSenderIsNotTheAccount, ERestrictedDynamicField will be
/// emitted.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(self, ctx);
    check_df<Name>(self, &name, false);

    dynamic_field::add(&mut self.id, name, value);
}

/// Remove a regular dynamic field from the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// restricted ones.
/// In case of violations: ETransactionSenderIsNotTheAccount, ERestrictedDynamicField will be
/// emitted.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_account(self, ctx);
    check_df<Name>(self, &name, false);

    dynamic_field::remove(&mut self.id, name)
}

/// Borrow a reference to a dynamic field from the account.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &IOTAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Borrow a mutable reference to a regular dynamic field from the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// restricted ones.
/// In case of violations: ETransactionSenderIsNotTheAccount, ERestrictedDynamicField will be
/// emitted.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut IOTAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    ensure_tx_sender_is_account(self, ctx);
    check_df<Name>(self, &name, false);

    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Return `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Rotate a reserved dynamic field.
///
/// Only the account itself can call this function and the dynamic field must refer be a
/// restricted one.
/// In case of violations: ETransactionSenderIsNotTheAccount, EInternalRestrictedDynamicField will be
/// emitted.
public fun rotate_reserved<Name: copy + drop + store, Value: drop + store>(
    self: &mut IOTAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(self, ctx);
    check_df<Name>(self, &name, true);

    let account_id = &mut self.id;
    dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
}

// --------------------------------------- Utilities ---------------------------------------
// These utility functions should be used for access validations while implementing an abstracted account.

/// Check that the sender of this transaction is the account.
public fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

/// Check if `name` is allowed to be used.
///
/// If `has_to_be_reserved` is set to false, then it returns true if name` refers to a reserved
/// dynamic field, otherwise it emits `EInternalRestrictedDynamicField`.
/// If `has_to_be_reserved` is set to false, then it returns true if `name` doesn't refer to a reserved
/// dynamic field, otherwise it emits `ERestrictedDynamicField`. It does not check in any way for the existence
/// of a regular dynamic field.
public fun check_df<Name: copy + drop + store>(
    self: &IOTAccount,
    name: &Name,
    has_to_be_reserved: bool,
) {
    let reserved_df_names: &vector<std::type_name::TypeName> = dynamic_field::borrow(
        &self.id,
        ReservedDfNames {},
    );
    let reserved_found =
        reserved_df_names.any!(|reserved| reserved == std::type_name::get<Name>()) || is_authenticate(name);

    if (has_to_be_reserved) {
        assert!(reserved_found, EInternalRestrictedDynamicField);
    } else {
        assert!(!reserved_found, ERestrictedDynamicField);
    }
}

fun is_authenticate<Name: copy + drop + store>(name: &Name): bool {
    // Check that `name` is not equal to `account::authenticator_df_name()`.
    (std::type_name::get<Name>() != std::type_name::get<vector<u8>>()) ||
        (bcs::to_bytes(name) != bcs::to_bytes(&account::authenticator_df_name()))
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun get_address(self: &IOTAccount): address {
    self.id.to_address()
}
