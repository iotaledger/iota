// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::attestor_registry_scenario_tests;

use iota::coin::{Self, Coin};
use iota::iota::IOTA;
use iota::test_scenario;
use iota::test_utils;
use iota_system::attestor_registry;
use iota_system::governance_test_utils::{advance_epoch, set_up_iota_system_state};
use iota_system::iota_system::{Self, IotaSystemState};
use iota_system::protocol_config;

const ATTESTOR: address = @0x42;
fun min_joining_bond(): u64 { attestor_registry::min_joining_bond() }
const ENABLE_EXTERNAL_ATTESTATION_FLAG: vector<u8> = b"enable_external_attestation";

/// Toggle the external flag; its prerequisites stay on.
fun set_feature_for_testing(enabled: bool) {
    protocol_config::set_feature_enabled_for_testing(b"enable_pcool_flow", true);
    protocol_config::set_feature_enabled_for_testing(b"enable_validator_attestation", true);
    protocol_config::set_feature_enabled_for_testing(ENABLE_EXTERNAL_ATTESTATION_FLAG, enabled);
}

// Seed-derived key + proof of possession for @0x42; regenerate with:
// cargo nextest run -p iota-types --lib print_attestor_move_fixtures --no-capture
fun ed25519_pubkey(): vector<u8> {
    x"00876edc0d843534980747592afce708167a0b6516b0b9be7fd6eb864d05c0ba61"
}

fun ed25519_pop(): vector<u8> {
    x"52a490fc6f760bd35b621542705b15230283e36f54456bcbe10a45bc4318e4d51e053716633f328ebb7d069e125163ae067b53cec268893f8a364c37525eeb04"
}

fun secp256k1_pubkey(): vector<u8> {
    x"0102253bda0005e6d0332d8f59bfadc6c682ae3a6797acda0b01bfcd078e371977d9"
}

fun secp256k1_pop(): vector<u8> {
    x"a0830232f65de7b7127431eaea46780634b19439a4c4e8872733a151af7ffeed31d6957732fe05e7a9ef03915d590e656866873f5812e1fbd7820fd7de687888"
}

#[test, expected_failure(abort_code = attestor_registry::EFeatureNotEnabled)]
fun test_register_attestor_requires_feature_flag() {
    set_feature_for_testing(false);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
    iota_system::register_attestor(
        &mut system_state,
        bond,
        ed25519_pubkey(),
        ed25519_pop(),
        b"name",
        b"desc",
        b"https://example.com",
        b"https://example.com/logo.png",
        scenario.ctx(),
    );
    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_advance_epoch_without_feature_does_not_create_registry() {
    set_feature_for_testing(false);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    advance_epoch(scenario);
    advance_epoch(scenario);
    scenario.next_tx(@0x0);
    {
        let system_state = scenario.take_shared<IotaSystemState>();
        assert!(!iota_system::attestor_registry_exists_for_testing(&system_state));
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_advance_epoch_with_feature_creates_registry() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    advance_epoch(scenario);
    scenario.next_tx(@0x0);
    {
        let system_state = scenario.take_shared<IotaSystemState>();
        assert!(iota_system::attestor_registry_exists_for_testing(&system_state));
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_register_activate_deregister_refund_through_system() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    // register
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
        iota_system::register_attestor(
            &mut system_state,
            bond,
            ed25519_pubkey(),
            ed25519_pop(),
            b"name",
            b"desc",
            b"https://example.com",
            b"https://example.com/logo.png",
            scenario.ctx(),
        );
        assert!(iota_system::active_attestor_count_for_testing(&mut system_state) == 0);
        test_scenario::return_shared(system_state);
    };

    // activation at the boundary
    advance_epoch(scenario);
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert!(iota_system::active_attestor_count_for_testing(&mut system_state) == 1);
        test_scenario::return_shared(system_state);
    };

    // deregister: removal + refund at the next boundary
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        iota_system::deregister_attestor(&mut system_state, scenario.ctx());
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert!(iota_system::active_attestor_count_for_testing(&mut system_state) == 0);
        test_scenario::return_shared(system_state);
        // refunded bond arrived as a Coin<IOTA>
        let refund = scenario.take_from_sender<Coin<IOTA>>();
        assert!(refund.value() == min_joining_bond());
        scenario.return_to_sender(refund);
    };

    scenario_val.end();
}

