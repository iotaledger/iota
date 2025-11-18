// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// simple authentication using abstract account

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    _public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
): address {
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );
    account::attach_auth_info_v1(&mut account.id, authenticator_compatibility_proof);
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

public fun authenticate_hello_world(
    _account: &AbstractAccount,
    msg: ascii::String,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    assert!(msg == ascii::string(b"HelloWorld"), 0);
}

//# programmable --sender A --inputs x"10" @test "abstract_account" "authenticate_hello_world" 7000000000
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));
//> 2: SplitCoins(Gas, [Input(4)]);
//> 3: TransferObjects([Result(2)], Result(1));

//# view-object 2,0

//# view-object 2,2

//# abstract --account immshared(2,2) --gas-payment 2,0 --auth-inputs "HelloWorld" --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# view-object 5,0
