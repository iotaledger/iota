// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::option;

use iota::auth_context::AuthContext;

// Test account struct
public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

#[allow(unused_field)]
public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
    t: T,
}

public struct NonObject has copy, drop, store {}

// Option

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun primitive_immutable_reference(
//    _account: &Account,
//    _arg: &Option<u8>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun primitive_mutable_reference(
//    _account: &Account,
//    _arg: &mut Option<u8>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// PASS
#[authenticator]
public entry fun primitive_by_value(
    _account: &Account,
    _arg: Option<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun non_object_immutable_ref(
//    _account: &Account,
//    _arg: &Option<NonObject>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun non_object_mutable_ref(
//    _account: &Account,
//    _arg: &mut Option<NonObject>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun non_object_by_value(
//    _account: &Account,
//    _arg: Option<NonObject>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Option and object

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun object_immutable_ref(
//    _account: &Account,
//    _objects: &Option<Object>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun object_mutable_ref(
//    _account: &Account,
//    _objects: &mut Option<Object>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun object_by_value(
//    _account: &Account,
//    objects: Option<Object>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Option and template

// FAIL unused value without 'drop'
//#[authenticator]
//public entry fun template_non_object_by_value<T>(
//    _account: &Account,
//    _arg: Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun template_non_object_immutable_ref<T>(
//    _account: &Account,
//    _arg: &Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun template_non_object_mutable_ref<T>(
//    _account: &Account,
//    _arg: &mut Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//public entry fun templated_non_object_by_value<T: copy + drop + store>(
//    _account: &Account,
//    _arg: Option<NonObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_non_object_immutable_ref<T: copy + drop + store>(
//    _account: &Account,
//    _arg: &Option<NonObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_non_object_mutable_ref<T: copy + drop + store>(
//    _account: &Account,
//    _arg: &mut Option<NonObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Option, template and object

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun template_object_immutable_reference<T: key>(
//    _account: &Account,
//    _objects: &Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun template_object_by_value<T: key + store>(
//    _account: &Account,
//    objects: Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun template_object_mutable_reference<T: key>(
//    _account: &Account,
//    _objects: &mut Option<T>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_immutable_ref<T: key + store>(
//    _account: &Account,
//    _objects: &Option<ObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_by_value<T: key + store>(
//    _account: &Account,
//    objects: Option<ObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun templated_object_mutable_ref<T: key + store>(
//    _account: &Account,
//    _objects: &mut Option<ObjectTemplated<T>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}
