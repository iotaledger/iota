// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Wrapped has key {
    id: UID,
}

public struct Account has key {
    id: UID,
}

public struct Wrapper<T> has key {
    id: UID,
    wrapped: vector<T>,
}

// FAIL Invalid parameter type
#[authenticator]
public fun template_non_object_by_value<T>(
    _account: &Account,
    objects: vector<T>, // <- fail here first
    wrapper: &mut Wrapper<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        wrapper.wrapped.push_back(object);
    });
}

public fun transfer_wrapped(wrapper: &mut Wrapper<Wrapped>) {
    let a = wrapper.wrapped.pop_back();
    transfer::transfer(a, @0x1);
}
