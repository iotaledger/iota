// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::vector;

use authenticate::account::Account;
use iota::auth_context::AuthContext;

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

// Vector

// PASS
public fun primitive_immutable_reference(
    _account: &Account,
    _arg: &vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun primitive_mutable_reference(
    _account: &Account,
    _arg: &mut vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
public fun primitive_by_value(
    _account: &Account,
    _arg: vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
public fun non_object_immutable_ref(
    _account: &Account,
    _arg: &vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun non_object_mutable_ref(
    _account: &Account,
    _arg: &mut vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun non_object_by_value(
    _account: &Account,
    _arg: vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector and object

// PASS
public fun object_immutable_ref(
    _account: &Account,
    _objects: &vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun object_mutable_ref(
    _account: &Account,
    _objects: &mut vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(lint(share_owned))]
public fun object_by_value(
    _account: &Account,
    objects: vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

// Vector and template

// error[E06001]: unused value without 'drop'
//public fun template_non_object_by_value<T>(
//    _account: &Account,
//    _arg: vector<T>,
//    _auth_ctx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// PASS
public fun template_non_object_immutable_ref<T>(
    _account: &Account,
    _arg: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun template_non_object_mutable_ref<T>(
    _account: &Account,
    _arg: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_non_object_by_value<T: copy + drop + store>(
    _account: &Account,
    _arg: vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
public fun templated_non_object_immutable_ref<T: copy + drop + store>(
    _account: &Account,
    _arg: &vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_non_object_mutable_ref<T: copy + drop + store>(
    _account: &Account,
    _arg: &mut vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector, template and object

// PASS
public fun template_object_immutable_reference<T: key>(
    _account: &Account,
    _objects: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun template_object_by_value<T: key + store>(
    _account: &Account,
    objects: vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

// FAIL
public fun template_object_mutable_reference<T: key>(
    _account: &Account,
    _objects: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// PASS
public fun templated_object_immutable_ref<T: key + store>(
    _account: &Account,
    _objects: &vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun templated_object_by_value<T: key + store>(
    _account: &Account,
    objects: vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        let ObjectTemplated { t } = object;
        transfer::public_share_object(t);
    });
}

// FAIL
public fun templated_object_mutable_ref<T: key + store>(
    _account: &Account,
    _objects: &mut vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
