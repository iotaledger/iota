// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::object;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct ObjectPrimitive<T: copy + drop + store> has key, store {
    id: iota::object::UID,
    t: T,
}

// FAIL
#[authenticator]
public fun template_copy_drop_store_by_value<T: copy + drop + store>(
    _account: &Account,
    object: ObjectPrimitive<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    let ObjectPrimitive { id, t } = object;
    object::delete(id);
}
