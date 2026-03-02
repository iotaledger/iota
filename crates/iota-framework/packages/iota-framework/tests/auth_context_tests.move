// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::auth_context_tests;

use iota::auth_context::{new_with_tx_inputs, digest};
use iota::ptb::new_programmable_transaction_for_testing;
use iota::ptb_call_arg::{
    new_call_arg_pure_for_testing,
    new_call_arg_object_for_testing,
    new_object_arg_imm_or_owned_for_testing,
    new_object_arg_shared_for_testing,
    new_object_arg_receiving_for_testing,
    new_object_ref_for_testing
};
use iota::ptb_command::{
    new_gas_coin_argument_for_testing,
    new_input_argument_for_testing,
    new_result_argument_for_testing,
    new_nested_result_argument_for_testing,
    new_programmable_move_call_for_testing,
    new_move_call_command_for_testing,
    new_transfer_objects_for_testing,
    new_transfer_objects_command_for_testing,
    new_split_coins_for_testing,
    new_split_coins_command_for_testing,
    new_merge_coins_for_testing,
    new_merge_coins_command_for_testing,
    new_publish_for_testing,
    new_publish_command_for_testing,
    new_make_move_vec_for_testing,
    new_make_move_vec_command_for_testing,
    new_upgrade_for_testing,
    new_upgrade_command_for_testing
};
use std::type_name;

const DIGEST: vector<u8> = b"00000000000000000000000000000001";
const OBJECT_DIGEST: vector<u8> = b"00000000000000000000000000000002";

// ---------------------------------------------------------------------------
// CallArg variants
// ---------------------------------------------------------------------------

#[test]
fun test_call_arg_pure_data() {
    let pure_arg = new_call_arg_pure_for_testing(b"hello");
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[pure_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    assert!(inputs.length() == 1);
    assert!(inputs[0].is_pure_data());
    assert!(inputs[0].as_pure_data() == option::some(b"hello"));
}

#[test]
fun test_call_arg_object_data() {
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0xA),
        42,
        OBJECT_DIGEST,
    );
    let obj_arg = new_object_arg_imm_or_owned_for_testing(obj_ref);
    let call_arg = new_call_arg_object_for_testing(obj_arg);
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[call_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    assert!(inputs.length() == 1);
    assert!(inputs[0].is_object_data());
}

// ---------------------------------------------------------------------------
// ObjectArg variants
// ---------------------------------------------------------------------------

#[test]
fun test_object_arg_imm_or_owned() {
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0xB),
        10,
        OBJECT_DIGEST,
    );
    let obj_arg = new_object_arg_imm_or_owned_for_testing(obj_ref);
    let call_arg = new_call_arg_object_for_testing(obj_arg);
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[call_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    let extracted_obj = inputs[0].as_object_data().destroy_some();
    assert!(extracted_obj.is_imm_or_owned_object());
    assert!(!extracted_obj.is_shared_object());
    assert!(!extracted_obj.is_receiving_object());
    assert!(extracted_obj.object_id().destroy_some().to_address() == @0xB);
    assert!(extracted_obj.object_version().destroy_some() == 10);
    assert!(extracted_obj.object_digest().destroy_some() == OBJECT_DIGEST);
}

#[test]
fun test_object_arg_shared() {
    let shared_arg = new_object_arg_shared_for_testing(
        object::id_from_address(@0xC),
        5,
        true,
    );
    let call_arg = new_call_arg_object_for_testing(shared_arg);
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[call_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    let extracted_obj = inputs[0].as_object_data().destroy_some();
    assert!(extracted_obj.is_shared_object());
    assert!(!extracted_obj.is_imm_or_owned_object());
    assert!(!extracted_obj.is_receiving_object());
    assert!(extracted_obj.object_id().destroy_some().to_address() == @0xC);
    assert!(extracted_obj.object_version().destroy_some() == 5);
    assert!(extracted_obj.is_mutable_shared_object().destroy_some() == true);
}

#[test]
fun test_object_arg_shared_immutable() {
    let shared_arg = new_object_arg_shared_for_testing(
        object::id_from_address(@0xD),
        7,
        false,
    );
    let call_arg = new_call_arg_object_for_testing(shared_arg);
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[call_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    let extracted_obj = inputs[0].as_object_data().destroy_some();
    assert!(extracted_obj.is_shared_object());
    assert!(extracted_obj.is_mutable_shared_object().destroy_some() == false);
}

#[test]
fun test_object_arg_receiving() {
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0xE),
        99,
        OBJECT_DIGEST,
    );
    let recv_arg = new_object_arg_receiving_for_testing(obj_ref);
    let call_arg = new_call_arg_object_for_testing(recv_arg);
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, vector[call_arg], vector[cmd]);

    let inputs = ctx.tx_inputs();
    let extracted_obj = inputs[0].as_object_data().destroy_some();
    assert!(extracted_obj.is_receiving_object());
    assert!(!extracted_obj.is_imm_or_owned_object());
    assert!(!extracted_obj.is_shared_object());
    assert!(extracted_obj.object_id().destroy_some().to_address() == @0xE);
    assert!(extracted_obj.object_version().destroy_some() == 99);
}

