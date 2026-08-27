// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests for validator metadata update restrictions:
// - an active validator must not be able to set metadata that duplicates
//   another active or pending validator's metadata;
// - only candidate validators can use the candidate update functions; in
//   particular, a pending validator must not be able to overwrite its
//   current keys, as that would bypass the duplicate checks;
// - only active validators can stage next-epoch metadata.

#[test_only]
module iota_system::validator_metadata_tests;

use iota::balance;
use iota::iota::IOTA;
use iota::test_scenario::{Self, Scenario};
use iota_system::governance_test_utils::{
    add_validator,
    add_validator_candidate,
    create_iota_system_state_for_testing,
    create_validator_for_testing
};
use iota_system::iota_system::IotaSystemState;
use iota_system::validator::{Self, ValidatorV1};

const VALIDATOR_1_ADDR: address =
    @0xaf76afe6f866d8426d2be85d6ef0b11f871a251d043b2f11e15563bf418f5a5a;
// Authority pubkey generated with seed [0; 32]
const VALIDATOR_1_AUTHORITY_PUBKEY: vector<u8> =
    x"99f25ef61f8032b914636460982c5cc6f134ef1ddae76657f2cbfec1ebfc8d097374080df6fcf0dcb8bc4b0d8e0af5d80ebbff2b4c599f54f42d6312dfc314276078c1cc347ebbbec5198be258513f386b930d02c2749a803e2330955ebd1a10";
// Proof of possession bound to the authority key and VALIDATOR_1_ADDR
const VALIDATOR_1_POP: vector<u8> =
    x"b01cc86f421beca7ab4cfca87c0799c4d038c199dd399fbec1924d4d4367866dba9e84d514710b91feb65316e4ceef43";
const VALIDATOR_1_NETWORK_PUBKEY: vector<u8> =
    x"20db2617f26d74ebe1c0db2d287ca219214434297b09620bb896d63e3cd2793e";
const VALIDATOR_1_PROTOCOL_PUBKEY: vector<u8> =
    x"4437ce19c70ea935445c8e88ae9536d7653ff9cec562e9503c0cb720d8586719";
const VALIDATOR_1_NAME: vector<u8> = b"ValidatorName";
const VALIDATOR_1_NET_ADDRESS: vector<u8> = b"/ip4/127.0.0.1/tcp/80";
const VALIDATOR_1_P2P_ADDRESS: vector<u8> = b"/ip4/127.0.0.1/udp/80";
const VALIDATOR_1_PRIMARY_ADDRESS: vector<u8> = b"/ip4/127.0.0.1/udp/80";

const VALIDATOR_2_ADDR: address =
    @0x8e3446145b0c7768839d71840df389ffa3b9742d0baaff326a3d453b595f87d7;
// Authority pubkey generated with seed [2; 32]
const VALIDATOR_2_AUTHORITY_PUBKEY: vector<u8> =
    x"adf2e2350fe9a58f3fa50777499f20331c4550ab70f6a4fb25a58c61b50b5366107b5c06332e71bb47aa99ce2d5c07fe0dab04b8af71589f0f292c50382eba6ad4c90acb010ab9db7412988b2aba1018aaf840b1390a8b2bee3fde35b4ab7fdf";
// Proof of possession bound to the authority key and VALIDATOR_2_ADDR
const VALIDATOR_2_POP: vector<u8> =
    x"926fdb08b2b46d802e3642044f215dcb049e6c17a376a272ffd7dba32739bb995370966698ab235ee172fbd974985cfe";
const VALIDATOR_2_NETWORK_PUBKEY: vector<u8> =
    x"21db2617f26d74ebe1c0db2d287ca219214434297b09620bb896d63e3cd2793e";
const VALIDATOR_2_PROTOCOL_PUBKEY: vector<u8> =
    x"4537ce19c70ea935445c8e88ae9536d7653ff9cec562e9503c0cb720d8586719";

