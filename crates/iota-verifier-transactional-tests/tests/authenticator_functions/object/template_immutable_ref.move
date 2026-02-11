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

public struct ObjectObject<T: key + store> has key, store {
    id: iota::object::UID,
    t: T,
}

public struct ObjectPrimitive<T: copy + drop + store> has key, store {
    id: iota::object::UID,
    t: T,
}

// PASS
#[authenticator]
public fun template_immutable_ref<T: store>(
    _account: &Account,
    _object: &Object<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
#[authenticator]
public fun template_key_store_immutable_ref<T: key + store>(
    _account: &Account,
    _object: &ObjectObject<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
#[authenticator]
public fun template_copy_drop_store_immutable_ref<T: copy + drop + store>(
    _account: &Account,
    _object: &ObjectPrimitive<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
