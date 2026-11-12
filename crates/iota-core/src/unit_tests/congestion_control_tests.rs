// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use iota_macros::{register_fail_point_arg, sim_test};
use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, Identifier, InputSharedObject, ObjectId,
    ObjectReference, ObjectVersion, RandomnessRound, SharedObjectReference, Transaction,
    TransactionDigest, TransactionEffects, TransactionKind, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    attestation::AttestedTransaction,
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    randomness_state::get_randomness_state_obj_initial_shared_version,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        TransactionAPI as _, TransactionEnvelope, TransactionKey, VerifiedTransaction,
    },
    utils::to_sender_signed_transaction,
};

use crate::{
    authority::{
        AuthorityState, ExecutionEnv,
        authority_per_epoch_store::{
            CongestionControlParameters, PreviouslyDeferredTransactions,
            consensus_quarantine::ConsensusCommitOutput,
        },
        authority_test_utils::{
            init_state_with_ids, init_state_with_objects, init_transfer_transaction,
        },
        authority_tests::{
            build_programmable_transaction, certify_shared_obj_transaction_no_execution,
            execute_programmable_transaction, send_and_confirm_transaction_,
        },
        move_integration_tests::build_and_publish_test_package,
        shared_object_congestion_tracker::{
            CongestionPerObjectDebt,
            shared_object_test_utils::new_congestion_tracker_with_initial_value_for_test,
        },
        shared_object_version_manager::{AssignedTxAndVersions, Schedulable},
        suggested_gas_price_calculator::suggested_gas_price_calculator_test_utils::new_suggested_gas_price_calculator_with_initial_values_for_test,
        test_authority_builder::TestAuthorityBuilder,
        transaction_deferral::DeferralKey,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::{
        ConsensusCommitInfo, SequencedConsensusTransaction, VerifiedSequencedConsensusTransaction,
    },
    move_call,
    test_utils::set_scheduler_env,
};

pub const TEST_ONLY_GAS_PRICE: u64 = 1000;
pub const TEST_ONLY_GAS_UNIT: u64 = 10_000;

// Note that TestSetup is currently purposely created for
// test_congestion_control_execution_cancellation.
struct TestSetup {
    setup_authority_state: Arc<AuthorityState>,
    protocol_config: ProtocolConfig,
    sender: Address,
    sender_key: AccountPrivateKey,
    package: ObjectReference,
    gas_object_id: ObjectId,
}

impl TestSetup {
    async fn new(
        max_execution_duration_per_commit: u64,
        max_congestion_limit_overshoot_per_commit: u64,
    ) -> Self {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

        let mut protocol_config =
            ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
        protocol_config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalGasBudget,
        );

        protocol_config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(
            max_execution_duration_per_commit,
        );
        protocol_config.set_max_congestion_limit_overshoot_per_commit_for_testing(
            max_congestion_limit_overshoot_per_commit,
        );

        // Set max deferral rounds to 0 to testr cancellation. All deferred transactions
        // will be cancelled.
        protocol_config.set_max_deferral_rounds_for_congestion_control_for_testing(0);

        let setup_authority_state = TestAuthorityBuilder::new()
            .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
            .with_protocol_config(protocol_config.clone())
            .build()
            .await;

        let gas_object_id = ObjectId::random();
        let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);
        setup_authority_state.insert_genesis_object(gas_object.clone());

        let package = build_and_publish_test_package(
            &setup_authority_state,
            &sender,
            &sender_key,
            &gas_object_id,
            "congestion_control",
            false,
        )
        .await;

        Self {
            setup_authority_state,
            protocol_config,
            sender,
            sender_key,
            package,
            gas_object_id,
        }
    }

    // Creates a shared object in `setup_authority_state` and returns the object
    // reference.
    async fn create_shared_object(&self) -> ObjectReference {
        let mut builder = ProgrammableTransactionBuilder::new();
        move_call! {
            builder,
            (self.package.object_id)::congestion_control::create_shared()
        };
        let pt = builder.finish();

        let create_shared_object_effects = execute_programmable_transaction(
            &self.setup_authority_state,
            &self.gas_object_id,
            &self.sender,
            &self.sender_key,
            pt,
            TEST_ONLY_GAS_UNIT,
        )
        .await
        .unwrap();
        assert!(
            create_shared_object_effects.status().is_success(),
            "Execution error {:?}",
            create_shared_object_effects.status()
        );
        assert_eq!(create_shared_object_effects.created().len(), 1);
        *create_shared_object_effects.created()[0].reference()
    }

    // Creates a owned object in `setup_authority_state` and returns the object
    // reference.
    async fn create_owned_object(&self) -> ObjectReference {
        let mut builder = ProgrammableTransactionBuilder::new();
        move_call! {
            builder,
            (self.package.object_id)::congestion_control::create_owned()
        };
        let pt = builder.finish();

        let create_owned_object_effects = execute_programmable_transaction(
            &self.setup_authority_state,
            &self.gas_object_id,
            &self.sender,
            &self.sender_key,
            pt,
            TEST_ONLY_GAS_UNIT,
        )
        .await
        .unwrap();
        assert!(
            create_owned_object_effects.status().is_success(),
            "Execution error {:?}",
            create_owned_object_effects.status()
        );
        assert_eq!(create_owned_object_effects.created().len(), 1);
        *create_owned_object_effects.created()[0].reference()
    }

    // Converts an object to a genesis object by setting its previous_transaction to
    // a genesis marker.
    fn convert_to_genesis_obj(obj: Object) -> Object {
        let mut genesis_obj = obj;
        genesis_obj.previous_transaction = TransactionDigest::GENESIS_MARKER;
        genesis_obj
    }

    // Returns a list of objects that can be used as genesis object for a brand new
    // authority state, including the gas object, the package object, and the
    // objects passed in `objects`.
    async fn create_genesis_objects_for_new_authority_state(
        &self,
        objects: &[ObjectId],
    ) -> Vec<Object> {
        let mut genesis_objects = Vec::new();
        genesis_objects.push(TestSetup::convert_to_genesis_obj(
            self.setup_authority_state
                .get_object(&self.package.object_id)
                .unwrap(),
        ));
        genesis_objects.push(TestSetup::convert_to_genesis_obj(
            self.setup_authority_state
                .get_object(&self.gas_object_id)
                .unwrap(),
        ));

        for obj in objects {
            genesis_objects.push(TestSetup::convert_to_genesis_obj(
                self.setup_authority_state.get_object(obj).unwrap(),
            ));
        }
        genesis_objects
    }
}

// Creates a transaction that touches the shared objects provided and the owned
// object provided.
async fn build_test_transaction(
    authority_state: &AuthorityState,
    package: &ObjectReference,
    sender: &Address,
    sender_key: &AccountPrivateKey,
    gas_object_id: &ObjectId,
    shared_objects: &[(ObjectId, Version)],
    owned_object: &ObjectReference,
    gas_units: u64,
) -> TransactionEnvelope {
    let mut txn_builder = ProgrammableTransactionBuilder::new();
    let mut args = vec![];
    for shared_object in shared_objects {
        args.push(
            txn_builder
                .obj(CallArg::Shared(SharedObjectReference::new(
                    shared_object.0,
                    shared_object.1,
                    true,
                )))
                .unwrap(),
        )
    }
    args.push(
        txn_builder
            .obj(CallArg::ImmutableOrOwned(*owned_object))
            .unwrap(),
    );
    match args.len() {
        1 => {
            move_call! {
                txn_builder,
                (package.object_id)::congestion_control::increment_one(args.pop().unwrap())
            };
        }
        2 => {
            move_call! {
                txn_builder,
                (package.object_id)::congestion_control::increment_two(args.pop().unwrap(), args.pop().unwrap())
            };
        }
        3 => {
            move_call! {
                txn_builder,
                (package.object_id)::congestion_control::increment_three(args.pop().unwrap(), args.pop().unwrap(), args.pop().unwrap())
            };
        }
        _ => panic!("Unsupported number of shared objects. Maximum supported is 2."),
    }
    let pt = txn_builder.finish();
    build_programmable_transaction(
        authority_state,
        gas_object_id,
        sender,
        sender_key,
        pt,
        gas_units,
    )
    .await
    .unwrap()
}

// Creates a transaction that touches the shared objects provided and the owned
// object provided. The transaction is passed through a fake consensus and then
// the congestion control before being executed.
async fn commit_and_execute_transaction(
    authority_state: &AuthorityState,
    package: &ObjectReference,
    sender: &Address,
    sender_key: &AccountPrivateKey,
    gas_object_id: &ObjectId,
    shared_objects: &[(ObjectId, Version)],
    owned_object: &ObjectReference,
    gas_units: u64,
) -> (TransactionEnvelope, TransactionEffects) {
    let transaction = build_test_transaction(
        authority_state,
        package,
        sender,
        sender_key,
        gas_object_id,
        shared_objects,
        owned_object,
        gas_units,
    )
    .await;

    let execution_effects =
        send_and_confirm_transaction_(authority_state, None, transaction.clone(), true)
            .await
            .unwrap()
            .1
            .into_data();
    (transaction, execution_effects)
}

// Tests that a transaction exceeding the deferral limit due to shared object
// congestion is cancelled:
//   1. Cancelled transaction should return correct error status.
//   2. Executing cancelled transaction with effects should result in the same transaction
//      cancellation.
//
// Run against both schedulers: a congestion-cancelled transaction is only
// "ready" because the availability check treats its cancelled sentinel input
// version as available; a regression there would strand cancelled transactions
// in the scheduler instead of executing them to cancelled effects.
#[sim_test]
async fn test_congestion_control_execution_cancellation_transaction_manager() {
    congestion_control_execution_cancellation(false).await;
}

#[sim_test]
async fn test_congestion_control_execution_cancellation_execution_scheduler() {
    congestion_control_execution_cancellation(true).await;
}

