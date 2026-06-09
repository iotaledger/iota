// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::receiving;

use iota::transfer::Receiving;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun receiving(_account: &Receiving<Account>, _actx: &AuthContext, _ctx: &TxContext) {}
