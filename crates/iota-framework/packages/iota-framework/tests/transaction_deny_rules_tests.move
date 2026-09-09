// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::transaction_deny_rules_tests;

use iota::event;
use iota::test_scenario::{Self, Scenario};
use iota::transaction_deny_rules::{Self, TransactionDenyRules, TransactionDenyRulesUpdated};

/// Apply a delta with all six switches off.
fun apply_delta(
    rules: &mut TransactionDenyRules,
    scenario: &mut Scenario,
    added_addresses: vector<address>,
    removed_addresses: vector<address>,
    added_objects: vector<ID>,
    removed_objects: vector<ID>,
    added_packages: vector<ID>,
    removed_packages: vector<ID>,
) {
    rules.update_for_testing(
        added_addresses,
        removed_addresses,
        added_objects,
        removed_objects,
        added_packages,
        removed_packages,
        false,
        false,
        false,
        false,
        false,
        false,
        scenario.ctx(),
    );
}

/// Assert the full stored state with all six switches off.
fun assert_lists(
    rules: &TransactionDenyRules,
    denied_addresses: vector<address>,
    denied_objects: vector<ID>,
    denied_packages: vector<ID>,
) {
    rules.assert_state_for_testing(
        denied_addresses,
        denied_objects,
        denied_packages,
        false,
        false,
        false,
        false,
        false,
        false,
    );
}

#[test]
fun create_starts_empty_and_deltas_accumulate() {
    let mut scenario = test_scenario::begin(@0x0);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.next_tx(@0x0);

    let mut rules = scenario.take_shared<TransactionDenyRules>();
    assert_lists(&rules, vector[], vector[], vector[]);

    // Distinct non-empty contents in every list pin the list parameters.
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xAA, @0xBB],
        vector[],
        vector[object::id_from_address(@0x1A)],
        vector[],
        vector[object::id_from_address(@0x2B), object::id_from_address(@0x2C)],
        vector[],
    );
    assert_lists(
        &rules,
        vector[@0xAA, @0xBB],
        vector[object::id_from_address(@0x1A)],
        vector[object::id_from_address(@0x2B), object::id_from_address(@0x2C)],
    );

    // A second delta accumulates on the stored state: removals and additions
    // combine with entries that stay untouched.
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xCC],
        vector[@0xAA],
        vector[],
        vector[object::id_from_address(@0x1A)],
        vector[],
        vector[object::id_from_address(@0x2B)],
    );
    assert_lists(
        &rules,
        vector[@0xBB, @0xCC],
        vector[],
        vector[object::id_from_address(@0x2C)],
    );

    test_scenario::return_shared(rules);
    scenario.end();
}

#[test]
fun tolerant_delta_is_a_noop() {
    let mut scenario = test_scenario::begin(@0x0);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.next_tx(@0x0);

    let mut rules = scenario.take_shared<TransactionDenyRules>();
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xAA],
        vector[],
        vector[],
        vector[],
        vector[],
        vector[],
    );

    // Re-adding a present key and removing absent keys must not abort and
    // must leave the state unchanged.
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xAA],
        vector[@0xBB],
        vector[],
        vector[object::id_from_address(@0x1A)],
        vector[],
        vector[],
    );
    assert_lists(&rules, vector[@0xAA], vector[], vector[]);

    test_scenario::return_shared(rules);
    scenario.end();
}

#[test]
fun update_emits_delta_event_with_lengths() {
    let mut scenario = test_scenario::begin(@0x0);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.next_tx(@0x0);

    let mut rules = scenario.take_shared<TransactionDenyRules>();
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xAA, @0xBB],
        vector[],
        vector[],
        vector[],
        vector[],
        vector[],
    );
    // The removal is tolerant (@0xCC is absent) but still reported in the
    // event: the event records the applied delta, the lengths the result.
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[],
        vector[@0xAA, @0xCC],
        vector[],
        vector[],
        vector[],
        vector[],
    );

    let events = event::events_by_type<TransactionDenyRulesUpdated>();
    assert!(events.length() == 2);
    let (added, removed, len) = transaction_deny_rules::event_addresses_for_testing(&events[1]);
    assert!(added == vector[]);
    assert!(removed == vector[@0xAA, @0xCC]);
    assert!(len == 1);

    test_scenario::return_shared(rules);
    scenario.end();
}

/// One switch per update: a single sample cannot distinguish the six boolean
/// fields, so pin each switch position separately.
#[test]
fun update_pins_each_switch() {
    let mut scenario = test_scenario::begin(@0x0);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.next_tx(@0x0);

    let mut rules = scenario.take_shared<TransactionDenyRules>();
    let mut hot = 0;
    while (hot < 6) {
        rules.update_for_testing(
            vector[],
            vector[],
            vector[],
            vector[],
            vector[],
            vector[],
            hot == 0,
            hot == 1,
            hot == 2,
            hot == 3,
            hot == 4,
            hot == 5,
            scenario.ctx(),
        );
        rules.assert_state_for_testing(
            vector[],
            vector[],
            vector[],
            hot == 0,
            hot == 1,
            hot == 2,
            hot == 3,
            hot == 4,
            hot == 5,
        );
        hot = hot + 1;
    };

    test_scenario::return_shared(rules);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::transaction_deny_rules::ENotSystemAddress)]
fun create_rejects_non_system_sender() {
    let mut scenario = test_scenario::begin(@0xA11CE);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::transaction_deny_rules::ENotSystemAddress)]
fun update_rejects_non_system_sender() {
    let mut scenario = test_scenario::begin(@0x0);
    transaction_deny_rules::create_for_testing(scenario.ctx());
    scenario.next_tx(@0xA11CE);

    let mut rules = scenario.take_shared<TransactionDenyRules>();
    apply_delta(
        &mut rules,
        &mut scenario,
        vector[@0xAA],
        vector[],
        vector[],
        vector[],
        vector[],
        vector[],
    );
    test_scenario::return_shared(rules);
    scenario.end();
}
