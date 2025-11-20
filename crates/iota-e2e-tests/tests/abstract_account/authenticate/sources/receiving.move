// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module authenticate::receiving;

use iota::auth_context::AuthContext;
use iota::transfer::Receiving;

// Receiving

public struct Object has key, store {
    id: iota::object::UID,
}

// FAIL
//#[authenticator]
public entry fun immutable_ref(
    _to_receive: &Receiving<Object>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// FAIL
//#[authenticator]
public entry fun by_value(
    to_receive: Receiving<Object>,
    parent: &mut Object,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {
    let object = transfer::public_receive(&mut parent.id, to_receive);
    let Object { id } = object;
    object::delete(id);
}

// FAIL
//#[authenticator]
public entry fun by_mutable_ref(
    _to_receive: &mut Receiving<Object>,
    _actx: &AuthContext,
    _ctx: &TxContext,
) {}

// Receiving and vector

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun vector_immutable_ref(
//    _objects: &vector<Receiving<Object>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun vector_mutable_ref(
//    _objects: &mut vector<Receiving<Object>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun vector_by_value(
//    to_receive: vector<Receiving<Object>>,
//    parent: &mut Object,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Receiving and option

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun option_immutable_ref(
//    _objects: &Option<Receiving<Object>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun option_mutable_ref(
//    _objects: &mut Option<Receiving<Object>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun option_by_value(
//    to_receive: Option<Receiving<Object>>,
//    parent: &mut Object,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}

// Receiving and datatype instantiation

// FAIL Invalid 'entry' parameter type
//#[authenticator]
//public entry fun datatype_inst_immutable_ref(
//    _to_receive: &VecMap<u8, Receiving<Object>>,
//    _actx: &AuthContext,
//    _ctx: &TxContext,
//) {}
