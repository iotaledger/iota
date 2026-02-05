// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::vector;

public struct Account has key {
    id: UID,
}

// FAIL
#[authenticator]
public fun template_object_by_value<T: key + store>(
    _account: &Account,
    objects: vector<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    objects.do!(|object| transfer::public_share_object(object));
}
