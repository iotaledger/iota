// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Nonces that a check accepts at most once.
///
/// A key holder approves an action by signing a readable message. A nonce in the message makes
/// the approval single-use: a source returns a `VerifiedNonce` whose value no other call ever
/// gets.
///
/// Callers depend on `VerifiedNonce`, never on where its value comes from, so a source can be
/// added or replaced without changing them.
module iota::verified_nonce;

use iota::bcs;
use iota::hash;

// === Structs ===

/// A nonce for the check `Check`, with a value that no other `VerifiedNonce` has.
///
/// It has no abilities: it can't be copied, stored or dropped, so it is used in the transaction
/// that took it. Only this module creates one, so its value always comes from a source.
public struct VerifiedNonce<phantom Check> {
    /// The 32-byte hash of the values the source took the nonce from.
    hash: vector<u8>,
}

// === Public Functions ===

/// Returns a nonce for `Check` taken from the transaction being executed.
///
/// Today the value comes from the transaction's first gas coin. It is the Blake2b-256 hash of
/// these bytes, in this order:
///
/// - the 32 bytes of the coin ID;
/// - the version the transaction lists for the coin, as 8 bytes, little-endian;
/// - an index that differs for each nonce taken in the transaction (0, 1, 2, …), as 8 bytes,
///   little-endian.
///
/// The key holder signs right before the transaction is sent: the approval stops working once the
/// coin version is used by any transaction, including one that aborts.
///
/// Aborts if the transaction has no gas coin.
public fun take_from_tx_context<Check>(ctx: &mut TxContext): VerifiedNonce<Check> {
    let (gas_coin, version, index) = ctx.take_gas_coin_nonce();

    let mut bytes = gas_coin.to_bytes();
    bytes.append(bcs::to_bytes(&version));
    bytes.append(bcs::to_bytes(&index));

    VerifiedNonce { hash: hash::blake2b256(&bytes) }
}

// === Package Functions ===

/// Destroys `nonce` and returns its 32-byte hash.
///
/// The caller must write the hash into the message it checks a signature for: the approval is
/// single-use only if the signed message names the nonce.
public(package) fun into_hash<Check>(nonce: VerifiedNonce<Check>): vector<u8> {
    let VerifiedNonce { hash } = nonce;
    hash
}

// === Test Functions ===

#[test_only]
/// Returns the hash of `nonce`.
public fun hash_for_testing<Check>(nonce: &VerifiedNonce<Check>): &vector<u8> {
    &nonce.hash
}
