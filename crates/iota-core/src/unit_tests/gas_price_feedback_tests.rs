// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_macros::sim_test;
use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, get_key_pair},
    effects::{TransactionEffects, TransactionEffectsAPI, UnchangedSharedKind},
    executable_transaction::VerifiedExecutableTransaction,
    execution_status::{CongestedObjects, ExecutionFailureStatus, ExecutionStatus},
    messages_consensus::ConsensusDeterminedVersionAssignments,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        ObjectArg, ProgrammableTransaction, Transaction, TransactionData, TransactionDataAPI,
        TransactionKind, VerifiedCertificate,
    },
    utils::to_sender_signed_transaction,
};
use move_core_types::ident_str;
use rand::seq::SliceRandom;

use crate::{
    authority::{
        AuthorityState,
        authority_tests::{
            certify_transaction, send_and_confirm_transaction_, send_batch_consensus_no_execution,
        },
        move_integration_tests::build_and_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    move_call,
};

/// Reference gas price used in gas price feedback mechanism tests.
const REFERENCE_GAS_PRICE_FOR_TESTS: u64 = 1_000;

/// Default gas units used in gas price feedback mechanism tests.
const DEFAULT_GAS_UNITS_FOR_TESTS: u64 = 10_000;

/// Container holding gas object ID, gas price, and gas budget.
struct GasDataForTests {
    gas_object_id: ObjectID,
    gas_price: u64,
    gas_budget: u64,
}

impl GasDataForTests {
    fn new(gas_object_id: ObjectID, gas_price: u64, gas_budget: u64) -> Self {
        Self {
            gas_object_id,
            gas_price,
            gas_budget,
        }
    }
}

struct GasPriceFeedbackTester {
    authority_state: Arc<AuthorityState>,
    protocol_config: ProtocolConfig,
    sender: IotaAddress,
    sender_key: AccountKeyPair,
    gas_object_ids: Vec<ObjectID>,
    package: ObjectRef,
    shared_counter_1: ObjectRef,
    shared_counter_2: ObjectRef,
}

impl GasPriceFeedbackTester {
    /// Create a new `GasPriceFeedbackTester`. Under the hood, this builds
    /// a new `AuthorityState` with protocol config parameters related to
    /// shared object congestion. This will also deploy a number of gas
    /// objects needed to send test transactions, and deploy a package with
    /// two shared counters and simple Move calls operating on those counters.
    async fn new(
        max_deferral_rounds_for_congestion_control: u64,
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
        max_execution_duration_per_commit: Option<u64>,
        assign_min_free_execution_slot: bool,
        enable_gas_price_feedback_mechanism: bool,
        num_gas_objects: usize,
    ) -> Self {
        let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();

        let mut protocol_config =
            ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
        protocol_config.set_max_deferral_rounds_for_congestion_control_for_testing(
            max_deferral_rounds_for_congestion_control,
        );
        protocol_config
            .set_per_object_congestion_control_mode_for_testing(per_object_congestion_control_mode);
        if let Some(max_execution_duration_per_commit) = max_execution_duration_per_commit {
            protocol_config
                .set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(
                    max_execution_duration_per_commit,
                );
        } else {
            protocol_config
                .disable_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing();
        }
        protocol_config.set_congestion_control_min_free_execution_slot_for_testing(
            assign_min_free_execution_slot,
        );
        protocol_config.set_congestion_control_gas_price_feedback_mechanism_for_testing(
            enable_gas_price_feedback_mechanism,
        );

        let authority_state = TestAuthorityBuilder::new()
            .with_reference_gas_price(REFERENCE_GAS_PRICE_FOR_TESTS)
            .with_protocol_config(protocol_config.clone())
            .build()
            .await;

        let gas_object_ids = (0..num_gas_objects)
            .map(|_| ObjectID::random())
            .collect::<Vec<_>>();
        let gas_objects = gas_object_ids
            .iter()
            .map(|gas_object_id| Object::with_id_owner_for_testing(*gas_object_id, sender))
            .collect::<Vec<_>>();
        authority_state.insert_genesis_objects(&gas_objects).await;

        let gas_object_id = gas_object_ids.first().unwrap();

        let package = build_and_publish_test_package(
            &authority_state,
            &sender,
            &sender_key,
            gas_object_id,
            "gas_price_feedback",
            false,
        )
        .await;

        let shared_counter_1 = Self::create_shared_counter(
            &authority_state,
            &package.0,
            gas_object_id,
            &sender,
            &sender_key,
        )
        .await;

        let shared_counter_2 = Self::create_shared_counter(
            &authority_state,
            &package.0,
            gas_object_id,
            &sender,
            &sender_key,
        )
        .await;

        Self {
            authority_state,
            protocol_config,
            sender,
            sender_key,
            gas_object_ids,
            package,
            shared_counter_1,
            shared_counter_2,
        }
    }

    /// Build and execute a transaction that creates a shared counter.
    async fn create_shared_counter(
        authority_state: &AuthorityState,
        package_id: &ObjectID,
        gas_object_id: &ObjectID,
        sender: &IotaAddress,
        sender_key: &AccountKeyPair,
    ) -> ObjectRef {
        let mut builder = ProgrammableTransactionBuilder::new();

        move_call! {
            builder,
            (*package_id)::gas_price_feedback::create_shared_counter()
        };

        let pt = builder.finish();

        let gas_object_ref = authority_state
            .get_object(gas_object_id)
            .await
            .unwrap()
            .unwrap()
            .compute_object_reference();

        let transaction_data = TransactionData::new_programmable(
            *sender,
            vec![gas_object_ref],
            pt,
            REFERENCE_GAS_PRICE_FOR_TESTS * DEFAULT_GAS_UNITS_FOR_TESTS,
            REFERENCE_GAS_PRICE_FOR_TESTS,
        );

        let transaction = to_sender_signed_transaction(transaction_data, sender_key);

        let effects = send_and_confirm_transaction_(authority_state, None, transaction, false)
            .await
            .unwrap()
            .1
            .into_data();

        assert!(
            effects.status().is_ok(),
            "Execution error {:?}",
            effects.status()
        );
        assert_eq!(effects.created().len(), 1);

        effects.created()[0].0
    }

    /// Build and sign a programmable transaction.
    async fn build_programmable_transaction(
        &self,
        pt: ProgrammableTransaction,
        gas_data: GasDataForTests,
    ) -> Transaction {
        let gas_object_ref = self
            .authority_state
            .get_object(&gas_data.gas_object_id)
            .await
            .unwrap()
            .unwrap()
            .compute_object_reference();

        let transaction_data = TransactionData::new_programmable(
            self.sender,
            vec![gas_object_ref],
            pt,
            gas_data.gas_budget,
            gas_data.gas_price,
        );

        to_sender_signed_transaction(transaction_data, &self.sender_key)
    }

    /// Certify a transaction signed by the user.
    async fn certify_transaction(&self, transaction: Transaction) -> VerifiedCertificate {
        certify_transaction(&self.authority_state, transaction)
            .await
            .unwrap()
    }

    /// Send certificates to consensus for scheduling.
    async fn send_certificates_to_consensus_for_scheduling(
        &self,
        certificates: &[VerifiedCertificate],
    ) -> Vec<VerifiedExecutableTransaction> {
        send_batch_consensus_no_execution(&self.authority_state, certificates, false).await
    }

    /// Enqueue scheduled transactions and execute them to effects.
    async fn enqueue_and_execute_scheduled_transactions(
        &self,
        transactions: Vec<VerifiedExecutableTransaction>,
    ) -> Vec<TransactionEffects> {
        let transaction_digests = transactions
            .iter()
            .map(|tx| *tx.digest())
            .collect::<Vec<_>>();

        self.authority_state.transaction_manager().enqueue(
            transactions,
            &self.authority_state.epoch_store_for_testing(),
        );

        self.authority_state
            .get_transaction_cache_reader()
            .notify_read_executed_effects(&transaction_digests)
            .await
            .unwrap()
    }

    /// Build and sign a programmable transaction that accesses both counters.
    /// `counter_1_mutable` and `counter_2_mutable` flags control how the
    /// counters are accessed: mutably or immutably.
    async fn build_access_both_counters_transaction(
        &self,
        gas_data: GasDataForTests,
        counter_1_mutable: bool,
        counter_2_mutable: bool,
    ) -> Transaction {
        let mut txn_builder = ProgrammableTransactionBuilder::new();

        let arg1 = txn_builder
            .obj(ObjectArg::SharedObject {
                id: self.shared_counter_1.0,
                initial_shared_version: self.shared_counter_1.1,
                mutable: counter_1_mutable,
            })
            .unwrap();

        let arg2 = txn_builder
            .obj(ObjectArg::SharedObject {
                id: self.shared_counter_2.0,
                initial_shared_version: self.shared_counter_2.1,
                mutable: counter_2_mutable,
            })
            .unwrap();

        if counter_1_mutable && counter_2_mutable {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::increment_both(arg1, arg2)
            };
        } else if counter_1_mutable && !counter_2_mutable {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::increment_first_read_second(arg1, arg2)
            };
        } else if !counter_1_mutable && counter_2_mutable {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::read_first_increment_second(arg1, arg2)
            };
        } else {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::read_both(arg1, arg2)
            };
        }

        let pt = txn_builder.finish();

        self.build_programmable_transaction(pt, gas_data).await
    }
}

