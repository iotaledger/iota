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

// Receiving and vector

// FAIL Invalid parameter type
#[authenticator]
public fun vector_immutable_ref(
    _account: &Account,
    _objects: &vector<Receiving<Object>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
