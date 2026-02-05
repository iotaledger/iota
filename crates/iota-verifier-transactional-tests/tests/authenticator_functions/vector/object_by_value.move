// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL
#[authenticator]
public fun object_by_value(
    _account: &Account,
    objects: vector<Object>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| { let Object { id } = object; object::delete(id) });
}
