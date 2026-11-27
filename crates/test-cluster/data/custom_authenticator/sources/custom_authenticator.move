// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module for `SmartAccount`s: an authenticator that is not the built-in
/// one, and an object that is not a `SmartAccount` with a public key attached.
module custom_authenticator::custom_authenticator;

use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::smart_account::SmartAccount;

/// Accepts every transaction of the account.
#[authenticator]
public fun authenticate(_: &SmartAccount, _: &AuthContext, _: &TxContext) {}

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
