// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Tests for `entry_only_account`.
///
/// Each group targets one specific enriched `AuthContext` field:
///
/// is_entry — `EnrichedProgrammableMoveCall.is_entry`
///   - entry MoveCall passes
///   - non-entry MoveCall is rejected
///
/// returns — `EnrichedProgrammableMoveCall.returns`
///   - void entry MoveCall passes
///   - entry MoveCall with return types is rejected
///
/// mutable — `ImmOrOwnedObjectArg.mutable`
///   - immutable object input passes
///   - mutable object input is rejected
///
/// pure_type_name — `EnrichedCallArg::Pure.type_name`
///   - demonstrates reading type names of pure inputs via `pure_input_types()`
///
/// non_move_call — non-MoveCall commands
///   - TransferObjects is always allowed (not subject to entry/void checks)
#[test_only]
module entry_only_account::entry_only_account_tests;

use entry_only_account::entry_only_account;
use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::enriched_call_arg::{Self, EnrichedCallArg};
use iota::enriched_command::{Self, EnrichedCommand};
use iota::ptb_command;
use iota::test_scenario::{Self as scen, Scenario};
use iota::tx_context as txc;
use iotaccount::iotaccount::IOTAccount;
use std::ascii;
use std::bcs;
use std::type_name;

// ── Test constants ────────────────────────────────────────────────────────────

const PK: vector<u8> =
    x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const DIGEST: vector<u8> =
    x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
const SIG: vector<u8> =
    x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";
const OBJECT_DIGEST: vector<u8> = b"00000000000000000000000000000002";

// ── Helpers ───────────────────────────────────────────────────────────────────

fun make_authenticator(): AuthenticatorFunctionRefV1<IOTAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun create_account(scenario: &mut Scenario): address {
    entry_only_account::create(PK, make_authenticator(), scen::ctx(scenario));
    scen::next_tx(scenario, @0x0);
    let account = scen::take_shared<IOTAccount>(scenario);
    let addr = account.account_address();
    scen::return_shared(account);
    addr
}

fun make_tx_context(sender: address): TxContext {
    txc::new(sender, DIGEST, 0, 0, 0)
}

fun auth_ctx(inputs: vector<EnrichedCallArg>, cmds: vector<EnrichedCommand>): AuthContext {
    auth_context::new_with_enriched_tx_inputs(
        b"00000000000000000000000000000000",
        inputs,
        cmds,
    )
}

fun pkg(): ID { object::id_from_bytes(iota::hash::blake2b256(&b"pkg")) }

// MoveCall with is_entry=true, returns=[]
fun void_entry_call(): EnrichedCommand {
    enriched_command::new_enriched_move_call_for_testing(
        pkg(), b"mod".to_ascii_string(), b"action".to_ascii_string(),
        true,
        vector[],
        vector[],
        vector[],
    )
}

// MoveCall with is_entry=false, returns=[]
fun non_entry_call(): EnrichedCommand {
    enriched_command::new_enriched_move_call_for_testing(
        pkg(), b"mod".to_ascii_string(), b"helper".to_ascii_string(),
        false,
        vector[],
        vector[],
        vector[],
    )
}

// MoveCall with is_entry=true, returns=[u64]  (non-void)
fun entry_call_with_return(): EnrichedCommand {
    enriched_command::new_enriched_move_call_for_testing(
        pkg(), b"mod".to_ascii_string(), b"query".to_ascii_string(),
        true,
        vector[],
        vector[],
        vector[type_name::get<u64>()],
    )
}

fun transfer_cmd(): EnrichedCommand {
    enriched_command::new_enriched_transfer_objects_for_testing(
        vector[ptb_command::new_gas_coin_argument_for_testing()],
        ptb_command::new_input_argument_for_testing(0),
    )
}

// ── is_entry: EnrichedProgrammableMoveCall.is_entry ──────────────────────────

