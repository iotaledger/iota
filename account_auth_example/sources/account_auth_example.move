module account_auth_example::main_m;

use iota::account::public_authenticate_registry;

// temporary placeholder
public struct AuthContext has drop {}

public struct MAIN_M has drop {}

const AUTHENTICATE_FUNCTIONS: vector<vector<u8>> = vector[b"arg_value"];

fun init(witness: MAIN_M, ctx: &mut TxContext) {
    public_authenticate_registry(
        &witness,
        AUTHENTICATE_FUNCTIONS,
        ctx,
    );
    //public_authenticate_registry(
    //    &witness,
    //    vector[b"arg_value"],
    //    ctx,
    //);
}

public fun will_fail_at_execution_time_because_otw(ctx: &mut TxContext) {
    let a = AuthContext {};
    public_authenticate_registry(
        &a,
        AUTHENTICATE_FUNCTIONS,
        ctx,
    );
}
