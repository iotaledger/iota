// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

use iota::auth_context::AuthContext;

// FAIL
#[authenticator]
public fun primitive(_account: u64, _actx: &AuthContext, _ctx: &TxContext) {}