// ---------------------------------------------------------------------------
// Argument variants
// ---------------------------------------------------------------------------

#[test]
fun test_argument_gas_coin() {
    let gas = new_gas_coin_argument_for_testing();
    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
        b"mod".to_ascii_string(),
        b"fun".to_ascii_string(),
        vector[],
        vector[gas],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    let call = commands[0].as_move_call().destroy_some();
    let args = call.arguments();
    assert!(args.length() == 1);
    assert!(args[0].is_gas_coin());
}

#[test]
fun test_argument_input() {
    let input_arg = new_input_argument_for_testing(3);
    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
        b"mod".to_ascii_string(),
        b"fun".to_ascii_string(),
        vector[],
        vector[input_arg],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    let call = commands[0].as_move_call().destroy_some();
    let args = call.arguments();
    assert!(args[0].is_input());
    assert!(args[0].input_index().destroy_some() == 3);
}

#[test]
fun test_argument_result() {
    let result_arg = new_result_argument_for_testing(1);
    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
        b"mod".to_ascii_string(),
        b"fun".to_ascii_string(),
        vector[],
        vector[result_arg],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    let call = commands[0].as_move_call().destroy_some();
    let args = call.arguments();
    assert!(args[0].is_result());
    assert!(args[0].result_command_index().destroy_some() == 1);
}

#[test]
fun test_argument_nested_result() {
    let nested_arg = new_nested_result_argument_for_testing(2, 5);
    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
        b"mod".to_ascii_string(),
        b"fun".to_ascii_string(),
        vector[],
        vector[nested_arg],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    let call = commands[0].as_move_call().destroy_some();
    let args = call.arguments();
    assert!(args[0].is_nested_result());
    assert!(args[0].nested_result_command_index().destroy_some() == 2);
    assert!(args[0].nested_result_inner_index().destroy_some() == 5);
}

// ---------------------------------------------------------------------------
// Command variants
// ---------------------------------------------------------------------------

#[test]
fun test_command_move_call() {
    let package_id = object::id_from_bytes(iota::hash::blake2b256(&b"0x123"));
    let tn = type_name::get<u16>();
    let input_arg = new_input_argument_for_testing(0);

    let move_call = new_programmable_move_call_for_testing(
        package_id,
        b"my_module".to_ascii_string(),
        b"my_function".to_ascii_string(),
        vector[tn],
        vector[input_arg],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands.length() == 1);
    assert!(commands[0].is_move_call());
    let call = commands[0].as_move_call().destroy_some();
    assert!(*call.module_name() == b"my_module".to_ascii_string());
    assert!(*call.function() == b"my_function".to_ascii_string());
    assert!(call.type_arguments().length() == 1);
    assert!(call.arguments().length() == 1);
}

#[test]
fun test_command_transfer_objects() {
    let objects = vector[new_input_argument_for_testing(0), new_result_argument_for_testing(1)];
    let recipient = new_input_argument_for_testing(2);
    let data = new_transfer_objects_for_testing(objects, recipient);
    let cmd = new_transfer_objects_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_transfer_objects());
    let extracted = commands[0].as_transfer_objects().destroy_some();
    assert!(extracted.objects().length() == 2);
    assert!(extracted.recipient().is_input());
    assert!(extracted.recipient().input_index().destroy_some() == 2);
}

