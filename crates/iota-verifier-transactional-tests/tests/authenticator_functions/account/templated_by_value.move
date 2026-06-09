// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

#[allow(unused_field)]
public struct Account<T: store> has key, store {
    id: UID,
    wrapped: T,
}

public struct Wrapper<T: store> has key {
    id: UID,
    wrapped: vector<Account<T>>,
}

// FAIL
#[authenticator]
public fun templated_by_value<T: store>(
    account: Account<T>, // <- fail here first
    wrapper: &mut Wrapper<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    wrapper.wrapped.push_back(account);
}
