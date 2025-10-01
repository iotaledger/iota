// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_account::iota_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::bcs;
use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;

#[error(code = 0)]
const EReservedDynamicFieldsListCannotBeSet: vector<u8> =
    b"'ReservedDfNames' field cannot be set by the user.";
#[error(code = 1)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
/// It should be emitted every time when an attempt at modifying a restricted dynamic field was made in
/// an inappropriate scope. This scope will be defined by the account implementers.
#[error(code = 2)]
const ECantModifyReservedDynamicField: vector<u8> =
    b"Restricted dynamic fields cannot be modified directly.";
/// It should be emitted when only restricted dynamic fields may be modified. For example rotating authenticator
/// keys.
#[error(code = 3)]
const EMustModifyReservedDynamicField: vector<u8> =
    b"Internal configuration changes can only modify the restricted dynamic fields.";
#[error(code = 1)]
const EOwnerPublicKeyCannotBeUsed: vector<u8> =
    b"The `OwnerPublicKey` type cannot be used as a name for user-defined dynamic fields.";
#[error(code = 2)]
const EAuthenticatorDynamicFieldNameCannotBeUsed: vector<u8> =
    b"The authenticator dynamic field system name cannot be used as a name for user-defined dynamic fields.";

// --------------------------------------- IOTAccountBuilder ---------------------------------------

/// Safely construct an IOTAccount.
///
/// The builder is entirely temporary. It cannot be copied, stored or dropped.
/// Account implementations are expected to call the builder in a single function call,
/// add the desired `AuthenticatorInfo` and all reserved dynamic fields necessary for the
/// operation of the account authentication logic.
/// Reserved fields can't be removed later, only their values may be replaced. For this reason
/// adding any dynamic fields that are not vital for the specified authenticator should be avoided.
public struct IOTAccountBuilder {
    account: IOTAccount,
}

/// Construct an IOTAccountBuilder and set the Authenticator.
///
/// The `AuthenticatorInfo` will be attached as a dynamic field with key provided by:
/// `account::authenticator_df_name()`.
public fun builder<Authenticator: copy + drop + store>(
    authenticator: Authenticator,
    ctx: &mut TxContext,
): IOTAccountBuilder {
    let mut builder = IOTAccountBuilder {
        account: IOTAccount { id: object::new(ctx) },
    };
    dynamic_field::add(&mut builder.account.id, get_reserved_dynamic_fields(), vector<DfKey>[]);
    builder.add_reserved_field(account::authenticator_df_name(), authenticator)
}

/// Attach a `Value` as a dynamic field to the builder.
public fun add_reserved_field<Name: copy + drop + store, Value: store>(
    mut self: IOTAccountBuilder,
    name: Name,
    value: Value,
): IOTAccountBuilder {
    let field_key = make_key(name);
    let reserved_dynamic_fields_key = get_reserved_dynamic_fields_key();
    assert!(field_key != reserved_dynamic_fields_key, EReservedDynamicFieldsListCannotBeSet);

    dynamic_field::add(&mut self.account.id, name, value);
    let reserved_keys: &mut vector<DfKey> = dynamic_field::borrow_mut(
        &mut self.account.id,
        get_reserved_dynamic_fields(),
    );
    // No need to check for duplicates, because dynamic_field::add above would fail on colliding keys
    // and in the builder one can only add fields.
    reserved_keys.push_back(field_key);

    self
}

/// Finish building the `IOTAccount` and share the object.
///
/// The call fails if no authenticator or reserved dynamic fields were set.
public fun finish(self: IOTAccountBuilder): IOTAccount {
    let IOTAccountBuilder { account } = self;
    account
}

/// Internal key type for reserved dynamic field identifiers.
///
/// They aren't meant to be used by callers/developers as `dynamic_field`
/// already handles differentiation bet
public struct DfKey has copy, drop, store {
    type_name: std::type_name::TypeName,
    value_bytes: vector<u8>,
}

