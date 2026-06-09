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
public fun template_copy_drop_store_mutable_ref<T: copy + drop + store>(
    _account: &Account,
    _object: &mut ObjectPrimitive<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
