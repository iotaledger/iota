// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Default IOTA account (`IotaDefaultAccount`) tied to an on-chain address.
///
/// An `IotaDefaultAccount` is always created through `iota::claim_registry`
/// which enforces ownership proof and duplicate-claim prevention. Once
/// created it supports Move-based authentication using the key stored in
/// the account, or a custom authenticator supplied at creation time via
/// `iota::claim_registry::claim_with_auth`.
///
/// # Object identity
///
/// The `ObjectID` of every `IotaDefaultAccount` equals the owner's address,
/// which is `Blake2b256([scheme_flag?] || public_key_bytes)` (see
/// `iota::claim_registry::derive_address_for_testing` for the exact rule).
/// This makes accounts directly addressable without an external lookup.
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

/// Scheme flag for accounts that use a caller-supplied Move-based authenticator
/// instead of the built-in per-scheme signature verifier.
///
/// Matches `SignatureScheme::MoveAuthenticator` (0x07) on the Rust side.
/// The built-in `authenticate` function will always reject such accounts with
/// `EAuthFailed`, so only the attached `auth_ref` function will be invoked by
/// the VM.
const SCHEME_MOVE_AUTHENTICATOR: u8 = 0x07;

/// SHA-256 hash flag for secp256k1/secp256r1 verification.
const HASH_SHA256: u8 = 1;

/// Expected byte length of an Ed25519 public key.
const ED25519_PUBLIC_KEY_LEN: u64 = 32;
/// Expected byte length of a compressed Secp256k1 or Secp256r1 public key.
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

// === Struct ===

/// Default IOTA account whose `ObjectID` equals the owner's on-chain address.
///
/// Created exclusively through `iota::claim_registry` to ensure that:
///   - the caller proved ownership of the address before the account was created,
///   - no duplicate accounts exist for the same address.
public struct IotaDefaultAccount has key {
    id: UID,
    /// Raw public key bytes of the account owner.
    public_key: vector<u8>,
    /// Signature scheme flag: SCHEME_ED25519 / SCHEME_SECP256K1 / SCHEME_SECP256R1,
    /// or SCHEME_MOVE_AUTHENTICATOR for accounts that use a caller-supplied Move-based authenticator.
    scheme: u8,
}

// === Package-internal constructors ===
//
// These are intentionally `public(package)` rather than `public` so that
// `IotaDefaultAccount` objects can only be created through `claim_registry`,
// which validates ownership and prevents bypassing the duplicate-claim check.

/// Create and share an `IotaDefaultAccount` with the built-in authenticator
/// (`iota_default_account::authenticate`).
///
/// `address` must equal `ctx.sender()` as validated by `claim_registry`
/// before this call.  The UID is created here — not passed in — so that the
/// Move bytecode verifier's "fresh UID" rule is satisfied.
public(package) fun new(addr: address, public_key: vector<u8>, scheme: u8) {
    let uid = object::new_uid_from_hash(addr);
    account::create_account_v1(
        IotaDefaultAccount { id: uid, public_key, scheme },
        make_auth_ref(),
    );
}

/// Create and share an `IotaDefaultAccount` with a caller-supplied
/// Move-based authenticator.
///
/// `auth_ref` must point to a function whose first parameter is
/// `&IotaDefaultAccount`. This enables attaching any Move-based
/// authentication logic — multisig, hardware key abstraction, custom
/// policies — instead of the built-in per-scheme signature verifier.
///
/// `address` must equal `ctx.sender()` as validated by `claim_registry`
/// before this call.
///
/// Pass `SCHEME_MOVE_AUTHENTICATOR` (0x07) as `scheme` to mark the account as using a
/// custom authenticator. For now this is a sentinel value; the `public_key`
/// field may carry arbitrary data defined by the custom auth or be empty.
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

/// Rotate the stored public key and/or scheme of a default account.
///
/// The caller must be the account owner: `ctx.sender() == object_id(account)`.
/// This is guaranteed by the ObjectID == address invariant established at claim
/// time, so only the address holder can satisfy this check.
///
/// TODO: rotating to a new pubkey shall be protected, i.e. proof of ownership
/// shall be checked. During the initial claim ctx.sender check satisfies this,
/// but for any subsequent rotation we need a cryptographic signature over some
/// rotation action-specific data from the new pubkey.
public fun rotate_key(
    account: &mut IotaDefaultAccount,
    new_public_key: vector<u8>,
    new_scheme: u8,
    ctx: &TxContext,
) {
    ensure_tx_sender_is_account(account, ctx);
    assert!(is_valid_scheme(new_scheme), EInvalidScheme);
    assert!(is_valid_public_key_length(new_scheme, &new_public_key), EInvalidPublicKeyLength);
    account.public_key = new_public_key;
    account.scheme = new_scheme;
    let new_auth = make_auth_ref();
    account::rotate_auth_function_ref_v1(account, new_auth);
}

// === Authenticate (Move-based authenticator) ===

/// Authenticate a `MoveAuthenticator` transaction for an `IotaDefaultAccount`.
///
/// The VM invokes this function when executing a `MoveAuthenticator`
/// transaction whose `object_to_authenticate` is an `IotaDefaultAccount`.
/// TODO: Switch to `auth_ctx.signing_digest()` once AuthContext exposes it
/// (tracked in iotaledger/iota#11039). The signing digest must match the one
/// used by the generic signature variants already present in the protocol
/// (`Blake2b256(BCS(IntentMessage(TransactionData)))`), so that clients do not
/// need to change their signing flows when using a MoveAuthenticator account.
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

public(package) fun scheme_ed25519_flag(): u8 { SCHEME_ED25519 }
public(package) fun scheme_secp256k1_flag(): u8 { SCHEME_SECP256K1 }
public(package) fun scheme_secp256r1_flag(): u8 { SCHEME_SECP256R1 }

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

// === Test only ===

#[test_only]
public fun scheme_ed25519(): u8 { SCHEME_ED25519 }

#[test_only]
public fun scheme_secp256k1(): u8 { SCHEME_SECP256K1 }

#[test_only]
public fun scheme_secp256r1(): u8 { SCHEME_SECP256R1 }

#[test_only]
public fun scheme_move_authenticator(): u8 { SCHEME_MOVE_AUTHENTICATOR }