#[test]
fun test_command_split_coins() {
    let coin = new_gas_coin_argument_for_testing();
    let amounts = vector[new_input_argument_for_testing(0), new_input_argument_for_testing(1)];
    let data = new_split_coins_for_testing(coin, amounts);
    let cmd = new_split_coins_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_split_coins());
    let extracted = commands[0].as_split_coins().destroy_some();
    assert!(extracted.coin().is_gas_coin());
    assert!(extracted.amounts().length() == 2);
}

#[test]
fun test_command_merge_coins() {
    let target = new_gas_coin_argument_for_testing();
    let sources = vector[new_result_argument_for_testing(0), new_result_argument_for_testing(1)];
    let data = new_merge_coins_for_testing(target, sources);
    let cmd = new_merge_coins_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_merge_coins());
    let extracted = commands[0].as_merge_coins().destroy_some();
    assert!(extracted.target_coin().is_gas_coin());
    assert!(extracted.source_coins().length() == 2);
}

#[test]
fun test_command_publish() {
    let modules = vector[b"module_bytes_1", b"module_bytes_2"];
    let dep_id = object::id_from_address(@0xF1);
    let data = new_publish_for_testing(modules, vector[dep_id]);
    let cmd = new_publish_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_publish());
    let extracted = commands[0].as_publish().destroy_some();
    assert!(extracted.modules().length() == 2);
    assert!(extracted.dependencies().length() == 1);
}

#[test]
fun test_command_make_move_vec() {
    let tn = type_name::get<u64>();
    let elements = vector[new_input_argument_for_testing(0), new_input_argument_for_testing(1)];
    let data = new_make_move_vec_for_testing(option::some(tn), elements);
    let cmd = new_make_move_vec_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_make_move_vec());
    let extracted = commands[0].as_make_move_vec().destroy_some();
    assert!(extracted.type_arg().is_some());
    assert!(extracted.elements().length() == 2);
}

#[test]
fun test_command_make_move_vec_no_type() {
    let elements = vector[new_input_argument_for_testing(0)];
    let data = new_make_move_vec_for_testing(option::none(), elements);
    let cmd = new_make_move_vec_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_make_move_vec());
    let extracted = commands[0].as_make_move_vec().destroy_some();
    assert!(extracted.type_arg().is_none());
}

#[test]
fun test_command_upgrade() {
    let modules = vector[b"upgraded_module_bytes"];
    let dep_id = object::id_from_address(@0xF2);
    let package_id = object::id_from_address(@0xF3);
    let ticket = new_input_argument_for_testing(0);
    let data = new_upgrade_for_testing(modules, vector[dep_id], package_id, ticket);
    let cmd = new_upgrade_command_for_testing(data);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let commands = ctx.tx_commands();
    assert!(commands[0].is_upgrade());
    let extracted = commands[0].as_upgrade().destroy_some();
    assert!(extracted.upgrade_modules().length() == 1);
    assert!(extracted.upgrade_dependencies().length() == 1);
    assert!(extracted.upgrade_package().to_address() == @0xF3);
    assert!(extracted.upgrade_ticket().is_input());
}

// ---------------------------------------------------------------------------
// ProgrammableTransaction
// ---------------------------------------------------------------------------

