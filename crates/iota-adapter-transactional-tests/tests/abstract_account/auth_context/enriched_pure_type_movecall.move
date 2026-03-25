// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests TypeName inference for a pure PTB input from a MoveCall parameter.
// The VM resolves the type from the loaded function signature;
// type_to_type_tag_with_subst substitutes TyParam(0) with the call-site type.

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::pure_types;

use simple_abstract_account::abstract_account::AbstractAccount;
use std::type_name;

/// Generic identity function. Return type is TyParam(0) at VM level;
/// type_to_type_tag_with_subst substitutes it with the concrete type arg.
public fun echo<T: copy + drop>(v: T): T { v }

// PTB: Input(0)=100u64 passed to echo<u64>(Input(0)).
// Expected: enriched_tx_inputs()[0].pure_type_name() == TypeName("u64")
#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let inputs = auth_ctx.enriched_tx_inputs();
    let pure_type = &inputs[0].pure_type_name().extract();
    assert!(pure_type == type_name::get<u64>(), 0);
}

//# init-abstract-account --sender A --package-metadata object(3,0) --inputs "pure_types" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: test::pure_types::echo<u64>(Input(0));
//> 1: SplitCoins(Gas, [Input(0)]);
//> 2: TransferObjects([Result(1)], Input(1));
