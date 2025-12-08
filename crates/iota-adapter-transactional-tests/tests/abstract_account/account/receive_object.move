// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// absract account can receive objects

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::coin::Coin;
use iota::iota::IOTA;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
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

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

public fun receive_object(
    self: &mut AbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    _ctx: &TxContext,
) {
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.id.to_address());
}

//# programmable --sender A --inputs object(1,1) "abstract_account" "authenticate"
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(0), Input(1), Input(2));
//> 1: test::abstract_account::create(Result(0));

//# view-object 2,1

//# set-address a_account object(2,1)

//# programmable --sender A --inputs 2000000000 @a_account
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# abstract --account immshared(2,1) --ptb-inputs object(2,1) receiving(5,0)
//> 0: test::abstract_account::receive_object(Input(0), Input(1));

//# view-object 5,0
