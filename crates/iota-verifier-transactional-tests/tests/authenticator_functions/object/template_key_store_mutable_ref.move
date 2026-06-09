// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::object;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct ObjectObject<T: key + store> has key, store {
    id: iota::object::UID,
    t: T,
}

// FAIL
#[authenticator]
public fun template_key_store_mutable_ref<T: key + store>(
    _account: &Account,
    _object: &mut ObjectObject<T>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