public(package) fun make_key<KeyType: copy + drop + store>(key: KeyType): DfKey {
    DfKey {
        type_name: std::type_name::get<KeyType>(),
        value_bytes: bcs::to_bytes(&key),
    }
}

/// The key by which the list of reserved dynamic fields,
/// can be queried from an `IOTAccount`.
public struct ReservedDynamicFields has copy, drop, store {}

/// Create the key for accessing reserved dynamic fields.
public fun get_reserved_dynamic_fields(): ReservedDynamicFields {
    ReservedDynamicFields {}
}

/// Create a `DfKey` for `ReservedDynamicFields`.
public(package) fun get_reserved_dynamic_fields_key(): DfKey {
    make_key(ReservedDynamicFields {})
}

/// This struct represents an IOTA account on-chain.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// It distinguishes between two classes of dynamic fields.
/// Reserved ones, used for managing the accounts internal state, such as unlock times and public keys
/// and regular ones which can be used for data storage.
///
/// Reserved fields are keyed by `DFKey` and listed under `ReservedDynamicFields`.
///
/// As regular data, dynamic fields may be added and removed as necessary, but restricted ones cannot.
/// Since they are part of the authentication logic, in general they should not be removed only rotated.
///
/// An `IOTAccount` cannot be constructed directly. To create an `IOTAccount` use `IOTAccountBuilder`.

/// This struct represents an IOTA account on-chain.
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
public struct IOTAccount has key {
    id: UID,
}

/// Share IOTAccount.
public fun share(self: IOTAccount) {
    iota::transfer::share_object(self);
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
    check_reserved_df_name(&name);

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
    check_reserved_df_name(&name);

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
    check_reserved_df_name(&name);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &IOTAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

// --------------------------------------- Authentication ---------------------------------------

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    self: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    let account_id = &mut self.id;

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    let owner_public_key = OwnerPublicKey {};

    dynamic_field::remove<_, vector<u8>>(account_id, owner_public_key);
    dynamic_field::add(account_id, owner_public_key, public_key);

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    let authenticator_df_name = account::authenticator_df_name();

    dynamic_field::remove<_, AuthenticatorInfoV1>(account_id, authenticator_df_name);
    dynamic_field::add(account_id, authenticator_df_name, authenticator);
}

/// Add a dynamic field to the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones.
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

/// Remove a dynamic field from the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones.
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

/// Borrow a mutable reference to a non-reserved dynamic field from the account.
///
/// Only the account itself can call this function and the dynamic field can't collide with any
/// reserved ones.
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
/// reserved one.
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
/// If `has_to_be_reserved` is set to false, then it returns true if `name` refers to a reserved
/// dynamic field, otherwise it emits `EMustModifyReservedDynamicField`.
/// If `has_to_be_reserved` is set to false, then it returns true if `name` doesn't refer to a reserved
/// dynamic field, otherwise it emits `ECantModifyReservedDynamicField`. It does not check in any way for the existence
/// of a regular dynamic field.
public fun check_df<Name: copy + drop + store>(
    self: &IOTAccount,
    name: &Name,
    has_to_be_reserved: bool,
) {
    let key = make_key(*name);
    let reserved_df_names: &vector<DfKey> = dynamic_field::borrow(
        &self.id,
        get_reserved_dynamic_fields(),
    );
    let reserved_found = reserved_df_names.any!(|reserved| reserved == &key);

    if (has_to_be_reserved) {
        assert!(reserved_found, EMustModifyReservedDynamicField);
    } else {
        assert!(!reserved_found, ECantModifyReservedDynamicField);
    }
}

// --------------------------------------- Utilities ---------------------------------------

/// An utility function to borrow the account-related public key.
fun borrow_public_key(self: &IOTAccount): &vector<u8> {
    dynamic_field::borrow(&self.id, OwnerPublicKey {})
}

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
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

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun create_owner_public_key_for_testing(): OwnerPublicKey {
    OwnerPublicKey {}
}

#[test_only]
public fun get_address(self: &IOTAccount): address {
    self.id.to_address()
}