async fn congestion_control_execution_cancellation(use_execution_scheduler: bool) {
    telemetry_subscribers::init_for_testing();
    // Select the scheduler before the authorities are built (the env vars are
    // read by ExecutionSchedulerWrapper::new).
    set_scheduler_env(use_execution_scheduler);

    // Creates a test setup with a protocol config such that the the congestion
    // limit is equal to one default transaction's gas budget, and the overshoot
    // allowed is also equal to one default transaction's gas budget.
    let default_tx_gas_budget = TEST_ONLY_GAS_UNIT * TEST_ONLY_GAS_PRICE;
    let test_setup = TestSetup::new(default_tx_gas_budget, default_tx_gas_budget).await;

    // Creates 2 shared objects and 1 owned object.
    let shared_object_1 = test_setup.create_shared_object().await;
    let shared_object_2 = test_setup.create_shared_object().await;
    let owned_object = test_setup.create_owned_object().await;

    // Gets objects that can be used as genesis objects for new authority states.
    let genesis_objects = test_setup
        .create_genesis_objects_for_new_authority_state(&[
            shared_object_1.object_id,
            shared_object_2.object_id,
            owned_object.object_id,
        ])
        .await;

    // Creates an authority state with the genesis objects for the actual test.
    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state.insert_genesis_objects(&genesis_objects);
    assert_eq!(
        authority_state.uses_execution_scheduler(),
        use_execution_scheduler
    );

    // The congestion limit, taking overshoot into account is
    // 2 * TEST_ONLY_GAS_PRICE * TEST_ONLY_GAS_UNIT. We set the initial debt to be
    // TEST_ONLY_GAS_PRICE * TEST_ONLY_GAS_UNIT + 1, so that the next transaction
    // touching shared_object_1 will be cancelled.
    let initial_debt = TEST_ONLY_GAS_PRICE * TEST_ONLY_GAS_UNIT + 1;

    let congestion_control_parameters = CongestionControlParameters::new_for_test(
        PerObjectCongestionControlMode::TotalGasBudget,
        test_setup
            .protocol_config
            .congestion_control_min_free_execution_slot(),
        test_setup
            .protocol_config
            .max_accumulated_txn_cost_per_object_in_mysticeti_commit_as_option(),
        test_setup
            .protocol_config
            .max_congestion_limit_overshoot_per_commit_as_option(),
        test_setup.protocol_config.max_gas_price(),
        test_setup
            .protocol_config
            .congestion_limit_overshoot_in_gas_price_feedback_mechanism(),
        test_setup
            .protocol_config
            .separate_gas_price_feedback_mechanism_for_randomness(),
    );

    // Initialize shared object queue in the tracker and gas price calculator so
    // that any transaction touches shared_object_1 should result in congestion
    // and cancellation.
    let congestion_control_parameters_1 = congestion_control_parameters.clone();
    register_fail_point_arg("initial_congestion_tracker", move || {
        Some(new_congestion_tracker_with_initial_value_for_test(
            &[(shared_object_1.object_id, initial_debt)],
            congestion_control_parameters_1.clone(),
        ))
    });
    let congestion_control_parameters_2 = congestion_control_parameters.clone();
    register_fail_point_arg("initial_suggested_gas_price_calculator", move || {
        Some(
            new_suggested_gas_price_calculator_with_initial_values_for_test(
                &[(shared_object_1.object_id, initial_debt, TEST_ONLY_GAS_PRICE)],
                congestion_control_parameters_2.clone(),
                TEST_ONLY_GAS_PRICE,
            ),
        )
    });

    let suggested_gas_price = TEST_ONLY_GAS_PRICE + 1;

    // Creates a second authority state with the same genesis objects to test
    // executing the cancelled transaction from its effects.
    let authority_state_2 = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state_2.insert_genesis_objects(&genesis_objects);

    // Runs a transaction that touches shared_object_1, shared_object_2 and a owned
    // object.
    let (congested_tx, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[
            (shared_object_1.object_id, shared_object_1.version),
            (shared_object_2.object_id, shared_object_2.version),
        ],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        TEST_ONLY_GAS_UNIT,
    )
    .await;

    // Transaction should be cancelled with `shared_object_1` and `shared_object_2`
    // as the congested objects, and the suggested gas price should be
    // `TEST_ONLY_GAS_PRICE`.
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
                congested_objects: vec![shared_object_1.object_id, shared_object_2.object_id],
                suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![
            InputSharedObject::Canceled(ObjectVersion::new(
                shared_object_1.object_id,
                Version::new_congested_with_suggested_gas_price(suggested_gas_price).unwrap()
            )),
            InputSharedObject::Canceled(ObjectVersion::new(
                shared_object_2.object_id,
                Version::new_congested_with_suggested_gas_price(suggested_gas_price).unwrap()
            ))
        ]
    );

    // Run the same transaction in `authority_state_2`, but using the above effects
    // for the execution.
    let (cert, _) = certify_shared_obj_transaction_no_execution(&authority_state_2, congested_tx)
        .await
        .unwrap();
    let assigned_versions = authority_state_2
        .epoch_store_for_testing()
        .acquire_shared_version_assignments_from_effects(
            &VerifiedExecutableTransaction::new_from_certificate(cert.clone()),
            &effects,
            authority_state_2.get_object_cache_reader().as_ref(),
        )
        .unwrap();
    let (effects_2, execution_error) = authority_state_2.execute_for_test(
        &cert,
        ExecutionEnv::new().with_assigned_versions(assigned_versions),
    );

    // Should result in the same cancellation.
    assert_eq!(
        execution_error.unwrap().to_execution_status().0,
        ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
            congested_objects: vec![shared_object_1.object_id, shared_object_2.object_id],
            suggested_gas_price,
        }
    );
    assert_eq!(&effects, effects_2.data())
}

