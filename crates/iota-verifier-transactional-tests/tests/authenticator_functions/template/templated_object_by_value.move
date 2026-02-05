// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::template;

public struct Account has key {
    id: UID,
}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

// FAIL Invalid parameter type
#[authenticator]
public fun templated_object_by_value<T: key + store>(
    _account: &Account,
    object: ObjectTemplated<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    let ObjectTemplated { t } = object;
    transfer::public_share_object(t);
}
