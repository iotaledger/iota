// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::auth_context_tests;

use std::type_name::{Self, TypeName};
use iota::programmable_transaction::{ProgrammableMoveCall ,new_input_argument,new_programmable_move_call, Argument, new_move_call, get_package_id, get_module_name,get_function_name};
use iota::object;
use std::ascii::String;

#[test]
fun create_ptb_move_call() {

    let mut ctx = tx_context::dummy();
    let id = object::new(&mut ctx);
    let package_id = id.to_inner();

    let mut arguments = vector::empty<Argument>();
    let input_arg = new_input_argument(0);

    let mut type_names = vector::empty<TypeName>();
    let tn = type_name::get<u16>();
    vector::push_back(&mut type_names, tn);
    vector::push_back(&mut arguments, input_arg);

    let programmable_move_call = new_programmable_move_call(
        package_id,
        b"aabb".to_ascii_string(), // module name
        b"ccdd".to_ascii_string(), // function name
        type_names, 
        arguments
    );

    let call = new_move_call(
        programmable_move_call
    );
    assert!(get_package_id(&programmable_move_call) == package_id);
    assert!(get_module_name(&programmable_move_call) == b"aabb".to_ascii_string());
    assert!(get_function_name(&programmable_move_call) == b"ccdd".to_ascii_string());

    id.delete();
}



