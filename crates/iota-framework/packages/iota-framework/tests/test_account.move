// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Mock account module used in claim_registry tests to verify the UID-based
/// creation flow: `claim_registry::claim` returns a fresh UID that is consumed
/// directly as the `id` field of the new object.
#[test_only]
module iota::test_account;

use iota::claim_registry::ClaimRegistry;
use iota::signature_scheme::{Self, SignatureScheme};

public struct Account has key {
    id: UID,
    scheme: u8,
}

public fun create(
    registry: &mut ClaimRegistry,
    scheme: SignatureScheme,
    public_key: vector<u8>,
    ctx: &mut TxContext,
) {
    let scheme_flag = scheme.flag();
    let uid = iota::claim_registry::claim(registry, scheme, public_key, ctx);
    transfer::transfer(Account { id: uid, scheme: scheme_flag }, ctx.sender());
}
