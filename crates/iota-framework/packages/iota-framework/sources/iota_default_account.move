// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Default IOTA account whose `ObjectID` equals the owner's address.
/// Created exclusively through `iota::claim_registry`, which enforces
/// ownership proof and prevents duplicate claims.
module iota::iota_default_account;

use iota::account;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use std::ascii;

// === Signature scheme flags (match Rust `SignatureScheme`) ===

const SCHEME_ED25519: u8 = 0x00;
const SCHEME_SECP256K1: u8 = 0x01;
const SCHEME_SECP256R1: u8 = 0x02;
/// Matches `SignatureScheme::MoveAuthenticator` (0x07). The built-in
/// `authenticate` always rejects this scheme; only the attached `auth_ref` is invoked.
const SCHEME_MOVE_AUTHENTICATOR: u8 = 0x07;

/// SHA-256 hash flag for secp256k1/secp256r1 verification.
const HASH_SHA256: u8 = 1;

const ED25519_PUBLIC_KEY_LEN: u64 = 32;
const COMPRESSED_PUBLIC_KEY_LEN: u64 = 33;

// === Errors ===

#[error(code = 0)]
const ENotAccountOwner: vector<u8> =
    b"Transaction sender is not the account owner.";

#[error(code = 1)]
const EAuthFailed: vector<u8> =
    b"Signature verification failed.";

#[error(code = 2)]
const EInvalidScheme: vector<u8> =
    b"Unknown or unsupported signature scheme.";

#[error(code = 3)]
const EInvalidPublicKeyLength: vector<u8> =
    b"Public key has an incorrect length for the given signature scheme.";

#[error(code = 4)]
const ECannotRotateCustomAuth: vector<u8> =
    b"Use rotate_auth_ref to update the authenticator of a MoveAuthenticator account.";

// === Struct ===

public struct IotaDefaultAccount has key {
    id: UID,
    /// Raw public key bytes (or arbitrary data for SCHEME_MOVE_AUTHENTICATOR).
    public_key: vector<u8>,
    /// One of SCHEME_ED25519 / SCHEME_SECP256K1 / SCHEME_SECP256R1 / SCHEME_MOVE_AUTHENTICATOR.
    scheme: u8,
}

// === Package-internal constructors ===
//
// `public(package)` ensures accounts are only created through `claim_registry`,
// which validates ownership and prevents bypassing the duplicate-claim check.

/// Create and share an `IotaDefaultAccount` with the built-in authenticator.
public(package) fun new(addr: address, public_key: vector<u8>, scheme: u8) {
    let uid = object::new_uid_from_hash(addr);
    account::create_account_v1(
        IotaDefaultAccount { id: uid, public_key, scheme },
        make_auth_ref(),
    );
}

/// Create and share an `IotaDefaultAccount` with a caller-supplied authenticator.
/// Use `SCHEME_MOVE_AUTHENTICATOR` (0x07) as `scheme`; `public_key` may be empty
/// or carry arbitrary data defined by the custom authenticator.
public(package) fun new_with_auth(
    addr: address,
    public_key: vector<u8>,
    scheme: u8,
    auth_ref: AuthenticatorFunctionRefV1<IotaDefaultAccount>,
) {
    let uid = object::new_uid_from_hash(addr);
    account::create_account_v1(
        IotaDefaultAccount { id: uid, public_key, scheme },
        auth_ref,
    );
}

// === Key rotation ===

/// Rotate the public key and scheme of a built-in-auth account.
/// Only the account owner can call this (`ctx.sender() == object_id(account)`).
/// For MOVE_AUTHENTICATOR accounts use `rotate_auth_ref` instead.
///
/// TODO: rotation should require a proof-of-ownership signature from the new key
/// (iotaledger/iota#11039).
public fun rotate_key(
    account: &mut IotaDefaultAccount,
    new_public_key: vector<u8>,
    new_scheme: u8,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(account, ctx);
    assert!(is_valid_scheme(new_scheme), EInvalidScheme);
    // Reject 0x07: the built-in verifier always fails for MOVE_AUTHENTICATOR.
    assert!(!is_move_authenticator_scheme(new_scheme), ECannotRotateCustomAuth);
    assert!(is_valid_public_key_length(new_scheme, &new_public_key), EInvalidPublicKeyLength);
    account.public_key = new_public_key;
    account.scheme = new_scheme;
    let new_auth = make_auth_ref();
    account::rotate_auth_function_ref_v1(account, new_auth);
}

