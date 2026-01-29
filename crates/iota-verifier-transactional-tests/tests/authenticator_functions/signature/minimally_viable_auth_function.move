// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

public struct Account has key {
    id: UID,
}

// PASS
#[authenticator]
public fun minimally_viable_auth_function(
    _account: &Account,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
