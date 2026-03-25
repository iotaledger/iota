// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Regression: auth_ctx.tx_inputs() must continue to return the plain
// (non-enriched) CallArgs after the enriched AuthContext additions.

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::legacy;

use simple_abstract_account::abstract_account::AbstractAccount;

// PTB: Input(0)=100u64 (pure), Input(1)=@A (pure).
// Checks: tx_inputs().length() == 2, both variants are pure data.
#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let inputs = auth_ctx.tx_inputs();
    assert!(inputs.length() == 2, 0);
    assert!(inputs[0].is_pure_data(), 1);
    assert!(inputs[1].is_pure_data(), 2);
}

//# init-abstract-account --sender A --package-metadata object(3,0) --inputs "legacy" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
