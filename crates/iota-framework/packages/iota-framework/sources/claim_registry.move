// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// On-chain registry for claiming addresses.
///
/// `claim` validates the public key (scheme, length, address derivation),
/// marks the sender's address as claimed, and returns a `ClaimedAddressTicket`
/// hot potato. The caller must consume it in the same PTB — typically by
/// passing it to an account-creation function.
module iota::claim_registry;

use iota::address as iota_address;
use iota::dynamic_field as df;
use iota::hash;

// === Signature scheme flags (match Rust `SignatureScheme`) ===

const SCHEME_ED25519: u8 = 0x00;
const SCHEME_SECP256K1: u8 = 0x01;
const SCHEME_SECP256R1: u8 = 0x02;
/// Matches `SignatureScheme::MoveAuthenticator` (0x07).
/// For this scheme address derivation is skipped — `public_key` may be empty
/// or carry arbitrary data; ownership is proved by the transaction signature.
/// DISCUSS: should we require an explicit address derivation rule for 0x07 too,
/// or is relying on ctx.sender() sufficient as the ownership proof?
const SCHEME_MOVE_AUTHENTICATOR: u8 = 0x07;

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

/// Hot potato returned by `claim`. Must be consumed in the same PTB.
/// Carries the proven sender address, raw public key, and scheme flag.
public struct ClaimedAddressTicket {
    account: address,
    public_key: vector<u8>,
    flag: u8,
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
/// claimed. Returns a `ClaimedAddressTicket` that must be consumed in the same
/// PTB (hot potato — no `drop` ability).
///
/// `scheme`: 0x00 Ed25519 | 0x01 Secp256k1 | 0x02 Secp256r1 | 0x07 MoveAuthenticator.
/// For crypto schemes, `public_key` must have the correct length and derive to
/// `ctx.sender()`. For 0x07, `public_key` may be empty or carry arbitrary data.
public fun claim(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: vector<u8>,
    ctx: &mut TxContext,
): ClaimedAddressTicket {
    assert!(is_valid_scheme(scheme), EInvalidScheme);
    assert!(is_valid_public_key_length(scheme, &public_key), EInvalidPublicKeyLength);
    if (!is_move_authenticator_scheme(scheme)) {
        let derived = derive_address(scheme, &public_key);
        assert!(derived == ctx.sender(), EAddressMismatch);
    };
    assert!(!df::exists_(&registry.id, ctx.sender()), EAlreadyClaimed);
    df::add(&mut registry.id, ctx.sender(), true);
    ClaimedAddressTicket { account: ctx.sender(), public_key, flag: scheme }
}

// === Public reads ===

public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Scheme helpers ===

public(package) fun scheme_ed25519(): u8 { SCHEME_ED25519 }
public(package) fun scheme_secp256k1(): u8 { SCHEME_SECP256K1 }
public(package) fun scheme_secp256r1(): u8 { SCHEME_SECP256R1 }
public(package) fun scheme_move_authenticator(): u8 { SCHEME_MOVE_AUTHENTICATOR }

// === Internal helpers ===

/// Mirrors Rust `IotaAddress::from(&PublicKey)`:
///   Ed25519:   Blake2b256(pubkey)           — no flag prefix
///   Secp256k1: Blake2b256([0x01] || pubkey)
///   Secp256r1: Blake2b256([0x02] || pubkey)
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
        || scheme == SCHEME_MOVE_AUTHENTICATOR
}

fun is_move_authenticator_scheme(scheme: u8): bool {
    scheme == SCHEME_MOVE_AUTHENTICATOR
}

fun is_valid_public_key_length(scheme: u8, public_key: &vector<u8>): bool {
    if (scheme == SCHEME_MOVE_AUTHENTICATOR) {
        true
    } else {
        let len = public_key.length();
        if (scheme == SCHEME_ED25519) { len == ED25519_PUBLIC_KEY_LEN }
        else { len == COMPRESSED_PUBLIC_KEY_LEN }
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

#[test_only]
public fun consume_ticket_for_testing(ticket: ClaimedAddressTicket): (address, vector<u8>, u8) {
    let ClaimedAddressTicket { account, public_key, flag } = ticket;
    (account, public_key, flag)
}
