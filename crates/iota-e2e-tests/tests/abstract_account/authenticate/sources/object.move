// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::object;

use authenticate::account::Account;
use iota::auth_context::AuthContext;

// Object

public struct Object has key, store {
    id: iota::object::UID,
}

// PASS
public fun immutable_ref(
    _account: &Account,
    _object: &Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(lint(share_owned))]
public fun by_value(_account: &Account, object: Object, _auth_ctx: &AuthContext, _ctx: &TxContext) {
    transfer::public_share_object(object);
}

// FAIL
public fun by_mutable_ref(
    _account: &Account,
    _object: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
