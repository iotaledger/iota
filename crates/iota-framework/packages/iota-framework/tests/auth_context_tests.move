// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::auth_context_tests;

use iota::auth_context::{new_with_tx_inputs, digest};
use iota::ptb_call_arg::{
    new_call_arg_pure_for_testing,
    new_object_arg_shared_for_testing,
    new_call_arg_object_for_testing
};
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

    let pure_call_arg = new_call_arg_pure_for_testing(b"pure");
    let shared_obj_arg = new_object_arg_shared_for_testing(
        object::id_from_address(@0x123),
        1,
        true,
    );
    let shared_obj_call_arg = new_call_arg_object_for_testing(shared_obj_arg);

    let digest = b"00000000000000000000000000000001";

    let ctx = new_with_tx_inputs(
        digest,
        vector[pure_call_arg, shared_obj_call_arg],
        vector[call],
    );

    assert!(ctx.digest() == digest);
    assert!(ctx.tx_inputs() == vector[pure_call_arg, shared_obj_call_arg]);
    assert!(ctx.tx_commands() == vector[call]);
}
