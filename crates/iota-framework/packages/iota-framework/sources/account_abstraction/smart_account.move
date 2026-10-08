// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// `SmartAccount` — a general-purpose on-chain account authenticated by any
/// `AuthenticatorFunctionRefV1`.
///
/// This module holds only what every `SmartAccount` needs: creating the account, managing its
/// dynamic fields, and rotating its authenticator. It knows nothing about any particular
/// authenticator. Other modules extend an account by adding dynamic fields under their own key
/// types. `iota::smart_account_builtin_auth` is such a module: it adds the built-in authenticator
/// for IOTA's standard signature schemes together with the public key it checks signatures
/// against.
///
/// `iota::smart_account_rotation_rules` lets other modules limit how the authenticator and the
/// account's fields are changed. While an account has rotation rules attached,
/// `rotate_auth_function_ref_v1` aborts, and the authenticator can only be rotated through that
/// module. While rules guard a field type `Name`, `add_field`, `remove_field`, `borrow_field_mut`
/// and `rotate_field` abort for `Name`, and the field can only be changed through that module.
///
/// `SmartAccount`s are created through the `SmartAccountBuilder` API: `builder_v1` allocates a new
/// object ID for the supplied authenticator. After optionally adding fields with `with_field`,
/// finalize with `build_v1`.
///
/// Once built, the account can only be changed by itself — the admin functions require the
/// transaction sender to be the smart account's address.
module iota::smart_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::claim;
use iota::dynamic_field;
use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheSmartAccount: vector<u8> =
    b"Transaction must be signed by the smart account.";
#[error(code = 1)]
const EAuthRotationRulesAttached: vector<u8> =
    b"The account has rotation rules attached; rotate through smart_account_rotation_rules.";
#[error(code = 2)]
const EFieldRotationRulesAttached: vector<u8> =
    b"The field has rotation rules attached; change it through smart_account_rotation_rules.";

// === Structs ===

/// General-purpose on-chain account object.
///
/// `SmartAccount`s can only be created via `SmartAccountBuilder` — use `builder_v1` to obtain
/// one, optionally add fields with `with_field`, then finalize with `build_v1`.
///
/// All data is stored as dynamic fields, keeping the struct stable across
/// upgrades and allowing arbitrary extensions.
public struct SmartAccount has key {
    id: UID,
}

/// Temporary builder for constructing a `SmartAccount` before it is registered on-chain.
///
/// The builder cannot be copied, stored, or dropped — it must be consumed by `build_v1`.
///
/// Use `with_field` before finalizing. It is the only way to add fields at creation time, since
/// post-creation the admin functions require the transaction sender to be the account's address.
public struct SmartAccountBuilder {
    account: SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
}

/// Dynamic field key present while the account has rotation rules attached.
public struct AuthRotationRulesAttachedKey has copy, drop, store {}

/// Dynamic field key present while rotation rules guard the field type `Name`.
public struct FieldRotationRulesAttachedKey<phantom Name> has copy, drop, store {}

// === SmartAccountBuilder Public Functions ===

/// Creates a `SmartAccountBuilder` for a new account with the provided authenticator.
///
/// The authenticator must be able to authenticate the account as built. For accounts backed by
/// a public key and the built-in authenticator, use `smart_account_builtin_auth::builder_v1`.
public fun builder_v1(
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &mut TxContext,
): SmartAccountBuilder {
    SmartAccountBuilder {
        account: SmartAccount { id: object::new(ctx) },
        authenticator,
    }
}

/// Adds a `Value` as a dynamic field to the account being built.
///
/// Aborts if a field with the same `name` already exists.
public fun with_field<Name: copy + drop + store, Value: store>(
    mut self: SmartAccountBuilder,
    name: Name,
    value: Value,
): SmartAccountBuilder {
    dynamic_field::add(&mut self.account.id, name, value);
    self
}

/// Finish building the account as a mutable shared object.
///
/// Emits an `account::MutableAccountCreated` event on success.
public fun build_v1(self: SmartAccountBuilder): address {
    let SmartAccountBuilder { account, authenticator } = self;
    let account_address = account.account_address();

    account::create_account_v1(account, authenticator);

    account_address
}

// === View Functions ===

/// Returns the account's address.
public fun account_address(self: &SmartAccount): address {
    self.id.to_address()
}

/// Returns `true` if and only if `self` has a dynamic field with the specified `name`.
public fun has_field<Name: copy + drop + store>(self: &SmartAccount, name: Name): bool {
    dynamic_field::exists_(&self.id, name)
}

