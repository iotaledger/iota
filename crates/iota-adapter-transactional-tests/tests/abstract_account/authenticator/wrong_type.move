// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// wrong account type type for the authenticator

//# init --addresses test=0x0 aa=0x0 --accounts A

//# publish-dependencies --paths crates/iota-adapter-transactional-tests/data/account_abstraction/abstract_account.move

//# publish --sender A --dependencies aa
module test::authenticate;

use iota::auth_context::AuthContext;

public struct AbstractAccount2 has key {
    id: UID,
}

#[authenticator]
public fun authenticate(_account: &AbstractAccount2, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-account --sender A --package-metadata object(3,1) --inputs "authenticate" "authenticate" --aa-create-fn-path aa::abstract_account::create --aa-type AbstractAccount
