// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::labeler_tests;

use iota::labeler;
use iota::test_scenario;

public struct FAKE_WITNESS has drop {}

#[test]
#[expected_failure(abort_code = labeler::ENotOneTimeWitness)]
fun test_create_cap_with_fake_witness() {
    // Set up a test environment.
    let sender = @0xA;
    let mut scenario = test_scenario::begin(sender);

    // Fake one time witness.
    let witness = FAKE_WITNESS {};

    // Create a new capability.
    let cap = labeler::create_labeler_cap(witness, scenario.ctx());

    // Cleanup.
    labeler::destroy_labeler_cap(cap);

    scenario.end();
}
