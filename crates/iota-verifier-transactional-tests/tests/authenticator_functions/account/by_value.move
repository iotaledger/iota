// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun by_value(account: Account, _actx: &AuthContext, _ctx: &TxContext) {
    let Account { id } = account;
    object::delete(id);
}
