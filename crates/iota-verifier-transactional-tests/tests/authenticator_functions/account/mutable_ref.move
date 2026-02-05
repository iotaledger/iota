// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun mutable_ref(_account: &mut Account, _actx: &AuthContext, _ctx: &TxContext) {}
