// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::receiving;

use iota::transfer::Receiving;
use iota::vec_map::VecMap;

public struct Account has key {
    id: UID,
}

public struct Object has key, store {
    id: iota::object::UID,
}

// Receiving and datatype instantiation

// FAIL Invalid parameter type
#[authenticator]
public fun datatype_inst_immutable_ref(
    _account: &Account,
    _to_receive: &VecMap<u8, Receiving<Object>>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}
