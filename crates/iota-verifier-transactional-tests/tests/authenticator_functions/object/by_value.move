// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::object;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL
#[authenticator]
public fun by_value(_account: &Account, object: Object, _actx: &AuthContext, _ctx: &TxContext) {
    let Object { id } = object;
    object::delete(id);
}
