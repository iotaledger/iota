// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

public struct Wrapper<T> has key {
    id: UID,
    wrapped: vector<T>,
}

// FAIL
#[authenticator]
public fun template_by_value<T: key>(
    account: T, // <- fail here first
    wrapper: &mut Wrapper<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    wrapper.wrapped.push_back(account);
}