// Tests that congestion control and debt tracking work as expected when there
// is a burst of traffic and overshoot is allowed.
#[sim_test]
async fn test_congestion_control_debt_tracking() {
    telemetry_subscribers::init_for_testing();

    // Creates a test setup with a protocol config such that the the congestion
    // limit is equal to one default transaction's gas budget, and the overshoot
    // allowed is twice the default transaction's gas budget.
    let default_tx_gas_budget = TEST_ONLY_GAS_UNIT * TEST_ONLY_GAS_PRICE;
    let test_setup = TestSetup::new(default_tx_gas_budget, 2 * default_tx_gas_budget).await;

    // Creates 2 shared objects and 1 owned object.
    let shared_object_1 = test_setup.create_shared_object().await;
    let shared_object_2 = test_setup.create_shared_object().await;
    let owned_object = test_setup.create_owned_object().await;

    // Gets objects that can be used as genesis objects for new authority states.
    let genesis_objects = test_setup
        .create_genesis_objects_for_new_authority_state(&[
            shared_object_1.object_id,
            shared_object_2.object_id,
            owned_object.object_id,
        ])
        .await;

    // Creates an authority state with the genesis objects.
    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state.insert_genesis_objects(&genesis_objects);

    // Commit 1: a transaction with gas budget 3*default_tx_gas_budget that touches
    // shared_object_1 and an owned object.
    // This will result in an overshoot of 2*default_tx_gas_budget, but should be
    // executed successfully.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[(shared_object_1.object_id, shared_object_1.version)],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;

    // Transaction should be a success as overshoot of 2*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_success());

    // Check that the debt stored in consensus quarantine is correct.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    // Shared object 1 should have a debt of 2*default_tx_gas_budget.
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, 2 * default_tx_gas_budget);
        assert_eq!(commit_round, 1);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    // Check that shared object 2 has no debt.
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    assert!(shared_object_2_debt.is_none());

    // Commit 2: a transaction with gas budget 0.5*default_tx_gas_budget that
    // touches shared_object_1, shared_object_2 and an owned object.
    // Due to the debt of 2*default_tx_gas_budget from Commit 1, this will result in
    // a total overshoot of 1.5*default_tx_gas_budget (overshoot of
    // default_gas_budget from existing debt, and an extra 0.5*default_gas_budget
    // from this tx), and should be executed successfully.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[
            (shared_object_1.object_id, shared_object_1.version),
            (shared_object_2.object_id, shared_object_2.version),
        ],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        TEST_ONLY_GAS_UNIT / 2,
    )
    .await;

    // Transaction should be a success as overshoot of 1.5*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_success());
    // Check that the debt stored in consensus quarantine is correct. Both shared
    // objects should have a debt of 1.5*default_tx_gas_budget.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, 3 * default_tx_gas_budget / 2);
        assert_eq!(commit_round, 2);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, 3 * default_tx_gas_budget / 2);
        assert_eq!(commit_round, 2);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }

    // Commit 3: a transaction with gas budget 2*default_tx_gas_budget that
    // touches shared_object_2 and an owned object.
    // Due to the debt of 1.5*default_tx_gas_budget for shared_object_2 from Commit
    // 2, this should result in an overshoot of 2.5*default_tx_gas_budget on
    // shared_object_2 (initial debt [1.5*default_gas_budget]
    // + transaction [2*default_gas_budget] - congestion limit
    // [default_gas_budget]) which exceeds the allowed
    // overshoot, and should be cancelled.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[(shared_object_2.object_id, shared_object_2.version)],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        2 * TEST_ONLY_GAS_UNIT,
    )
    .await;

    // The expected suggested gas price should be the reference gas price because
    // there is no transaction responsible for the debt, only the overshoot from
    // previous commits, and their gas price is irrelevant.
    let expected_suggested_gas_price = TEST_ONLY_GAS_PRICE;

    // Transaction should be cancelled with `shared_object_2`
    // as the congested objects, and the suggested gas price should be
    // `TEST_ONLY_GAS_PRICE`.
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
                congested_objects: vec![shared_object_2.object_id],
                suggested_gas_price: expected_suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![InputSharedObject::Canceled(ObjectVersion::new(
            shared_object_2.object_id,
            Version::new_congested_with_suggested_gas_price(expected_suggested_gas_price).unwrap()
        )),]
    );

    // Check that the debt stored in consensus quarantine is correct. Shared object
    // 1 should still have a stored debt of 1.5*default_tx_gas_budget from
    // commit 2 that has carried over because because it was not updated in commit 3
    // as there was not transaction touching it. Shared object 2 should have a
    // debt of 0.5*default_tx_gas_budget from commit 3 because it was updated in
    // the consensus quarantine even though the execution was cancelled.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, 3 * default_tx_gas_budget / 2);
        assert_eq!(commit_round, 2);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, default_tx_gas_budget / 2);
        assert_eq!(commit_round, 3);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }

    // Commit 4: a transaction with gas budget 2.5*default_tx_gas_budget that
    // touches shared_object_1 and an owned object.
    // The debt of 1.5*default_gas_budget on shared object 1 from commit 2 should be
    // reduced to 0.5*default_gas_budget for commit round 4 because round 3 was
    // skipped, reducing it by the congestion limit of default_gas_budget.
    // Therefore, this transaction should be executed successfully as the total
    // overshoot will be 2*default_gas_budget (initial debt [0.5*default_gas_budget]
    // + transaction [2.5*default_gas_budget] - congestion limit
    // [default_gas_budget]).
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[(shared_object_1.object_id, shared_object_1.version)],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        5 * TEST_ONLY_GAS_UNIT / 2,
    )
    .await;

    // Transaction should be executed successfully as overshoot of
    // 2*default_tx_gas_budget is allowed.
    assert!(effects.status().is_success());

    // Check that the debt stored in consensus quarantine is correct. Shared object
    // 1 should now have a debt of 2*default_tx_gas_budget from commit 4 and shared
    // object 2 should still have a stored debt of 0.5*default_tx_gas_budget from
    // commit 3. This debt is effectively worth nothing in commit 5 because it will
    // be reduced by default_tx_gas_budget due to the skipped round.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, 2 * default_tx_gas_budget);
        assert_eq!(commit_round, 4);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, default_tx_gas_budget / 2);
        assert_eq!(commit_round, 3);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }

    // Commit 5: a transaction with gas budget of 1.5*default_tx_gas_budget that
    // touches both shared objects and an owned object. The transaction should be
    // cancelled because there is an initial debt of 2*default_tx_gas_budget on
    // shared object 1, resulting in a total overshoot of
    // 2.5*default_tx_gas_budget.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[
            (shared_object_1.object_id, shared_object_1.version),
            (shared_object_2.object_id, shared_object_2.version),
        ],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        3 * TEST_ONLY_GAS_UNIT / 2,
    )
    .await;

    // The expected suggested gas price should be the reference gas price because
    // there is no transaction responsible for the debt, only the overshoot from
    // previous commits, and their gas price is irrelevant.
    let expected_suggested_gas_price = TEST_ONLY_GAS_PRICE;

    // Transaction should be cancelled with both shared objects as the congested
    // objects, and the suggested gas price should be `TEST_ONLY_GAS_PRICE`.
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
                congested_objects: vec![shared_object_1.object_id, shared_object_2.object_id],
                suggested_gas_price: expected_suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![
            InputSharedObject::Canceled(ObjectVersion::new(
                shared_object_1.object_id,
                Version::new_congested_with_suggested_gas_price(expected_suggested_gas_price)
                    .unwrap()
            )),
            InputSharedObject::Canceled(ObjectVersion::new(
                shared_object_2.object_id,
                Version::new_congested_with_suggested_gas_price(expected_suggested_gas_price)
                    .unwrap()
            ))
        ]
    );

    // Check that the debt stored in consensus quarantine is correct. Shared object
    // 1 should now have debt reduced from 2*default_tx_gas_budget to
    // default_tx_gas_budget. The debt of shared object 1 should be updated in
    // consensus quarantine because there is a positive debt remaining which
    // triggers an update. Shared object 2 still has no debt, so no update is made
    // to consensus quarantine. We should still see the debt of
    // 0.5*default_tx_gas_budget from commit 3.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, default_tx_gas_budget);
        assert_eq!(commit_round, 5);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, default_tx_gas_budget / 2);
        assert_eq!(commit_round, 3);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }

    // Commit 6: a transaction with gas budget 3*default_tx_gas_budget that touches
    // only an owned object. The shared object debt from commit 5 should not have
    // any impact so this transaction should be executed successfully.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;
    // Transaction should be a success as there is no shared object involved.
    assert!(effects.status().is_success());

    // The debt on shared object 1 should still be stored as default_tx_gas_budget
    // from commit 5 as it was not updated in commit 6. The debt on shared
    // object 2 should still be stored as 0.5*default_tx_gas_budget from commit 3.
    // Both of these debts are effectively worth nothing in commit 6 because they
    // will be reduced by default_tx_gas_budget for each skipped round.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, default_tx_gas_budget);
        assert_eq!(commit_round, 5);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, default_tx_gas_budget / 2);
        assert_eq!(commit_round, 3);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }

    // Commit 7: The effective debt on both shared objects is none, so a transaction
    // with gas budget of 3*default_tx_gas_budget that touches both of them
    // and an owned object should be executed successfully.
    let (_, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[
            (shared_object_1.object_id, shared_object_1.version),
            (shared_object_2.object_id, shared_object_2.version),
        ],
        &authority_state
            .get_object(&owned_object.object_id)
            .unwrap()
            .object_ref(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;
    // Transaction should be a success as overshoot of 2*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_success());

    // The debt on both shared objects should should have been updated in storage to
    // 2*default_tx_gas_budget.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_1_debt {
        assert_eq!(debt, 2 * default_tx_gas_budget);
        assert_eq!(commit_round, 7);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
    let shared_object_2_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_2.object_id])
        .expect("Failed to load initial object debts for testing.")
        .pop()
        .unwrap();
    if let Some(CongestionPerObjectDebt::V1(commit_round, debt)) = shared_object_2_debt {
        assert_eq!(debt, 2 * default_tx_gas_budget);
        assert_eq!(commit_round, 7);
    } else {
        panic!("Unexpected debt stored in consensus quarantine.");
    }
}

// A randomness-using transaction scheduled through the combined congestion
// tracker, competing with a regular transaction for the single execution
// worker. `process_consensus_transactions` is driven directly with
// `randomness_round: Some(..)` (the commit boundary's DKG gate cannot be
// passed in a unit test), and with the combined list pre-ordered by gas price
// the way `PostConsensusTxReorder` orders it in production. Covers both
// directions of the competition and the split of scheduled transactions and
// checkpoint roots by randomness use.
#[tokio::test]
async fn test_combined_tracker_schedules_randomness_with_regular_transactions() {
    telemetry_subscribers::init_for_testing();

    // One execution worker and a per-commit budget of one transaction (each
    // costs one unit under TotalTxCount), so exactly one transaction per
    // commit is scheduled and the other is deferred.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        // The combined tracker schedules randomness-using transactions with the
        // rest, so the separate randomness gas price mechanism is turned off
        // alongside it.
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(10);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_1_id = ObjectId::random();
    let object_2_id = ObjectId::random();
    let gas_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    let mut genesis = vec![(sender, object_1_id), (sender, object_2_id)];
    genesis.extend(gas_ids.iter().map(|id| (sender, *id)));
    let authority = init_state_with_ids(genesis).await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let random_version =
        get_randomness_state_obj_initial_shared_version(authority.get_object_store()).unwrap();

    // A randomness-using transaction: takes the singleton Randomness object at
    // `0x8` by immutable reference. Never executed here (only scheduled), so a
    // placeholder move call is enough to declare the input.
    let make_randomness_tx = |gas_id: &ObjectId, gas_price: u64| {
        let gas = authority.get_object(gas_id).unwrap();
        let data = Transaction::new_move_call(
            sender,
            ObjectId::FRAMEWORK,
            Identifier::from_static("object_basics"),
            Identifier::from_static("set_value"),
            vec![],
            gas.object_ref(),
            vec![
                CallArg::Shared(SharedObjectReference::new(
                    ObjectId::RANDOMNESS_STATE,
                    random_version,
                    false,
                )),
                CallArg::Pure(16u64.to_le_bytes().to_vec()),
            ],
            gas_price * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
            gas_price,
        )
        .unwrap();
        epoch_store
            .verify_transaction(to_sender_signed_transaction(data, &sender_key))
            .unwrap()
    };
    let make_regular_tx = |object_id: &ObjectId, gas_id: &ObjectId, gas_price: u64| {
        let object = authority.get_object(object_id).unwrap();
        let gas = authority.get_object(gas_id).unwrap();
        init_transfer_transaction(
            &authority,
            sender,
            &sender_key,
            dbg_addr(2),
            object.object_ref(),
            gas.object_ref(),
            gas_price * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            gas_price,
        )
    };
    let seq = |tx: &VerifiedTransaction| {
        VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
                tx.clone().into(),
                None,
            ))),
            tracking_id: Default::default(),
        })
    };

    // Runs one commit's scheduling over the combined (gas-price-ordered) list
    // and returns the scheduled digests split by randomness use plus the
    // final checkpoint root sets.
    let schedule = |combined: Vec<VerifiedSequencedConsensusTransaction>, round: u64| {
        let epoch_store = epoch_store.clone();
        let authority = &authority;
        async move {
            let mut output = ConsensusCommitOutput::new(round, round);
            // Roots are prefilled per transaction by randomness use, as the
            // commit boundary does; scheduling filters out deferred ones.
            let mut non_randomness_roots = BTreeSet::new();
            let mut randomness_roots = BTreeSet::new();
            for tx in &combined {
                let digest = tx.0.transaction.executable_transaction_digest().unwrap();
                if tx.0.is_user_tx_with_randomness() {
                    randomness_roots.insert(TransactionKey::Digest(digest));
                } else {
                    non_randomness_roots.insert(TransactionKey::Digest(digest));
                }
            }
            let protocol_config = epoch_store.protocol_config();
            let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
                PerObjectCongestionControlMode::TotalTxCount,
                protocol_config.congestion_control_min_free_execution_slot(),
                protocol_config.max_accumulated_txn_cost_per_object_in_mysticeti_commit_as_option(),
                protocol_config.max_congestion_limit_overshoot_per_commit_as_option(),
                protocol_config.max_gas_price(),
                protocol_config.congestion_limit_overshoot_in_gas_price_feedback_mechanism(),
                protocol_config.separate_gas_price_feedback_mechanism_for_randomness(),
            );
            congestion_control_parameters.set_max_concurrent_execution_workers_for_test(1);
            let tracker = new_congestion_tracker_with_initial_value_for_test(
                &[],
                congestion_control_parameters,
            );
            let (
                non_randomness,
                randomness,
                _notifications,
                _reconfig,
                _final_round,
                _root,
                assigned,
            ) = epoch_store
                .process_consensus_transactions(
                    &mut output,
                    &combined,
                    &[],
                    &[],
                    &Arc::new(CheckpointServiceNoop {}),
                    authority.get_object_cache_reader().as_ref(),
                    &ConsensusCommitInfo::new_for_test(round, round, 0, true),
                    &mut non_randomness_roots,
                    &mut randomness_roots,
                    PreviouslyDeferredTransactions::default(),
                    None,
                    false,
                    Some(RandomnessRound::new(0)),
                    &authority.metrics,
                    tracker,
                    // Combined mode: no separate randomness tracker.
                    None,
                )
                .await
                .unwrap();
            // The update is assigned under its round key, which is also the key
            // the scheduler looks its env up by.
            assert!(
                assigned
                    .into_map()
                    .contains_key(&TransactionKey::RandomnessRound(
                        epoch_store.epoch(),
                        RandomnessRound::new(0),
                    ))
            );
            // The randomness queue starts with the round's state update, the
            // one schedulable that is not a transaction yet; the regular queue
            // must not carry it.
            assert!(matches!(
                randomness.first(),
                Some(Schedulable::RandomnessStateUpdate(..))
            ));
            (
                non_randomness
                    .iter()
                    .map(|schedulable| {
                        *schedulable
                            .as_tx()
                            .expect("only transactions are scheduled outside the randomness queue")
                            .digest()
                    })
                    .collect::<Vec<_>>(),
                randomness
                    .iter()
                    .filter_map(|schedulable| schedulable.as_tx())
                    .map(|tx| *tx.digest())
                    .collect::<Vec<_>>(),
                non_randomness_roots,
                randomness_roots,
            )
        }
    };

    // The higher-priced randomness transaction wins the single worker slot;
    // the regular transaction is deferred and filtered from the roots.
    let randomness_high = make_randomness_tx(&gas_ids[0], 2 * rgp);
    let regular_low = make_regular_tx(&object_1_id, &gas_ids[1], rgp);
    let (non_randomness, randomness, non_randomness_roots, randomness_roots) =
        schedule(vec![seq(&randomness_high), seq(&regular_low)], 1).await;
    assert_eq!(
        randomness,
        vec![*randomness_high.digest()],
        "the randomness transaction must be scheduled through the combined tracker"
    );
    assert!(
        non_randomness.is_empty(),
        "the regular transaction must be deferred by the shared worker budget"
    );
    assert_eq!(
        randomness_roots,
        BTreeSet::from([TransactionKey::Digest(*randomness_high.digest())])
    );
    assert!(non_randomness_roots.is_empty());

    // The other direction: the higher-priced regular transaction wins, and
    // the randomness transaction is deferred by congestion even though
    // randomness is available this round.
    let regular_high = make_regular_tx(&object_2_id, &gas_ids[2], 2 * rgp);
    let randomness_low = make_randomness_tx(&gas_ids[3], rgp);
    let (non_randomness, randomness, non_randomness_roots, randomness_roots) =
        schedule(vec![seq(&regular_high), seq(&randomness_low)], 2).await;
    assert_eq!(non_randomness, vec![*regular_high.digest()]);
    assert!(
        randomness.is_empty(),
        "the randomness transaction must be deferred by congestion, not scheduled"
    );
    assert_eq!(
        non_randomness_roots,
        BTreeSet::from([TransactionKey::Digest(*regular_high.digest())])
    );
    assert!(randomness_roots.is_empty());

    // Both losers were deferred (within the deferral limit), not cancelled.
    assert_eq!(
        authority
            .metrics
            .consensus_handler_deferred_transactions
            .get(),
        2
    );
    assert_eq!(
        authority
            .metrics
            .consensus_handler_cancelled_transactions
            .get(),
        0
    );
}