#[test]
fun test_programmable_transaction() {
    let pure_arg = new_call_arg_pure_for_testing(b"data");
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0x1),
        1,
        OBJECT_DIGEST,
    );
    let imm_arg = new_call_arg_object_for_testing(new_object_arg_imm_or_owned_for_testing(obj_ref));
    let shared_arg = new_call_arg_object_for_testing(
        new_object_arg_shared_for_testing(object::id_from_address(@0x2), 3, true),
    );
    let recv_ref = new_object_ref_for_testing(
        object::id_from_address(@0x3),
        4,
        OBJECT_DIGEST,
    );
    let recv_arg = new_call_arg_object_for_testing(
        new_object_arg_receiving_for_testing(recv_ref),
    );

    let inputs = vector[pure_arg, imm_arg, shared_arg, recv_arg];

    // Build a variety of commands
    let move_call_cmd = new_move_call_command_for_testing(
        new_programmable_move_call_for_testing(
            object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
            b"mod".to_ascii_string(),
            b"fn".to_ascii_string(),
            vector[],
            vector[new_input_argument_for_testing(0)],
        ),
    );
    let transfer_cmd = new_transfer_objects_command_for_testing(
        new_transfer_objects_for_testing(
            vector[new_result_argument_for_testing(0)],
            new_input_argument_for_testing(1),
        ),
    );
    let split_cmd = new_split_coins_command_for_testing(
        new_split_coins_for_testing(
            new_gas_coin_argument_for_testing(),
            vector[new_input_argument_for_testing(0)],
        ),
    );
    let merge_cmd = new_merge_coins_command_for_testing(
        new_merge_coins_for_testing(
            new_gas_coin_argument_for_testing(),
            vector[new_result_argument_for_testing(2)],
        ),
    );
    let publish_cmd = new_publish_command_for_testing(
        new_publish_for_testing(vector[b"mod_bytes"], vector[]),
    );
    let make_vec_cmd = new_make_move_vec_command_for_testing(
        new_make_move_vec_for_testing(
            option::some(type_name::get<u8>()),
            vector[new_input_argument_for_testing(0)],
        ),
    );
    let upgrade_cmd = new_upgrade_command_for_testing(
        new_upgrade_for_testing(
            vector[b"upgraded"],
            vector[],
            object::id_from_address(@0xAA),
            new_input_argument_for_testing(0),
        ),
    );

    let commands = vector[
        move_call_cmd,
        transfer_cmd,
        split_cmd,
        merge_cmd,
        publish_cmd,
        make_vec_cmd,
        upgrade_cmd,
    ];

    // Test via ProgrammableTransaction struct
    let ptb = new_programmable_transaction_for_testing(inputs, commands);
    assert!(ptb.inputs().length() == 4);
    assert!(ptb.commands().length() == 7);

    // Also verify round-trip through auth context
    let ctx = new_with_tx_inputs(DIGEST, inputs, commands);
    let digest = DIGEST;

    assert!(ctx.digest() == &digest);
    assert!(ctx.tx_inputs() == inputs);
    assert!(ctx.tx_inputs().length() == 4);
    assert!(ctx.tx_commands() == commands);
    assert!(ctx.tx_commands().length() == 7);

    // Verify individual command variants survived the round-trip
    let rt_commands = ctx.tx_commands();
    assert!(rt_commands[0].is_move_call());
    assert!(rt_commands[1].is_transfer_objects());
    assert!(rt_commands[2].is_split_coins());
    assert!(rt_commands[3].is_merge_coins());
    assert!(rt_commands[4].is_publish());
    assert!(rt_commands[5].is_make_move_vec());
    assert!(rt_commands[6].is_upgrade());

    // Verify individual input variants survived the round-trip
    let rt_inputs = ctx.tx_inputs();
    assert!(rt_inputs[0].is_pure_data());
    assert!(rt_inputs[1].is_object_data());
    assert!(rt_inputs[1].as_object_data().destroy_some().is_imm_or_owned_object());
    assert!(rt_inputs[2].is_object_data());
    assert!(rt_inputs[2].as_object_data().destroy_some().is_shared_object());
    assert!(rt_inputs[3].is_object_data());
    assert!(rt_inputs[3].as_object_data().destroy_some().is_receiving_object());
}

// ---------------------------------------------------------------------------
// All inputs combined
// ---------------------------------------------------------------------------

#[test]
fun test_all_call_arg_and_object_arg_variants() {
    let pure = new_call_arg_pure_for_testing(b"bytes");

    let imm_ref = new_object_ref_for_testing(
        object::id_from_address(@0x10),
        1,
        OBJECT_DIGEST,
    );
    let imm = new_call_arg_object_for_testing(new_object_arg_imm_or_owned_for_testing(imm_ref));

    let shared = new_call_arg_object_for_testing(
        new_object_arg_shared_for_testing(object::id_from_address(@0x20), 2, false),
    );

    let recv_ref = new_object_ref_for_testing(
        object::id_from_address(@0x30),
        3,
        OBJECT_DIGEST,
    );
    let recv = new_call_arg_object_for_testing(new_object_arg_receiving_for_testing(recv_ref));

    let all_inputs = vector[pure, imm, shared, recv];
    let cmd = make_noop_move_call_command();

    let ctx = new_with_tx_inputs(DIGEST, all_inputs, vector[cmd]);

    let rt = ctx.tx_inputs();
    assert!(rt.length() == 4);
    assert!(rt[0].is_pure_data());
    assert!(rt[1].as_object_data().destroy_some().is_imm_or_owned_object());
    assert!(rt[2].as_object_data().destroy_some().is_shared_object());
    assert!(rt[3].as_object_data().destroy_some().is_receiving_object());
}