/// Void entry MoveCall → passes (is_entry=true, returns=[])
#[test]
fun test_is_entry_void_call_passes() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[], vector[void_entry_call()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

/// Non-entry MoveCall → rejected with ENonEntryFunctionCall (is_entry=false)
#[test]
#[expected_failure(abort_code = entry_only_account::ENonEntryFunctionCall)]
fun test_is_entry_non_entry_call_rejected() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[], vector[non_entry_call()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

// ── returns: EnrichedProgrammableMoveCall.returns ────────────────────────────

/// Entry function with non-empty returns → rejected with EFunctionMustBeVoid
#[test]
#[expected_failure(abort_code = entry_only_account::EFunctionMustBeVoid)]
fun test_returns_entry_call_with_returns_rejected() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[], vector[entry_call_with_return()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

/// entry+void then entry+returns in sequence → fails on the second command
#[test]
#[expected_failure(abort_code = entry_only_account::EFunctionMustBeVoid)]
fun test_returns_second_command_with_returns_rejected() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account,
            SIG,
            &auth_ctx(vector[], vector[void_entry_call(), entry_call_with_return()]),
            &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

// ── mutable: ImmOrOwnedObjectArg.mutable ─────────────────────────────────────

/// Immutable object input (mutable=false) → passes
#[test]
fun test_mutable_immutable_object_input_passes() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);

        let input = enriched_call_arg::new_imm_or_owned_for_testing(
            object::id_from_bytes(iota::hash::blake2b256(&b"obj")),
            1,
            OBJECT_DIGEST,
            false, // mutable = false
            type_name::get<u64>(),
        );

        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[input], vector[void_entry_call()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

/// Mutable object input (mutable=true) → rejected with ENoMutableObjectInputs
#[test]
#[expected_failure(abort_code = entry_only_account::ENoMutableObjectInputs)]
fun test_mutable_mutable_object_input_rejected() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);

        // mutable=true simulates the VM resolving that the callee takes `&mut T`.
        let input = enriched_call_arg::new_imm_or_owned_for_testing(
            object::id_from_bytes(iota::hash::blake2b256(&b"obj")),
            1,
            OBJECT_DIGEST,
            true, // mutable = true
            type_name::get<u64>(),
        );

        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[input], vector[void_entry_call()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

// ── pure_type_name: EnrichedCallArg::Pure.type_name ──────────────────────────

/// Demonstrates reading the enriched `type_name` field from `Pure` inputs
/// via the `pure_input_types()` view helper.
#[test]
fun test_pure_type_name_is_readable() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);

        let pure_input = enriched_call_arg::new_pure_for_testing(
            bcs::to_bytes(&42u64),
            type_name::get<u64>(),
        );
        let a_ctx = auth_ctx(vector[pure_input], vector[void_entry_call()]);

        // Authenticate succeeds (Pure inputs are not restricted by this account).
        entry_only_account::authenticate(&account, SIG, &a_ctx, &ctx);

        // The type_name field is visible via the view helper.
        let type_names = entry_only_account::pure_input_types(&a_ctx);
        assert!(type_names.length() == 1);
        assert!(type_names[0] == type_name::get<u64>());

        scen::return_shared(account);
    };
    scen::end(sc);
}

/// Multiple Pure inputs — all type names are collected in order.
#[test]
fun test_pure_type_name_multiple_inputs_collected() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);

        let inputs = vector[
            enriched_call_arg::new_pure_for_testing(
                bcs::to_bytes(&1u8), type_name::get<u8>(),
            ),
            enriched_call_arg::new_pure_for_testing(
                bcs::to_bytes(&1000u64), type_name::get<u64>(),
            ),
        ];
        let a_ctx = auth_ctx(inputs, vector[void_entry_call()]);
        entry_only_account::authenticate(&account, SIG, &a_ctx, &ctx);

        let type_names = entry_only_account::pure_input_types(&a_ctx);
        assert!(type_names.length() == 2);
        assert!(type_names[0] == type_name::get<u8>());
        assert!(type_names[1] == type_name::get<u64>());

        scen::return_shared(account);
    };
    scen::end(sc);
}

// ── non_move_call: non-MoveCall commands ─────────────────────────────────────

/// TransferObjects is not a MoveCall — it is always allowed.
#[test]
fun test_non_move_call_transfer_objects_allowed() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[], vector[transfer_cmd()]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}

/// Empty PTB (no commands, no inputs) — signature-only check.
#[test]
fun test_non_move_call_empty_ptb_passes() {
    let mut sc = scen::begin(@0x0);
    let addr = create_account(&mut sc);
    scen::next_tx(&mut sc, addr);
    {
        let account = scen::take_shared<IOTAccount>(&sc);
        let ctx = make_tx_context(addr);
        entry_only_account::authenticate(
            &account, SIG, &auth_ctx(vector[], vector[]), &ctx,
        );
        scen::return_shared(account);
    };
    scen::end(sc);
}