// Test that everything goes well (i.e., no transactions are deferred or
// cancelled) if per-object congestion control mode is None.
#[sim_test]
async fn per_object_congestion_control_mode_is_none() {
    let max_deferral_rounds_for_congestion_control = 0;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::None;
    let max_execution_duration_per_commit = Some(1);
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = true;
    let num_gas_objects = 10;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for (i, gas_object_id) in tester.gas_object_ids.iter().enumerate() {
        let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + i as u64;
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            gas_price,
            gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, true)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );
    assert!(matches!(
        scheduled_transactions[0].data().transaction_data().kind(),
        TransactionKind::ConsensusCommitPrologueV1(..)
    ));

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // All transactions should be successfully executed.
    for effects in effects_vec {
        assert!(effects.status().is_ok());
    }
}

// Test that everything goes well (i.e., no transactions are deferred or
// cancelled) if `max_execution_duration_per_commit` is set None.
#[sim_test]
async fn max_execution_duration_per_commit_is_none() {
    let max_deferral_rounds_for_congestion_control = 0;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalTxCount;
    let max_execution_duration_per_commit = None;
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = true;
    let num_gas_objects = 10;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for (i, gas_object_id) in tester.gas_object_ids.iter().enumerate() {
        let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + i as u64;
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            gas_price,
            gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, true)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );
    assert!(matches!(
        scheduled_transactions[0].data().transaction_data().kind(),
        TransactionKind::ConsensusCommitPrologueV1(..)
    ));

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // All transactions should be successfully executed.
    for effects in effects_vec {
        assert!(effects.status().is_ok());
    }
}

