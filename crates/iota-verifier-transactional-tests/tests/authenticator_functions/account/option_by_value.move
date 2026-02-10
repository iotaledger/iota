// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

use std::option::Option;

public struct Account has key {
    id: UID,
}

public struct Wrapper<T> has key {
    id: UID,
    wrapped: vector<T>,
}

// FAIL
#[authenticator]
public fun option_by_value<T: key>(
    account: Option<Account>, // <- fail here first
    wrapper: &mut Wrapper<Account>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    account.do!(|acc| wrapper.wrapped.push_back(acc));
}
