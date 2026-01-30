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

// Receiving and option

// FAIL Invalid parameter type
#[authenticator]
public fun option_immutable_ref(
    _account: &Account,
    _objects: &Option<Receiving<Object>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
