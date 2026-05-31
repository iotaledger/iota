// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Mock account module used in claim_registry tests to verify the UID-based
/// creation flow: `claim_registry::claim` returns a fresh UID that is consumed
/// directly as the `id` field of the new object.
#[test_only]
module iota::test_account;

use iota::claim_registry::ClaimRegistry;
use iota::public_key::PublicKey;

public struct Account has key {
    id: UID,
    scheme: u8,
}

public fun create(
    registry: &mut ClaimRegistry,
    public_key: PublicKey,
    ctx: &mut TxContext,
) {
    let flag = public_key.scheme().flag();
    let uid = iota::claim_registry::claim(registry, public_key, ctx);
    transfer::share_object(Account { id: uid, scheme: flag });
}
