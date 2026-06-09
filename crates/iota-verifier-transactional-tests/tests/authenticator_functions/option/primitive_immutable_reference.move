// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::option;

public struct Account has key {
    id: UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun primitive_immutable_reference(
    _account: &Account,
    _arg: &Option<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
