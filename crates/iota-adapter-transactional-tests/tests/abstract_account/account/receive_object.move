// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// absract account can receive objects

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::authenticate;

use iota::account;
use iota::authenticator_function;
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
    let authenticator = authenticator_function::create_auth_function_ref_v1<AbstractAccount>(
        package_metadata,
        module_name,
        function_name,
    );

    let account = AbstractAccount { id: object::new(ctx) };

    let account_address = object::id_address(&account);

    account::create_account_v1(account, authenticator);

    account_address
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

//# init-abstract-account --sender A --package-metadata object(1,1) --inputs "authenticate" "authenticate" --create-function test::authenticate::create --account-type test::authenticate::AbstractAccount

//# view-object 2,2

//# set-address a_account object(2,2)

//# programmable --sender A --inputs 2000000000 @a_account
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));

//# abstract --account immshared(2,2) --ptb-inputs object(2,2) receiving(5,0)
//> 0: test::authenticate::receive_object(Input(0), Input(1));

//# view-object 5,0
