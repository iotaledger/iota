// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::receiving;

use iota::auth_context::AuthContext;
use iota::transfer::Receiving;
use iota::vec_map::VecMap;

// Receiving

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL
public fun immutable_ref(
    _to_receive: &Receiving<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(lint(share_owned))]
public fun by_value(
    to_receive: Receiving<Object>,
    parent: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    let object = transfer::public_receive(&mut parent.id, to_receive);
    transfer::public_share_object(object);
}

// FAIL
public fun by_mutable_ref(
    _to_receive: &mut Receiving<Object>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// Receiving and vector

// FAIL
public fun vector_immutable_ref(
    _objects: &vector<Receiving<Object>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun vector_mutable_ref(
    _objects: &mut vector<Receiving<Object>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(lint(share_owned))]
public fun vector_by_value(
    to_receive: vector<Receiving<Object>>,
    parent: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    to_receive.do!(|to_receive| {
        let object = transfer::public_receive(&mut parent.id, to_receive);
        transfer::public_share_object(object);
    });
}

// Receiving and option

// FAIL
public fun option_immutable_ref(
    _objects: &Option<Receiving<Object>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
public fun option_mutable_ref(
    _objects: &mut Option<Receiving<Object>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
#[allow(lint(share_owned))]
public fun option_by_value(
    to_receive: Option<Receiving<Object>>,
    parent: &mut Object,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {
    to_receive.do!(|to_receive| {
        let object = transfer::public_receive(&mut parent.id, to_receive);
        transfer::public_share_object(object);
    });
}

// Receiving and datatype instantiation

// FAIL
public fun datatype_inst_immutable_ref(
    _to_receive: &VecMap<u8, Receiving<Object>>,
    _auth_ctx: &AuthContext,
    _ctx: &TxContext,
) {}