// Test that the suggested gas price calculator panics if
// `max_execution_duration_per_commit` is set too low such that even
// one transaction cannot fit in a commit.
#[tokio::test]
#[should_panic] // because `max_execution_duration_per_commit` is set too low.
async fn max_execution_duration_per_commit_too_low_in_total_tx_count_mode() {
    let max_deferral_rounds_for_congestion_control = 10;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalTxCount;
    // Intentionally set to 0 so that even one transaction will not fit in a
    // single commit.
    let max_execution_duration_per_commit = Some(0);
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = true;
    let num_gas_objects = 2;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for (i, gas_object_id) in tester.gas_object_ids.iter().enumerate() {
        let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + i as u64;
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            gas_price,
            gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, true)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let _scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
}

// Test that the suggested gas price calculator panics if
// `max_execution_duration_per_commit` is set too low such that even
// one transaction cannot fit in a commit.
#[tokio::test]
#[should_panic] // because `max_execution_duration_per_commit` is set too low.
async fn max_execution_duration_per_commit_too_low_in_total_gas_budget_mode() {
    let max_deferral_rounds_for_congestion_control = 10;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalGasBudget;
    // Intentionally set too low so that even one transaction will not fit in a
    // single commit.
    let max_execution_duration_per_commit =
        Some(REFERENCE_GAS_PRICE_FOR_TESTS * DEFAULT_GAS_UNITS_FOR_TESTS);
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = true;
    let num_gas_objects = 2;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for (i, gas_object_id) in tester.gas_object_ids.iter().enumerate() {
        let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + i as u64;
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            gas_price,
            gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, true)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let _scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
}

