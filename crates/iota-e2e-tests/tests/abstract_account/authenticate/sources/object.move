// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::object;

use iota::auth_context::AuthContext;

// Test account struct
public struct Account has key {
    id: UID,
}

// Object

public struct Object has key, store {
    id: iota::object::UID,
}

// PASS
#[authenticator]
public entry fun immutable_ref(
    _account: &Account,
    _object: &Object,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun by_value(
    _account: &Account,
    object: Object,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    let Object { id } = object;
    object::delete(id);
}

// FAIL
//#[authenticator]
public entry fun by_mutable_ref(
    _account: &Account,
    _object: &mut Object,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