/// A transaction running past the commit's budget leaves a debt for its
/// dependents, and each root its commit in `root_commits`, only when the
/// epoch enables transaction dependencies.
#[tokio::test]
async fn test_transaction_dependency_bookkeeping_follows_the_protocol_flag() {
    telemetry_subscribers::init_for_testing();
    let default_tx_gas_budget = TEST_ONLY_GAS_UNIT * TEST_ONLY_GAS_PRICE;
    for enabled in [true, false] {
        let test_setup = TestSetup::new(default_tx_gas_budget, 2 * default_tx_gas_budget).await;
        let shared_object = test_setup.create_shared_object().await;
        let owned_object = test_setup.create_owned_object().await;
        let genesis_objects = test_setup
            .create_genesis_objects_for_new_authority_state(&[
                shared_object.object_id,
                owned_object.object_id,
            ])
            .await;
        let mut protocol_config = test_setup.protocol_config.clone();
        protocol_config.set_enable_transaction_dependencies_for_testing(enabled);
        let authority_state = TestAuthorityBuilder::new()
            .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
            .with_protocol_config(protocol_config)
            .build()
            .await;
        authority_state.insert_genesis_objects(&genesis_objects);

        // Ends at three times the budget, within the allowed overshoot.
        let (transaction, effects) = commit_and_execute_transaction(
            &authority_state,
            &test_setup.package,
            &test_setup.sender,
            &test_setup.sender_key,
            &test_setup.gas_object_id,
            &[(shared_object.object_id, shared_object.version)],
            &authority_state
                .get_object(&owned_object.object_id)
                .unwrap()
                .object_ref(),
            3 * TEST_ONLY_GAS_UNIT,
        )
        .await;
        assert!(effects.status().is_success());

        let epoch_store = authority_state.epoch_store_for_testing();
        let root_commit = epoch_store.root_commit_of(transaction.digest()).unwrap();
        let debt = epoch_store.load_stored_transaction_debts_for_testing(&[*transaction.digest()]);
        if enabled {
            assert!(root_commit.is_some());
            assert_eq!(debt, vec![Some((1, 2 * default_tx_gas_budget))]);
        } else {
            assert_eq!(root_commit, None);
            assert_eq!(debt, vec![None]);
        }
    }
}

// An owned-object-only transaction that cannot be scheduled within the
// execution-worker limit is deferred and, once past the deferral limit,
// cancelled: it is still executed, charged gas, and produces failure effects
// carrying a suggested gas price, so it reaches checkpoints and bumps its
// owned object versions like any other executed transaction. Re-executing it
// from those effects on a second authority (the checkpoint replay path) must
// reproduce identical effects.
#[sim_test]
async fn test_execution_worker_congestion_cancels_owned_object_only_tx() {
    telemetry_subscribers::init_for_testing();

    // One execution worker and a per-commit limit of one transaction, so only
    // one transaction can be scheduled per commit; no deferral allowance, so
    // the transaction that does not fit is cancelled immediately.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        // The combined tracker schedules randomness-using transactions with the
        // rest, so the separate randomness gas price mechanism is turned off
        // alongside it.
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_1_id = ObjectId::random();
    let object_2_id = ObjectId::random();
    let gas_1_id = ObjectId::random();
    let gas_2_id = ObjectId::random();
    let authority = init_state_with_ids(vec![
        (sender, object_1_id),
        (sender, object_2_id),
        (sender, gas_1_id),
        (sender, gas_2_id),
    ])
    .await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    // Snapshot the genesis objects for the replaying authority before
    // anything executes.
    let genesis_objects: Vec<Object> = [object_1_id, object_2_id, gas_1_id, gas_2_id]
        .iter()
        .map(|id| authority.get_object(id).unwrap())
        .collect();

    // Two owned-object-only transfer transactions with no object overlap.
    let mut transactions = Vec::new();
    for (object_id, gas_id) in [(object_1_id, gas_1_id), (object_2_id, gas_2_id)] {
        let object = authority.get_object(&object_id).unwrap();
        let gas = authority.get_object(&gas_id).unwrap();
        transactions.push(init_transfer_transaction(
            &authority,
            sender,
            &sender_key,
            dbg_addr(2),
            object.object_ref(),
            gas.object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        ));
    }

    let sequenced_transactions = transactions
        .iter()
        .map(|tx| {
            SequencedConsensusTransaction::new_test(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(Box::new(
                    AttestedTransaction::new(tx.clone().into(), None),
                )),
                tracking_id: Default::default(),
            })
        })
        .collect();

    let checkpoint_service = Arc::new(CheckpointServiceNoop {});
    let (executable_transactions, assigned_versions) = epoch_store
        .process_consensus_transactions_for_tests(
            sequenced_transactions,
            &checkpoint_service,
            authority.get_object_cache_reader().as_ref(),
            &authority.metrics,
            true,
            authority.as_ref(),
        )
        .await
        .unwrap();
    let assigned_versions = assigned_versions.into_map();

    // Both transactions are scheduled for execution: one normally, the other
    // in cancelled mode.
    assert_eq!(executable_transactions.len(), 2);

    let mut cancelled = Vec::new();
    for schedulable in &executable_transactions {
        let tx = schedulable
            .as_tx()
            .expect("the commit schedules only transactions here");
        let env = ExecutionEnv::new().with_assigned_versions(
            assigned_versions
                .get(&tx.key())
                .cloned()
                .unwrap_or_default(),
        );
        let (effects, _) = authority
            .try_execute_immediately(tx, env, &epoch_store)
            .unwrap();
        let is_cancelled = match effects.status() {
            ExecutionStatus::Success => false,
            ExecutionStatus::Failure {
                error:
                    ExecutionError::ExecutionCanceledDueToExecutionWorkerCongestion {
                        suggested_gas_price,
                    },
                command: None,
            } => {
                // The execution workers are congested, not any particular
                // object, so no congested objects are reported. The suggested
                // gas price must beat the competing transaction's price.
                assert!(*suggested_gas_price > rgp);
                true
            }
            other => panic!("expected success or congestion cancellation, got {other:?}"),
        };
        if is_cancelled {
            cancelled.push((tx.clone(), effects));
        }
    }
    assert_eq!(
        cancelled.len(),
        1,
        "exactly one transaction fits the single execution worker"
    );
    let (cancelled_tx, effects) = cancelled.pop().unwrap();

    // The cancelled execution charged gas (the gas object was mutated), and
    // the cancellation left no shared object entries in the effects.
    let gas_object_id = cancelled_tx.transaction().gas()[0].object_id;
    assert!(
        effects
            .mutated()
            .iter()
            .any(|mutated| mutated.reference().object_id == gas_object_id)
    );
    assert!(effects.input_shared_objects().is_empty());

    // Re-execute from the effects on a second authority holding the same
    // genesis objects: the gas object cancellation is reconstructed from the
    // execution status and must reproduce identical effects.
    let authority_2 = init_state_with_objects(genesis_objects).await;
    let epoch_store_2 = authority_2.epoch_store_for_testing();
    let assigned_versions_2 = epoch_store_2
        .acquire_shared_version_assignments_from_effects(
            &cancelled_tx,
            &effects,
            authority_2.get_object_cache_reader().as_ref(),
        )
        .unwrap();
    // The second authority executes the transaction outside the consensus
    // handler, so it is assigned a commit first, as the handler would.
    epoch_store_2.assign_to_a_new_commit_for_testing(&[cancelled_tx.key()]);
    let (effects_2, _) = authority_2
        .try_execute_immediately(
            &cancelled_tx,
            ExecutionEnv::new().with_assigned_versions(assigned_versions_2),
            &epoch_store_2,
        )
        .unwrap();
    assert_eq!(effects, effects_2);
}

