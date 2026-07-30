// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Minting of deterministic `UID`s for claimed addresses.
///
/// `claim` takes an already-validated `PublicKey`, verifies that it derives
/// the transaction sender's address, and returns a deterministic `UID` bound
/// to that address, so the address can be used as the id of a new object
/// on-chain. Callers with raw bytes use `iota::public_key::from_prefixed_bytes`
/// to construct the `PublicKey` before calling `claim`.
///
/// Double-claim prevention is not enforced here: the sequencer rejects a
/// `ClaimAccount` transaction for an address that is already explicit before
/// it reaches execution, so `claim` can never mint a second `UID` for the
/// same address.
///
/// `claim` is `public(package)` — only modules within the iota-framework package
/// may call it directly.  External callers use the built-in account modules.
module iota::claim_registry;

use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

#[error(code = 1)]
const ENotSystemAddress: vector<u8> = b"ClaimRegistry can only be created in a system transaction.";

// === Structs ===

/// Singleton shared object reserved for account bookkeeping.
public struct ClaimRegistry has key {
    id: UID,
}

// === Genesis ===

/// Create and share the `ClaimRegistry` singleton. Called once during genesis.
#[allow(unused_function)]
fun create(ctx: &TxContext) {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);
    transfer::share_object(ClaimRegistry {
        id: object::claim_registry(),
    });
}

// === Claim ===

/// Returns a deterministic `UID` bound to `ctx.sender()`. The caller must
/// immediately use the `UID` as the `id` field of a new on-chain object —
/// `UID` has no `drop` ability, so leaving it unconsumed is a compile error.
///
/// Aborts with `EAddressMismatch` if `public_key` does not derive to the
/// sender.
///
/// `public(package)` — only callable from within the iota-framework package.
public(package) fun claim(public_key: PublicKey, ctx: &TxContext): UID {
    let derived_addr = public_key.to_iota_address();
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    object::new_uid_from_hash(derived_addr)
}

// === Test only ===

#[test_only]
public fun create_for_testing(ctx: &mut TxContext) {
    create(ctx);
}
