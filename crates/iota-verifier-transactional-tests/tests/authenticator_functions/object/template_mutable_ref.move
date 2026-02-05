// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::object;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct Object<T> has key, store {
    id: iota::object::UID,
    t: T,
}

// FAIL
#[authenticator]
public fun template_mutable_ref<T: store>(
    _account: &Account,
    _object: &mut Object<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
