// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{panic, sync::Arc};

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
        transaction_deferral::DeferralKey,
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
            // Set the allowed overshoot to 0 to simplify the test.
            protocol_config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
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

    /// Build and sign a programmable transaction that accesses one counter.
    /// The `mutable` flag control how the counter is accessed: mutably or
    /// immutably. The `first` flag control whether the first or the second
    /// counter is accessed.
    async fn build_access_one_counter_transaction(
        &self,
        gas_data: GasDataForTests,
        mutable: bool,
        first: bool,
    ) -> Transaction {
        let mut txn_builder = ProgrammableTransactionBuilder::new();

        let counter = if first {
            self.shared_counter_1
        } else {
            self.shared_counter_2
        };

        let arg = txn_builder
            .obj(ObjectArg::SharedObject {
                id: counter.0,
                initial_shared_version: counter.1,
                mutable,
            })
            .unwrap();

        if mutable {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::increment_one(arg)
            };
        } else {
            move_call! {
                txn_builder,
                (self.package.0)::gas_price_feedback::read_one(arg)
            };
        }

        let pt = txn_builder.finish();

        self.build_programmable_transaction(pt, gas_data).await
    }

    async fn create_certificates_for_non_trivial_case(&self) -> Vec<VerifiedCertificate> {
        let max_gp = self.protocol_config.max_gas_price();
        // (gas price, gas budget, counter_1_mutable, counter_2_mutable)
        let data = [
            (max_gp, 3_000_000_000, Some(true), Some(false)), // 0
            (1_011, 1_000_000_000, Some(false), Some(true)),  // 1
            (1_010, 4_000_000_000, Some(false), Some(true)),  // 2
            (1_009, 2_000_000_000, None, Some(true)),         // 3
            (1_008, 1_000_000_001, None, Some(false)),        // 4
            (1_007, 5_000_000_000, None, Some(true)),         // 5
            (1_006, 5_000_000_001, Some(true), Some(true)),   // 6
            (1_005, 8_000_000_000, Some(true), Some(true)),   // 7
            (1_004, 4_000_000_000, Some(true), None),         // 8
            (1_003, 2_000_000_000, Some(true), None),         // 9
            (1_002, 1_000_000_001, Some(false), Some(false)), // 10
            (1_001, 5_000_000_001, Some(true), Some(false)),  // 11
            (1_000, 9_000_000_000, Some(false), Some(true)),  // 12
        ];

        let mut certificates = vec![];
        for (index, data) in data.into_iter().enumerate() {
            let gas_data = GasDataForTests::new(self.gas_object_ids[index], data.0, data.1);

            let transaction = if data.2.is_some() && data.3.is_some() {
                self.build_access_both_counters_transaction(
                    gas_data,
                    data.2.unwrap(),
                    data.3.unwrap(),
                )
                .await
            } else if data.2.is_some() && data.3.is_none() {
                self.build_access_one_counter_transaction(gas_data, data.2.unwrap(), true)
                    .await
            } else if data.2.is_none() && data.3.is_some() {
                self.build_access_one_counter_transaction(gas_data, data.3.unwrap(), false)
                    .await
            } else {
                panic!("At least one counter must be accessed in transactions.");
            };

            certificates.push(self.certify_transaction(transaction).await);
        }

        certificates
    }
}

