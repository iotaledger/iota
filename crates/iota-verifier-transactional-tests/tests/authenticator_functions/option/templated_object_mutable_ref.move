// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::option;

public struct Account has key {
    id: UID,
}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

// FAIL Invalid parameter type
#[authenticator]
public fun templated_object_mutable_ref<T: key + store>(
    _account: &Account,
    _objects: &mut Option<ObjectTemplated<T>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
