// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::template;

public struct Account has key {
    id: UID,
}

// PASS
#[authenticator]
public fun object_immutable_ref<T: key>(
    _account: &Account,
    _object: &T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
