// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// On-chain registry for claiming addresses.
///
/// `claim` validates the public key (scheme, length, address derivation),
/// marks the sender's address as claimed, and returns a deterministic `UID`
/// for the new account object.
module iota::claim_registry;

use iota::address as iota_address;
use iota::dynamic_field as df;
use iota::hash;

// === Signature scheme flags (match Rust `SignatureScheme`) ===

// 0x04 BLS12381 — not supported for user addresses.
// 0x05 ZkLoginAuthenticator — requires special address derivation; not yet supported.
const SCHEME_ED25519: u8 = 0x00;
const SCHEME_SECP256K1: u8 = 0x01;
const SCHEME_SECP256R1: u8 = 0x02;
const SCHEME_MULTISIG: u8 = 0x03;
const SCHEME_PASSKEY: u8 = 0x06;

const ED25519_PUBLIC_KEY_LEN: u64 = 32;
const COMPRESSED_PUBLIC_KEY_LEN: u64 = 33;

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

// === Structs ===

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
        id: object::new_uid_from_hash(@0x10),
    });
}

// === Claim ===

/// Validate the public key for the given `scheme` and mark `ctx.sender()` as
/// claimed.
/// Return a `UID` for the new account object, derived from the sender's address.
///
/// Supported schemes:
///   0x00 Ed25519   | 0x01 Secp256k1 | 0x02 Secp256r1
///   0x03 MultiSig  | 0x06 Passkey
/// For all schemes `public_key` must derive to `ctx.sender()`.
public fun claim(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: vector<u8>,
    ctx: &mut TxContext,
): UID {
    assert!(is_valid_scheme(scheme), EInvalidScheme);
    assert!(is_valid_public_key_length(scheme, &public_key), EInvalidPublicKeyLength);
    let derived_addr = derive_address(scheme, &public_key);
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    assert!(!is_claimed(registry, derived_addr), EAlreadyClaimed);
    df::add(&mut registry.id, derived_addr, true);
    object::new_uid_from_hash(derived_addr)
}

// === Public reads ===

public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Scheme helpers ===

public(package) fun scheme_ed25519(): u8 { SCHEME_ED25519 }
public(package) fun scheme_secp256k1(): u8 { SCHEME_SECP256K1 }
public(package) fun scheme_secp256r1(): u8 { SCHEME_SECP256R1 }
public(package) fun scheme_multisig(): u8 { SCHEME_MULTISIG }
public(package) fun scheme_passkey(): u8 { SCHEME_PASSKEY }

// === Internal helpers ===

/// Mirrors Rust `IotaAddress::from(&PublicKey)`:
///   Ed25519:   Blake2b256(pubkey)           — no flag prefix
///   Secp256k1: Blake2b256([0x01] || pubkey)
///   Secp256r1: Blake2b256([0x02] || pubkey)
///   MultiSig:  Blake2b256([0x03] || pubkey)
///   Passkey:   Blake2b256([0x06] || pubkey)
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

fun is_valid_scheme(scheme: u8): bool {
    scheme == SCHEME_ED25519
        || scheme == SCHEME_SECP256K1
        || scheme == SCHEME_SECP256R1
        || scheme == SCHEME_MULTISIG
        || scheme == SCHEME_PASSKEY
}

fun is_valid_public_key_length(scheme: u8, public_key: &vector<u8>): bool {
    let len = public_key.length();
    if (scheme == SCHEME_ED25519) {
        len == ED25519_PUBLIC_KEY_LEN
    } else if (scheme == SCHEME_MULTISIG) {
        len > 0 // Variable-length composite key; structural validation happens at the Rust layer.
    } else {
        len == COMPRESSED_PUBLIC_KEY_LEN // Secp256k1, Secp256r1, Passkey
    }
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
