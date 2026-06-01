// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// On-chain registry for claiming addresses.
///
/// `claim` takes an already-validated `PublicKey`, marks the sender's address
/// as claimed, and returns a deterministic `UID` for the new account object.
/// Callers with raw bytes use `iota::public_key::from_prefixed_bytes` to construct
/// the `PublicKey` before calling `claim`.
///
/// `claim` is `public(package)` — only modules within the iota-framework package
/// may call it directly.  External callers use the built-in account modules.
/// For transactional-test coverage, `test_claim_account` is provided as a
/// `public` wrapper that creates a `DummyAccount` object.
module iota::claim_registry;

use iota::dynamic_field as df;
use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

#[error(code = 1)]
const EAlreadyClaimed: vector<u8> =
    b"This address has already been claimed.";

#[error(code = 4)]
const ENotSystemAddress: vector<u8> =
    b"ClaimRegistry can only be created by the system address (@0x0).";

// === Structs ===

/// Singleton shared object tracking claimed addresses via dynamic fields.
public struct ClaimRegistry has key {
    id: UID,
}

/// Minimal account object used by `test_claim_account` to exercise the full
/// claim flow from an external PTB without publishing a separate module.
public struct DummyAccount has key {
    id: UID,
}

// === Genesis ===

/// Create and share the `ClaimRegistry` singleton. Called once during genesis.
#[allow(unused_function)]
fun create(ctx: &TxContext) {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);
    transfer::share_object(ClaimRegistry {
        id: object::new_uid_from_hash(@0x10),
    });
}

// === Claim ===

/// Mark `ctx.sender()` as claimed and return a deterministic `UID` for the new
/// account object.
///
/// The returned `UID` is a hot potato — it has no `drop` ability and must be
/// consumed (typically as the `id` field of a new `key` struct) in the same PTB.
///
/// Aborts with `EAddressMismatch` if `public_key` does not derive to the sender,
/// or `EAlreadyClaimed` if the address was already claimed.
///
/// `public(package)` — only callable from within the iota-framework package.
public(package) fun claim(
    registry: &mut ClaimRegistry,
    public_key: PublicKey,
    ctx: &TxContext,
): UID {
    let derived_addr = public_key.to_iota_address();
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    assert!(!is_claimed(registry, derived_addr), EAlreadyClaimed);
    df::add(&mut registry.id, derived_addr, true);
    object::new_uid_from_hash(derived_addr)
}

// === Public reads ===

public fun is_claimed(registry: &ClaimRegistry, addr: address): bool {
    df::exists_(&registry.id, addr)
}

// === Test only ===

#[test_only]
public fun create_for_testing(ctx: &mut TxContext) {
    create(ctx);
}

#[test_only]
public fun derive_address_for_testing(prefixed_bytes: &vector<u8>): address {
    iota::public_key::from_prefixed_bytes(*prefixed_bytes).to_iota_address()
}

/// Creates a `DummyAccount` owned by the sender.
///
/// Provided so that transactional tests can exercise the full `claim` flow
/// directly from a PTB without publishing an external module.
public fun test_claim_account(
    registry: &mut ClaimRegistry,
    public_key: PublicKey,
    ctx: &mut TxContext,
) {
    transfer::share_object(DummyAccount { id: claim(registry, public_key, ctx) });
}
