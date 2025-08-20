module account_auth_example::template_m;

// temporary placeholder
public struct AuthContext has drop {}

public fun bind_to_test_module() {}

public struct Object has key, store {
    id: iota::object::UID,
}

// Template

#[allow(unused_field)]
public struct NonObjectTemplated<T: copy + drop + store> has copy, drop, store {
    t: T,
}

public fun template_primitive_success<T: copy + drop + store>(
    _arg: T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_immutable_ref_success<T: copy + drop + store>(
    _arg: &NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_mutable_ref_fail<T: copy + drop + store>(
    _arg: &mut NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_non_object_by_value_fail<T: copy + drop + store>(
    _arg: NonObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Template and object

public fun template_object_immutable_ref_success<T: key>(
    _object: &T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun template_object_by_value_fail<T: key + store>(
    object: T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    transfer::public_share_object(object);
}

public fun template_object_mutable_ref_fail<T: key>(
    _object: &mut T,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(unused_field)]
public struct ObjectTemplated<T: key + store> has copy, drop, store {
    t: T,
}

public fun templated_object_immutable_ref_success<T: key + store>(
    _object: &ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

public fun templated_object_by_value_fail<T: key + store>(
    object: ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let ObjectTemplated { t } = object;
    transfer::public_share_object(t);
}

public fun templated_object_mutable_ref_fail<T: key + store>(
    _object: &mut ObjectTemplated<T>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