/// Borrows a reference to a dynamic field from the account.
///
/// Aborts if no field with the specified `name` exists.
public fun borrow_field<Name: copy + drop + store, Value: store>(
    self: &SmartAccount,
    name: Name,
): &Value {
    dynamic_field::borrow(&self.id, name)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
///
/// Aborts if no authenticator is attached.
public fun borrow_auth_function_ref_v1(
    self: &SmartAccount,
): &AuthenticatorFunctionRefV1<SmartAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

/// Returns `true` if and only if the account has rotation rules attached, in which case its
/// authenticator can only be rotated through `smart_account_rotation_rules`.
public fun has_auth_rotation_rules(self: &SmartAccount): bool {
    dynamic_field::exists_(&self.id, AuthRotationRulesAttachedKey {})
}

/// Returns `true` if and only if rotation rules guard the field type `Name`, in which case
/// fields of that type can only be changed through `smart_account_rotation_rules`.
public fun has_field_rotation_rules<Name: copy + drop + store>(self: &SmartAccount): bool {
    dynamic_field::exists_(&self.id, FieldRotationRulesAttachedKey<Name> {})
}

/// Aborts if the sender of this transaction is not the account itself.
public fun ensure_tx_sender_is_smart_account(self: &SmartAccount, ctx: &TxContext) {
    assert!(self.account_address() == ctx.sender(), ETransactionSenderIsNotTheSmartAccount);
}

// === Admin Functions ===

/// Adds a dynamic field to the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if rotation rules guard the field type `Name`; use `smart_account_rotation_rules` then.
/// Aborts if a field with the same `name` already exists.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_smart_account(self, ctx);
    assert!(!self.has_field_rotation_rules<Name>(), EFieldRotationRulesAttached);

    dynamic_field::add(&mut self.id, name, value);
}

/// Removes a dynamic field from the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if rotation rules guard the field type `Name`; use `smart_account_rotation_rules` then.
/// Aborts if no field with the specified `name` exists.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_smart_account(self, ctx);
    assert!(!self.has_field_rotation_rules<Name>(), EFieldRotationRulesAttached);

    dynamic_field::remove(&mut self.id, name)
}

/// Borrows a mutable reference to a dynamic field from the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if rotation rules guard the field type `Name`; use `smart_account_rotation_rules` then.
/// Aborts if no field with the specified `name` exists.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    ensure_tx_sender_is_smart_account(self, ctx);
    assert!(!self.has_field_rotation_rules<Name>(), EFieldRotationRulesAttached);

    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Replaces a dynamic field with a new value and returns the previous one.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if rotation rules guard the field type `Name`; use `smart_account_rotation_rules` then.
/// Aborts if no field with the specified `name` exists.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_smart_account(self, ctx);
    assert!(!self.has_field_rotation_rules<Name>(), EFieldRotationRulesAttached);

    let account_id = &mut self.id;
    let previous_value = dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}

/// Rotates the attached authenticator and returns the previous one.
///
/// Accepts any authenticator and does not check that the account can still be authenticated
/// afterwards: rotating to the built-in authenticator while no public key is attached leaves the
/// account unable to send any transaction. To switch to or from the built-in authenticator, use
/// `smart_account_builtin_auth::rotate_to_builtin_auth_v1` and
/// `smart_account_builtin_auth::rotate_to_custom_auth_v1`, which attach and detach the public key
/// together with the rotation.
///
/// Emits an `account::AuthenticatorFunctionRefV1Rotated` event upon success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the account has rotation rules attached; use `smart_account_rotation_rules` then.
public fun rotate_auth_function_ref_v1(
    self: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    ensure_tx_sender_is_smart_account(self, ctx);
    assert!(!self.has_auth_rotation_rules(), EAuthRotationRulesAttached);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

// === Package Functions ===

/// Creates a `SmartAccountBuilder` whose account ID is the claimed sender address.
///
/// Aborts if `public_key` does not derive the sender's address.
public(package) fun new_claim_builder(
    public_key: PublicKey,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): SmartAccountBuilder {
    SmartAccountBuilder {
        account: SmartAccount { id: claim::claim_address(public_key, ctx) },
        authenticator,
    }
}

/// Borrows the `UID` of the account being built.
public(package) fun borrow_uid_mut(self: &mut SmartAccountBuilder): &mut UID {
    &mut self.account.id
}

/// Borrows the account's `UID`.
public(package) fun uid(self: &SmartAccount): &UID {
    &self.id
}

/// Borrows the account's `UID` mutably.
public(package) fun uid_mut(self: &mut SmartAccount): &mut UID {
    &mut self.id
}

/// Records whether the account with `account_id` has rotation rules attached.
public(package) fun set_has_auth_rotation_rules(account_id: &mut UID, has_rules: bool) {
    let attached = dynamic_field::exists_(account_id, AuthRotationRulesAttachedKey {});
    if (has_rules && !attached) {
        dynamic_field::add(account_id, AuthRotationRulesAttachedKey {}, true);
    } else if (!has_rules && attached) {
        dynamic_field::remove<_, bool>(account_id, AuthRotationRulesAttachedKey {});
    }
}

/// Records whether rotation rules guard the field type `Name` on the account with `account_id`.
public(package) fun set_has_field_rotation_rules<Name: copy + drop + store>(
    account_id: &mut UID,
    has_rules: bool,
) {
    let attached = dynamic_field::exists_(account_id, FieldRotationRulesAttachedKey<Name> {});
    if (has_rules && !attached) {
        dynamic_field::add(account_id, FieldRotationRulesAttachedKey<Name> {}, true);
    } else if (!has_rules && attached) {
        dynamic_field::remove<_, bool>(account_id, FieldRotationRulesAttachedKey<Name> {});
    }
}

/// Rotates the attached authenticator without checking the sender or the rotation rules.
///
/// The caller must have checked both.
public(package) fun rotate_auth_function_ref_v1_unchecked(
    self: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account::rotate_auth_function_ref_v1(self, authenticator)
}
