// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// wrong account type type for the authenticator

//# init --addresses test=0x0 --accounts A

//# publish --sender A
module test::abstract_account;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;

public struct AbstractAccount has key {
    id: UID,
}

public struct AbstractAccount2 has key {
    id: UID,
}

public fun create(authenticator: AuthenticatorInfoV1<AbstractAccount>, ctx: &mut TxContext) {
    let mut account = AbstractAccount { id: object::new(ctx) };
    let authenticator_compatibility_proof = account::check_auth_info_v1_compatibility(
        &account,
        authenticator,
    );

    account::attach_auth_info_v1(&mut account.id, authenticator_compatibility_proof);
    iota::transfer::share_object(account);
}

public fun authenticate(_account: &AbstractAccount2, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs @test "abstract_account" "authenticate"
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(0), Input(1), Input(2));
//> 1: test::abstract_account::create(Result(0));
