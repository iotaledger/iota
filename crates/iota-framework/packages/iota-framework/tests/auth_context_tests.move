// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::auth_context_tests;

use iota::auth_context::{new_with_tx_inputs, digest};
use iota::ptb_command::{
    new_input_argument_for_testing,
    new_programmable_move_call_for_testing,
    new_move_call_command_for_testing
};
use std::type_name;

#[test]
fun create_auth_context() {
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
    let mut arguments = vector[];
    let input_arg = new_input_argument_for_testing(0);

    let mut type_names = vector[];
    let tn = type_name::get<u16>();
    type_names.push_back(tn);
    arguments.push_back(input_arg);

    let programmable_move_call = new_programmable_move_call_for_testing(
        package_id,
        b"aabb".to_ascii_string(), // module name
        b"ccdd".to_ascii_string(), // function name
        type_names,
        arguments,
    );

    let call = new_move_call_command_for_testing(programmable_move_call);

    let ctx = new_with_tx_inputs(
        vector[1, 2, 3],
        vector[],
        vector[call],
    );

    assert!(ctx.digest() == vector[1, 2, 3]);
    assert!(ctx.tx_inputs() == vector[]);
    assert!(ctx.tx_commands() == vector[call]);
}
