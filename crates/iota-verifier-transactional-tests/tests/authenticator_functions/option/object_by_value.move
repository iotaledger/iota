// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::option;

public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun object_by_value(
    _account: &Account,
    objects: Option<Object>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        transfer::public_share_object(object);
    });
}
