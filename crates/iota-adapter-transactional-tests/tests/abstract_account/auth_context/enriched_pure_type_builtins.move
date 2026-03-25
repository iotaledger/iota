// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests TypeName inference for pure PTB inputs from built-in commands.
// Types come from protocol constants, not from the VM:
//   SplitCoins amounts   -> u64   (by IOTA PTB spec)
//   TransferObjects recipient -> address (by IOTA PTB spec)

//# init --addresses test=0x0 simple_abstract_account=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/simple_abstract_account.move

//# publish --sender A --dependencies simple_abstract_account
module test::pure_types;

use simple_abstract_account::abstract_account::AbstractAccount;
use std::type_name;

// PTB: Input(0)=100u64 -> SplitCoins amount -> u64
//      Input(1)=@A     -> TransferObjects recipient -> address
// Expected: inputs[0].pure_type_name() == TypeName("u64")
//           inputs[1].pure_type_name() == TypeName("address")
#[authenticator]
public fun authenticate(
    _account: &AbstractAccount,
    auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let inputs = auth_ctx.enriched_tx_inputs();
    let pure_type1 = &inputs[0].pure_type_name().extract();
    assert!(pure_type1 == type_name::get<u64>(), 0);
    let pure_type2 = &inputs[1].pure_type_name().extract();
    assert!(pure_type2 == type_name::get<address>(), 1);
}

//# init-abstract-account --sender A --package-metadata object(3,0) --inputs "pure_types" "authenticate" --create-function simple_abstract_account::abstract_account::create --account-type simple_abstract_account::abstract_account::AbstractAccount

//# view-object 4,2

//# abstract --account immshared(4,2) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
