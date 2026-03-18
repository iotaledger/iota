// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::auth_context_enriched_tests;

use iota::auth_context::new_with_tx_inputs;
use iota::enriched_call_arg::EnrichedCallArg;
use iota::enriched_command::EnrichedCommand;
use iota::ptb_call_arg::{
    new_call_arg_pure_for_testing,
    new_call_arg_object_for_testing,
    new_object_arg_imm_or_owned_for_testing,
    new_object_arg_shared_for_testing,
    new_object_arg_receiving_for_testing,
    new_object_ref_for_testing,
};
use iota::ptb_command::{
    new_gas_coin_argument_for_testing,
    new_input_argument_for_testing,
    new_result_argument_for_testing,
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
    new_upgrade_command_for_testing,
};
use std::type_name;

const DIGEST: vector<u8> = b"00000000000000000000000000000001";
const OBJECT_DIGEST: vector<u8> = b"00000000000000000000000000000002";

// ---------------------------------------------------------------------------
// enriched_tx_inputs: Pure
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_pure() {
    let pure = new_call_arg_pure_for_testing(b"hello");
    let cmd = make_noop_move_call_command();
    let ctx = new_with_tx_inputs(DIGEST, vector[pure], vector[cmd]);

    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_pure());
}

// ---------------------------------------------------------------------------
// enriched_tx_inputs: ImmOrOwnedObject
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_imm_or_owned() {
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0xA),
        7,
        OBJECT_DIGEST,
    );
    let arg = new_call_arg_object_for_testing(new_object_arg_imm_or_owned_for_testing(obj_ref));
    let cmd = make_noop_move_call_command();
    let ctx = new_with_tx_inputs(DIGEST, vector[arg], vector[cmd]);

    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_imm_or_owned_object());
    assert!(!enriched[0].is_shared_object());
    assert!(!enriched[0].is_receiving());
    assert!(!enriched[0].is_pure());
}

// ---------------------------------------------------------------------------
// enriched_tx_inputs: SharedObject
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_shared_object() {
    let arg = new_call_arg_object_for_testing(
        new_object_arg_shared_for_testing(object::id_from_address(@0xB), 5, true),
    );
    let cmd = make_noop_move_call_command();
    let ctx = new_with_tx_inputs(DIGEST, vector[arg], vector[cmd]);

    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_shared_object());
}

// ---------------------------------------------------------------------------
// enriched_tx_inputs: Receiving
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_receiving() {
    let obj_ref = new_object_ref_for_testing(
        object::id_from_address(@0xC),
        12,
        OBJECT_DIGEST,
    );
    let arg = new_call_arg_object_for_testing(new_object_arg_receiving_for_testing(obj_ref));
    let cmd = make_noop_move_call_command();
    let ctx = new_with_tx_inputs(DIGEST, vector[arg], vector[cmd]);

    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_receiving());
}

// ---------------------------------------------------------------------------
// enriched_tx_inputs: all variants
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_all_variants() {
    let pure = new_call_arg_pure_for_testing(b"bytes");
    let imm = new_call_arg_object_for_testing(new_object_arg_imm_or_owned_for_testing(
        new_object_ref_for_testing(object::id_from_address(@0x10), 1, OBJECT_DIGEST),
    ));
    let shared = new_call_arg_object_for_testing(
        new_object_arg_shared_for_testing(object::id_from_address(@0x20), 2, false),
    );
    let recv = new_call_arg_object_for_testing(new_object_arg_receiving_for_testing(
        new_object_ref_for_testing(object::id_from_address(@0x30), 3, OBJECT_DIGEST),
    ));

    let ctx = new_with_tx_inputs(
        DIGEST,
        vector[pure, imm, shared, recv],
        vector[make_noop_move_call_command()],
    );

    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 4);
    assert!(enriched[0].is_pure());
    assert!(enriched[1].is_imm_or_owned_object());
    assert!(enriched[2].is_shared_object());
    assert!(enriched[3].is_receiving());
}

