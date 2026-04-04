// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Claim Registry — on-chain registry for creating default IOTA accounts.
///
/// A default account (`IotaDefaultAccount`) can be claimed by any address by
/// calling one of the `claim_*` entry functions or `claim_with_auth`. The
/// operation:
///   1. Validates the provided public key (scheme, length, address derivation).
///   2. Ensures the address has not been claimed before.
///   3. Creates an `IotaDefaultAccount` with `ObjectID == sender address`.
///   4. Attaches an authenticator so subsequent transactions can use
///      `MoveAuthenticator` with that account.
module iota::claim_registry;

use iota::address as iota_address;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field as df;
use iota::hash;
use iota::iota_default_account::{Self, IotaDefaultAccount};

// Scheme flags and key-length validation are defined in `iota_default_account`
// and delegated to it via `public(package)` helpers to avoid duplication.

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

#[error(code = 1)]
const EAlreadyClaimed: vector<u8> =
    b"This address has already been claimed.";

#[error(code = 2)]
const EInvalidScheme: vector<u8> =
    b"Unknown or unsupported signature scheme.";

#[error(code = 3)]
const EInvalidPublicKeyLength: vector<u8> =
    b"Public key has an incorrect length for the given signature scheme.";

#[error(code = 4)]
const ENotGenesis: vector<u8> =
    b"ClaimRegistry can only be created during genesis.";

// === Struct ===

/// Singleton shared object that tracks which addresses have been claimed.
/// Each claimed address is stored as a dynamic field on this object's UID,
/// which allows full nodes to index and query claimed addresses efficiently.
public struct ClaimRegistry has key {
    id: UID,
}

// === Genesis ===

/// Create and share the `ClaimRegistry` singleton.
/// Called exactly once during genesis from address @0x0.
#[allow(unused_function)]
fun create(ctx: &TxContext) {
    assert!(ctx.sender() == @0x0, ENotGenesis);
    transfer::share_object(ClaimRegistry {
        id: object::new_uid_from_hash(@0x11),
    });
}

// === Claim entry points (built-in authenticator) ===

/// Claim the sender's address using an Ed25519 public key.
/// `public_key` must be the 32-byte Ed25519 public key whose derived address
/// equals `ctx.sender()`. Creates an `IotaDefaultAccount` with the built-in
/// per-scheme authenticator.
public entry fun claim_ed25519(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_ed25519_flag(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_ed25519_flag());
}

/// Claim the sender's address using a Secp256k1 public key.
/// `public_key` must be the 33-byte compressed Secp256k1 public key whose
/// derived address equals `ctx.sender()`.
public entry fun claim_secp256k1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_secp256k1_flag(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_secp256k1_flag());
}

/// Claim the sender's address using a Secp256r1 public key.
/// `public_key` must be the 33-byte compressed Secp256r1 public key whose
/// derived address equals `ctx.sender()`.
public entry fun claim_secp256r1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_secp256r1_flag(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_secp256r1_flag());
}

// === Claim with custom authenticator ===

/// Claim the sender's address and attach a caller-supplied Move-based
/// authenticator instead of the default per-scheme signature verifier.
///
/// `scheme` may be SCHEME_ED25519 (0x00), SCHEME_SECP256K1 (0x01),
/// SCHEME_SECP256R1 (0x02), or SCHEME_MOVE_AUTHENTICATOR (0x07).
///
/// For the cryptographic schemes (Ed25519 / Secp256k1 / Secp256r1), `public_key`
/// must have the correct length and derive to `ctx.sender()`.
///
/// For SCHEME_MOVE_AUTHENTICATOR (0x07), pass this sentinel to indicate that the account
/// relies entirely on the caller-supplied `auth_ref` for authentication. The
/// `public_key` may be empty or carry arbitrary data defined by the custom
/// authenticator; the built-in key-to-address derivation check is skipped.
/// For now this is a sentinel value — a richer representation will be added
/// in the future.
///
/// `auth_ref` must point to a function whose first parameter is
/// `&IotaDefaultAccount`. This enables attaching any Move-based authentication
/// logic — multisig, hardware key abstraction, custom policies — in place of
/// the built-in signature verifier.
///
/// Example PTB usage:
/// ```
/// let auth_ref = my_package::my_auth::make_ref();
/// claim_registry::claim_with_auth(registry, 0x07, vector[], auth_ref);
/// ```
public fun claim_with_auth(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: vector<u8>,
    auth_ref: AuthenticatorFunctionRefV1<IotaDefaultAccount>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, scheme, &public_key, ctx);
    iota_default_account::new_with_auth(ctx.sender(), public_key, scheme, auth_ref);
}

// === Public reads ===

/// Return `true` if the given address has already been claimed.
public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Internal helpers ===

/// Validate the claim parameters and mark the sender's address in the registry.
/// Must be called before `iota_default_account::new` or `new_with_auth`.
fun validate_and_mark_claimed(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: &vector<u8>,
    ctx: &TxContext,
) {
    // Guard against unsupported schemes — prevents silently-broken accounts
    // if a new entry point is added that passes an invalid scheme.
    assert!(iota_default_account::is_valid_scheme(scheme), EInvalidScheme);

    assert!(iota_default_account::is_valid_public_key_length(scheme, public_key), EInvalidPublicKeyLength);

    // For cryptographic schemes, verify that the provided public key derives to
    // the transaction sender. The transaction signature already proved the sender
    // knows the private key, so this check binds the stored pubkey to the address.
    //
    // For SCHEME_MOVE_AUTHENTICATOR the derivation is skipped: the custom
    // authenticator is responsible for key-to-address binding, and the
    // transaction signature on the protocol level already proves ownership of
    // ctx.sender().
    if (!iota_default_account::is_move_authenticator_scheme(scheme)) {
        let derived = derive_address(scheme, public_key);
        assert!(derived == ctx.sender(), EAddressMismatch);
    };

    assert!(!df::exists_(&registry.id, ctx.sender()), EAlreadyClaimed);
    df::add(&mut registry.id, ctx.sender(), true);
}

/// Compute the IOTA address for the given signature scheme and public key.
/// Mirrors the Rust `IotaAddress::from(&PublicKey)` /
/// `SignatureScheme::update_hasher_with_flag`:
///   - Ed25519:   Blake2b256(pubkey_bytes)           — NO flag prefix (special case)
///   - Secp256k1: Blake2b256([0x01] || pubkey_bytes)
///   - Secp256r1: Blake2b256([0x02] || pubkey_bytes)
fun derive_address(scheme: u8, public_key: &vector<u8>): address {
    let data = if (scheme == iota_default_account::scheme_ed25519_flag()) {
        *public_key
    } else {
        let mut v = vector[scheme];
        v.append(*public_key);
        v
    };
    iota_address::from_bytes(hash::blake2b256(&data))
}

// === Test only ===

#[test_only]
public fun create_for_testing(ctx: &mut TxContext) {
    create(ctx);
}

#[test_only]
/// Expose address derivation for tests that need to compute the correct sender
/// address before calling a claim function.
public fun derive_address_for_testing(scheme: u8, public_key: &vector<u8>): address {
    derive_address(scheme, public_key)
}
