// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The public key an account authenticates with: attach, detach and rotate it on any account's
/// `UID`.
///
/// An account holds at most one such key, in a dynamic field that only this module can name. The
/// built-in authenticator (`builtin_authenticator_functions`) and custom authenticators read it
/// with `borrow_public_key`.
///
/// ```move
/// public_key_authentication::attach_public_key(
///     &mut account.id,
///     public_key::create(scheme, raw_pk_bytes),
/// );
/// ```
module iota::public_key_authentication;

use iota::dynamic_field;
use iota::event;
use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 0)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 1)]
const EPublicKeyAlreadyAttached: vector<u8> = b"Public key already attached.";

// === Structs ===

/// Dynamic field key of the public key attached to an account.
public struct PublicKeyFieldName has copy, drop, store {}

// === Events ===

/// Event: emitted when a public key is attached to an account.
public struct PublicKeyAttached has copy, drop {
    account_id: ID,
    public_key: PublicKey,
}

/// Event: emitted when a public key is detached from an account.
public struct PublicKeyDetached has copy, drop {
    account_id: ID,
    public_key: PublicKey,
}

/// Event: emitted when a public key is rotated on an account.
public struct PublicKeyRotated has copy, drop {
    account_id: ID,
    from: PublicKey,
    to: PublicKey,
}

// === Public Functions ===

/// Attaches `public_key` to the account.
///
/// Emits a `PublicKeyAttached` event on success.
///
/// Aborts if a public key is already attached.
public fun attach_public_key(account_id: &mut UID, public_key: PublicKey) {
    assert!(!has_public_key(account_id), EPublicKeyAlreadyAttached);

    dynamic_field::add(account_id, PublicKeyFieldName {}, public_key);
    event::emit(PublicKeyAttached { account_id: account_id.to_inner(), public_key });
}

/// Detaches and returns the public key attached to the account.
///
/// Emits a `PublicKeyDetached` event on success.
///
/// Aborts if no public key is attached.
public fun detach_public_key(account_id: &mut UID): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let public_key = dynamic_field::remove(account_id, PublicKeyFieldName {});
    event::emit(PublicKeyDetached { account_id: account_id.to_inner(), public_key });
    public_key
}

/// Replaces the attached public key with `public_key`, of any supported scheme, and returns the
/// previous key.
///
/// Emits a `PublicKeyRotated` event on success.
///
/// Aborts if no public key is attached.
public fun rotate_public_key(account_id: &mut UID, public_key: PublicKey): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let previous_public_key = dynamic_field::remove(account_id, PublicKeyFieldName {});
    dynamic_field::add(account_id, PublicKeyFieldName {}, public_key);
    event::emit(PublicKeyRotated {
        account_id: account_id.to_inner(),
        from: previous_public_key,
        to: public_key,
    });
    previous_public_key
}

// === View Functions ===

/// Returns `true` if and only if the account has a public key attached.
public fun has_public_key(account_id: &UID): bool {
    dynamic_field::exists_(account_id, PublicKeyFieldName {})
}

/// Borrows the public key attached to the account.
///
/// Aborts if no public key is attached.
public fun borrow_public_key(account_id: &UID): &PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    dynamic_field::borrow(account_id, PublicKeyFieldName {})
}