// Test that everything goes well (i.e., no transactions are deferred or
// cancelled) if per-object congestion control mode is None.
#[sim_test]
async fn per_object_congestion_control_mode_is_none() {
    let num_gas_objects = 10;
    let tester = GasPriceFeedbackTester::new(
        0,                                    // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::None, // per_object_congestion_control_mode
        Some(1),                              // max_execution_duration_per_commit
        true,                                 // assign_min_free_execution_slot
        true,                                 // enable_gas_price_feedback_mechanism
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

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

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
    let num_gas_objects = 10;
    let tester = GasPriceFeedbackTester::new(
        0,                                            // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalTxCount, // per_object_congestion_control_mode
        None,                                         // max_execution_duration_per_commit
        true,                                         // assign_min_free_execution_slot
        true,                                         // enable_gas_price_feedback_mechanism
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

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

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

// Test that the suggested gas price calculator return the correct gas price
// if there are transactions with estimated execution duration larger than
// `max_execution_duration_per_commit`, that is such, transactions cannot
// be scheduled.
#[sim_test]
async fn transaction_duration_exceeds_max_execution_duration_per_commit() {
    let num_gas_objects = 3;
    let gas_budget_of_scheduled_tx =
        (REFERENCE_GAS_PRICE_FOR_TESTS + 2) * DEFAULT_GAS_UNITS_FOR_TESTS;
    let tester = GasPriceFeedbackTester::new(
        0,                                              // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalGasBudget, // per_object_congestion_control_mode
        Some(gas_budget_of_scheduled_tx),               // max_execution_duration_per_commit
        true,                                           // assign_min_free_execution_slot
        true,                                           // enable_gas_price_feedback_mechanism
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    // Should be cancelled as it does not fit, suggested gas price must be equal
    // the reference gas price as there are no congested objects.
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[0],
        REFERENCE_GAS_PRICE_FOR_TESTS + 2,
        tester.protocol_config.max_tx_gas(),
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, true, true)
        .await;
    certificates.push(tester.certify_transaction(transaction).await);
    // Should be scheduled.
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[1],
        REFERENCE_GAS_PRICE_FOR_TESTS + 2,
        gas_budget_of_scheduled_tx,
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, true, true)
        .await;
    certificates.push(tester.certify_transaction(transaction).await);
    // Should be cancelled as it does not fit, suggested gas price must be equal
    // gas price of the scheduled transaction + 1.
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[2],
        REFERENCE_GAS_PRICE_FOR_TESTS + 1,
        tester.protocol_config.max_tx_gas(),
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, true, true)
        .await;
    certificates.push(tester.certify_transaction(transaction).await);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

    let expected_suggested_gas_price_2 = scheduled_transactions[2]
        .data()
        .transaction_data()
        .gas_price()
        + 1;

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![
            (
                *scheduled_transactions[1].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            REFERENCE_GAS_PRICE_FOR_TESTS,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            REFERENCE_GAS_PRICE_FOR_TESTS,
                        ),
                    ),
                ],
            ),
            (
                *scheduled_transactions[3].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_2,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_2,
                        ),
                    ),
                ],
            ),
        ];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
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
    // The second transaction should be scheduled.
    assert!(effects_vec[2].status().is_ok());

    // The first transaction should be cancelled
    if let ExecutionStatus::Failure { error, command } = effects_vec[1].status() {
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
            assert_eq!(*suggested_gas_price, REFERENCE_GAS_PRICE_FOR_TESTS);
        } else {
            panic!(
                "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
            );
        }
    } else {
        panic!("The transaction must be cancelled.")
    }
    // Check if unchanged_shared_objects in effects of the cancelled transaction
    // are correct
    assert_eq!(
        effects_vec[1].unchanged_shared_objects(),
        vec![
            (
                tester.shared_counter_1.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        REFERENCE_GAS_PRICE_FOR_TESTS
                    )
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        REFERENCE_GAS_PRICE_FOR_TESTS
                    )
                )
            ),
        ]
    );

    // The third transaction should be cancelled
    if let ExecutionStatus::Failure { error, command } = effects_vec[3].status() {
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
            assert_eq!(*suggested_gas_price, expected_suggested_gas_price_2);
        } else {
            panic!(
                "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
            );
        }
    } else {
        panic!("The transaction must be cancelled.")
    }
    // Check if unchanged_shared_objects in effects of the cancelled transaction
    // are correct
    assert_eq!(
        effects_vec[3].unchanged_shared_objects(),
        vec![
            (
                tester.shared_counter_1.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price_2
                    )
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price_2
                    )
                )
            ),
        ]
    );
}