/// Attach or replace a custom authenticator. Sets scheme to SCHEME_MOVE_AUTHENTICATOR (0x07).
public fun rotate_auth_ref(
    account: &mut IotaDefaultAccount,
    new_public_key: vector<u8>,
    new_auth_ref: AuthenticatorFunctionRefV1<IotaDefaultAccount>,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(account, ctx);
    account.scheme = SCHEME_MOVE_AUTHENTICATOR;
    account.public_key = new_public_key;
    account::rotate_auth_function_ref_v1(account, new_auth_ref);
}

// === Authenticate (Move-based authenticator) ===

/// Built-in authenticator invoked by the VM for MoveAuthenticator transactions.
/// TODO: switch to `auth_ctx.signing_digest()` once available (iotaledger/iota#11039)
/// so the digest matches generic signature variants and clients need no changes.
#[authenticator]
public fun authenticate(
    account: &IotaDefaultAccount,
    signature: vector<u8>,
    _auth_ctx: &iota::auth_context::AuthContext,
    ctx: &TxContext,
) {
    let digest = ctx.digest();
    let pubkey = &account.public_key;
    let ok = if (account.scheme == SCHEME_ED25519) {
        ed25519::ed25519_verify(&signature, pubkey, digest)
    } else if (account.scheme == SCHEME_SECP256K1) {
        ecdsa_k1::secp256k1_verify(&signature, pubkey, digest, HASH_SHA256)
    } else if (account.scheme == SCHEME_SECP256R1) {
        ecdsa_r1::secp256r1_verify(&signature, pubkey, digest, HASH_SHA256)
    } else {
        false
    };
    assert!(ok, EAuthFailed);
}

// === Public reads ===

public fun public_key(account: &IotaDefaultAccount): &vector<u8> {
    &account.public_key
}

public fun scheme(account: &IotaDefaultAccount): u8 {
    account.scheme
}

public(package) fun scheme_ed25519(): u8 { SCHEME_ED25519 }
public(package) fun scheme_secp256k1(): u8 { SCHEME_SECP256K1 }
public(package) fun scheme_secp256r1(): u8 { SCHEME_SECP256R1 }
public(package) fun scheme_move_authenticator(): u8 { SCHEME_MOVE_AUTHENTICATOR }

// === Internal helpers ===

fun make_auth_ref(): AuthenticatorFunctionRefV1<IotaDefaultAccount> {
    authenticator_function::create_for_framework<IotaDefaultAccount>(
        ascii::string(b"iota_default_account"),
        ascii::string(b"authenticate"),
    )
}

fun ensure_tx_sender_is_account(account: &IotaDefaultAccount, ctx: &TxContext) {
    assert!(ctx.sender() == object::id_address(account), ENotAccountOwner);
}

public(package) fun is_move_authenticator_scheme(scheme: u8): bool {
    scheme == SCHEME_MOVE_AUTHENTICATOR
}

public(package) fun is_valid_scheme(scheme: u8): bool {
    scheme == SCHEME_ED25519
        || scheme == SCHEME_SECP256K1
        || scheme == SCHEME_SECP256R1
        || scheme == SCHEME_MOVE_AUTHENTICATOR
}

public(package) fun is_valid_public_key_length(scheme: u8, public_key: &vector<u8>): bool {
    if (scheme == SCHEME_MOVE_AUTHENTICATOR) {
        true // custom auth: public_key may be empty or carry arbitrary data
    } else {
        let len = public_key.length();
        if (scheme == SCHEME_ED25519) {
            len == ED25519_PUBLIC_KEY_LEN
        } else {
            len == COMPRESSED_PUBLIC_KEY_LEN
        }
    }
}
