// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

public struct Wrapped has store {
    cd: u8,
}

#[allow(unused_field)]
public struct Account<T: store> has key {
    id: UID,
    wrapped: T,
}

// PASS
#[authenticator]
public fun concrete(_account: &Account<Wrapped>, _actx: &AuthContext, _ctx: &TxContext) {}
