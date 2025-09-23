module account_auth_example::main_m;

use iota::auth_context::AuthContext;

public struct AUTH_SOME_AUTHENTICATE_FN has drop {}

// WON'T BUILD WITH THESE
// public struct AUTH_FAIL has drop {}
// public struct AUTH_NOT_AUTHENTICATE has drop {}

public fun some_authenticate_fn(_val: u8, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun not_authenticate(_val: u8, _ctx: &TxContext) {}
