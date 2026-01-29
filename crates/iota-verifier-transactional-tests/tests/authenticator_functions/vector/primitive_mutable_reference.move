// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun primitive_mutable_reference(
    _account: &Account,
    _arg: &mut vector<u8>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
