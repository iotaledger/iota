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
    new_programmable_transaction,
    new_pure,
    pure_data,
    object_data,
    split_coins_data
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

#[test]
#[expected_failure(abort_code = iota::programmable_transaction::EInvalidEnumVariant)]
fun test_pure_data_type_safety() {
    let pure_bytes = vector[1u8, 2u8, 3u8, 4u8];
    let pure_arg = new_pure(pure_bytes);

    // Test that pure_data returns the correct reference
    let retrieved_data = pure_data(&pure_arg);

    // Verify the data matches
    assert!(retrieved_data.length() == 4);
    assert!(*retrieved_data == pure_bytes);

    // Test that we can access individual elements through the reference
    // assert!((*retrieved_data)[0] == 1u8);
    // assert!((*retrieved_data)[3] == 4u8);

    //failing assert
    object_data(&pure_arg); // this will arise abort EInvalidEnumVariant
}

#[test]
#[expected_failure(abort_code = iota::programmable_transaction::EInvalidArgumentType)]
fun test_object_data_type_safety() {
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

    split_coins_data(&call); //this will arise abort EInvalidArgumentType
}
