// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::receiving;

use iota::transfer::Receiving;

public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL Invalid parameter type
#[authenticator]
public fun vector_by_value(
    _account: &Account,
    to_receive: vector<Receiving<Object>>,
    parent: &mut Object,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
