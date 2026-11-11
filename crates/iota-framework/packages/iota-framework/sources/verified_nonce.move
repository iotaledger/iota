// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Values that can be used only once, for checks of a signature made outside
/// the transaction.
///
/// A source checks a value, marks it used and returns a `VerifiedNonce`. The
/// only source so far is the lease of the transaction.
module iota::verified_nonce;

use iota::address;
use iota::hex;

// === Errors ===

#[error(code = 0)]
const ENoLease: vector<u8> = b"The transaction has no lease.";

#[error(code = 1)]
const ENonceKeyAlreadyTaken: vector<u8> =
    b"The nonce key was already taken in this transaction.";

// === Structs ===

/// A nonce that its source has checked and marked used, for the check `Check`.
///
/// It has no abilities, so it cannot be copied, dropped or kept for a later
/// transaction.
public struct VerifiedNonce<phantom Check> {
    /// The lines the source adds to the signed message: where the nonce comes
    /// from and its value.
    lines: vector<u8>,
}

// === Sources ===

/// Returns a nonce made of the sender, the lease of the transaction and
/// `nonce_key`.
///
/// The lease is taken when validators accept the transaction for execution,
/// and stays taken if the transaction aborts. A signature over this nonce is
/// therefore used up by the first transaction that carries it.
///
/// The lease is held until the last valid epoch of the transaction,
/// `ctx.last_valid()`. A signature over this nonce must not be accepted after
/// that epoch, so a check that uses it takes that epoch as the expiry of the
/// signed message.
///
/// Aborts with `ENoLease` if the transaction has no lease, and with
/// `ENonceKeyAlreadyTaken` if `nonce_key` was already taken in this
/// transaction.
public fun verify_lease_nonce<Check>(nonce_key: u64, ctx: &mut TxContext): VerifiedNonce<Check> {
    let lease = ctx.lease();
    assert!(lease.is_some(), ENoLease);
    assert!(take_lease_nonce_key(nonce_key), ENonceKeyAlreadyTaken);

    VerifiedNonce { lines: lease_lines(lease.destroy_some(), ctx.sender(), nonce_key) }
}

// === Internal ===

/// The lines `nonce source: lease 0x<hex> of 0x<sender hex>` and
/// `nonce key: <decimal>`, joined by a newline.
fun lease_lines(lease: vector<u8>, sender: address, nonce_key: u64): vector<u8> {
    let mut lines = b"nonce source: lease 0x";
    lines.append(hex::encode(lease));
    lines.append(b" of 0x");
    lines.append(address::to_string(sender).into_bytes());
    lines.append(b"\nnonce key: ");
    lines.append(nonce_key.to_string().into_bytes());
    lines
}

/// Records that `nonce_key` was taken under the lease of the transaction.
/// Returns false if it was already taken in this transaction.
native fun take_lease_nonce_key(nonce_key: u64): bool;

// === Test only ===

#[test_only]
/// Destroys the nonce and returns its message lines.
public fun destroy_for_testing<Check>(self: VerifiedNonce<Check>): vector<u8> {
    let VerifiedNonce { lines } = self;
    lines
}