// A transaction with shared inputs deferred by execution-worker congestion
// (rather than congestion on its objects) reports no congested objects from
// the scheduler. Once past the deferral limit it is cancelled through the
// existing shared object cancellation path, with all of its shared inputs
// treated as congested.
#[sim_test]
async fn test_execution_worker_congestion_cancels_shared_object_tx() {
    telemetry_subscribers::init_for_testing();

    // The congestion limit fits exactly one default transaction per commit,
    // with a single execution worker and no overshoot: of two transactions
    // touching disjoint objects, one is scheduled and the other is deferred
    // by the worker limit alone and then cancelled (no deferral allowance).
    let default_tx_gas_budget = TEST_ONLY_GAS_UNIT * TEST_ONLY_GAS_PRICE;
    let mut test_setup = TestSetup::new(default_tx_gas_budget, 0).await;
    test_setup
        .protocol_config
        .set_max_concurrent_execution_workers_for_testing(1);
    // The combined tracker schedules randomness-using transactions with the
    // rest, so the separate randomness gas price mechanism is turned off
    // alongside it.
    test_setup
        .protocol_config
        .set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);

    let shared_object_1 = test_setup.create_shared_object().await;
    let shared_object_2 = test_setup.create_shared_object().await;
    let owned_object_1 = test_setup.create_owned_object().await;
    let owned_object_2 = test_setup.create_owned_object().await;

    // A second gas object so the two transactions have disjoint inputs.
    let gas_2_id = ObjectId::random();
    let gas_2 = Object::with_id_owner_for_testing(gas_2_id, test_setup.sender);
    test_setup
        .setup_authority_state
        .insert_genesis_object(gas_2);

    let genesis_objects = test_setup
        .create_genesis_objects_for_new_authority_state(&[
            shared_object_1.object_id,
            shared_object_2.object_id,
            owned_object_1.object_id,
            owned_object_2.object_id,
            gas_2_id,
        ])
        .await;

    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state.insert_genesis_objects(&genesis_objects);
    let epoch_store = authority_state.epoch_store_for_testing();

    // Each transaction touches its own shared object, owned object, and gas
    // object.
    let mut transactions = Vec::new();
    for (shared_object, owned_object_id, gas_id) in [
        (
            &shared_object_1,
            owned_object_1.object_id,
            test_setup.gas_object_id,
        ),
        (&shared_object_2, owned_object_2.object_id, gas_2_id),
    ] {
        transactions.push(
            build_test_transaction(
                &authority_state,
                &test_setup.package,
                &test_setup.sender,
                &test_setup.sender_key,
                &gas_id,
                &[(shared_object.object_id, shared_object.version)],
                &authority_state
                    .get_object(&owned_object_id)
                    .unwrap()
                    .object_ref(),
                TEST_ONLY_GAS_UNIT,
            )
            .await,
        );
    }

    let sequenced_transactions = transactions
        .iter()
        .map(|tx| {
            SequencedConsensusTransaction::new_test(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(Box::new(
                    AttestedTransaction::new(tx.clone(), None),
                )),
                tracking_id: Default::default(),
            })
        })
        .collect();

    let checkpoint_service = Arc::new(CheckpointServiceNoop {});
    let (executable_transactions, assigned_versions) = epoch_store
        .process_consensus_transactions_for_tests(
            sequenced_transactions,
            &checkpoint_service,
            authority_state.get_object_cache_reader().as_ref(),
            &authority_state.metrics,
            true,
            authority_state.as_ref(),
        )
        .await
        .unwrap();
    let assigned_versions = assigned_versions.into_map();
    assert_eq!(executable_transactions.len(), 2);

    let shared_input_of = |digest| {
        transactions
            .iter()
            .zip([&shared_object_1, &shared_object_2])
            .find(|(tx, _)| tx.digest() == digest)
            .map(|(_, shared_object)| shared_object.object_id)
            .unwrap()
    };

    let mut cancellations = 0;
    for schedulable in &executable_transactions {
        let tx = schedulable
            .as_tx()
            .expect("the commit schedules only transactions here");
        let env = ExecutionEnv::new().with_assigned_versions(
            assigned_versions
                .get(&tx.key())
                .cloned()
                .unwrap_or_default(),
        );
        let (effects, _) = authority_state
            .try_execute_immediately(tx, env, &epoch_store)
            .unwrap();
        let shared_input = shared_input_of(tx.digest());
        match effects.status() {
            ExecutionStatus::Success => {}
            ExecutionStatus::Failure {
                error:
                    ExecutionError::ExecutionCanceledDueToSharedObjectCongestionV2 {
                        congested_objects,
                        suggested_gas_price,
                    },
                command: None,
            } => {
                // The worker pool, not an object, was congested, so all of
                // the transaction's shared inputs are treated as congested.
                assert_eq!(congested_objects, &vec![shared_input]);
                assert!(*suggested_gas_price > 0);
                assert_eq!(
                    effects.input_shared_objects(),
                    vec![InputSharedObject::Canceled(ObjectVersion::new(
                        shared_input,
                        Version::new_congested_with_suggested_gas_price(*suggested_gas_price)
                            .unwrap()
                    ))]
                );
                cancellations += 1;
            }
            other => panic!("expected success or congestion cancellation, got {other:?}"),
        }
    }
    assert_eq!(
        cancellations, 1,
        "exactly one transaction fits the single execution worker"
    );
}

