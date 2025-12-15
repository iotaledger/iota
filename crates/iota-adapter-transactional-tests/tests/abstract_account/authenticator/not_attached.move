// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticator is not attached to the abstract account

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::AuthenticatorInfoV1;
use iota::auth_context::AuthContext;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    _public_key: vector<u8>,
    _authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let account = AbstractAccount { id: object::new(ctx) };
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs x"10" object(1,1) "abstract_account" "authenticate" 7000000000
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));
//> 2: SplitCoins(Gas, [Input(4)]);
//> 3: TransferObjects([Result(2)], Result(1));

//# view-object 2,1

//# view-object 2,0

//# abstract --account immshared(2,1) --gas-payment 2,0 --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
