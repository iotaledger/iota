// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::programmable_transaction_tests;

use iota::ptb::new_programmable_transaction_for_testing;
use iota::ptb_call_arg::{new_call_arg_pure_for_testing, as_pure_data, as_object_data};
use iota::ptb_command::{
    new_input_argument_for_testing,
    new_programmable_move_call_for_testing,
    new_move_call_command_for_testing,
    package,
    module_name,
    function,
    as_split_coins
};
use std::type_name;

#[test]
fun create_ptb_move_call() {
    let move_call_package = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
    let mut arguments = vector[];
    let input_arg = new_input_argument_for_testing(0);

    let mut type_names = vector[];
    let tn = type_name::get<u16>();
    type_names.push_back(tn);
    arguments.push_back(input_arg);

    let programmable_move_call = new_programmable_move_call_for_testing(
        move_call_package,
        b"my_module".to_ascii_string(), // module name
        b"my_function".to_ascii_string(), // function name
        type_names,
        arguments,
    );

    let call = new_move_call_command_for_testing(programmable_move_call);
    // Create a programmable transaction with a double move call and no inputs
    let ptb = new_programmable_transaction_for_testing(
        vector[],
        vector[call, call],
    );

    assert!(ptb.commands() == vector[call, call]);
    assert!(programmable_move_call.package() == move_call_package);
    assert!(programmable_move_call.module_name() == b"my_module".to_ascii_string());
    assert!(programmable_move_call.function() == b"my_function".to_ascii_string());
}

#[test]
fun test_pure_data_type_safety() {
    let pure_bytes = vector[1u8, 2u8, 3u8, 4u8];
    let pure_arg = new_call_arg_pure_for_testing(pure_bytes);

    // Test that pure_data returns the correct reference
    let retrieved_data = pure_arg.as_pure_data();

    // Verify the data matches
    assert!(retrieved_data.is_some());
    let retrieved_data = retrieved_data.borrow();
    assert!(retrieved_data.length() == 4);
    assert!(*retrieved_data == pure_bytes);

    assert!(pure_arg.as_object_data().is_none()); // this will not be an object data
}

#[test]
fun test_object_data_type_safety() {
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
    let mut arguments = vector[];
    let input_arg = new_input_argument_for_testing(0);

    let mut type_names = vector[];
    let tn = type_name::get<u16>();
    type_names.push_back(tn);
    arguments.push_back(input_arg);

    let programmable_move_call = new_programmable_move_call_for_testing(
        package_id,
        b"my_module".to_ascii_string(), // module name
        b"my_function".to_ascii_string(), // function name
        type_names,
        arguments,
    );

    let call = new_move_call_command_for_testing(programmable_move_call);

    assert!(call.as_split_coins().is_none()); // this will not be a split coins command
}
