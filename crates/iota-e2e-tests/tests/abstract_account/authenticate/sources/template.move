// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::template;

use iota::auth_context::AuthContext;

// Test account struct
public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// Template

#[allow(unused_field)]
public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
    t: T,
}

// PASS
#[authenticator]
public entry fun primitive<T: copy + drop + store>(
    _account: &Account,
    _arg: T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_non_object_immutable_ref<T: copy + drop + store>(
//    _account: &Account,
//    _arg: &NonObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_non_object_mutable_ref<T: copy + drop + store>(
//    _account: &Account,
//    _arg: &mut NonObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_non_object_by_value<T: copy + drop + store>(
//    _account: &Account,
//    _arg: NonObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Template and object

// PASS
#[authenticator]
public entry fun object_immutable_ref<T: key>(
    _account: &Account,
    _object: &T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun object_by_value<T: key + store>(
    _account: &Account,
    object: T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    transfer::public_share_object(object);
}

// FAIL
//#[authenticator]
public entry fun object_mutable_ref<T: key>(
    _account: &Account,
    _object: &mut T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_immutable_ref<T: key + store>(
//    _account: &Account,
//    _object: &ObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_by_value<T: key + store>(
//    _account: &Account,
//    object: ObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_mutable_ref<T: key + store>(
//    _account: &Account,
//    _object: &mut ObjectTemplated<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}
