// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Counters that make approvals of keys for new accounts single-use.
///
/// A creator owns a `NonceFactory` with a fixed number of nonce keys, each with its own counter.
/// An approval names the factory, a nonce key and that key's next value, and taking the value
/// moves that counter on. Approvals under different nonce keys can be pending at the same time and
/// be used in any order; under one nonce key they are used in the order of their values.
///
/// The factory has no `store`, so it stays with its owner, and only the owner can pass it to a
/// transaction. Its size is fixed when it is created, so it doesn't grow with use.
module iota::nonce_factory;

// === Errors ===

#[error(code = 0)]
const EInvalidNonceKeyCount: vector<u8> = b"A nonce factory has from 1 to 64 nonce keys.";
#[error(code = 1)]
const ENonceKeyOutOfRange: vector<u8> = b"The factory has no such nonce key.";
#[error(code = 2)]
const EUnexpectedNonce: vector<u8> = b"The nonce is not the next value of its nonce key.";

// === Constants ===

const MAX_NONCE_KEYS: u64 = 64;

// === Structs ===

/// Counters for approvals of keys for new accounts, one per nonce key.
public struct NonceFactory has key {
    id: UID,
    counters: vector<u64>,
}

// === Public Functions ===

/// Creates a factory with `nonce_keys` nonce keys, owned by the sender.
///
/// It must exist before the first approval naming it is signed, since the approval names its ID.
///
/// Aborts if `nonce_keys` is 0 or more than 64.
public fun new(nonce_keys: u64, ctx: &mut TxContext) {
    assert!(nonce_keys > 0 && nonce_keys <= MAX_NONCE_KEYS, EInvalidNonceKeyCount);

    let factory = NonceFactory {
        id: object::new(ctx),
        counters: vector::tabulate!(nonce_keys, |_| 0),
    };
    transfer::transfer(factory, ctx.sender());
}

/// Deletes the factory. Approvals that name it can no longer be used.
public fun destroy(self: NonceFactory) {
    let NonceFactory { id, counters: _ } = self;
    id.delete();
}

/// Moves the counter of `nonce_key` on, so the approval for its current value can no longer be
/// used.
///
/// Aborts if the factory has no such nonce key.
public fun skip(self: &mut NonceFactory, nonce_key: u64) {
    let next = self.next_value(nonce_key);
    self.use_nonce(nonce_key, next);
}

// === View Functions ===

/// Returns the number of nonce keys of the factory. They are numbered from 0.
public fun nonce_keys(self: &NonceFactory): u64 {
    self.counters.length()
}

/// Returns the value the next approval under `nonce_key` must name.
///
/// Aborts if the factory has no such nonce key.
public fun next_value(self: &NonceFactory, nonce_key: u64): u64 {
    assert!(nonce_key < self.counters.length(), ENonceKeyOutOfRange);
    self.counters[nonce_key]
}

// === Package Functions ===

/// Checks that `expected` is the next value under `nonce_key` and moves the counter on.
///
/// Aborts if the factory has no such nonce key, or if `expected` is not its next value.
public(package) fun use_nonce(self: &mut NonceFactory, nonce_key: u64, expected: u64) {
    assert!(self.next_value(nonce_key) == expected, EUnexpectedNonce);
    let counter = &mut self.counters[nonce_key];
    *counter = *counter + 1;
}
