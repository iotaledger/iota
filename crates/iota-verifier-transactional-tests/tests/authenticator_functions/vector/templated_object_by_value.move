// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

// FAIL Invalid parameter type
#[authenticator]
public fun templated_object_by_value<T: key + store>(
    _account: &Account,
    objects: vector<ObjectTemplated<T>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        let ObjectTemplated { t } = object;
        transfer::public_share_object(t);
    });
}
