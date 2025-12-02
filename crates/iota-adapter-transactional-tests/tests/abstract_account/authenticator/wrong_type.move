// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// wrong account type type for the authenticator 

//# init --addresses test=0x0 --accounts A --default-aa

//# publish --sender A --dependencies aa
module test::authenticate;

use iota::auth_context::AuthContext;

public struct AbstractAccount2 has key {
    id: UID,
}

public fun authenticate(_account: &AbstractAccount2, _auth_ctx: &AuthContext, _ctx: &TxContext) {}

//# init-abstract-acc --sender A test authenticate authenticate