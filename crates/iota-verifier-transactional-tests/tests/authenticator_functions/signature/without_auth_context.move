// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun without_auth_context(_account: &Account, _ctx: &TxContext) {}
