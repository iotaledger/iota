// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// On-chain registry for claiming default IOTA accounts.
/// Each `claim_*` call validates the public key, ensures no duplicate claim,
/// and creates an `IotaDefaultAccount` with `ObjectID == sender address`.
module iota::claim_registry;

use iota::address as iota_address;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field as df;
use iota::hash;
use iota::iota_default_account::{Self, IotaDefaultAccount};

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

/// Singleton shared object tracking claimed addresses via dynamic fields.
public struct ClaimRegistry has key {
    id: UID,
}

// === Genesis ===

/// Create and share the `ClaimRegistry` singleton. Called once during genesis.
#[allow(unused_function)]
fun create(ctx: &TxContext) {
    assert!(ctx.sender() == @0x0, ENotGenesis);
    transfer::share_object(ClaimRegistry {
        id: object::new_uid_from_hash(@0x11),
    });
}

// === Claim entry points (built-in authenticator) ===

/// Claim with a 32-byte Ed25519 public key. Derived address must equal `ctx.sender()`.
public entry fun claim_ed25519(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_ed25519(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_ed25519());
}

/// Claim with a 33-byte compressed Secp256k1 public key. Derived address must equal `ctx.sender()`.
public entry fun claim_secp256k1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_secp256k1(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_secp256k1());
}

/// Claim with a 33-byte compressed Secp256r1 public key. Derived address must equal `ctx.sender()`.
public entry fun claim_secp256r1(
    registry: &mut ClaimRegistry,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    validate_and_mark_claimed(registry, iota_default_account::scheme_secp256r1(), &public_key, ctx);
    iota_default_account::new(ctx.sender(), public_key, iota_default_account::scheme_secp256r1());
}

// === Claim with custom authenticator ===

/// Claim and attach a caller-supplied Move-based authenticator.
///
/// `scheme`: one of 0x00 / 0x01 / 0x02 / 0x07 (SCHEME_MOVE_AUTHENTICATOR).
/// For crypto schemes, `public_key` must have correct length and derive to `ctx.sender()`.
/// For 0x07, address derivation is skipped; `public_key` may be empty or carry
/// arbitrary data meaningful to the custom authenticator.
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

public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Internal helpers ===

fun validate_and_mark_claimed(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: &vector<u8>,
    ctx: &TxContext,
) {
    assert!(iota_default_account::is_valid_scheme(scheme), EInvalidScheme);
    assert!(iota_default_account::is_valid_public_key_length(scheme, public_key), EInvalidPublicKeyLength);
    // For crypto schemes, verify the public key derives to the transaction sender.
    // For SCHEME_MOVE_AUTHENTICATOR, this check is skipped — the custom auth is
    // responsible for key binding; the tx signature already proves sender ownership.
    // DISCUSS: should we require an explicit address derivation rule for 0x07 too,
    // or is relying on ctx.sender() sufficient as the ownership proof?
    if (!iota_default_account::is_move_authenticator_scheme(scheme)) {
        let derived = derive_address(scheme, public_key);
        assert!(derived == ctx.sender(), EAddressMismatch);
    };
    assert!(!df::exists_(&registry.id, ctx.sender()), EAlreadyClaimed);
    df::add(&mut registry.id, ctx.sender(), true);
}

/// Mirrors Rust `IotaAddress::from(&PublicKey)`:
///   Ed25519:   Blake2b256(pubkey)          — no flag prefix
///   Secp256k1: Blake2b256([0x01] || pubkey)
///   Secp256r1: Blake2b256([0x02] || pubkey)
fun derive_address(scheme: u8, public_key: &vector<u8>): address {
    let data = if (scheme == iota_default_account::scheme_ed25519()) {
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
public fun derive_address_for_testing(scheme: u8, public_key: &vector<u8>): address {
    derive_address(scheme, public_key)
}