// Test that everything works well if the gas price feedback mechanism is
// turned off: specifically, old `ExecutionCancelledDueToSharedObjectCongestion`
// and `SequenceNumber::CONGESTED_PRIOR_TO_GAS_PRICE_FEEDBACK` should appear.
#[sim_test]
async fn gas_price_feedback_mechanism_is_turned_off() {
    let num_gas_objects = 2;
    let tester = GasPriceFeedbackTester::new(
        0,                                            // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalTxCount, // per_object_congestion_control_mode
        Some(1),                                      // max_execution_duration_per_commit
        true,                                         // assign_min_free_execution_slot
        false,                                        // enable_gas_price_feedback_mechanism
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

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
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
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
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
            panic!("ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestion.");
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
    let num_gas_objects = 2;
    let tester = GasPriceFeedbackTester::new(
        0,                                                 // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalGasBudget,    // per_object_congestion_control_mode
        Some(max_gas_price * DEFAULT_GAS_UNITS_FOR_TESTS), // max_execution_duration_per_commit
        true,                                              // assign_min_free_execution_slot
        true,                                              // enable_gas_price_feedback_mechanism
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

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

    let expected_suggested_gas_price = tester.protocol_config.max_gas_price();

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![(
            *scheduled_transactions[2].digest(),
            vec![
                (
                    tester.shared_counter_1.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price,
                    ),
                ),
                (
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price,
                    ),
                ),
            ],
        )];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
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
            assert_eq!(*suggested_gas_price, expected_suggested_gas_price);
        } else {
            panic!(
                "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
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
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price
                    )
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price
                    )
                )
            ),
        ]
    );
}

// Test that suggested gas price for a cancelled transactions is the
// lowest suggested gas price over multiple commits in which the
// transaction was deferred.
#[sim_test]
async fn gas_price_feedback_mechanism_for_multiple_commits() {
    let max_execution_duration_per_commit = 1;
    let num_gas_objects = 2;
    let tester = GasPriceFeedbackTester::new(
        1,                                            // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalTxCount, // per_object_congestion_control_mode
        Some(max_execution_duration_per_commit),
        true, // assign_min_free_execution_slot
        true, // enable_gas_price_feedback_mechanism
        num_gas_objects,
    )
    .await;

    // Prepare certificates for consensus commit round 1
    let mut certificates = vec![];
    // Create a certificate that should be deferred
    let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS;
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[0],
        gas_price,
        gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, true, true)
        .await;
    let should_defer_certificate = tester.certify_transaction(transaction).await;
    certificates.push(should_defer_certificate.clone());
    // Create a certificate that should be scheduled
    let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + 5;
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[1],
        gas_price,
        gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, false, true)
        .await;
    let should_schedule_certificate_1 = tester.certify_transaction(transaction).await;
    certificates.push(should_schedule_certificate_1.clone());

    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len() as u64,
        // +1 because of consensus commit prologue transaction
        max_execution_duration_per_commit + 1,
    );

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(vec![])
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
    }
    // The second scheduled transaction should be one paying higher gas price
    assert_eq!(
        scheduled_transactions[1].digest(),
        should_schedule_certificate_1.digest()
    );

    // Checks that deferred transactions are formed correctly
    let deferred_transactions = tester
        .authority_state
        .epoch_store_for_testing()
        .get_all_deferred_transactions_for_test();
    assert_eq!(deferred_transactions.len(), 1);
    assert_eq!(deferred_transactions[0].1.len(), 1);
    assert!(matches!(
        deferred_transactions[0].0,
        DeferralKey::ConsensusRound { .. }
    ));
    assert_eq!(
        deferred_transactions[0].1[0].suggested_gas_price(),
        Some(should_schedule_certificate_1.gas_price() + 1)
    );

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len() as u64,
        // +1 because of consensus commit prologue transaction
        max_execution_duration_per_commit + 1,
    );

    // Both scheduled transactions should be successfully executed
    for effects in effects_vec {
        assert!(effects.status().is_ok());
    }

    // Prepare certificates for consensus commit round 2
    let mut certificates = vec![];
    // Create a certificate that should be scheduled
    let gas_price = REFERENCE_GAS_PRICE_FOR_TESTS + 10;
    let gas_data = GasDataForTests::new(
        tester.gas_object_ids[1],
        gas_price,
        gas_price * DEFAULT_GAS_UNITS_FOR_TESTS,
    );
    let transaction = tester
        .build_access_both_counters_transaction(gas_data, false, true)
        .await;
    let should_schedule_certificate_2 = tester.certify_transaction(transaction).await;
    certificates.push(should_schedule_certificate_2.clone());

    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), 1);

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +2 because one consensus commit prologue transaction and one cancelled transaction
        certificates.len() + 2,
    );

    // Suggested gas price must be gas price of the scheduled certificate in the
    // first commit round (not the current commit round) plus one
    let expected_suggested_gas_price = should_schedule_certificate_1.gas_price() + 1;

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![(
            *scheduled_transactions[2].digest(),
            vec![
                (
                    tester.shared_counter_1.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price,
                    ),
                ),
                (
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price,
                    ),
                ),
            ],
        )];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
    }
    // The second scheduled transaction should be one paying higher gas price
    assert_eq!(
        scheduled_transactions[1].digest(),
        should_schedule_certificate_2.digest()
    );
    // The third scheduled transaction should be the canceled transaction
    assert_eq!(
        scheduled_transactions[2].digest(),
        should_defer_certificate.digest()
    );

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +2 because one consensus commit prologue transaction and one cancelled transaction
        certificates.len() + 2,
    );

    // `ConsensusCommitPrologueV1` should be successfully executed
    assert!(effects_vec[0].status().is_ok());
    // The first scheduled transaction should be successfully executed
    assert!(effects_vec[1].status().is_ok());

    // The second scheduled transaction should be cancelled
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
            assert_eq!(*suggested_gas_price, expected_suggested_gas_price);
        } else {
            panic!(
                "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
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
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price
                    )
                )
            ),
            (
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price
                    )
                )
            ),
        ]
    );
}

