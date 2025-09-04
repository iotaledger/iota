module account_auth_example::main_m;

use iota::auth_context::AuthContext;

public struct AUTH_ARG_VALUE has drop {}

public fun arg_value(_val: u8, _auth_ctx: &AuthContext, _ctx: &TxContext) {}
