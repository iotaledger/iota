// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// A custom account module calls `claim_registry::claim`, receives a deterministic
// UID, and uses it directly to create the account object — no field accessors
// or consume helpers needed. The UID is a hot potato by nature (no `drop`).
// Key 7f51... was chosen so that Blake2b256(key) == address(A).

//# init --accounts A --addresses custom_account=0x0

//# publish --sender A
module custom_account::account;

use iota::claim_registry::ClaimRegistry;

public struct Account has key {
    id: UID,
}

public fun create(
    registry: &mut ClaimRegistry,
    scheme: u8,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    let uid = iota::claim_registry::claim(registry, scheme, public_key, ctx);
    transfer::transfer(Account { id: uid }, ctx.sender());
}

//# programmable --sender A --inputs object(0x10) 0u8 x"7f51463aeb76d88dc9b75e637250b220c49cf5b7967dbf17c1f9fa7c594a0928"
//> custom_account::account::create(Input(0), Input(1), Input(2));

//# view-object 2,0