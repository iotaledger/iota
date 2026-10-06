// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The public key of a `SmartAccount`: attach, detach and rotate it, for use by any authenticator.
///
/// The key is stored under `public_key::PublicKeyFieldName`, the same field `iota::public_key`
/// manages for every account type, so the built-in authenticator and custom authenticators read
/// the same key. This module does not look at the account's authenticator: detaching the key
/// while the built-in authenticator is attached leaves the account unable to send any
/// transaction.
///
/// A custom authenticator can check a signature against the attached key:
///
/// ```move
/// public fun authenticate(account: &SmartAccount, signature: vector<u8>, ...) {
///     let public_key = smart_account_public_key::borrow_public_key(account);
///     // verify `signature` with `public_key`
/// }
/// ```
module iota::smart_account_public_key;

use iota::public_key::{Self, PublicKey};
use iota::smart_account::{SmartAccount, SmartAccountBuilder};

// === Errors ===

#[error(code = 0)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 1)]
const EPublicKeyAlreadyAttached: vector<u8> = b"Public key already attached.";

// === SmartAccountBuilder Functions ===

/// Attaches `public_key` to the account being built.
///
/// Emits a `public_key::PublicKeyAttached` event on success.
///
/// Aborts if a public key is already attached.
public fun with_public_key(
    builder: SmartAccountBuilder,
    public_key: PublicKey,
): SmartAccountBuilder {
    public_key::emit_public_key_attached(
        object::id_from_address(builder.builder_account_address()),
        public_key,
    );
    builder.with_field(public_key::public_key_field_name(), public_key)
}

// === View Functions ===

/// Returns `true` if and only if the account has a public key attached.
public fun has_public_key(account: &SmartAccount): bool {
    account.has_field(public_key::public_key_field_name())
}

/// Borrows the public key attached to the account.
///
/// Aborts if no public key is attached.
public fun borrow_public_key(account: &SmartAccount): &PublicKey {
    assert!(has_public_key(account), EPublicKeyMissing);

    account.borrow_field(public_key::public_key_field_name())
}

// === Admin Functions ===

/// Attaches `public_key` to the account. The authenticator is unchanged.
///
/// Emits a `public_key::PublicKeyAttached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if a public key is already attached.
public fun attach_public_key(account: &mut SmartAccount, public_key: PublicKey, ctx: &TxContext) {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(!has_public_key(account), EPublicKeyAlreadyAttached);

    account.add_field(public_key::public_key_field_name(), public_key, ctx);
    public_key::emit_public_key_attached(object::id(account), public_key);
}

/// Detaches and returns the public key attached to the account. The authenticator is unchanged.
///
/// Detaching the key while the account's authenticator is the built-in one leaves the account
/// unable to send any transaction; rotate to another authenticator first.
///
/// Emits a `public_key::PublicKeyDetached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
public fun detach_public_key(account: &mut SmartAccount, ctx: &TxContext): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_public_key(account), EPublicKeyMissing);

    let public_key = account.remove_field(public_key::public_key_field_name(), ctx);
    public_key::emit_public_key_detached(object::id(account), public_key);
    public_key
}

/// Replaces the attached public key with `public_key`, of any supported scheme, and returns the
/// previous key. The authenticator is unchanged.
///
/// Emits a `public_key::PublicKeyRotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
public fun rotate_public_key(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_public_key(account), EPublicKeyMissing);

    let previous_public_key = account.rotate_field(
        public_key::public_key_field_name(),
        public_key,
        ctx,
    );
    public_key::emit_public_key_rotated(object::id(account), previous_public_key, public_key);
    previous_public_key
}
