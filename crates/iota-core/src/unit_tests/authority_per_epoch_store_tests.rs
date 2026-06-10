// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use iota_protocol_config::PerObjectCongestionControlMode;
use iota_types::{
    attestation::{Attestation, AttestationData},
    base_types::{ObjectID, TransactionDigest},
};
use starfish_config::AuthorityIndex;
use tokio::time::timeout;

use crate::{
    authority::{
        authority_per_epoch_store::CongestionControlParameters,
        shared_object_congestion_tracker::{
            SequencingResult, SharedObjectCongestionTracker,
            shared_object_test_utils::{TEST_ONLY_GAS_PRICE, build_transaction},
        },
        test_authority_builder::TestAuthorityBuilder,
    },
    transaction_manager::VerifiedExecutableAttestedTransaction,
};

#[tokio::test]
async fn test_notify_read_executed_transactions_to_checkpoint() {
    let authority_state = TestAuthorityBuilder::new().build().await;
    let store = authority_state.epoch_store_for_testing();
    let checkpoint_sequence_1 = 10;
    let checkpoint_sequence_2 = 12;

    let txes_to_be_notified = vec![
        TransactionDigest::random(),
        TransactionDigest::random(),
        TransactionDigest::random(),
    ];

    // Insert only the first transaction already
    store
        .insert_finalized_transactions(
            vec![txes_to_be_notified[0]].as_slice(),
            checkpoint_sequence_1,
            0,
        )
        .expect("Should not fail");

    // Now register to get notified for the addition of some of the above
    // transactions
    let txes_to_be_notified_cloned = txes_to_be_notified.clone();
    let handle = tokio::spawn(async move {
        let notify = store.transactions_executed_in_checkpoint_notify(txes_to_be_notified_cloned);
        notify.await
    });

    // Now insert the rest of the transactions
    let store = authority_state.epoch_store_for_testing();
    store
        .insert_finalized_transactions(&txes_to_be_notified[1..], checkpoint_sequence_2, 0)
        .expect("Should not fail");

    // We should get notified about all the transactions having been executed via
    // checkpoints
    let _ = timeout(Duration::from_secs(5), handle)
        .await
        .expect("Should not timeout")
        .expect("Should not fail");

    // And the transactions should be found into the table
    let result = store
        .multi_get_transaction_checkpoint(txes_to_be_notified.as_slice())
        .expect("Should not fail");
    assert_eq!(result.len(), txes_to_be_notified.len());

    assert_eq!(result[0].unwrap(), checkpoint_sequence_1);
    assert_eq!(result[1].unwrap(), checkpoint_sequence_2);
    assert_eq!(result[2].unwrap(), checkpoint_sequence_2);
}

/// Under `TotalComputationCost`, the estimated execution duration is the
/// attested computation cost when present, and `gas_budget / gas_price`
/// otherwise.
#[test]
fn test_get_estimated_execution_duration_total_computation_cost_mode() {
    let params = CongestionControlParameters::new_for_test(
        PerObjectCongestionControlMode::TotalComputationCost,
        false,           // congestion_control_min_free_execution_slot
        Some(1_000_000), // max_execution_duration_per_commit
        Some(0),         // max_congestion_limit_overshoot_per_commit
        0,               // max_gas_price (irrelevant here)
        false,           // use_congestion_limit_overshoot_in_gas_price_feedback_mechanism
        true,            // use_separate_gas_price_feedback_mechanism_for_randomness
    );

    let gas_budget = 12_345;
    let attested_cost = 9_876;

    // Attested transaction returns its attested computation cost.
    let attested_tx = attest(
        build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE),
        attested_cost,
    );
    assert_eq!(
        params.get_estimated_execution_duration(&attested_tx),
        attested_cost,
    );

    // Unattested transaction falls back to gas_budget converted to gas units.
    let unattested_tx = build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE);
    assert_eq!(
        params.get_estimated_execution_duration(&unattested_tx),
        gas_budget / TEST_ONLY_GAS_PRICE,
    );

    // Unattested transaction with a zero gas price: the `gas_budget / gas_price`
    // fallback must not divide by zero.
    let zero_gas_price_tx = build_transaction(&[], gas_budget, 0);
    assert_eq!(
        params.get_estimated_execution_duration(&zero_gas_price_tx),
        0,
    );

    // An attestation with a zero computation cost should not use the gas-budget
    // fallback.
    let zero_cost_attested_tx = attest(build_transaction(&[], gas_budget, TEST_ONLY_GAS_PRICE), 0);
    assert_eq!(
        params.get_estimated_execution_duration(&zero_cost_attested_tx),
        0,
    );
}

