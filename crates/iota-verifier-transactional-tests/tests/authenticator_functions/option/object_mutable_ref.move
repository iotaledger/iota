// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::option;

public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun object_mutable_ref(
    _account: &Account,
    _objects: &mut Option<Object>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