// A cancelled transaction without shared inputs that pays with multiple gas
// coins: the cancellation version is carried on the first gas coin, the
// cancelled execution still smashes the coins (charging the merged first coin
// and deleting the rest), and re-execution from the effects reproduces
// identical effects.
#[sim_test]
async fn test_execution_worker_congestion_cancels_tx_with_multiple_gas_coins() {
    telemetry_subscribers::init_for_testing();

    // Same setup as the owned-object-only cancellation test: one execution
    // worker, one transaction per commit, no deferral allowance.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(1);
        // The combined tracker schedules randomness-using transactions with the
        // rest, so the separate randomness gas price mechanism is turned off
        // alongside it.
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_ids = [ObjectId::random(), ObjectId::random()];
    let gas_ids = [
        [ObjectId::random(), ObjectId::random()],
        [ObjectId::random(), ObjectId::random()],
    ];
    let authority = init_state_with_ids(
        object_ids
            .iter()
            .chain(gas_ids.iter().flatten())
            .map(|id| (sender, *id))
            .collect::<Vec<_>>(),
    )
    .await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let genesis_objects: Vec<Object> = object_ids
        .iter()
        .chain(gas_ids.iter().flatten())
        .map(|id| authority.get_object(id).unwrap())
        .collect();

    // Two owned-object-only transfers, each paying with two gas coins.
    let mut transactions = Vec::new();
    for (object_id, tx_gas_ids) in object_ids.iter().zip(&gas_ids) {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .transfer_object(
                dbg_addr(2),
                authority.get_object(object_id).unwrap().object_ref(),
            )
            .unwrap();
        let data = Transaction::new_with_gas_coins(
            TransactionKind::new_programmable(builder.finish()),
            sender,
            tx_gas_ids
                .iter()
                .map(|id| authority.get_object(id).unwrap().object_ref())
                .collect(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        transactions.push(to_sender_signed_transaction(data, &sender_key));
    }

    let sequenced_transactions = transactions
        .iter()
        .map(|tx| {
            SequencedConsensusTransaction::new_test(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(Box::new(
                    AttestedTransaction::new(tx.clone(), None),
                )),
                tracking_id: Default::default(),
            })
        })
        .collect();

    let checkpoint_service = Arc::new(CheckpointServiceNoop {});
    let (executable_transactions, assigned_versions) = epoch_store
        .process_consensus_transactions_for_tests(
            sequenced_transactions,
            &checkpoint_service,
            authority.get_object_cache_reader().as_ref(),
            &authority.metrics,
            true,
            authority.as_ref(),
        )
        .await
        .unwrap();
    let assigned_versions = assigned_versions.into_map();
    assert_eq!(executable_transactions.len(), 2);

    let mut cancelled = Vec::new();
    for schedulable in &executable_transactions {
        let tx = schedulable
            .as_tx()
            .expect("the commit schedules only transactions here");
        let env = ExecutionEnv::new().with_assigned_versions(
            assigned_versions
                .get(&tx.key())
                .cloned()
                .unwrap_or_default(),
        );
        let (effects, _) = authority
            .try_execute_immediately(tx, env, &epoch_store)
            .unwrap();
        let is_cancelled = match effects.status() {
            ExecutionStatus::Success => false,
            ExecutionStatus::Failure {
                error:
                    ExecutionError::ExecutionCanceledDueToExecutionWorkerCongestion {
                        suggested_gas_price,
                    },
                command: None,
            } => {
                assert!(*suggested_gas_price > rgp);
                true
            }
            other => panic!("expected success or congestion cancellation, got {other:?}"),
        };
        if is_cancelled {
            cancelled.push((tx.clone(), effects));
        }
    }
    assert_eq!(
        cancelled.len(),
        1,
        "exactly one transaction fits the single execution worker"
    );
    let (cancelled_tx, effects) = cancelled.pop().unwrap();

    // Gas was smashed and charged despite the cancellation: the first gas
    // coin (the cancellation carrier) is mutated, the second is deleted.
    let gas_coins = cancelled_tx.transaction().gas();
    assert_eq!(gas_coins.len(), 2);
    assert!(
        effects
            .mutated()
            .iter()
            .any(|mutated| mutated.reference().object_id == gas_coins[0].object_id)
    );
    assert!(
        effects
            .deleted()
            .iter()
            .any(|obj_ref| obj_ref.object_id == gas_coins[1].object_id)
    );
    assert!(effects.input_shared_objects().is_empty());

    // Re-execution from the effects must reproduce identical effects.
    let authority_2 = init_state_with_objects(genesis_objects).await;
    let epoch_store_2 = authority_2.epoch_store_for_testing();
    let assigned_versions_2 = epoch_store_2
        .acquire_shared_version_assignments_from_effects(
            &cancelled_tx,
            &effects,
            authority_2.get_object_cache_reader().as_ref(),
        )
        .unwrap();
    // The second authority executes the transaction outside the consensus
    // handler, so it is assigned a commit first, as the handler would.
    epoch_store_2.assign_to_a_new_commit_for_testing(&[cancelled_tx.key()]);
    let (effects_2, _) = authority_2
        .try_execute_immediately(
            &cancelled_tx,
            ExecutionEnv::new().with_assigned_versions(assigned_versions_2),
            &epoch_store_2,
        )
        .unwrap();
    assert_eq!(effects, effects_2);
}

// A transaction is scheduled only once the transaction it depends on is: it
// is deferred while its dependency is deferred at the same commit, and the
// congestion tracker defers it again while its dependency's execution ends
// past the commit's budget. Two execution workers and a budget of one
// transaction per worker per commit make every step deterministic.
#[tokio::test]
async fn test_dependent_is_deferred_until_its_dependency_is_scheduled() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_concurrent_execution_workers_for_testing(2);
        config.set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(10);
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    let authority = init_state_with_ids(gas_ids.iter().map(|id| (sender, *id))).await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let transfer = |gas_id: &ObjectId, gas_price: u64, dependency: Option<TransactionDigest>| {
        let gas = authority.get_object(gas_id).unwrap();
        let mut builder = TestTransactionBuilder::new(sender, gas.object_ref(), gas_price)
            .transfer_iota(Some(1), dbg_addr(2));
        if let Some(dependency) = dependency {
            builder = builder.with_transaction_dependency(dependency, epoch_store.epoch());
        }
        epoch_store
            .verify_transaction(builder.build_and_sign(&sender_key))
            .unwrap()
    };
    let seq = |tx: &VerifiedTransaction| {
        VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
                tx.clone().into(),
                None,
            ))),
            tracking_id: Default::default(),
        })
    };

    // Runs one commit's scheduling and returns the scheduled digests.
    let schedule = |transactions: Vec<VerifiedSequencedConsensusTransaction>,
                    previously_deferred: PreviouslyDeferredTransactions,
                    round: u64| {
        let epoch_store = epoch_store.clone();
        let authority = &authority;
        async move {
            let mut output = ConsensusCommitOutput::new(round, round);
            let mut non_randomness_roots: BTreeSet<_> = transactions
                .iter()
                .map(|tx| {
                    TransactionKey::Digest(
                        tx.0.transaction.executable_transaction_digest().unwrap(),
                    )
                })
                .collect();
            let mut randomness_roots = BTreeSet::new();
            let protocol_config = epoch_store.protocol_config();
            let mut congestion_control_parameters = CongestionControlParameters::new_for_test(
                PerObjectCongestionControlMode::TotalTxCount,
                protocol_config.congestion_control_min_free_execution_slot(),
                protocol_config.max_accumulated_txn_cost_per_object_in_mysticeti_commit_as_option(),
                protocol_config.max_congestion_limit_overshoot_per_commit_as_option(),
                protocol_config.max_gas_price(),
                protocol_config.congestion_limit_overshoot_in_gas_price_feedback_mechanism(),
                protocol_config.separate_gas_price_feedback_mechanism_for_randomness(),
            );
            congestion_control_parameters.set_max_concurrent_execution_workers_for_test(2);
            let tracker = new_congestion_tracker_with_initial_value_for_test(
                &[],
                congestion_control_parameters,
            );
            let (
                non_randomness,
                _randomness,
                _notifications,
                _reconfig,
                _final_round,
                _root,
                _assigned,
            ) = epoch_store
                .process_consensus_transactions(
                    &mut output,
                    &transactions,
                    &[],
                    &[],
                    &Arc::new(CheckpointServiceNoop {}),
                    authority.get_object_cache_reader().as_ref(),
                    &ConsensusCommitInfo::new_for_test(round, round, 0, true),
                    &mut non_randomness_roots,
                    &mut randomness_roots,
                    previously_deferred,
                    None,
                    false,
                    None,
                    &authority.metrics,
                    tracker,
                    None,
                )
                .await
                .unwrap();
            non_randomness
                .iter()
                .map(|schedulable| *schedulable.as_tx().unwrap().digest())
                .collect::<Vec<_>>()
        }
    };
    let dependency_deferrals = || {
        authority
            .metrics
            .consensus_handler_dependency_deferred_transactions
            .get()
    };

    // Commit 1: the two highest-priced transactions take both workers, the
    // dependency is deferred by congestion, and its dependent is deferred
    // with it.
    let first = transfer(&gas_ids[0], 4 * rgp, None);
    let second = transfer(&gas_ids[1], 3 * rgp, None);
    let dependency = transfer(&gas_ids[2], 2 * rgp, None);
    let dependent = transfer(&gas_ids[3], rgp, Some(*dependency.digest()));
    let scheduled = schedule(
        vec![seq(&first), seq(&second), seq(&dependency), seq(&dependent)],
        PreviouslyDeferredTransactions::default(),
        1,
    )
    .await;
    assert_eq!(scheduled, vec![*first.digest(), *second.digest()]);
    assert_eq!(dependency_deferrals(), 1);

    // Commit 2: the dependency takes a worker. A worker is free for its
    // dependent, but the dependent could start only when the dependency
    // ends, past the budget, so it is deferred for the dependency again.
    let deferred_key = DeferralKey::new_for_consensus_round(2, 1);
    let previously_deferred: PreviouslyDeferredTransactions = [
        (*dependency.digest(), (deferred_key, None)),
        (*dependent.digest(), (deferred_key, None)),
    ]
    .into_iter()
    .collect();
    let scheduled = schedule(
        vec![seq(&dependency), seq(&dependent)],
        previously_deferred,
        2,
    )
    .await;
    assert_eq!(scheduled, vec![*dependency.digest()]);
    assert_eq!(dependency_deferrals(), 2);

    // Commit 3: the dependency ended within the previous commit's budget, so
    // the dependent is scheduled.
    let previously_deferred: PreviouslyDeferredTransactions = [(
        *dependent.digest(),
        (DeferralKey::new_for_consensus_round(3, 1), None),
    )]
    .into_iter()
    .collect();
    let scheduled = schedule(vec![seq(&dependent)], previously_deferred, 3).await;
    assert_eq!(scheduled, vec![*dependent.digest()]);
    assert_eq!(dependency_deferrals(), 2);
}

/// Sequencing of transactions on shared objects with transaction dependencies
/// enabled and per-object congestion control counting transactions, at most
/// `limit` per object and commit, and the lowest free slot taken across
/// resources, as on every chain. With `workers`, the combined tracker
/// schedules regular and randomness-using transactions from one list; without,
/// each phase has its own tracker.
struct SharedObjectSequencing {
    authority: Arc<AuthorityState>,
    sender: Address,
    sender_key: AccountPrivateKey,
    gas_ids: Vec<ObjectId>,
    workers: Option<u16>,
}

impl SharedObjectSequencing {
    async fn new(limit: u64, gas_objects: usize, workers: Option<u16>) -> Self {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let mut protocol_config =
            ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
        protocol_config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        protocol_config
            .set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(limit);
        protocol_config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        protocol_config.set_congestion_control_min_free_execution_slot_for_testing(true);
        protocol_config.set_max_deferral_rounds_for_congestion_control_for_testing(10);
        protocol_config.set_enable_transaction_dependencies_for_testing(true);
        if let Some(workers) = workers {
            protocol_config.set_max_concurrent_execution_workers_for_testing(workers);
            protocol_config
                .set_separate_gas_price_feedback_mechanism_for_randomness_for_testing(false);
        }
        let authority = TestAuthorityBuilder::new()
            .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
            .with_protocol_config(protocol_config)
            .build()
            .await;
        let gas_ids: Vec<ObjectId> = (0..gas_objects).map(|_| ObjectId::random()).collect();
        for gas_id in &gas_ids {
            authority.insert_genesis_object(Object::with_id_owner_for_testing(*gas_id, sender));
        }
        Self {
            authority,
            sender,
            sender_key,
            gas_ids,
            workers,
        }
    }

    fn shared_object(&self) -> Object {
        let object = Object::shared_for_testing();
        self.authority.insert_genesis_object(object.clone());
        object
    }

