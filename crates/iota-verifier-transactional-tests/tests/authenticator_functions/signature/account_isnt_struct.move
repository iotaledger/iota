// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

use iota::auth_context::AuthContext;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun account_isnt_struct(_account: u64, _actx: &AuthContext, _ctx: &TxContext) {}
