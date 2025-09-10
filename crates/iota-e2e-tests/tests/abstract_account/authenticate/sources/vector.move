module authenticate::m;

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

public fun primitive_immutable_reference(
    _arg: &vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun primitive_mutable_reference(
    _arg: &mut vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun primitive_by_value(_arg: vector<u8>, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun non_object_immutable_ref(
    _arg: &vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun non_object_mutable_ref(
    _arg: &mut vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun non_object_by_value(
    _arg: vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector and object

public fun object_immutable_ref(
    _objects: &vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun object_mutable_ref(
    _objects: &mut vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(lint(share_owned))]
public fun object_by_value(objects: vector<Object>, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    objects.do!(|object| transfer::public_share_object(object));
}

// Vector and template

// error[E06001]: unused value without 'drop'
//public fun template_non_object_by_value<T>(
//    _arg: vector<T>,
//    _auth_ctx: &AuthContext,
//    _ctx: &TxContext,
//) {}

public fun template_non_object_immutable_ref<T>(
    _arg: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun template_non_object_mutable_ref<T>(
    _arg: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_by_value<T: copy + drop + store>(
    _arg: vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_immutable_ref<T: copy + drop + store>(
    _arg: &vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_mutable_ref<T: copy + drop + store>(
    _arg: &mut vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector, template and object

public fun template_object_immutable_reference<T: key>(
    _objects: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun template_object_by_value<T: key + store>(
    objects: vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

public fun template_object_mutable_reference<T: key>(
    _objects: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_object_immutable_ref<T: key + store>(
    _objects: &vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_object_by_value<T: key + store>(
    objects: vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        let ObjectTemplated { t } = object;
        transfer::public_share_object(t);
    });
}

public fun templated_object_mutable_ref<T: key + store>(
    _objects: &mut vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