// Test gas price feedback mechanism in `TotalTxCount` mode in non-trivial case.
#[sim_test]
async fn gas_price_feedback_mechanism_non_trivial_case_total_tx_count_mode() {
    let num_gas_objects = 13;
    let tester = GasPriceFeedbackTester::new(
        0,                                            // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalTxCount, // per_object_congestion_control_mode
        Some(3),                                      // max_execution_duration_per_commit
        true,                                         // assign_min_free_execution_slot
        true,                                         // enable_gas_price_feedback_mechanism
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let certificates = tester.create_certificates_for_non_trivial_case().await;
    assert_eq!(certificates.len(), num_gas_objects);
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    let mut shuffled_certificates = certificates.clone();
    shuffled_certificates.shuffle(&mut rand::thread_rng());

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&shuffled_certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // Recall the structure of these certificates:
    // (gas price, gas budget, counter_1_mutable, counter_2_mutable)
    // [
    //     (100K, 3_000_000_000, Some(true),  Some(false)), // 0
    //     (1011, 1_000_000_000, Some(false), Some(true)),  // 1
    //     (1010, 4_000_000_000, Some(false), Some(true)),  // 2
    //     (1009, 2_000_000_000, None,        Some(true)),  // 3
    //     (1008, 1_000_000_001, None,        Some(false)), // 4
    //     (1007, 5_000_000_000, None,        Some(true)),  // 5
    //     (1006, 5_000_000_001, Some(true),  Some(true)),  // 6
    //     (1005, 8_000_000_000, Some(true),  Some(true)),  // 7
    //     (1004, 4_000_000_000, Some(true),  None),        // 8
    //     (1003, 2_000_000_000, Some(true),  None),        // 9
    //     (1002, 1_000_000_001, Some(false), Some(false)), // 10
    //     (1001, 5_000_000_001, Some(true),  Some(false)), // 11
    //     (1000, 9_000_000_000, Some(false), Some(true)),  // 12
    // ];

    // Allocations of mutably accessed shared objects should look as follows:
    // |-------------------------------------|------------|
    // |     object_1     |     object_2     | start time |
    // |__________________|__________________|____________|
    // |------------------|------------------|---- 3      |
    // | cert. 9 (g=1003) | cert. 2 (g=1010) |            |
    // |------------------|------------------|---- 2      |
    // | cert. 8 (g=1004) | cert. 1 (g=1011) |            |
    // |------------------|------------------|---- 1      |
    // | cert. 0 (g=100K) | cert. 3 (g=1009) |            |
    // |-------------------------------------|---- 0 -----|
    // That is, certificates 4, 5, 6, 7, 10, 11, 12 should be cancelled.

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

    // As can be seen from the illustration above:
    let expected_suggested_gas_price_for_object_1 = certificates[9].gas_price() + 1;
    let expected_suggested_gas_price_for_object_2 = certificates[2].gas_price() + 1;
    let expected_suggested_gas_price_for_both_objects =
        expected_suggested_gas_price_for_object_1.max(expected_suggested_gas_price_for_object_2);

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![
            (
                *certificates[4].digest(),
                vec![(
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price_for_object_2,
                    ),
                )],
            ),
            (
                *certificates[5].digest(),
                vec![(
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price_for_object_2,
                    ),
                )],
            ),
            (
                *certificates[6].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                ],
            ),
            (
                *certificates[7].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                ],
            ),
            (
                *certificates[10].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                ],
            ),
            (
                *certificates[11].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                ],
            ),
            (
                *certificates[12].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects,
                        ),
                    ),
                ],
            ),
        ];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
    }

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // `ConsensusCommitPrologueV1` and first 6 scheduled transactions should be
    // successfully executed
    for effects in effects_vec.iter().take(7) {
        assert!(effects.status().is_ok());
    }

    // The rest of transactions should be cancelled:
    //
    // Transactions that touch shared counter 2:
    for effects in effects_vec.iter().skip(7).take(2) {
        if let ExecutionStatus::Failure { error, command } = effects.status() {
            assert!(command.is_none());
            if let ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                congested_objects,
                suggested_gas_price,
            } = error
            {
                // Check is returned congested_objects and suggested_gas_price are correct.
                assert_eq!(
                    *congested_objects,
                    CongestedObjects(vec![tester.shared_counter_2.0])
                );
                assert_eq!(
                    *suggested_gas_price,
                    expected_suggested_gas_price_for_object_2
                );
            } else {
                panic!(
                    "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
                );
            }
        } else {
            panic!("Transaction should have been be cancelled.")
        }
        // Check if unchanged_shared_objects in effects of the cancelled transaction
        // are correct
        assert_eq!(
            effects.unchanged_shared_objects(),
            vec![(
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price_for_object_2
                    )
                )
            ),]
        );
    }
    // Transactions that touch both shared counters:
    for effects in effects_vec.iter().skip(9).take(5) {
        if let ExecutionStatus::Failure { error, command } = effects.status() {
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
                assert_eq!(
                    *suggested_gas_price,
                    expected_suggested_gas_price_for_both_objects
                );
            } else {
                panic!(
                    "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
                );
            }
        } else {
            panic!("Transaction should have been be cancelled.")
        }
        // Check if unchanged_shared_objects in effects of the cancelled transaction
        // are correct
        assert_eq!(
            effects.unchanged_shared_objects(),
            vec![
                (
                    tester.shared_counter_1.0,
                    UnchangedSharedKind::Cancelled(
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects
                        )
                    )
                ),
                (
                    tester.shared_counter_2.0,
                    UnchangedSharedKind::Cancelled(
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price_for_both_objects
                        )
                    )
                ),
            ]
        );
    }
}