// ---------------------------------------------------------------------------
// enriched_tx_inputs: empty
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_inputs_empty() {
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[]);
    let enriched = ctx.enriched_tx_inputs();
    assert!(enriched.length() == 0);
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: MoveCall
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_move_call() {
    let cmd = make_noop_move_call_command();
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_move_call());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: TransferObjects
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_transfer_objects() {
    let data = new_transfer_objects_for_testing(
        vector[new_result_argument_for_testing(0)],
        new_input_argument_for_testing(1),
    );
    let cmd = new_transfer_objects_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_transfer_objects());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: SplitCoins
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_split_coins() {
    let data = new_split_coins_for_testing(
        new_gas_coin_argument_for_testing(),
        vector[new_input_argument_for_testing(0)],
    );
    let cmd = new_split_coins_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_split_coins());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: MergeCoins
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_merge_coins() {
    let data = new_merge_coins_for_testing(
        new_gas_coin_argument_for_testing(),
        vector[new_result_argument_for_testing(0)],
    );
    let cmd = new_merge_coins_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_merge_coins());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: Publish
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_publish() {
    let data = new_publish_for_testing(vector[b"module_bytes"], vector[]);
    let cmd = new_publish_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_publish());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: MakeMoveVec
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_make_move_vec() {
    let tn = type_name::get<u64>();
    let data = new_make_move_vec_for_testing(
        option::some(tn),
        vector[new_input_argument_for_testing(0)],
    );
    let cmd = new_make_move_vec_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_make_move_vec());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: Upgrade
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_upgrade() {
    let data = new_upgrade_for_testing(
        vector[b"upgraded"],
        vector[],
        object::id_from_address(@0xAA),
        new_input_argument_for_testing(0),
    );
    let cmd = new_upgrade_command_for_testing(data);
    let ctx = new_with_tx_inputs(DIGEST, vector[], vector[cmd]);

    let enriched = ctx.enriched_tx_commands();
    assert!(enriched.length() == 1);
    assert!(enriched[0].is_upgrade());
}

// ---------------------------------------------------------------------------
// enriched_tx_commands: all seven variants
// ---------------------------------------------------------------------------

#[test]
fun test_enriched_commands_all_variants() {
    let tn = type_name::get<u16>();
    let commands = vector[
        make_noop_move_call_command(),
        new_transfer_objects_command_for_testing(
            new_transfer_objects_for_testing(
                vector[new_result_argument_for_testing(0)],
                new_input_argument_for_testing(1),
            ),
        ),
        new_split_coins_command_for_testing(
            new_split_coins_for_testing(
                new_gas_coin_argument_for_testing(),
                vector[new_input_argument_for_testing(0)],
            ),
        ),
        new_merge_coins_command_for_testing(
            new_merge_coins_for_testing(
                new_gas_coin_argument_for_testing(),
                vector[new_result_argument_for_testing(0)],
            ),
        ),
        new_publish_command_for_testing(
            new_publish_for_testing(vector[b"mod"], vector[]),
        ),
        new_make_move_vec_command_for_testing(
            new_make_move_vec_for_testing(option::some(tn), vector[]),
        ),
        new_upgrade_command_for_testing(
            new_upgrade_for_testing(
                vector[b"upgraded"],
                vector[],
                object::id_from_address(@0xBB),
                new_input_argument_for_testing(0),
            ),
        ),
    ];

    let ctx = new_with_tx_inputs(DIGEST, vector[], commands);
    let enriched = ctx.enriched_tx_commands();

    assert!(enriched.length() == 7);
    assert!(enriched[0].is_move_call());
    assert!(enriched[1].is_transfer_objects());
    assert!(enriched[2].is_split_coins());
    assert!(enriched[3].is_merge_coins());
    assert!(enriched[4].is_publish());
    assert!(enriched[5].is_make_move_vec());
    assert!(enriched[6].is_upgrade());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fun make_noop_move_call_command(): iota::ptb_command::Command {
    let call = new_programmable_move_call_for_testing(
        object::id_from_bytes(iota::hash::blake2b256(&b"noop")),
        b"noop".to_ascii_string(),
        b"noop".to_ascii_string(),
        vector[],
        vector[],
    );
    new_move_call_command_for_testing(call)
}
