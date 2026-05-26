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
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme::{Self, SignatureScheme};

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

#[error(code = 1)]
const EAlreadyClaimed: vector<u8> =
    b"This address has already been claimed.";

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
/// claimed. Returns a `UID` for the new account object, derived from the
/// sender's address.
///
/// The returned `UID` is a hot potato — it has no `drop` ability and must be
/// consumed (typically as the `id` field of a new `key` struct) in the same PTB.
///
/// Supported schemes:
///   0x00 Ed25519   | 0x01 Secp256k1 | 0x02 Secp256r1
///   0x03 MultiSig  | 0x06 Passkey
///
/// Scheme validity and byte-length are validated via `iota::public_key::create`.
/// Aborts with `EAddressMismatch` if the key does not derive to the sender, or
/// `EAlreadyClaimed` if the address was already claimed.
public fun claim(
    registry: &mut ClaimRegistry,
    scheme: SignatureScheme,
    raw_bytes: vector<u8>,
    ctx: &mut TxContext,
): UID {
    let pk = public_key::create(scheme, raw_bytes);
    let derived_addr = derive_address(&pk);
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    assert!(!is_claimed(registry, derived_addr), EAlreadyClaimed);
    df::add(&mut registry.id, derived_addr, true);
    object::new_uid_from_hash(derived_addr)
}

// === Public reads ===

public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Internal helpers ===

/// Mirrors Rust `IotaAddress::from(&PublicKey)`:
///   Ed25519:   Blake2b256(pubkey)           — no flag prefix
///   Secp256k1: Blake2b256([0x01] || pubkey)
///   Secp256r1: Blake2b256([0x02] || pubkey)
///   MultiSig:  Blake2b256([0x03] || pubkey)
///   Passkey:   Blake2b256([0x06] || pubkey)
fun derive_address(pk: &PublicKey): address {
    let scheme = pk.scheme();
    let raw = *pk.raw_bytes();
    // Ed25519 is hashed without a flag prefix; all other schemes prepend the flag byte.
    let data = if (scheme == signature_scheme::ed25519()) {
        raw
    } else {
        let mut v = vector[scheme.flag()];
        v.append(raw);
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
public fun derive_address_for_testing(scheme: SignatureScheme, raw_bytes: &vector<u8>): address {
    derive_address(&public_key::create(scheme, *raw_bytes))
}
