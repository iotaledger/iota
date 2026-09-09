module iota::key_management;

use iota::dynamic_field as df;
use iota::event;
use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 10)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 11)]
const EPublicKeyAlreadyAttached: vector<u8> = b"Public key already attached.";

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

// === Structs ===

/// Dynamic field key, where the system will look for a potential public key.
public struct PublicKeyFieldName has copy, drop, store {}

// === Public Functions ===

/// Attaches `public_key` to the account. Aborts if a public key is already attached.
///
/// Call this before obtaining an authenticator function ref and passing it to
/// `account::create_account_v1`.
///
/// Emits a `PublicKeyAttached` event on success.
public fun attach_public_key(account_id: &mut UID, public_key: PublicKey) {
    attach_public_key_pkg(account_id, public_key);

    let event = PublicKeyAttached {
        account_id: account_id.to_inner(),
        public_key,
    };
    event::emit(event);
}

/// Detaches and returns the public key attached to the account. Aborts if no public key is
/// currently attached.
///
/// Use this when migrating away from a built-in authenticator to a custom one.
///
/// Emits a `PublicKeyDetached` event on success.
public fun detach_public_key(account_id: &mut UID): PublicKey {
    let public_key = detach_public_key_pkg(account_id);

    let event = PublicKeyDetached {
        account_id: account_id.to_inner(),
        public_key,
    };
    event::emit(event);

    public_key
}

/// Replaces the existing public key with `public_key` and returns the previous key.
/// Aborts if no public key is currently attached.
///
/// Call this before obtaining a new authenticator function ref and passing it to
/// `account::rotate_auth_function_ref_v1`.
///
/// Emits a `PublicKeyRotated` event on success.
public fun rotate_public_key(account_id: &mut UID, public_key: PublicKey): PublicKey {
    let prev_public_key = rotate_public_key_pkg(account_id, public_key);

    let event = PublicKeyRotated {
        account_id: account_id.to_inner(),
        from: prev_public_key,
        to: public_key,
    };
    event::emit(event);

    prev_public_key
}

// === View Functions ===

/// Returns true if the account has a public key attached.
public fun has_public_key(account_id: &UID): bool {
    df::exists_(account_id, public_key_field_name())
}

/// Borrows the public key attached to the account. Aborts if no public key is
/// currently attached.
public fun borrow_public_key(account_id: &UID): &PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    df::borrow(account_id, public_key_field_name())
}

// === Admin Functions ===

// === Package Functions ===
public(package) fun attach_public_key_pkg(account_id: &mut UID, public_key: PublicKey) {
    assert!(!has_public_key(account_id), EPublicKeyAlreadyAttached);

    df::add(account_id, public_key_field_name(), public_key);
}

public(package) fun detach_public_key_pkg(account_id: &mut UID): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    df::remove(account_id, public_key_field_name())
}

public(package) fun rotate_public_key_pkg(account_id: &mut UID, public_key: PublicKey): PublicKey {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let df_name = public_key_field_name();

    let prev_public_key = df::remove(account_id, df_name);
    df::add(account_id, df_name, public_key);

    prev_public_key
}

// === Private Functions ===

/// A utility function to construct the dynamic field name for the public key field.
fun public_key_field_name(): PublicKeyFieldName {
    PublicKeyFieldName {}
}