#[test]
fun test_rotate_attestor_key_through_system() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
        iota_system::register_attestor(
            &mut system_state,
            bond,
            ed25519_pubkey(),
            ed25519_pop(),
            b"name",
            b"desc",
            b"https://example.com",
            b"https://example.com/logo.png",
            scenario.ctx(),
        );
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);

    // stage a replacement key; a swapped delegation aborts here on EInvalidPubkey
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        iota_system::rotate_attestor_key(
            &mut system_state,
            secp256k1_pubkey(),
            secp256k1_pop(),
            scenario.ctx(),
        );
        test_scenario::return_shared(system_state);
    };

    // the staged key is applied in place at the next boundary
    advance_epoch(scenario);
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert!(iota_system::active_attestor_count_for_testing(&mut system_state) == 1);
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_low_bond_eviction_through_system() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
        iota_system::register_attestor(
            &mut system_state,
            bond,
            ed25519_pubkey(),
            ed25519_pop(),
            b"name",
            b"desc",
            b"https://example.com",
            b"https://example.com/logo.png",
            scenario.ctx(),
        );
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);

    // slash below the threshold, then cross the boundary
    scenario.next_tx(@0x0);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let slashed = iota_system::slash_attestor_for_testing(
            &mut system_state,
            ATTESTOR,
            min_joining_bond() - 1,
        );
        test_utils::destroy(slashed);
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);

    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        assert!(iota_system::active_attestor_count_for_testing(&mut system_state) == 0);
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_metadata_lifecycle_through_system() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
        iota_system::register_attestor(
            &mut system_state,
            bond,
            ed25519_pubkey(),
            ed25519_pop(),
            b"attestor-one",
            b"an attestor",
            b"https://example.com",
            b"https://example.com/logo.png",
            scenario.ctx(),
        );
        assert!(iota_system::attestor_metadata_exists_for_testing(&system_state, ATTESTOR));
        iota_system::update_attestor_name(&mut system_state, b"attestor-two", scenario.ctx());
        test_scenario::return_shared(system_state);
    };

    // pending deregistration refunds immediately and drops the metadata
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        iota_system::deregister_attestor(&mut system_state, scenario.ctx());
        assert!(!iota_system::attestor_metadata_exists_for_testing(&system_state, ATTESTOR));
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}

#[test]
fun test_metadata_removed_at_boundary_exit() {
    set_feature_for_testing(true);
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        let bond = coin::mint_for_testing<IOTA>(min_joining_bond(), scenario.ctx());
        iota_system::register_attestor(
            &mut system_state, bond, ed25519_pubkey(), ed25519_pop(),
            b"n", b"d", b"https://u", b"https://l", scenario.ctx(),
        );
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);

    // active deregistration: metadata survives until the boundary
    scenario.next_tx(ATTESTOR);
    {
        let mut system_state = scenario.take_shared<IotaSystemState>();
        iota_system::deregister_attestor(&mut system_state, scenario.ctx());
        assert!(iota_system::attestor_metadata_exists_for_testing(&system_state, ATTESTOR));
        test_scenario::return_shared(system_state);
    };
    advance_epoch(scenario);
    scenario.next_tx(ATTESTOR);
    {
        let system_state = scenario.take_shared<IotaSystemState>();
        assert!(!iota_system::attestor_metadata_exists_for_testing(&system_state, ATTESTOR));
        test_scenario::return_shared(system_state);
    };
    scenario_val.end();
}
