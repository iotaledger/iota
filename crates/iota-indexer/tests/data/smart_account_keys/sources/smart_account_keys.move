// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module for the indexing of smart account keys: an object that is not a
/// `SmartAccount` with a public key attached, and an authenticator that is not
/// the built-in one.
module smart_account_keys::smart_account_keys;

use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::smart_account::SmartAccount;

/// An object that is not a `SmartAccount`.
public struct Holder has key {
    id: UID,
}

/// Creates a `Holder` with `public_key` attached in the same field a
/// `SmartAccount` holds its key, and transfers it to the sender.
public fun create_holder_with_key(public_key: PublicKey, ctx: &mut TxContext) {
    let mut holder = Holder { id: object::new(ctx) };
    builtin_authenticator_functions::attach_public_key(&mut holder.id, public_key);
    transfer::transfer(holder, ctx.sender());
}

/// Accepts every transaction of the account.
#[authenticator]
public fun authenticate(_: &SmartAccount, _: &AuthContext, _: &TxContext) {}
