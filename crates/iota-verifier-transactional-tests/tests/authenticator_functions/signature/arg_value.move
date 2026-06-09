// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

public struct Account has key {
    id: UID,
}

// PASS
#[authenticator]
public fun arg_value(_account: &Account, _val: u8, _actx: &AuthContext, _ctx: &TxContext) {}
