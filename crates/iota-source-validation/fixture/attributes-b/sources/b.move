// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module b::b;

use iota::auth_context::AuthContext;

public struct Account has key {
    id: UID,
}

#[view]
public fun b(): u64 {
    42
}

#[authenticator]
public fun authenticate(_account: &Account, _auth_ctx: &AuthContext, _ctx: &TxContext) {}
