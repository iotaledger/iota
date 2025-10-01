// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::template;

use iota::auth_context::AuthContext;

public struct Object has key, store {
    id: iota::object::UID,
}

// Template

#[allow(unused_field)]
public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
    t: T,
}

// PASS
public fun primitive<T: copy + drop + store>(
    _arg: T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
public fun templated_non_object_immutable_ref<T: copy + drop + store>(
    _arg: &NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_non_object_mutable_ref<T: copy + drop + store>(
    _arg: &mut NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_non_object_by_value<T: copy + drop + store>(
    _arg: NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Template and object

// PASS
public fun object_immutable_ref<T: key>(
    _object: &T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun object_by_value<T: key + store>(
    object: T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    transfer::public_share_object(object);
}

// FAIL
public fun object_mutable_ref<T: key>(
    _object: &mut T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

// PASS
public fun templated_object_immutable_ref<T: key + store>(
    _object: &ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_object_by_value<T: key + store>(
    object: ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let ObjectTemplated { t } = object;
    transfer::public_share_object(t);
}

// FAIL
public fun templated_object_mutable_ref<T: key + store>(
    _object: &mut ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