/// Attaches a validator attestation with the given `estimated_computation_cost`
/// to a transaction produced by `build_transaction`.
fn attest(
    tx: VerifiedExecutableAttestedTransaction,
    estimated_computation_cost: u64,
) -> VerifiedExecutableAttestedTransaction {
    let (inner, _) = tx.into_parts();
    VerifiedExecutableAttestedTransaction::new(
        inner,
        Some(Attestation::Validator {
            payload: AttestationData::V1 {
                estimated_computation_cost,
                object_versions: vec![],
            },
            attestor_index: AuthorityIndex::new_for_test(0),
        }),
    )
}

/// Within `TotalComputationCost` mode, a commit of attested transactions is
/// scheduled differently from a commit of the same transactions without an
/// attestation: attested txs use the (cheap) attested cost, while unattested
/// txs fall back to the (much larger) gas budget per the documented fallback
/// in `get_estimated_execution_duration`.
///
/// Note this is an in-mode contrast, not a production comparison: under
/// `TotalTxCount` (the production default for chains without validator
/// attestation), unattested txs are billed at one unit each.
#[test]
fn test_total_computation_cost_attested_vs_unattested_commit_scheduling() {
    // Per-commit limit is large enough to schedule three attested transactions
    // (cost 30 each → cumulative 90) but too small to schedule even a single
    // unattested transaction whose gas budget converts to 200 gas units
    // (`gas_budget / gas_price`) > limit 100.
    const MAX_EXECUTION_DURATION_PER_COMMIT: u64 = 100;
    const TX_GAS_BUDGET: u64 = 200 * TEST_ONLY_GAS_PRICE;
    const TX_ATTESTED_COST: u64 = 30;

    let params = CongestionControlParameters::new_for_test(
        PerObjectCongestionControlMode::TotalComputationCost,
        false,                                   // min_free_execution_slot
        Some(MAX_EXECUTION_DURATION_PER_COMMIT), // max_execution_duration_per_commit
        Some(0),                                 // overshoot
        0,                                       // max_gas_price (irrelevant)
        false,
        true,
    );

    let shared_obj = ObjectID::random();

    // --- Attested commit: three transactions all schedule, end-to-end. ---
    let mut tracker = SharedObjectCongestionTracker::new(std::iter::empty(), params.clone());
    for i in 0..3 {
        let tx = attest(
            build_transaction(&[(shared_obj, true)], TX_GAS_BUDGET, TEST_ONLY_GAS_PRICE),
            TX_ATTESTED_COST,
        );
        let shared_input_objects = tx.shared_input_objects();
        tracker.initialize_object_execution_slots(&shared_input_objects);
        match tracker.try_schedule(&tx, &HashMap::new(), 0) {
            SequencingResult::Schedule(start_time) => {
                assert_eq!(
                    start_time,
                    i * TX_ATTESTED_COST,
                    "attested tx #{i} should be scheduled back-to-back",
                );
                tracker.bump_object_execution_slots(&tx, start_time);
            }
            SequencingResult::Defer(_, congested) => {
                panic!("attested tx #{i} should schedule, got defer on {congested:?}")
            }
        }
    }

    // --- Unattested commit: the very first transaction defers. ---
    let mut tracker = SharedObjectCongestionTracker::new(std::iter::empty(), params);
    let tx = build_transaction(&[(shared_obj, true)], TX_GAS_BUDGET, TEST_ONLY_GAS_PRICE);
    tracker.initialize_object_execution_slots(&tx.shared_input_objects());
    match tracker.try_schedule(&tx, &HashMap::new(), 0) {
        SequencingResult::Defer(_, congested) => {
            assert_eq!(congested, vec![shared_obj]);
        }
        SequencingResult::Schedule(start_time) => {
            panic!("unattested tx should defer, got schedule at {start_time}");
        }
    }
}
