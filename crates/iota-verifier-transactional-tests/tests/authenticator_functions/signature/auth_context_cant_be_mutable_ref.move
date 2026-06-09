// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::signature;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun auth_context_cant_be_mutable_ref(
    _account: &Account,
    _actx: &mut AuthContext,
    _ctx: &TxContext,
) {}