    /// A transaction mutating `shared_objects` at `gas_price`, paying with
    /// the gas object at `gas_index`, depending on `dependency` when given,
    /// and reading the randomness state when `uses_randomness`.
    fn transaction(
        &self,
        gas_index: usize,
        shared_objects: &[&Object],
        gas_price: u64,
        dependency: Option<TransactionDigest>,
        uses_randomness: bool,
    ) -> VerifiedTransaction {
        let mut builder = ProgrammableTransactionBuilder::new();
        let mut args: Vec<_> = shared_objects
            .iter()
            .map(|object| {
                builder
                    .obj(CallArg::Shared(SharedObjectReference::new(
                        object.id(),
                        object.version(),
                        true,
                    )))
                    .unwrap()
            })
            .collect();
        if uses_randomness {
            let randomness_version =
                get_randomness_state_obj_initial_shared_version(self.authority.get_object_store())
                    .unwrap();
            args.push(
                builder
                    .obj(CallArg::Shared(SharedObjectReference::new(
                        ObjectId::RANDOMNESS_STATE,
                        randomness_version,
                        false,
                    )))
                    .unwrap(),
            );
        }
        builder.programmable_move_call(
            ObjectId::random(),
            Identifier::from_static("congestion_control"),
            Identifier::from_static("increment"),
            vec![],
            args,
        );
        let gas = self.authority.get_object(&self.gas_ids[gas_index]).unwrap();
        let mut test_builder =
            TestTransactionBuilder::new(self.sender, gas.object_ref(), gas_price)
                .with_gas_budget(gas_price * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS)
                .programmable(builder.finish());
        if let Some(dependency) = dependency {
            let epoch = self.authority.epoch_store_for_testing().epoch();
            test_builder = test_builder.with_transaction_dependency(dependency, epoch);
        }
        self.authority
            .epoch_store_for_testing()
            .verify_transaction(test_builder.build_and_sign(&self.sender_key))
            .unwrap()
    }

    /// Sequences one commit, `regular` and `randomness` being the two lists
    /// in the order the ordering pass produces (with the combined tracker,
    /// everything is in `regular`), and returns the scheduled digests of both
    /// phases in scheduling order with the assigned shared-object versions.
    async fn sequence(
        &self,
        regular: Vec<VerifiedSequencedConsensusTransaction>,
        randomness: Vec<VerifiedSequencedConsensusTransaction>,
        previously_deferred: PreviouslyDeferredTransactions,
        round: u64,
        randomness_round: Option<RandomnessRound>,
    ) -> (Vec<TransactionDigest>, AssignedTxAndVersions) {
        let epoch_store = self.authority.epoch_store_for_testing();
        let mut output = ConsensusCommitOutput::new(round, round);
        let roots = |transactions: &[VerifiedSequencedConsensusTransaction]| {
            transactions
                .iter()
                .map(|tx| {
                    TransactionKey::Digest(
                        tx.0.transaction.executable_transaction_digest().unwrap(),
                    )
                })
                .collect::<BTreeSet<_>>()
        };
        let mut non_randomness_roots = roots(&regular);
        let mut randomness_roots = roots(&randomness);
        let protocol_config = epoch_store.protocol_config();
        let mut parameters = CongestionControlParameters::new_for_test(
            PerObjectCongestionControlMode::TotalTxCount,
            true,
            protocol_config.max_accumulated_txn_cost_per_object_in_mysticeti_commit_as_option(),
            protocol_config.max_congestion_limit_overshoot_per_commit_as_option(),
            protocol_config.max_gas_price(),
            protocol_config.congestion_limit_overshoot_in_gas_price_feedback_mechanism(),
            protocol_config.separate_gas_price_feedback_mechanism_for_randomness(),
        );
        if let Some(workers) = self.workers {
            parameters.set_max_concurrent_execution_workers_for_test(workers);
        }
        let tracker = new_congestion_tracker_with_initial_value_for_test(&[], parameters.clone());
        let randomness_tracker = self
            .workers
            .is_none()
            .then(|| new_congestion_tracker_with_initial_value_for_test(&[], parameters));
        let (
            scheduled,
            scheduled_randomness,
            _notifications,
            _reconfig,
            _final_round,
            _root,
            assigned,
        ) = epoch_store
            .process_consensus_transactions(
                &mut output,
                &regular,
                &randomness,
                &[],
                &Arc::new(CheckpointServiceNoop {}),
                self.authority.get_object_cache_reader().as_ref(),
                &ConsensusCommitInfo::new_for_test(round, round, 0, true),
                &mut non_randomness_roots,
                &mut randomness_roots,
                previously_deferred,
                None,
                false,
                randomness_round,
                &self.authority.metrics,
                tracker,
                randomness_tracker,
            )
            .await
            .unwrap();
        let scheduled = scheduled
            .iter()
            .chain(scheduled_randomness.iter())
            .filter_map(|schedulable| schedulable.as_tx().map(|tx| *tx.digest()))
            .collect();
        (scheduled, assigned)
    }

    fn dependency_deferrals(&self) -> u64 {
        self.authority
            .metrics
            .consensus_handler_dependency_deferred_transactions
            .get()
    }
}

fn sequenced(transaction: &VerifiedTransaction) -> VerifiedSequencedConsensusTransaction {
    VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
        kind: ConsensusTransactionKind::UserTransactionV1(Box::new(AttestedTransaction::new(
            transaction.clone().into(),
            None,
        ))),
        tracking_id: Default::default(),
    })
}

/// The version of `object` assigned to `transaction`, when it was scheduled.
fn assigned_version(
    assigned: &AssignedTxAndVersions,
    transaction: &VerifiedTransaction,
    object: ObjectId,
) -> Option<Version> {
    assigned
        .0
        .iter()
        .find(|(key, _)| *key == TransactionKey::Digest(*transaction.digest()))
        .and_then(|(_, versions)| {
            versions
                .iter()
                .find(|assignment| assignment.object_id == object)
                .map(|assignment| assignment.version)
        })
}

/// A dependency deferred by congestion on an object its dependent does not
/// touch takes the dependent with it, so the dependent is assigned no version
/// of their common object. At the next commit both are scheduled, the
/// dependent after the dependency's end and with the higher version.
#[tokio::test]
async fn test_dependent_is_deferred_with_a_dependency_deferred_for_another_object() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(2, 5, None).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let congested = sequencing.shared_object();
    let first = sequencing.transaction(0, &[&congested], 5 * rgp, None, false);
    let second = sequencing.transaction(1, &[&congested], 4 * rgp, None, false);
    let dependency = sequencing.transaction(2, &[&common, &congested], 3 * rgp, None, false);
    let dependent =
        sequencing.transaction(3, &[&common], 2 * rgp, Some(*dependency.digest()), false);
    let unrelated = sequencing.transaction(4, &[&common], rgp, None, false);

    // Commit 1: `first` and `second` take the commit's two slots on
    // `congested` while `common` stays free.
    let (scheduled, assigned) = sequencing
        .sequence(
            vec![
                sequenced(&first),
                sequenced(&second),
                sequenced(&dependency),
                sequenced(&dependent),
                sequenced(&unrelated),
            ],
            vec![],
            PreviouslyDeferredTransactions::default(),
            1,
            None,
        )
        .await;
    assert_eq!(
        scheduled,
        vec![*first.digest(), *unrelated.digest(), *second.digest()]
    );
    assert_eq!(assigned_version(&assigned, &dependent, common.id()), None);
    assert_eq!(sequencing.dependency_deferrals(), 1);

    // Commit 2: both are reloaded, the dependency first.
    let deferred_key = DeferralKey::new_for_consensus_round(2, 1);
    let previously_deferred = [
        (*dependency.digest(), (deferred_key, None)),
        (*dependent.digest(), (deferred_key, None)),
    ]
    .into_iter()
    .collect();
    let (scheduled, assigned) = sequencing
        .sequence(
            vec![sequenced(&dependency), sequenced(&dependent)],
            vec![],
            previously_deferred,
            2,
            None,
        )
        .await;
    assert_eq!(scheduled, vec![*dependency.digest(), *dependent.digest()]);
    assert!(
        assigned_version(&assigned, &dependency, common.id()).unwrap()
            < assigned_version(&assigned, &dependent, common.id()).unwrap()
    );
    assert_eq!(sequencing.dependency_deferrals(), 1);
}

/// With the lowest free slot taken across resources, a transaction processed
/// later can be placed in a gap before the slot of one processed earlier. A
/// dependent is never placed before its dependency's end, so its version of
/// their common object stays above the dependency's.
#[tokio::test]
async fn test_dependent_takes_no_free_gap_before_its_dependency() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(3, 4, None).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let busy = sequencing.shared_object();
    let first = sequencing.transaction(0, &[&busy], 4 * rgp, None, false);
    let dependency = sequencing.transaction(1, &[&common, &busy], 3 * rgp, None, false);
    let dependent =
        sequencing.transaction(2, &[&common], 2 * rgp, Some(*dependency.digest()), false);
    let unrelated = sequencing.transaction(3, &[&common], rgp, None, false);

    // `first` occupies `busy` until 1, so the dependency starts at 1 and
    // leaves a gap on `common` before its slot. `unrelated` takes the gap;
    // the dependent starts when the dependency ends, at 2.
    let (scheduled, assigned) = sequencing
        .sequence(
            vec![
                sequenced(&first),
                sequenced(&dependency),
                sequenced(&dependent),
                sequenced(&unrelated),
            ],
            vec![],
            PreviouslyDeferredTransactions::default(),
            1,
            None,
        )
        .await;
    assert_eq!(
        scheduled,
        vec![
            *first.digest(),
            *unrelated.digest(),
            *dependency.digest(),
            *dependent.digest()
        ]
    );
    let version = |transaction| assigned_version(&assigned, transaction, common.id()).unwrap();
    assert!(version(&unrelated) < version(&dependency));
    assert!(version(&dependency) < version(&dependent));
    assert_eq!(sequencing.dependency_deferrals(), 0);
}

/// A transaction on owned objects only, between two transactions on a common
/// object in a chain of dependencies, starts when its dependency ends, so the
/// last transaction takes no free gap on the common object before the first
/// one. Otherwise the last one would hold the lower version of the common
/// object while the first one, which it waits for, needs it.
async fn chain_through_an_owned_object_transaction_takes_no_free_gap(workers: Option<u16>) {
    let sequencing = SharedObjectSequencing::new(4, 4, workers).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let busy = sequencing.shared_object();
    let first = sequencing.transaction(0, &[&busy], 4 * rgp, None, false);
    let head = sequencing.transaction(1, &[&common, &busy], 3 * rgp, None, false);
    let owned = sequencing.transaction(2, &[], 2 * rgp, Some(*head.digest()), false);
    let tail = sequencing.transaction(3, &[&common], rgp, Some(*owned.digest()), false);

    // `first` occupies `busy` until 1, so `head` runs from 1 to 2 and leaves a
    // gap on `common` before its slot. `owned` runs from 2 to 3 and `tail`
    // from 3 to 4.
    let (scheduled, assigned) = sequencing
        .sequence(
            vec![
                sequenced(&first),
                sequenced(&head),
                sequenced(&owned),
                sequenced(&tail),
            ],
            vec![],
            PreviouslyDeferredTransactions::default(),
            1,
            None,
        )
        .await;
    assert_eq!(
        scheduled,
        vec![
            *first.digest(),
            *head.digest(),
            *owned.digest(),
            *tail.digest()
        ]
    );
    assert!(
        assigned_version(&assigned, &head, common.id()).unwrap()
            < assigned_version(&assigned, &tail, common.id()).unwrap()
    );
    assert_eq!(sequencing.dependency_deferrals(), 0);
}

