// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Claim Registry for creating default IOTA accounts derived from public keys.
///
/// A default account (`IotaDefaultAccount`) can be created by any address by
/// calling one of the `claim_*` entry functions. The operation:
///   1. Verifies that the provided public key derives to `ctx.sender()`.
///   2. Ensures the address has not been claimed before (via `ClaimRegistry`).
///   3. Creates an `IotaDefaultAccount` object whose `ObjectID == sender address`.
///   4. Registers a Move-based authenticator that uses the same signature scheme
///      as the proof, so subsequent transactions can use `MoveAuthenticator`.
module iota::claim_registry;

use iota::account;
use iota::address as iota_address;
use iota::authenticator_function;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hash;
use iota::table::{Self, Table};
use std::ascii;

// === Signature scheme flags (match Rust `SignatureScheme`) ===

/// Ed25519 signature scheme flag.
const SCHEME_ED25519: u8 = 0x00;
/// Secp256k1 signature scheme flag.
const SCHEME_SECP256K1: u8 = 0x01;
/// Secp256r1 signature scheme flag.
const SCHEME_SECP256R1: u8 = 0x02;

/// Hash flag for SHA-256 used in secp256k1/secp256r1 verification.
/// Matches the `SHA256 = 1` constant in `ecdsa_k1` / `ecdsa_r1` Move modules.
const HASH_SHA256: u8 = 1;

/// Expected byte length of an Ed25519 public key.
const ED25519_PUBLIC_KEY_LEN: u64 = 32;
/// Expected byte length of a compressed Secp256k1 or Secp256r1 public key.
const COMPRESSED_PUBLIC_KEY_LEN: u64 = 33;

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

#[error(code = 1)]
const EAlreadyClaimed: vector<u8> =
    b"This address has already been claimed.";

#[error(code = 2)]
const ENotAccountOwner: vector<u8> =
    b"Transaction sender is not the account owner.";

#[error(code = 3)]
const EInvalidScheme: vector<u8> =
    b"Unknown or unsupported signature scheme.";

#[error(code = 4)]
const EAuthFailed: vector<u8> =
    b"Signature verification failed.";

#[error(code = 5)]
const EInvalidPublicKeyLength: vector<u8> =
    b"Public key has an incorrect length for the given signature scheme.";

#[error(code = 6)]
const ENotGenesis: vector<u8> =
    b"ClaimRegistry can only be created during genesis.";

// === Structs ===

/// Default IOTA account whose `ObjectID` equals the owner's on-chain address.
/// Created via one of the `claim_*` entry functions.
public struct IotaDefaultAccount has key {
    id: UID,
    /// Raw public key bytes of the account owner.
    public_key: vector<u8>,
    /// Signature scheme flag (SCHEME_ED25519 / SCHEME_SECP256K1 / SCHEME_SECP256R1).
    scheme: u8,
}

/// Singleton shared object that tracks which addresses have been claimed.
/// Prevents a second claim on the same address.
public struct ClaimRegistry has key {
    id: UID,
    /// Set of claimed addresses. The `bool` value is a dummy placeholder — only
    /// key existence matters.
    claimed_addresses: Table<address, bool>,
}

// === Genesis ===

/// Create and share the `ClaimRegistry` singleton.
/// Called exactly once during genesis from address @0x0.
#[allow(unused_function)]
fun create(ctx: &mut TxContext) {
    assert!(ctx.sender() == @0x0, ENotGenesis);
    transfer::share_object(ClaimRegistry {
        id: object::new_uid_from_hash(@0x11),
        claimed_addresses: table::new(ctx),
    });
}

// === Claim entry points ===

/// Claim the sender's address using an Ed25519 public key.
/// `public_key` must be the 32-byte Ed25519 public key whose address equals
/// `ctx.sender()`.
public entry fun claim_ed25519(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    claim_internal(registry, SCHEME_ED25519, public_key, ctx)
}

/// Claim the sender's address using a Secp256k1 public key.
/// `public_key` must be the 33-byte compressed Secp256k1 public key whose
/// address equals `ctx.sender()`.
public entry fun claim_secp256k1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    claim_internal(registry, SCHEME_SECP256K1, public_key, ctx)
}

/// Claim the sender's address using a Secp256r1 public key.
/// `public_key` must be the 33-byte compressed Secp256r1 public key whose
/// address equals `ctx.sender()`.
public entry fun claim_secp256r1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    claim_internal(registry, SCHEME_SECP256R1, public_key, ctx)
}

// === Key rotation ===

/// Rotate the stored public key and/or scheme of a default account.
///
/// The caller must be the account owner: `ctx.sender() == object_id(account)`.
/// This is guaranteed by the ObjectID == address invariant established at claim
/// time, so only the address holder can satisfy this check.
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
/// The VM calls this function when processing a `MoveAuthenticator` transaction
/// whose `object_to_authenticate` is an `IotaDefaultAccount`.
///
/// `signature` is provided by the user as `MoveAuthenticatorV1.call_args[0]`.
/// It must be a valid signature over `auth_ctx.digest()` using the key stored
/// in the account:
///   - Ed25519:   64-byte signature, signed directly over `digest`.
///   - Secp256k1: 64-byte (r,s) signature, signed over `SHA256(digest)`.
///   - Secp256r1: 64-byte (r,s) signature, signed over `SHA256(digest)`.
#[authenticator]
public fun authenticate(
    account: &IotaDefaultAccount,
    signature: vector<u8>,
    _auth_ctx: &iota::auth_context::AuthContext,
    ctx: &TxContext,
) {
    let digest = ctx.digest();
    let pubkey = &account.public_key;
    let scheme = account.scheme;

    let ok = if (scheme == SCHEME_ED25519) {
        ed25519::ed25519_verify(&signature, pubkey, digest)
    } else if (scheme == SCHEME_SECP256K1) {
        ecdsa_k1::secp256k1_verify(&signature, pubkey, digest, HASH_SHA256)
    } else if (scheme == SCHEME_SECP256R1) {
        ecdsa_r1::secp256r1_verify(&signature, pubkey, digest, HASH_SHA256)
    } else {
        false
    };

    assert!(ok, EAuthFailed);
}

