module account_auth_example::option_m;

use iota::auth_context::AuthContext;

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

// Option

public fun option_primitive_immutable_reference_success(
    _arg: &Option<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_primitive_mutable_reference_fail(
    _arg: &mut Option<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_primitive_by_value_success(
    _arg: Option<u8>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_non_object_immutable_ref_success(
    _arg: &Option<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_non_object_mutable_ref_fail(
    _arg: &mut Option<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_non_object_by_value_fail(
    _arg: Option<NonObject>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Option and object

public fun option_object_immutable_ref_success(
    _objects: &Option<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_object_mutable_ref_fail(
    _objects: &mut Option<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(lint(share_owned))]
public fun option_object_by_value_fail(
    objects: Option<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

// Option and template

// error[E06001]: unused value without 'drop'
//public fun option_template_non_object_by_value_success<T>(
//    _arg: Option<T>,
//    _auth_ctx: &AuthContext,
//    _ctx: &TxContext,
//) {}

public fun option_template_non_object_immutable_ref_success<T>(
    _arg: &Option<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_template_non_object_mutable_ref_fail<T>(
    _arg: &mut Option<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_templated_non_object_by_value_fail<T: copy + drop + store>(
    _arg: Option<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_templated_non_object_immutable_ref_success<T: copy + drop + store>(
    _arg: &Option<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_templated_non_object_mutable_ref_fail<T: copy + drop + store>(
    _arg: &mut Option<NonObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Option, template and object

public fun option_template_object_immutable_reference_success<T: key>(
    _objects: &Option<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_template_object_by_value_fail<T: key + store>(
    objects: Option<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}

public fun option_template_object_mutable_reference_fail<T: key>(
    _objects: &mut Option<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_templated_object_immutable_ref_success<T: key + store>(
    _objects: &Option<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun option_templated_object_by_value_fail<T: key + store>(
    objects: Option<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| {
        let ObjectTemplated { t } = object;
        transfer::public_share_object(t);
    });
}

public fun option_templated_object_mutable_ref_fail<T: key + store>(
    _objects: &mut Option<ObjectTemplated<T>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
