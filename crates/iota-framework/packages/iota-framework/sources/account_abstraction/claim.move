// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Claiming of the address derived from a public key, so that it can be used as
/// the `UID` of a new on-chain object.
///
/// The `ClaimAccount` transaction kind drives `claim_address` through
/// `iota::smart_account`. Nothing here stops a second claim of an address:
/// the node's post-consensus validation keeps at most one claim per address,
/// and drops any later one before it executes.
module iota::claim;

use iota::public_key::PublicKey;

// === Errors ===

#[error(code = 0)]
const EAddressMismatch: vector<u8> =
    b"The public key does not correspond to the transaction sender address.";

// === Claim ===

/// Returns a deterministic `UID` bound to `ctx.sender()`. The caller must
/// immediately use the `UID` as the `id` field of a new on-chain object —
/// `UID` has no `drop` ability, so leaving it unconsumed is a compile error.
///
/// It does not check whether the address was claimed before: the node's
/// post-consensus validation keeps at most one claim per address.
///
/// Aborts with `EAddressMismatch` if `public_key` does not derive to the
/// sender.
///
/// Only callable from within the iota-framework package.
public(package) fun claim_address(public_key: PublicKey, ctx: &TxContext): UID {
    let derived_addr = public_key.to_iota_address();
    assert!(derived_addr == ctx.sender(), EAddressMismatch);
    object::new_uid_from_hash(derived_addr)
}