#[tokio::test]
async fn test_chain_through_an_owned_object_transaction_takes_no_free_gap() {
    telemetry_subscribers::init_for_testing();
    chain_through_an_owned_object_transaction_takes_no_free_gap(None).await;
}

#[tokio::test]
async fn test_chain_through_an_owned_object_transaction_takes_no_free_gap_with_workers() {
    telemetry_subscribers::init_for_testing();
    chain_through_an_owned_object_transaction_takes_no_free_gap(Some(4)).await;
}

/// A chain of dependencies, each on its own object, advances one transaction
/// per commit when the budget per object is one transaction. Waiting for a
/// dependency is a deferral for congestion, so once the chain has waited past
/// the deferral limit, none of it stays deferred: each transaction deferred
/// for its dependency's end is cancelled, and a transaction depending on a
/// cancelled one is scheduled.
#[tokio::test]
async fn test_dependency_chain_waits_no_longer_than_the_deferral_limit() {
    telemetry_subscribers::init_for_testing();
    const LENGTH: usize = 15;
    let sequencing = SharedObjectSequencing::new(1, LENGTH, None).await;
    let max_deferral_rounds = sequencing
        .authority
        .epoch_store_for_testing()
        .protocol_config()
        .max_deferral_rounds_for_congestion_control();
    let rgp = TEST_ONLY_GAS_PRICE;
    let objects: Vec<Object> = (0..LENGTH).map(|_| sequencing.shared_object()).collect();
    let mut chain: Vec<VerifiedTransaction> = Vec::with_capacity(LENGTH);
    for (index, object) in objects.iter().enumerate() {
        let dependency = chain.last().map(|previous| *previous.digest());
        chain.push(sequencing.transaction(index, &[object], rgp, dependency, false));
    }
    // Every transaction left after the first commit was first deferred there.
    let deferred_since_round_1 =
        |round, remaining: &[VerifiedTransaction]| -> PreviouslyDeferredTransactions {
            remaining
                .iter()
                .map(|tx| {
                    (
                        *tx.digest(),
                        (DeferralKey::new_for_consensus_round(round, 1), None),
                    )
                })
                .collect()
        };

    let mut next = 0;
    for round in 1..=max_deferral_rounds {
        let previously_deferred = if round == 1 {
            PreviouslyDeferredTransactions::default()
        } else {
            deferred_since_round_1(round, &chain[next..])
        };
        let (scheduled, _) = sequencing
            .sequence(
                chain[next..].iter().map(sequenced).collect(),
                vec![],
                previously_deferred,
                round,
                None,
            )
            .await;
        assert_eq!(scheduled, vec![*chain[next].digest()]);
        next += 1;
    }

    let round = max_deferral_rounds + 1;
    let (scheduled, assigned) = sequencing
        .sequence(
            chain[next..].iter().map(sequenced).collect(),
            vec![],
            deferred_since_round_1(round, &chain[next..]),
            round,
            None,
        )
        .await;
    assert_eq!(
        scheduled.into_iter().collect::<BTreeSet<_>>(),
        chain[next..].iter().map(|tx| *tx.digest()).collect()
    );
    for index in next..LENGTH {
        let version = assigned_version(&assigned, &chain[index], objects[index].id()).unwrap();
        let cancelled = (index - next) % 2 == 1;
        assert_eq!(version.is_congested(), cancelled, "transaction {index}");
        if cancelled {
            assert_eq!(
                version.get_congested_version_suggested_gas_price().unwrap(),
                rgp
            );
        }
    }
}

/// A transaction waiting for a dependency that is still deferred is cancelled
/// once its own deferral would pass the limit, like any deferral for
/// congestion: through its shared inputs, or through its gas object when it
/// has none. The dependency, deferred until randomness is available, stays
/// deferred.
#[tokio::test]
async fn test_dependent_of_a_deferred_dependency_is_cancelled_past_the_deferral_limit() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(2, 3, None).await;
    let max_deferral_rounds = sequencing
        .authority
        .epoch_store_for_testing()
        .protocol_config()
        .max_deferral_rounds_for_congestion_control();
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let other = sequencing.shared_object();
    let dependency = sequencing.transaction(0, &[&common], rgp, None, true);
    let shared_dependent =
        sequencing.transaction(1, &[&other], rgp, Some(*dependency.digest()), false);
    let owned_dependent = sequencing.transaction(2, &[], rgp, Some(*dependency.digest()), false);

    // All three were first deferred at round 1.
    let round = max_deferral_rounds + 1;
    let previously_deferred = [
        (
            *dependency.digest(),
            (DeferralKey::new_for_randomness(1), None),
        ),
        (
            *shared_dependent.digest(),
            (DeferralKey::new_for_consensus_round(round, 1), None),
        ),
        (
            *owned_dependent.digest(),
            (DeferralKey::new_for_consensus_round(round, 1), None),
        ),
    ]
    .into_iter()
    .collect();
    let (scheduled, assigned) = sequencing
        .sequence(
            vec![sequenced(&shared_dependent), sequenced(&owned_dependent)],
            vec![sequenced(&dependency)],
            previously_deferred,
            round,
            None,
        )
        .await;

    assert_eq!(
        scheduled.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([*shared_dependent.digest(), *owned_dependent.digest()])
    );
    for (transaction, cancelled_object) in [
        (&shared_dependent, other.id()),
        (&owned_dependent, sequencing.gas_ids[2]),
    ] {
        let version = assigned_version(&assigned, transaction, cancelled_object).unwrap();
        assert!(version.is_congested(), "{version:?}");
        assert_eq!(
            version.get_congested_version_suggested_gas_price().unwrap(),
            rgp
        );
    }
}

/// With the combined tracker, a dependent outside the randomness phase is
/// deferred when its dependency is a root of that phase at the same commit:
/// the regular phase is assigned versions first, so the dependent would
/// otherwise hold the lower version of their common object while waiting
/// for the higher one. It is scheduled at the next commit.
#[tokio::test]
async fn test_regular_dependent_of_a_randomness_phase_root_is_deferred_with_the_combined_tracker() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(2, 2, Some(2)).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let dependency = sequencing.transaction(0, &[&common], 2 * rgp, None, true);
    let dependent = sequencing.transaction(1, &[&common], rgp, Some(*dependency.digest()), false);

    let (scheduled, assigned) = sequencing
        .sequence(
            vec![sequenced(&dependency), sequenced(&dependent)],
            vec![],
            PreviouslyDeferredTransactions::default(),
            1,
            Some(RandomnessRound::new(0)),
        )
        .await;
    assert_eq!(scheduled, vec![*dependency.digest()]);
    assert_eq!(assigned_version(&assigned, &dependent, common.id()), None);
    assert_eq!(sequencing.dependency_deferrals(), 1);

    let previously_deferred = [(
        *dependent.digest(),
        (DeferralKey::new_for_consensus_round(2, 1), None),
    )]
    .into_iter()
    .collect();
    let (scheduled, _) = sequencing
        .sequence(
            vec![sequenced(&dependent)],
            vec![],
            previously_deferred,
            2,
            None,
        )
        .await;
    assert_eq!(scheduled, vec![*dependent.digest()]);
    assert_eq!(sequencing.dependency_deferrals(), 1);
}

/// The other direction needs no deferral: a randomness-using dependent of a
/// regular root is scheduled at the same commit, into the randomness phase,
/// with the higher version of their common object.
#[tokio::test]
async fn test_randomness_dependent_of_a_regular_root_is_scheduled_with_the_combined_tracker() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(2, 2, Some(2)).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let dependency = sequencing.transaction(0, &[&common], 2 * rgp, None, false);
    let dependent = sequencing.transaction(1, &[&common], rgp, Some(*dependency.digest()), true);

    let (scheduled, assigned) = sequencing
        .sequence(
            vec![sequenced(&dependency), sequenced(&dependent)],
            vec![],
            PreviouslyDeferredTransactions::default(),
            1,
            Some(RandomnessRound::new(0)),
        )
        .await;
    assert_eq!(scheduled, vec![*dependency.digest(), *dependent.digest()]);
    assert!(
        assigned_version(&assigned, &dependency, common.id()).unwrap()
            < assigned_version(&assigned, &dependent, common.id()).unwrap()
    );
    assert_eq!(sequencing.dependency_deferrals(), 0);
}

/// With one tracker per phase, the regular list is processed first, so a
/// regular dependent of a randomness-using dependency at the same commit is
/// deferred before the dependency is a root.
#[tokio::test]
async fn test_regular_dependent_of_a_randomness_dependency_is_deferred_with_two_trackers() {
    telemetry_subscribers::init_for_testing();
    let sequencing = SharedObjectSequencing::new(2, 2, None).await;
    let rgp = TEST_ONLY_GAS_PRICE;
    let common = sequencing.shared_object();
    let dependency = sequencing.transaction(0, &[&common], 2 * rgp, None, true);
    let dependent = sequencing.transaction(1, &[&common], rgp, Some(*dependency.digest()), false);

    let (scheduled, assigned) = sequencing
        .sequence(
            vec![sequenced(&dependent)],
            vec![sequenced(&dependency)],
            PreviouslyDeferredTransactions::default(),
            1,
            Some(RandomnessRound::new(0)),
        )
        .await;
    assert_eq!(scheduled, vec![*dependency.digest()]);
    assert_eq!(assigned_version(&assigned, &dependent, common.id()), None);
    assert_eq!(sequencing.dependency_deferrals(), 1);
}
