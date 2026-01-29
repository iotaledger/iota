// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::account;

use std::option::Option;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun option_mutable_ref(
    _account: &mut Option<Account>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