// Test that everything works well if the gas price feedback mechanism is
// turned off: specifically, old `ExecutionCancelledDueToSharedObjectCongestion`
// and `SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK` should appear.
#[sim_test]
async fn gas_price_feedback_mechanism_is_turned_off() {
    // All deferred transactions will be cancelled
    let max_deferral_rounds_for_congestion_control = 0;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalTxCount;
    let max_execution_duration_per_commit = Some(1);
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = false;
    let num_gas_objects = 2;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for (i, gas_object_id) in tester.gas_object_ids.iter().enumerate() {
        let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + i as u64;
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            gas_price,
            gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, true)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // The first executed transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![(
            *scheduled_transactions[2].digest(),
            vec![
                (
                    tester.shared_counter_1.0,
                    SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK,
                ),
                (
                    tester.shared_counter_2.0,
                    SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK,
                ),
            ],
        )];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a `ConsensusCommitPrologueV1` transaction.");
    }

    // Confirm that gas price order of scheduled transactions is descending
    assert_eq!(
        scheduled_transactions[1]
            .data()
            .transaction_data()
            .gas_price(),
        REFERENCE_GAS_PRICE_FOR_TESTS + 1
    );
    assert_eq!(
        scheduled_transactions[2]
            .data()
            .transaction_data()
            .gas_price(),
        REFERENCE_GAS_PRICE_FOR_TESTS
    );

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // `ConsensusCommitPrologueV1` should be successfully executed
    assert!(effects_vec[0].status().is_ok());
    // The first transaction should be successfully executed
    assert!(effects_vec[1].status().is_ok());

    // The second transaction should be cancelled
    if let ExecutionStatus::Failure { error, command } = effects_vec[2].status() {
        assert!(command.is_none());
        if let ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestion {
            congested_objects,
        } = error
        {
            // Check is returned congested_objects are correct.
            assert_eq!(
                *congested_objects,
                CongestedObjects(vec![tester.shared_counter_1.0, tester.shared_counter_2.0])
            );
        } else {
            panic!(
                "`ExecutionFailureStatus` must be `ExecutionCancelledDueToSharedObjectCongestion`."
            );
        }
    } else {
        panic!("The second transaction must be cancelled.")
    }

    // Check if unchanged_shared_objects in effects of the cancelled transaction
    // are correct
    assert_eq!(
        effects_vec[2].unchanged_shared_objects(),
        vec![
            (
                tester.shared_counter_1.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK
                )
            ),
        ]
    );
}

// Test that suggested gas price does not exceed the max gas price set in
// the protocol.
#[sim_test]
async fn gas_price_feedback_mechanism_with_max_gas_price() {
    let max_gas_price = 100_000;

    // All deferred transactions will be cancelled
    let max_deferral_rounds_for_congestion_control = 0;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalGasBudget;
    let max_execution_duration_per_commit = Some(max_gas_price * DEFAULT_GAS_UNITS_FOR_TESTS);
    let assign_min_free_execution_slot = true;
    let enable_gas_price_feedback_mechanism = true;
    let num_gas_objects = 2;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        assign_min_free_execution_slot,
        enable_gas_price_feedback_mechanism,
        num_gas_objects,
    )
    .await;
    assert_eq!(max_gas_price, tester.protocol_config.max_gas_price());

    // Prepare certificates
    let mut certificates = vec![];
    for gas_object_id in tester.gas_object_ids.iter() {
        let gas_data = GasDataForTests::new(
            *gas_object_id,
            max_gas_price,
            max_gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
        );
        let transaction = tester
            .build_access_both_counters_transaction(gas_data, true, false)
            .await;
        let certificate = tester.certify_transaction(transaction).await;

        certificates.push(certificate);
    }
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    let suggested_gas_price = tester.protocol_config.max_gas_price();

    // The first executed transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![(
            *scheduled_transactions[2].digest(),
            vec![
                (
                    tester.shared_counter_1.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price),
                ),
                (
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price),
                ),
            ],
        )];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a `ConsensusCommitPrologueV1` transaction.");
    }

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // `ConsensusCommitPrologueV1` should be successfully executed
    assert!(effects_vec[0].status().is_ok());
    // The first transaction should be successfully executed
    assert!(effects_vec[1].status().is_ok());

    // The second transaction should be cancelled
    if let ExecutionStatus::Failure { error, command } = effects_vec[2].status() {
        assert!(command.is_none());
        if let ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
            congested_objects,
            suggested_gas_price,
        } = error
        {
            // Check is returned congested_objects and suggested_gas_price are correct.
            assert_eq!(
                *congested_objects,
                CongestedObjects(vec![tester.shared_counter_1.0, tester.shared_counter_2.0])
            );
            assert_eq!(*suggested_gas_price, tester.protocol_config.max_gas_price());
        } else {
            panic!(
                "`ExecutionFailureStatus` must be `ExecutionCancelledDueToSharedObjectCongestion`."
            );
        }
    } else {
        panic!("The second transaction must be cancelled.")
    }

    // Check if unchanged_shared_objects in effects of the cancelled transaction
    // are correct
    assert_eq!(
        effects_vec[2].unchanged_shared_objects(),
        vec![
            (
                tester.shared_counter_1.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price)
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price)
                )
            ),
        ]
    );
}
