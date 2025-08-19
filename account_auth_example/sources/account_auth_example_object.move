module account_auth_example::object_m;

// temporary placeholder
public struct AuthContext has drop {}

public fun bind_to_test_module() {}

// Object

public struct Object has key, store {
    id: iota::object::UID,
}

public fun object_immutable_ref_success(
    _object: &Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(lint(share_owned))]
public fun object_by_value_fail(object: Object, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    transfer::public_share_object(object);
}

public fun object_by_mutable_ref_fail(
    _object: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
