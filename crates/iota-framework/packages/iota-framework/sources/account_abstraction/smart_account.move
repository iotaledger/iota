// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// `SmartAccount` — an on-chain account with built-in support for IOTA's standard
/// signature schemes (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey).
/// Any `AuthenticatorFunctionRefV1` can also be used via `SmartAccountBuilder`
/// or by rotating the authenticator after creation.
///
/// `SmartAccount`s are created through the `SmartAccountBuilder` API:
///
/// - `builder_v1`: allocates a new object ID; the caller supplies any
///   `AuthenticatorFunctionRefV1`.
/// - `builtin_auth_builder_v1`: allocates a new object ID; selects the built-in
///   authenticator matching the provided `PublicKey`'s signature scheme.
/// - `claim_builder_v1`: for addresses that already exist on-chain.
///   `ClaimRegistry` records each address once to prevent double-claiming and
///   ensures the new account object's ID matches the sender's address.
///
/// After optionally adding fields with `with_field`, finalize with
/// `build_v1` (mutable) or `build_immutable_v1` (immutable).
///
/// Once built, dynamic fields can only be managed by the account itself — the admin
/// functions require the transaction sender to be the smart account's address.
///
/// `SmartAccount` kinds:
///
/// - **Mutable** accounts can have their authenticator rotated after creation
///   and support adding, removing, and mutating dynamic fields via the admin
///   functions in this module.
/// - **Immutable** accounts are frozen at creation; neither the authenticator
///   nor any dynamic fields can ever be changed.
module iota::smart_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions;
use iota::claim_registry::ClaimRegistry;
use iota::dynamic_field;
use iota::public_key::PublicKey;
use iota::signature_scheme::{Self, SignatureScheme};

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheSmartAccount: vector<u8> =
    b"Transaction must be signed by the smart account.";

#[error(code = 10)]
const EInvalidSignatureScheme: vector<u8> = b"Invalid signature scheme.";

// === Structs ===

/// General-purpose on-chain account object.
///
/// `SmartAccount`s can only be created via `SmartAccountBuilder` — use `builder_v1`,
/// `builtin_auth_builder_v1`, or `claim_builder_v1` to obtain one, optionally
/// add fields with `with_field`, then finalize with `build_v1` or `build_immutable_v1`.
///
/// All data is stored as dynamic fields, keeping the struct stable across
/// upgrades and allowing arbitrary extensions.
public struct SmartAccount has key {
    id: UID,
}

/// Temporary builder for constructing a `SmartAccount` before it is registered on-chain.
///
/// The builder cannot be copied, stored, or dropped — it must be consumed by
/// `build_v1` or `build_immutable_v1`.
///
/// Use `with_field` to add dynamic fields before finalizing. This is the only
/// way to add fields at creation time, since post-creation the admin functions
/// require the transaction sender to be the account's address.
public struct SmartAccountBuilder {
    account: SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
}

// === SmartAccountBuilder Public Functions ===

/// Creates a `SmartAccountBuilder` for a new account with the provided authenticator.
///
/// Use this when you want to supply a custom `AuthenticatorFunctionRefV1`.
/// For accounts backed by a built-in signature scheme, prefer `builtin_auth_builder_v1`.
public fun builder_v1(
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &mut TxContext,
): SmartAccountBuilder {
    SmartAccountBuilder {
        account: SmartAccount { id: object::new(ctx) },
        authenticator,
    }
}

/// Creates a `SmartAccountBuilder` for a new account backed by the built-in authenticator
/// for `public_key`'s signature scheme.
///
/// The public key is stored as a dynamic field on the account so the authenticator
/// can validate future transactions.
///
/// Aborts if `public_key`'s signature scheme is not supported.
public fun builtin_auth_builder_v1(
    public_key: PublicKey,
    ctx: &mut TxContext,
): SmartAccountBuilder {
    let mut account = SmartAccount { id: object::new(ctx) };
    builtin_authenticator_functions::attach_public_key(&mut account.id, public_key);

    SmartAccountBuilder {
        account,
        authenticator: resolve_builtin_authenticator(public_key.scheme()),
    }
}

