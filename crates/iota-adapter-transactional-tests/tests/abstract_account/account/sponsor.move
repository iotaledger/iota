// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authenticate test for abstract accounts with sponsorship

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    _public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
) {
    let account = AbstractAccount { id: object::new(ctx) };

    account::create_account_v1(account, authenticator);
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs x"10" object(1,1) "abstract_account" "authenticate" 7000000000
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));

//# view-object 2,1

//# abstract --account immshared(2,1) --sponsor A --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 4,0
