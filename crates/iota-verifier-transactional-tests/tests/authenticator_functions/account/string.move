// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

use std::string::String;

// FAIL
#[authenticator]
public fun string(_account: String, _actx: &AuthContext, _ctx: &TxContext) {}
