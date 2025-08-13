// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::programmable_transaction_tests;

use iota::programmable_transaction::{
    new_input_argument,
    new_programmable_move_call,
    new_move_call,
    package_id,
    module_name,
    function_name,
    new_programmable_transaction
};
use std::type_name;

#[test]
fun create_ptb_move_call() {
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
    let mut arguments = vector[];
    let input_arg = new_input_argument(0);

    let mut type_names = vector[];
    let tn = type_name::get<u16>();
    type_names.push_back(tn);
    arguments.push_back(input_arg);

    let programmable_move_call = new_programmable_move_call(
        package_id,
        b"aabb".to_ascii_string(), // module name
        b"ccdd".to_ascii_string(), // function name
        type_names,
        arguments,
    );

    let call = new_move_call(programmable_move_call);
    // Create a programmable transaction with a double move call and no inputs
    let ptb = new_programmable_transaction(
        vector[],
        vector[call, call],
    );

    assert!(ptb.commands() == vector[call, call]);
    assert!(programmable_move_call.package_id() == package_id);
    assert!(programmable_move_call.module_name() == b"aabb".to_ascii_string());
    assert!(programmable_move_call.function_name() == b"ccdd".to_ascii_string());
}
