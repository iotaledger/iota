// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota_system::attestor_registry_scenario_tests;

use iota::coin;
use iota::iota::IOTA;
use iota::test_scenario;
use iota_system::attestor_registry;
use iota_system::governance_test_utils::{advance_epoch, set_up_iota_system_state};
use iota_system::iota_system::{Self, IotaSystemState};

const ATTESTOR: address = @0x42;
const MIN_JOINING_BOND: u64 = 2_000_000_000_000;

// Real `flag || raw_key` ed25519 key; the native rejects arbitrary bytes.
fun ed25519_pubkey(): vector<u8> {
    x"00d04a166e8dcd71127be0012f3e882c9b8c355af7d43dd98f8200b69eb17e312f"
}

// `enable_validator_attestation` is off at the version `iota move test` runs
// at, so the entry points abort. The happy path through the system is
// covered by the Rust e2e test (which enables the flag) and by the
// struct-level unit tests.
#[test, expected_failure(abort_code = attestor_registry::EFeatureNotEnabled)]
fun test_register_attestor_requires_feature_flag() {
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(ATTESTOR);
    let scenario = &mut scenario_val;

    scenario.next_tx(ATTESTOR);
    let mut system_state = scenario.take_shared<IotaSystemState>();
    let bond = coin::mint_for_testing<IOTA>(MIN_JOINING_BOND, scenario.ctx());
    iota_system::register_attestor(&mut system_state, bond, ed25519_pubkey(), scenario.ctx());
    test_scenario::return_shared(system_state);
    scenario_val.end();
}

#[test]
fun test_advance_epoch_without_registry_is_noop() {
    set_up_iota_system_state(vector[@0x1, @0x2]);
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    // no registration ever happened; epoch change must succeed
    advance_epoch(scenario);
    advance_epoch(scenario);
    scenario_val.end();
}
