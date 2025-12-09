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

public fun create(
    _public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1<AbstractAccount>,
    ctx: &mut TxContext,
) {
    let account = AbstractAccount { id: object::new(ctx) };

    account::create_account_v1(account, authenticator);
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount2, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# programmable --sender A --inputs x"10" object(1,1) "abstract_account" "authenticate"
//> 0: iota::account::create_auth_info_v1<test::abstract_account::AbstractAccount>(Input(1), Input(2), Input(3));
//> 1: test::abstract_account::create(Input(0), Result(0));