// ---------------------------------------------------------------------------
// All argument variants in a single command
// ---------------------------------------------------------------------------

#[test]
fun test_all_argument_variants_in_move_call() {
    let gas = new_gas_coin_argument_for_testing();
    let input = new_input_argument_for_testing(0);
    let result = new_result_argument_for_testing(1);
    let nested = new_nested_result_argument_for_testing(2, 3);

    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"pkg")),
        b"m".to_ascii_string(),
        b"f".to_ascii_string(),
        vector[],
        vector[gas, input, result, nested],
    );
    let cmd = new_move_call_command_for_testing(move_call);

    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let call = ctx.tx_commands()[0].as_move_call().destroy_some();
    let args = call.arguments();
    assert!(args.length() == 4);
    assert!(args[0].is_gas_coin());
    assert!(args[1].is_input());
    assert!(args[1].input_index().destroy_some() == 0);
    assert!(args[2].is_result());
    assert!(args[2].result_command_index().destroy_some() == 1);
    assert!(args[3].is_nested_result());
    assert!(args[3].nested_result_command_index().destroy_some() == 2);
    assert!(args[3].nested_result_inner_index().destroy_some() == 3);
}

// ---------------------------------------------------------------------------
// Edge case: empty inputs and commands
// ---------------------------------------------------------------------------

#[test]
fun test_empty_inputs_and_commands() {
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[]);
    let digest = DIGEST;

    assert!(ctx.digest() == &digest);
    assert!(ctx.tx_inputs().length() == 0);
    assert!(ctx.tx_commands().length() == 0);
}

// ---------------------------------------------------------------------------
// Several AuthContext instances in a test scenario
// ---------------------------------------------------------------------------

#[test]
fun test_several_auth_context_instances_in_test_scenario() {
    let digest1 = DIGEST;
    let pure_arg1 = new_call_arg_pure_for_testing(b"hello");
    let cmd1 = make_noop_move_call_command();

    let ctx1 = new_with_tx_inputs(digest1, vector[pure_arg1], vector[cmd1]);

    let ctx1_digest_ref = ctx1.digest();
    let ctx1_tx_inputs_ref = ctx1.tx_inputs();
    let ctx1_tx_commands_ref = ctx1.tx_commands();

    assert!(ctx1_digest_ref == digest1);
    assert!(ctx1_tx_inputs_ref == vector[pure_arg1]);
    assert!(ctx1_tx_commands_ref == vector[cmd1]);

    let digest2 = b"11111111111111111111111111111111";
    let pure_arg2 = new_call_arg_pure_for_testing(b"world");
    let cmd2 = new_transfer_objects_command_for_testing(
        new_transfer_objects_for_testing(
            vector[new_result_argument_for_testing(0)],
            new_input_argument_for_testing(1),
        ),
    );

    let ctx2 = new_with_tx_inputs(digest2, vector[pure_arg2], vector[cmd2]);

    // The data returned by the `ctx1` instance should be updated
    assert!(ctx1.digest() == digest2);
    assert!(ctx1.tx_inputs() == vector[pure_arg2]);
    assert!(ctx1.tx_commands() == vector[cmd2]);

    assert!(ctx2.digest() == digest2);
    assert!(ctx2.tx_inputs() == vector[pure_arg2]);
    assert!(ctx2.tx_commands() == vector[cmd2]);

    // Old links are still valid and point to the old data
    assert!(ctx1_digest_ref == digest1);
    assert!(ctx1_tx_inputs_ref == vector[pure_arg1]);
    assert!(ctx1_tx_commands_ref == vector[cmd1]);
}

// ---------------------------------------------------------------------------
// Error case: bad digest length
// ---------------------------------------------------------------------------

#[test]
#[expected_failure(abort_code = iota::auth_context::EBadAuthDigestLength)]
fun test_bad_digest_length() {
    let _ctx = new_with_tx_inputs(b"too_short", vector[], vector[]);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fun make_noop_move_call_command(): iota::ptb_command::Command {
    let move_call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"noop")),
        b"noop".to_ascii_string(),
        b"noop".to_ascii_string(),
        vector[],
        vector[],
    );
    new_move_call_command_for_testing(move_call)
}