// Test gas price feedback mechanism in `TotalGasBudget` mode in non-trivial
// case.
#[sim_test]
async fn gas_price_feedback_mechanism_non_trivial_case_total_gas_budget_mode() {
    let num_gas_objects = 13;
    let tester = GasPriceFeedbackTester::new(
        0,                                              // max_deferral_rounds_for_congestion_control
        PerObjectCongestionControlMode::TotalGasBudget, // per_object_congestion_control_mode
        Some(9_000_000_000),                            // max_execution_duration_per_commit
        true,                                           // assign_min_free_execution_slot
        true,                                           // enable_gas_price_feedback_mechanism
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let certificates = tester.create_certificates_for_non_trivial_case().await;
    assert_eq!(certificates.len(), num_gas_objects);
    // Shuffle certificates so that they do not have any specific order in
    // terms of gas price.
    let mut shuffled_certificates = certificates.clone();
    shuffled_certificates.shuffle(&mut rand::thread_rng());

    let scheduled_transactions = tester
        .send_certificates_to_consensus_for_scheduling(&shuffled_certificates)
        .await;
    assert_eq!(
        scheduled_transactions.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // Recall the structure of these certificates:
    // (gas price, gas budget, counter_1_mutable, counter_2_mutable)
    // [
    //     (100K, 3_000_000_000, Some(true),  Some(false)), // 0
    //     (1011, 1_000_000_000, Some(false), Some(true)),  // 1
    //     (1010, 4_000_000_000, Some(false), Some(true)),  // 2
    //     (1009, 2_000_000_000, None,        Some(true)),  // 3
    //     (1008, 1_000_000_001, None,        Some(false)), // 4
    //     (1007, 5_000_000_000, None,        Some(true)),  // 5
    //     (1006, 5_000_000_001, Some(true),  Some(true)),  // 6
    //     (1005, 8_000_000_000, Some(true),  Some(true)),  // 7
    //     (1004, 4_000_000_000, Some(true),  None),        // 8
    //     (1003, 2_000_000_000, Some(true),  None),        // 9
    //     (1002, 1_000_000_001, Some(false), Some(false)), // 10
    //     (1001, 5_000_000_001, Some(true),  Some(false)), // 11
    //     (1000, 9_000_000_000, Some(false), Some(true)),  // 12
    // ];

    // Allocations of mutably accessed shared objects should look as follows:
    // |-------------------------------------------------|------------|
    // |        object_1        |        object_2        | start time |
    // |________________________|________________________|____________|
    // |------------------------|------------------------|---- 9B     |
    // |                        |                        |            |
    // | cert. 9 (g=5000, d=2B) |------------------------|---- 8B     |
    // |                        |                        |            |
    // |------------------------|                        |---- 7B     |
    // |                        |                        |            |
    // |                        | cert. 2 (g=8000, d=4B) |---- 6B     |
    // |                        |                        |            |
    // | cert. 8 (g=6000, d=4B) |                        |---- 5B     |
    // |                        |                        |            |
    // |                        |------------------------|---- 4B     |
    // |                        | cert. 1 (g=9000, d=1B) |            |
    // |------------------------|------------------------|---- 3B     |
    // |                        |                        |            |
    // |                        |------------------------|---- 2B     |
    // | cert. 0 (g=100K, d=3M) |                        |            |
    // |                        | cert. 3 (g=7000, d=2B) |---- 1B     |
    // |                        |                        |            |
    // |-------------------------------------------------|---- 0 -----|
    // That is, certificates 4, 5, 6, 7, 10, 11, 12 should be cancelled.

    // Checks that there are no deferred transactions
    assert!(
        tester
            .authority_state
            .epoch_store_for_testing()
            .get_all_deferred_transactions_for_test()
            .is_empty()
    );

    // The first scheduled transaction should be `ConsensusCommitPrologueV1`
    if let TransactionKind::ConsensusCommitPrologueV1(prologue_tx) =
        scheduled_transactions[0].data().transaction_data().kind()
    {
        // Check if `ConsensusDeterminedVersionAssignments` are correct.
        let cancelled_txs = vec![
            (
                *certificates[4].digest(),
                vec![(
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        certificates[2].gas_price() + 1,
                    ),
                )],
            ),
            (
                *certificates[5].digest(),
                vec![(
                    tester.shared_counter_2.0,
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        certificates[2].gas_price() + 1,
                    ),
                )],
            ),
            (
                *certificates[6].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[1].gas_price() + 1,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[1].gas_price() + 1,
                        ),
                    ),
                ],
            ),
            (
                *certificates[7].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[0].gas_price(),
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[0].gas_price(),
                        ),
                    ),
                ],
            ),
            (
                *certificates[10].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[2].gas_price() + 1,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[2].gas_price() + 1,
                        ),
                    ),
                ],
            ),
            (
                *certificates[11].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[1].gas_price() + 1,
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[1].gas_price() + 1,
                        ),
                    ),
                ],
            ),
            (
                *certificates[12].digest(),
                vec![
                    (
                        tester.shared_counter_1.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[0].gas_price(),
                        ),
                    ),
                    (
                        tester.shared_counter_2.0,
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            certificates[0].gas_price(),
                        ),
                    ),
                ],
            ),
        ];
        assert_eq!(
            prologue_tx.consensus_determined_version_assignments,
            ConsensusDeterminedVersionAssignments::CancelledTransactions(cancelled_txs)
        );
    } else {
        panic!("First scheduled transaction must be a ConsensusCommitPrologueV1 transaction.");
    }

    let effects_vec = tester
        .enqueue_and_execute_scheduled_transactions(scheduled_transactions)
        .await;
    assert_eq!(
        effects_vec.len(),
        // +1 because of consensus commit prologue transaction
        certificates.len() + 1,
    );

    // `ConsensusCommitPrologueV1` and first 6 scheduled transactions should be
    // successfully executed
    for effects in effects_vec.iter().take(7) {
        assert!(effects.status().is_ok());
    }

    // The rest of transactions should be cancelled:
    //
    // Transactions that touch shared counter 2:
    let expected_suggested_gas_price = certificates[2].gas_price() + 1;
    for effects in effects_vec.iter().skip(7).take(2) {
        if let ExecutionStatus::Failure { error, command } = effects.status() {
            assert!(command.is_none());
            if let ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                congested_objects,
                suggested_gas_price,
            } = error
            {
                // Check is returned congested_objects and suggested_gas_price are correct.
                assert_eq!(
                    *congested_objects,
                    CongestedObjects(vec![tester.shared_counter_2.0])
                );
                assert_eq!(*suggested_gas_price, expected_suggested_gas_price);
            } else {
                panic!(
                    "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
                );
            }
        } else {
            panic!("Transaction should have been be cancelled.")
        }
        // Check if unchanged_shared_objects in effects of the cancelled transaction
        // are correct
        assert_eq!(
            effects.unchanged_shared_objects(),
            vec![(
                tester.shared_counter_2.0,
                UnchangedSharedKind::Cancelled(
                    SequenceNumber::new_congested_with_suggested_gas_price(
                        expected_suggested_gas_price
                    )
                )
            ),]
        );
    }
    // Transactions that touch both shared counters:
    for (i, effects) in effects_vec.iter().skip(9).take(5).enumerate() {
        let expected_suggested_gas_price = if i == 0 || i == 3 {
            certificates[1].gas_price() + 1
        } else if i == 1 || i == 4 {
            certificates[0].gas_price()
        } else if i == 2 {
            certificates[2].gas_price() + 1
        } else {
            panic!("Expected only 5 effects to iterate.")
        };

        if let ExecutionStatus::Failure { error, command } = effects.status() {
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
                assert_eq!(*suggested_gas_price, expected_suggested_gas_price);
            } else {
                panic!(
                    "ExecutionFailureStatus must be ExecutionCancelledDueToSharedObjectCongestionV2."
                );
            }
        } else {
            panic!("Transaction should have been be cancelled.")
        }
        // Check if unchanged_shared_objects in effects of the cancelled transaction
        // are correct
        assert_eq!(
            effects.unchanged_shared_objects(),
            vec![
                (
                    tester.shared_counter_1.0,
                    UnchangedSharedKind::Cancelled(
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price
                        )
                    )
                ),
                (
                    tester.shared_counter_2.0,
                    UnchangedSharedKind::Cancelled(
                        SequenceNumber::new_congested_with_suggested_gas_price(
                            expected_suggested_gas_price
                        )
                    )
                ),
            ]
        );
    }
}
