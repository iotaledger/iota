// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The public key of a `SmartAccount`: attach, detach and rotate it, for use by any authenticator.
///
/// The key is stored in the same dynamic field that `builtin_authenticator_functions` manages for
/// every account type, so the built-in authenticator and custom authenticators read the same key.
/// This module does not look at the account's authenticator: detaching the key while the
/// built-in authenticator is attached leaves the account unable to send any transaction.
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

use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::smart_account::{SmartAccount, SmartAccountBuilder};

// === SmartAccountBuilder Functions ===

/// Attaches `public_key` to the account being built.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event on success.
///
/// Aborts if a public key is already attached.
public fun with_public_key(
    mut builder: SmartAccountBuilder,
    public_key: PublicKey,
): SmartAccountBuilder {
    builtin_authenticator_functions::attach_public_key(builder.builder_uid_mut(), public_key);
    builder
}

// === View Functions ===

/// Returns `true` if and only if the account has a public key attached.
public fun has_public_key(account: &SmartAccount): bool {
    builtin_authenticator_functions::has_public_key(account.uid())
}

/// Borrows the public key attached to the account.
///
/// Aborts if no public key is attached.
public fun borrow_public_key(account: &SmartAccount): &PublicKey {
    builtin_authenticator_functions::borrow_public_key(account.uid())
}

// === Admin Functions ===

/// Attaches `public_key` to the account. The authenticator is unchanged.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if a public key is already attached.
public fun attach_public_key(account: &mut SmartAccount, public_key: PublicKey, ctx: &TxContext) {
    account.ensure_tx_sender_is_smart_account(ctx);

    builtin_authenticator_functions::attach_public_key(account.uid_mut(), public_key);
}

/// Detaches and returns the public key attached to the account. The authenticator is unchanged.
///
/// Detaching the key while the account's authenticator is the built-in one leaves the account
/// unable to send any transaction; rotate to another authenticator first.
///
/// Emits a `builtin_authenticator_functions::PublicKeyDetached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
public fun detach_public_key(account: &mut SmartAccount, ctx: &TxContext): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);

    builtin_authenticator_functions::detach_public_key(account.uid_mut())
}

/// Replaces the attached public key with `public_key`, of any supported scheme, and returns the
/// previous key. The authenticator is unchanged.
///
/// Emits a `builtin_authenticator_functions::PublicKeyRotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
public fun rotate_public_key(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);

    builtin_authenticator_functions::rotate_public_key(account.uid_mut(), public_key)
}
