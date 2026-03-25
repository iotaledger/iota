// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Regression: auth_ctx.tx_commands() must continue to return the plain
// (non-enriched) Commands after the enriched AuthContext additions.

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::legacy;

use simple_abstract_account::abstract_account::AbstractAccount;

// PTB: SplitCoins, TransferObjects.
// Checks: tx_commands().length() == 2, correct variant for each command.
#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let cmds = auth_ctx.tx_commands();
    assert!(cmds.length() == 2, 0);
    assert!(cmds[0].is_split_coins(), 1);
    assert!(cmds[1].is_transfer_objects(), 2);
}

//# init-abstract-account --sender A --package-metadata object(3,0) --inputs "legacy" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
