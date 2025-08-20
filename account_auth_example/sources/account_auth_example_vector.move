module account_auth_example::vector_m;

// temporary placeholder
public struct AuthContext has drop {}

public fun bind_to_test_module() {}

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

public fun vector_primitive_immutable_reference_success(
    _arg: &vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_primitive_mutable_reference_fail(
    _arg: &mut vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_primitive_by_value_success(
    _arg: vector<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_non_object_immutable_ref_success(
    _arg: &vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_non_object_mutable_ref_fail(
    _arg: &mut vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_non_object_by_value_fail(
    _arg: vector<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector and object

public fun vector_object_immutable_ref_success(
    _objects: &vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_object_mutable_ref_fail(
    _objects: &mut vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_object_by_value_fail(
    objects: vector<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

// Vector and template

// error[E06001]: unused value without 'drop'
//public fun vector_template_non_object_by_value_success<T>(
//    _arg: vector<T>,
//    _auth_ctx: &AuthContext,
//    _ctx: &TxContext,
//) {}

public fun vector_template_non_object_immutable_ref_success<T>(
    _arg: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_template_non_object_mutable_ref_fail<T>(
    _arg: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_templated_non_object_by_value_fail<T: copy + drop + store>(
    _arg: vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_templated_non_object_immutable_ref_success<T: copy + drop + store>(
    _arg: &vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_templated_non_object_mutable_ref_fail<T: copy + drop + store>(
    _arg: &mut vector<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Vector, template and object

public fun vector_template_object_immutable_reference_success<T: key>(
    _objects: &vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_template_object_by_value_fail<T: key + store>(
    objects: vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

public fun vector_template_object_mutable_reference_fail<T: key>(
    _objects: &mut vector<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_templated_object_immutable_ref_success<T: key + store>(
    _objects: &vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun vector_templated_object_by_value_fail<T: key + store>(
    objects: vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        let ObjectTemplated { t } = object;
        transfer::public_share_object(t);
    });
}

public fun vector_templated_object_mutable_ref_fail<T: key + store>(
    _objects: &mut vector<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