/// Creates a `SmartAccountBuilder` for an existing on-chain address backed by the
/// built-in authenticator for `public_key`'s signature scheme.
///
/// `registry` records the sender's address to prevent double-claiming and
/// ensures the new account object's ID matches the sender's address.
///
/// Aborts if `public_key` does not correspond to the sender's address.
/// Aborts if the address has already been claimed.
/// Aborts if `public_key`'s signature scheme is not supported.
public fun claim_builder_v1(
    registry: &mut ClaimRegistry,
    public_key: PublicKey,
    ctx: &mut TxContext,
): SmartAccountBuilder {
    let mut account = SmartAccount { id: registry.claim(public_key, ctx) };
    builtin_authenticator_functions::attach_public_key(&mut account.id, public_key);

    SmartAccountBuilder {
        account,
        authenticator: resolve_builtin_authenticator(public_key.scheme()),
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
/// Emits a `MutableAccountCreated` event on success.
public fun build_v1(self: SmartAccountBuilder): address {
    let SmartAccountBuilder { account, authenticator } = self;
    let account_address = account.account_address();

    account::create_account_v1(account, authenticator);

    account_address
}

/// Finish building the account as an immutable object.
///
/// The authenticator and dynamic fields are frozen at this point and can never be changed.
///
/// Emits an `ImmutableAccountCreated` event on success.
public fun build_immutable_v1(self: SmartAccountBuilder): address {
    let SmartAccountBuilder { account, authenticator } = self;
    let account_address = account.account_address();

    account::create_immutable_account_v1(account, authenticator);

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

/// Returns `true` if and only if `self` has a built-in authenticator public key attached.
public fun has_builtin_auth_public_key(self: &SmartAccount): bool {
    builtin_authenticator_functions::has_public_key(&self.id)
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

/// Borrows the built-in authenticator public key attached to the account.
///
/// Aborts if no public key is currently attached.
public fun borrow_builtin_auth_public_key(self: &SmartAccount): &PublicKey {
    builtin_authenticator_functions::borrow_public_key(&self.id)
}

/// Borrows a reference to the attached `AuthenticatorFunctionRefV1` instance.
///
/// Aborts if no authenticator is attached.
public fun borrow_auth_function_ref_v1(
    self: &SmartAccount,
): &AuthenticatorFunctionRefV1<SmartAccount> {
    account::borrow_auth_function_ref_v1(&self.id)
}

// === Admin Functions ===

/// Adds a dynamic field to the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if a field with the same `name` already exists.
public fun add_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_smart_account(self, ctx);

    dynamic_field::add(&mut self.id, name, value);
}

/// Attaches built-in authenticator `public_key` to the account.
///
/// Use this when migrating away from a custom authenticator to a built-in one.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if a public key is already attached.
public fun attach_builtin_auth_public_key(
    self: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_smart_account(self, ctx);

    builtin_authenticator_functions::attach_public_key(&mut self.id, public_key);
}

/// Removes a dynamic field from the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no field with the specified `name` exists.
public fun remove_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_smart_account(self, ctx);

    dynamic_field::remove(&mut self.id, name)
}

/// Detaches and returns the built-in authenticator public key attached to the account.
///
/// Use this when migrating away from a built-in authenticator to a custom one.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is currently attached.
public fun detach_builtin_auth_public_key(self: &mut SmartAccount, ctx: &TxContext): PublicKey {
    ensure_tx_sender_is_smart_account(self, ctx);

    builtin_authenticator_functions::detach_public_key(&mut self.id)
}

/// Borrows a mutable reference to a dynamic field from the account.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no field with the specified `name` exists.
public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    ensure_tx_sender_is_smart_account(self, ctx);

    dynamic_field::borrow_mut(&mut self.id, name)
}

/// Replaces a dynamic field with a new value and returns the previous one.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no field with the specified `name` exists.
public fun rotate_field<Name: copy + drop + store, Value: store>(
    self: &mut SmartAccount,
    name: Name,
    value: Value,
    ctx: &TxContext,
): Value {
    ensure_tx_sender_is_smart_account(self, ctx);

    let account_id = &mut self.id;
    let previous_value = dynamic_field::remove<_, Value>(account_id, name);
    dynamic_field::add(account_id, name, value);
    previous_value
}

/// Replaces the existing built-in authenticator public key with `public_key`
/// and returns the previous key.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is currently attached.
public fun rotate_builtin_auth_public_key(
    self: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): PublicKey {
    ensure_tx_sender_is_smart_account(self, ctx);

    builtin_authenticator_functions::rotate_public_key(&mut self.id, public_key)
}

/// Rotates the attached authenticator.
///
/// Aborts if the transaction sender is not the account.
public fun rotate_auth_function_ref_v1(
    self: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    ensure_tx_sender_is_smart_account(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

// === Package Functions ===

// === Private Functions ===

/// Maps a `SignatureScheme` to the corresponding built-in `AuthenticatorFunctionRefV1`.
///
/// Aborts with `EInvalidSignatureScheme` for any scheme not supported by the built-in authenticators.
fun resolve_builtin_authenticator(
    signature_scheme: SignatureScheme,
): AuthenticatorFunctionRefV1<SmartAccount> {
    if (signature_scheme == signature_scheme::ed25519()) {
        builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<SmartAccount>()
    } else if (signature_scheme == signature_scheme::secp256k1()) {
        builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<SmartAccount>()
    } else if (signature_scheme == signature_scheme::secp256r1()) {
        builtin_authenticator_functions::secp256r1_authenticator_function_ref_v1<SmartAccount>()
    } else if (signature_scheme == signature_scheme::multisig()) {
        builtin_authenticator_functions::multisig_authenticator_function_ref_v1<SmartAccount>()
    } else if (signature_scheme == signature_scheme::passkey()) {
        builtin_authenticator_functions::passkey_authenticator_function_ref_v1<SmartAccount>()
    } else {
        abort EInvalidSignatureScheme
    }
}

/// Check that the sender of this transaction is the account itself.
fun ensure_tx_sender_is_smart_account(self: &SmartAccount, ctx: &TxContext) {
    assert!(self.account_address() == ctx.sender(), ETransactionSenderIsNotTheSmartAccount);
}

// === Test Functions ===
