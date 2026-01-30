// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// authenticator is not attached to the abstract account

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::authenticate;

use iota::package_metadata::PackageMetadataV1;
use std::ascii;

public struct AbstractAccount has key {
    id: UID,
}

public fun create(
    _package_metadata: &PackageMetadataV1,
    _module_name: ascii::String,
    _function_name: ascii::String,
    ctx: &mut TxContext,
): address {
    let account = AbstractAccount { id: object::new(ctx) };
    let account_address = object::id_address(&account);
    iota::transfer::share_object(account);
    account_address
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-account --sender A --package-metadata object(1,1) --inputs "authenticate" "authenticate" --create-function test::authenticate::create --account-type test::authenticate::AbstractAccount

//# view-object 2,1

//# abstract --account immshared(2,1) --ptb-inputs 100 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(1));
