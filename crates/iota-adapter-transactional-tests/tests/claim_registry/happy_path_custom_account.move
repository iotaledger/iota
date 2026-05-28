// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A custom account module receives an already-constructed `PublicKey` and passes it
// directly to `claim_registry::claim`, which returns a deterministic UID used as the
// `id` field of the new account object.
// Key 7f51... was chosen so that Blake2b256(key) == address(A) in the test framework.

//# init --accounts A --addresses custom_account=0x0

//# publish --sender A
module custom_account::account;

use iota::claim_registry::ClaimRegistry;
use iota::public_key::PublicKey;

public struct Account has key {
    id: UID,
}

public fun create(
    registry: &mut ClaimRegistry,
    public_key: PublicKey,
    ctx: &mut TxContext,
) {
    let uid = iota::claim_registry::claim(registry, public_key, ctx);
    transfer::transfer(Account { id: uid }, ctx.sender());
}

//# programmable --sender A --inputs object(0x10) x"007f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> iota::public_key::from_prefixed_bytes(Input(1));
//> 1: custom_account::account::create(Input(0), Result(0));

//# view-object 2,0