// === Public reads ===

/// Return a reference to the stored public key bytes.
public fun public_key(account: &IotaDefaultAccount): &vector<u8> {
    &account.public_key
}

/// Return the stored signature scheme flag.
public fun scheme(account: &IotaDefaultAccount): u8 {
    account.scheme
}

/// Return `true` if the given address has already been claimed.
public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    registry.claimed_addresses.contains(addr)
}

// === Internal helpers ===

fun claim_internal(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: vector<u8>,
    ctx: &TxContext,
) {
    // 1. Guard against future callers passing an unsupported scheme.
    //    Currently unreachable via the public entry points, but prevents a
    //    silently-broken account if a new entry point is added incorrectly.
    assert!(is_valid_scheme(scheme), EInvalidScheme);

    // 2. Validate the public key length for the given scheme.
    assert!(is_valid_public_key_length(scheme, &public_key), EInvalidPublicKeyLength);

    // 3. Verify that the public key derives to the transaction sender.
    //    The transaction itself is already signed with the corresponding
    //    private key, so this implicitly proves ownership.
    let derived = derive_address(scheme, &public_key);
    assert!(derived == ctx.sender(), EAddressMismatch);

    // 2. Prevent duplicate claims.
    assert!(!registry.claimed_addresses.contains(ctx.sender()), EAlreadyClaimed);
    registry.claimed_addresses.add(ctx.sender(), true);

    // 3. Build the authenticator reference pointing to `claim_registry::authenticate`.
    let auth_ref = make_auth_ref();

    // 4. Create IotaDefaultAccount with ObjectID == sender address.
    let account_obj = IotaDefaultAccount {
        id: object::new_uid_from_hash(ctx.sender()),
        public_key,
        scheme,
    };

    // 5. Register as a shared mutable account with the Move-based authenticator.
    //    The bytecode verifier enforces that IotaDefaultAccount is defined in
    //    this module, which satisfies the create_account_v1 constraint.
    account::create_account_v1(account_obj, auth_ref);
}

/// Build an `AuthenticatorFunctionRefV1` pointing to `claim_registry::authenticate`
/// in the iota-framework package (@0x2).
fun make_auth_ref(): authenticator_function::AuthenticatorFunctionRefV1<IotaDefaultAccount> {
    authenticator_function::create_for_framework<IotaDefaultAccount>(
        ascii::string(b"claim_registry"),
        ascii::string(b"authenticate"),
    )
}

/// Compute the IOTA address for the given signature scheme and public key.
/// Mirrors the Rust `IotaAddress::from(&PublicKey)` / `SignatureScheme::update_hasher_with_flag`:
///   - Ed25519:   Blake2b256(pubkey_bytes)           — NO flag prefix (special case)
///   - Secp256k1: Blake2b256([0x01] || pubkey_bytes)
///   - Secp256r1: Blake2b256([0x02] || pubkey_bytes)
fun derive_address(scheme: u8, public_key: &vector<u8>): address {
    let data = if (scheme == SCHEME_ED25519) {
        *public_key
    } else {
        let mut v = vector[scheme];
        v.append(*public_key);
        v
    };
    iota_address::from_bytes(hash::blake2b256(&data))
}

/// Assert that the transaction sender equals the account's object address.
/// Valid because ObjectID == address is the invariant established at claim time.
fun ensure_tx_sender_is_account(account: &IotaDefaultAccount, ctx: &TxContext) {
    assert!(ctx.sender() == object::id_address(account), ENotAccountOwner);
}

fun is_valid_scheme(scheme: u8): bool {
    scheme == SCHEME_ED25519 || scheme == SCHEME_SECP256K1 || scheme == SCHEME_SECP256R1
}

fun is_valid_public_key_length(scheme: u8, public_key: &vector<u8>): bool {
    let len = public_key.length();
    if (scheme == SCHEME_ED25519) {
        len == ED25519_PUBLIC_KEY_LEN
    } else {
        len == COMPRESSED_PUBLIC_KEY_LEN
    }
}

// === Test only ===

#[test_only]
public fun create_for_testing(ctx: &mut TxContext) {
    create(ctx);
}

#[test_only]
public fun scheme_ed25519(): u8 { SCHEME_ED25519 }

#[test_only]
public fun scheme_secp256k1(): u8 { SCHEME_SECP256K1 }

#[test_only]
public fun scheme_secp256r1(): u8 { SCHEME_SECP256R1 }

#[test_only]
/// Expose address derivation for tests that need to compute the correct sender address.
public fun derive_address_for_testing(scheme: u8, public_key: &vector<u8>): address {
    derive_address(scheme, public_key)
}
