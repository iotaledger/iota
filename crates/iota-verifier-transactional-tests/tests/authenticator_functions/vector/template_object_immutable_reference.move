// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun template_object_immutable_reference<T: key>(
    _account: &Account,
    _objects: &vector<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
