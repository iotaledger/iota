// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

// PASS
#[authenticator]
public fun primitive_by_value(
    _account: &Account,
    _arg: vector<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
