// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::object;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct Object<T> has key, store {
    id: iota::object::UID,
    t: T,
}

public struct Wrapper<T> has key {
    id: UID,
    wrapped: vector<T>,
}

// FAIL
#[authenticator]
public fun template_by_value<T>(
    _account: &Account,
    object: Object<T>, // <- fail here first
    wrapper: &mut Wrapper<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    let Object { id, t } = object;
    object::delete(id);
    wrapper.wrapped.push_back(t);
}