const PENDING_VALIDATOR_ADDR: address =
    @0x1a4623343cd42be47d67314fce0ad042f3c82685544bc91d8c11d24e74ba7357;
// Authority pubkey generated with seed [0; 32]
const PENDING_VALIDATOR_AUTHORITY_PUBKEY: vector<u8> =
    x"99f25ef61f8032b914636460982c5cc6f134ef1ddae76657f2cbfec1ebfc8d097374080df6fcf0dcb8bc4b0d8e0af5d80ebbff2b4c599f54f42d6312dfc314276078c1cc347ebbbec5198be258513f386b930d02c2749a803e2330955ebd1a10";
// Proof of possession bound to the authority key and PENDING_VALIDATOR_ADDR
const PENDING_VALIDATOR_POP: vector<u8> =
    x"8b93fc1b33379e2796d361c4056f0f04ad5aea7f4a8c02eaac57340ff09b6dc158eb1945eece103319167f420daf0cb3";

// === Duplicate Metadata Tests ===

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_name() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_name(VALIDATOR_1_NAME, scenario.ctx());

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_net_address() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_network_address(
        VALIDATOR_1_NET_ADDRESS,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_p2p_address() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_p2p_address(VALIDATOR_1_P2P_ADDRESS, scenario.ctx());

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_primary_address() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_primary_address(
        VALIDATOR_1_PRIMARY_ADDRESS,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_network_pubkey() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_network_pubkey(
        VALIDATOR_1_NETWORK_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_protocol_pubkey() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_protocol_pubkey(
        VALIDATOR_1_PROTOCOL_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

// Copying another validator's authority key is rejected even earlier than the
// duplicate check: its proof of possession is bound to the other validator's
// address, so metadata validation fails.
#[expected_failure(abort_code = iota_system::validator::EInvalidProofOfPossession)]
#[test]
fun cannot_set_duplicate_authority_pubkey() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_authority_pubkey(
        VALIDATOR_1_AUTHORITY_PUBKEY,
        VALIDATOR_1_POP,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::EDuplicateValidator)]
#[test]
fun cannot_set_duplicate_next_epoch_primary_address() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;
    let new_primary_address = b"/ip4/99.99.99.99/udp/80";

    // Validator 1 stages a new primary address.
    scenario.next_tx(VALIDATOR_1_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_primary_address(new_primary_address, scenario.ctx());
    test_scenario::return_shared(system_state);

    // Validator 2 tries to stage the same primary address.
    scenario.next_tx(VALIDATOR_2_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_primary_address(new_primary_address, scenario.ctx());

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

// === Network vs Protocol Key Cross-Check Tests ===

// The staged next-epoch network key must also differ from the *active*
// protocol key (and vice versa); otherwise the promoted metadata would
// end up with an identical network and protocol key pair.

#[expected_failure(abort_code = iota_system::validator::EMetadataInvalidNetPubkey)]
#[test]
fun cannot_set_next_epoch_network_pubkey_equal_to_protocol_pubkey() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_1_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_network_pubkey(
        VALIDATOR_1_PROTOCOL_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator::EMetadataInvalidProtocolPubkey)]
#[test]
fun cannot_set_next_epoch_protocol_pubkey_equal_to_network_pubkey() {
    let mut scenario_val = set_up_two_active_validators();
    let scenario = &mut scenario_val;

    scenario.next_tx(VALIDATOR_1_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_protocol_pubkey(
        VALIDATOR_1_NETWORK_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

// === Pending Validator Restriction Tests ===

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_network_address() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_network_address(
        b"/ip4/127.0.0.9/udp/80",
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_p2p_address() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_p2p_address(b"/ip4/127.0.0.9/udp/80", scenario.ctx());

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_primary_address() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_primary_address(
        b"/ip4/127.0.0.9/udp/80",
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_authority_pubkey() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_authority_pubkey(
        PENDING_VALIDATOR_AUTHORITY_PUBKEY,
        PENDING_VALIDATOR_POP,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_protocol_pubkey() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_protocol_pubkey(
        VALIDATOR_2_PROTOCOL_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotValidatorCandidate)]
#[test]
fun pending_validator_cannot_update_candidate_network_pubkey() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_candidate_validator_network_pubkey(
        VALIDATOR_2_NETWORK_PUBKEY,
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[expected_failure(abort_code = iota_system::validator_set::ENotAValidator)]
#[test]
fun pending_validator_cannot_update_next_epoch_metadata() {
    let mut scenario_val = set_up_with_pending_validator();
    let scenario = &mut scenario_val;

    scenario.next_tx(PENDING_VALIDATOR_ADDR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    system_state.update_validator_next_epoch_network_address(
        b"/ip4/127.0.0.9/udp/80",
        scenario.ctx(),
    );

    test_scenario::return_shared(system_state);
    scenario_val.end();
}

// === Test Setup ===

/// Set up a system state with two active validators with distinct,
/// fully valid metadata.
fun set_up_two_active_validators(): Scenario {
    let mut scenario = test_scenario::begin(@0x0);
    let ctx = scenario.ctx();
    let validators = vector[
        new_validator_for_testing(
            VALIDATOR_1_ADDR,
            VALIDATOR_1_AUTHORITY_PUBKEY,
            VALIDATOR_1_NETWORK_PUBKEY,
            VALIDATOR_1_PROTOCOL_PUBKEY,
            VALIDATOR_1_POP,
            VALIDATOR_1_NAME,
            b"/ip4/127.0.0.1",
            ctx,
        ),
        new_validator_for_testing(
            VALIDATOR_2_ADDR,
            VALIDATOR_2_AUTHORITY_PUBKEY,
            VALIDATOR_2_NETWORK_PUBKEY,
            VALIDATOR_2_PROTOCOL_PUBKEY,
            VALIDATOR_2_POP,
            b"ValidatorName2",
            b"/ip4/127.0.0.2",
            ctx,
        ),
    ];
    create_iota_system_state_for_testing(validators, 1000, 0, ctx);
    scenario
}

fun new_validator_for_testing(
    addr: address,
    authority_pubkey: vector<u8>,
    network_pubkey: vector<u8>,
    protocol_pubkey: vector<u8>,
    pop: vector<u8>,
    name: vector<u8>,
    address_prefix: vector<u8>,
    ctx: &mut TxContext,
): ValidatorV1 {
    let mut net_address = address_prefix;
    net_address.append(b"/tcp/80");
    let mut p2p_address = address_prefix;
    p2p_address.append(b"/udp/80");

    validator::new_for_testing(
        addr,
        authority_pubkey,
        network_pubkey,
        protocol_pubkey,
        pop,
        name,
        b"description",
        b"image_url",
        b"project_url",
        net_address,
        p2p_address,
        p2p_address,
        option::some(balance::create_for_testing<IOTA>(100_000_000_000)),
        1,
        0,
        true,
        ctx,
    )
}

/// Set up a system state with one active validator and one pending validator
/// that has passed the candidate stage via `request_add_validator`.
fun set_up_with_pending_validator(): Scenario {
    let mut scenario = test_scenario::begin(@0x0);
    let ctx = scenario.ctx();
    let validators = vector[create_validator_for_testing(@0x1, 100, 0, ctx)];
    create_iota_system_state_for_testing(validators, 100, 0, ctx);

    add_validator_candidate(
        PENDING_VALIDATOR_ADDR,
        b"PendingValidatorName",
        b"/ip4/127.0.0.3/udp/81",
        PENDING_VALIDATOR_AUTHORITY_PUBKEY,
        PENDING_VALIDATOR_POP,
        &mut scenario,
    );
    add_validator(PENDING_VALIDATOR_ADDR, &mut scenario);
    scenario
}
