// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::template;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun object_by_value<T: key + store>(
    _account: &Account,
    object: T,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    transfer::public_share_object(object);
}
