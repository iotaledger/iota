// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// absract account can receive objects

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::authenticate;

use iota::account;
use iota::auth_context::AuthContext;
use iota::coin::Coin;
use iota::iota::IOTA;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let authenticator = account::create_auth_info_v1<AbstractAccount>(
        package_metadata,
        module_name,
        function_name,
    );
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );
    account::attach_auth_info_v1(account.uid_mut(), authenticator_compatibility_proof);
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

public fun uid_mut(self: &mut AbstractAccount): &mut UID {
    &mut self.id
}

public fun receive_object(
    self: &mut AbstractAccount,
    coin: transfer::Receiving<Coin<IOTA>>,
    _ctx: &TxContext,
) {
    let received_coin = transfer::public_receive(&mut self.id, coin);
    transfer::public_transfer(received_coin, self.id.to_address());
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs object(1,1) "authenticate" "authenticate"
//> 0: test::authenticate::create(Input(0), Input(1), Input(2));

//# view-object 2,1

//# set-address a_account object(2,1)

//# programmable --sender A --inputs 2000000000 @a_account
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# abstract --account immshared(2,1) --ptb-inputs object(2,1) receiving(5,0)
//> 0: test::authenticate::receive_object(Input(0), Input(1));

//# view-object 5,0
