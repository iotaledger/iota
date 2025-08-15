module account_auth_example::m;

// temporary placeholder
public struct AuthContext has drop {}

public fun bind_to_test_module() {}

public fun minimally_viable_auth_function(_auth_ctx: &AuthContext, _ctx: &TxContext) {}

#[allow(unused_function)]
fun has_to_be_public_auth_function(_auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun at_least_two_args(_ctx: &TxContext) {}

public fun auth_context_cant_be_value(_auth_ctx: AuthContext, _ctx: &TxContext) {}

public fun auth_context_cant_be_mutable_ref(_auth_ctx: &mut AuthContext, _ctx: &TxContext) {}

public fun tx_context_cant_be_value(_auth_ctx: &AuthContext, _ctx: TxContext) {}

public fun tx_context_cant_be_mutable_ref(_auth_ctx: &AuthContext, _ctx: &mut TxContext) {}

public fun auth_context_isnt_struct(_auth_ctx: u64, _ctx: &TxContext) {}

public fun tx_context_isnt_struct(_auth_ctx: &AuthContext, _ctx: u64) {}

public struct Object has key {
    id: iota::object::UID,
}

public fun arg_immutable_ref(_object: &Object, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun arg_value(_val: u8, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun arg_mutable_value(mut _val: u8, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun arg_template<T: drop>(_val: T, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

#[allow(unused_field)]
public struct Templated<T: drop> has drop {
    t: T
}

public fun arg_templated_struct<T: drop>(_val: Templated<T>, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public struct Drop has drop {}

public fun with_vector(_data: vector<Drop>, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun with_vector_template<T: drop>(_data: vector<Templated<T>>, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun with_signer(_s: signer, _auth_ctx: &AuthContext, _ctx: &TxContext) {}
