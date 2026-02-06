// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

public struct Wrapped has store {
    cd: u8,
}

public struct Wrapped2 has store {
    cd: u8,
}

#[allow(unused_field)]
public struct Account<T: store, U: store> has key {
    id: UID,
    wrapped: T,
    wrapped_u: U,
}

// PASS
#[authenticator]
public fun concrete_multiple(
    _account: &Account<Wrapped, Wrapped2>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
