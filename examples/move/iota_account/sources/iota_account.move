// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_account::iota_account;

// === Imports ===

use iota::account::{Self, AuthenticatorInfoV1};
use iota::bcs;
use iota::dynamic_field;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
/// It should be emitted every time when an attempt at modifying a restricted dynamic field was made in
/// an inappropriate scope. This scope will be defined by the account implementers.
#[error(code = 1)]
const ECantModifyReservedDynamicField: vector<u8> =
    b"Restricted dynamic fields cannot be modified directly.";

// === Constants ===

// === Structs ===

/// The key by which the list of reserved dynamic fields,
/// can be queried from an `IOTAccount`.
public struct ReservedDynamicFields has copy, drop, store {}

/// Safely construct an IOTAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
///
/// Account implementations are expected to call the builder in a single function call,
/// add the desired authenticator info and all reserved dynamic fields necessary for the
/// operation of the account authentication logic.
/// All reserved field `Name`s will be stored under key `ReservedDynamicFields` which is managed
/// by the builder.
///
/// For convenience the regular dynamic fields may be added at this stage as well.
public struct IOTAccountBuilder {
    account: IOTAccount,
}

/// Internal key type for reserved dynamic field identifiers.
///
/// They aren't meant to be used by callers/developers as `dynamic_field`
/// already handles differentiation better. Only necessary for our internally
/// managed `ReservedDynamicFields`.
public struct DynamicFieldKey has copy, drop, store {
    type_name: std::type_name::TypeName,
    value_bytes: vector<u8>,
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
    let mut builder = IOTAccountBuilder {
        account: IOTAccount { id: object::new(ctx) },
    };
    dynamic_field::add(
        &mut builder.account.id,
        ReservedDynamicFields {},
        vector<DynamicFieldKey>[],
    );
    builder.add_reserved_field(account::authenticator_df_name(), authenticator)
}

/// Attach a `Value` as a reserved dynamic field to the builder.
public fun add_reserved_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    let field_key = make_dynamic_field_key(name);

    dynamic_field::add(&mut self.account.id, name, value);
    let reserved_keys: &mut vector<DynamicFieldKey> = dynamic_field::borrow_mut(
        &mut self.account.id,
        ReservedDynamicFields {},
    );
    // No need to check for duplicates, because dynamic_field::add above would fail on colliding keys
    // and in the builder one can only add fields.
    reserved_keys.push_back(field_key);

    self
}

/// Attach a `Value` as a regular dynamic field to the builder.
public fun add_regular_field<Name: copy + drop + store, Value: store>(
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

    // Check if `name` is allowed to be used.
    check_reserved_dynamic_field_name(self, &name);

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

    // Check if `name` is allowed to be used.
    check_reserved_dynamic_field_name(self, &name);

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

    // Check if `name` is allowed to be used.
    check_reserved_dynamic_field_name(self, &name);

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

public fun borrow_reserved_dynamic_fields(self: &IOTAccount): &vector<DynamicFieldKey> {
    self.borrow_field(ReservedDynamicFields {})
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

/// Check if `name` is allowed to be used.
///
/// Checks if `name` refers to a reserved dynamic field, in which case it asserts.
/// Otherwise it allows execution to continue.
public fun check_reserved_dynamic_field_name<Name: copy + drop + store>(
    self: &IOTAccount,
    name: &Name,
) {
    let key = make_dynamic_field_key(*name);
    let reserved_dynamic_field_names: &vector<DynamicFieldKey> = dynamic_field::borrow(
        &self.id,
        ReservedDynamicFields {},
    );
    let reserved_found = reserved_dynamic_field_names.any!(|reserved| reserved == &key);

    assert!(!reserved_found, ECantModifyReservedDynamicField);
}

// === Public-Package Functions ===

// This can't be private as it is used in the IOTAccountBuilder test.
public(package) fun make_dynamic_field_key<KeyType: copy + drop + store>(
    key: KeyType,
): DynamicFieldKey {
    DynamicFieldKey {
        type_name: std::type_name::get<KeyType>(),
        value_bytes: bcs::to_bytes(&key),
    }
}

// === Private Functions ===

// === Test Functions ===
