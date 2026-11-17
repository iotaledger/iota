// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An unpublished dependency with an authenticator function, whose metadata a
/// bundled publish must keep.
module dep::dep;

public struct Account has key {
    id: UID,
}

#[authenticator]
public fun authenticate(
    _account: &Account,
    _signature: vector<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
