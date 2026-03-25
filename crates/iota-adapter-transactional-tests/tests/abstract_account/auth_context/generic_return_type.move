// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::authenticate;

use simple_abstract_account::abstract_account::AbstractAccount;
use std::type_name;

/// Generic function: return type is TyParam(0) at the VM level.
/// type_to_type_tag_with_subst substitutes it with the concrete type arg.
public fun echo<T: copy + drop>(v: T): T { v }

#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let cmds = auth_context::enriched_tx_commands(auth_ctx);
    // PTB command 0 is: test::authenticate::echo<u64>(Input(0))
    let call = cmds[0].as_move_call().extract();
    let returns = call.returns();
    // With substitution:    returns = [TypeName("u64")], length 1 - passes.
    // Without substitution: returns = [],                length 0 - aborts.
    assert!(returns.length() == 1, 0);
    assert!(returns[0] == type_name::get<u64>(), 1);
}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: test::authenticate::echo<u64>(Input(0));
//> 1: SplitCoins(Gas, [Input(0)]);
//> 2: TransferObjects([Result(1)], Input(1));
