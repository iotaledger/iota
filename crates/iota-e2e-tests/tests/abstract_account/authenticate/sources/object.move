module authenticate::object;

use iota::auth_context::AuthContext;

// Object

public struct Object has key, store {
    id: iota::object::UID,
}

public fun immutable_ref(
    _object: &Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

#[allow(lint(share_owned))]
public fun by_value(object: Object, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    transfer::public_share_object(object);
}

public fun by_mutable_ref(
    _object: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
