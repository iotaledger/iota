// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_macros::{register_fail_point_arg, sim_test};
use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    effects::{InputSharedObject, TransactionEffects, TransactionEffectsAPI},
    executable_transaction::VerifiedExecutableTransaction,
    execution_status::{CongestedObjects, ExecutionFailureStatus, ExecutionStatus},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg, Transaction},
};
use move_core_types::ident_str;

use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::CongestionControlParameters,
        authority_tests::{
            build_programmable_transaction, certify_shared_obj_transaction_no_execution,
            execute_programmable_transaction, send_and_confirm_transaction_,
        },
        move_integration_tests::build_and_publish_test_package,
        shared_object_congestion_tracker::{
            CongestionPerObjectDebt,
            shared_object_test_utils::new_congestion_tracker_with_initial_value_for_test,
        },
        suggested_gas_price_calculator::suggested_gas_price_calculator_test_utils::new_suggested_gas_price_calculator_with_initial_values_for_test,
        test_authority_builder::TestAuthorityBuilder,
    },
    move_call,
};

pub const TEST_ONLY_GAS_PRICE: u64 = 1000;
pub const TEST_ONLY_GAS_UNIT: u64 = 10_000;

// Note that TestSetup is currently purposely created for
// test_congestion_control_execution_cancellation.
struct TestSetup {
    setup_authority_state: Arc<AuthorityState>,
    protocol_config: ProtocolConfig,
    sender: IotaAddress,
    sender_key: AccountKeyPair,
    package: ObjectRef,
    gas_object_id: ObjectID,
}

impl TestSetup {
    async fn new(
        max_execution_duration_per_commit: u64,
        max_congestion_limit_overshoot_per_commit: u64,
    ) -> Self {
        let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();

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

        let gas_object_id = ObjectID::random();
        let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);
        setup_authority_state
            .insert_genesis_object(gas_object.clone())
            .await;

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
    async fn create_shared_object(&self) -> ObjectRef {
        let mut builder = ProgrammableTransactionBuilder::new();
        move_call! {
            builder,
            (self.package.0)::congestion_control::create_shared()
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
            create_shared_object_effects.status().is_ok(),
            "Execution error {:?}",
            create_shared_object_effects.status()
        );
        assert_eq!(create_shared_object_effects.created().len(), 1);
        create_shared_object_effects.created()[0].0
    }

    // Creates a owned object in `setup_authority_state` and returns the object
    // reference.
    async fn create_owned_object(&self) -> ObjectRef {
        let mut builder = ProgrammableTransactionBuilder::new();
        move_call! {
            builder,
            (self.package.0)::congestion_control::create_owned()
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
            create_owned_object_effects.status().is_ok(),
            "Execution error {:?}",
            create_owned_object_effects.status()
        );
        assert_eq!(create_owned_object_effects.created().len(), 1);
        create_owned_object_effects.created()[0].0
    }

    // Converts an object to a genesis object by setting its previous_transaction to
    // a genesis marker.
    fn convert_to_genesis_obj(obj: Object) -> Object {
        let mut genesis_obj = obj;
        genesis_obj.previous_transaction = TransactionDigest::genesis_marker();
        genesis_obj
    }

    // Returns a list of objects that can be used as genesis object for a brand new
    // authority state, including the gas object, the package object, and the
    // objects passed in `objects`.
    async fn create_genesis_objects_for_new_authority_state(
        &self,
        objects: &[ObjectID],
    ) -> Vec<Object> {
        let mut genesis_objects = Vec::new();
        genesis_objects.push(TestSetup::convert_to_genesis_obj(
            self.setup_authority_state
                .get_object(&self.package.0)
                .await
                .unwrap(),
        ));
        genesis_objects.push(TestSetup::convert_to_genesis_obj(
            self.setup_authority_state
                .get_object(&self.gas_object_id)
                .await
                .unwrap(),
        ));

        for obj in objects {
            genesis_objects.push(TestSetup::convert_to_genesis_obj(
                self.setup_authority_state.get_object(obj).await.unwrap(),
            ));
        }
        genesis_objects
    }
}

// Creates a transaction that touches the shared objects provided and the owned
// object provided. The transaction is passed through a fake consensus and then
// the congestion control before being executed.
async fn commit_and_execute_transaction(
    authority_state: &AuthorityState,
    package: &ObjectRef,
    sender: &IotaAddress,
    sender_key: &AccountKeyPair,
    gas_object_id: &ObjectID,
    shared_objects: &[(ObjectID, SequenceNumber)],
    owned_object: &ObjectRef,
    gas_units: u64,
) -> (Transaction, TransactionEffects) {
    let mut txn_builder = ProgrammableTransactionBuilder::new();
    let mut args = vec![];
    for shared_object in shared_objects {
        args.push(
            txn_builder
                .obj(ObjectArg::SharedObject {
                    id: shared_object.0,
                    initial_shared_version: shared_object.1,
                    mutable: true,
                })
                .unwrap(),
        )
    }
    args.push(
        txn_builder
            .obj(ObjectArg::ImmOrOwnedObject(*owned_object))
            .unwrap(),
    );
    match args.len() {
        1 => {
            move_call! {
                txn_builder,
                (package.0)::congestion_control::increment_one(args.pop().unwrap())
            };
        }
        2 => {
            move_call! {
                txn_builder,
                (package.0)::congestion_control::increment_two(args.pop().unwrap(), args.pop().unwrap())
            };
        }
        3 => {
            move_call! {
                txn_builder,
                (package.0)::congestion_control::increment_three(args.pop().unwrap(), args.pop().unwrap(), args.pop().unwrap())
            };
        }
        _ => panic!("Unsupported number of shared objects. Maximum supported is 2."),
    }
    let pt = txn_builder.finish();
    let transaction = build_programmable_transaction(
        authority_state,
        gas_object_id,
        sender,
        sender_key,
        pt,
        gas_units,
    )
    .await
    .unwrap();

    let execution_effects =
        send_and_confirm_transaction_(authority_state, None, transaction.clone(), true)
            .await
            .unwrap()
            .1
            .into_data();
    (transaction, execution_effects)
}

// Tests execution aspect of cancelled transaction due to shared object
// congestion. Mainly tests that
//   1. Cancelled transaction should return correct error status.
//   2. Executing cancelled transaction with effects should result in the same
//      transaction cancellation.
#[sim_test]
async fn test_congestion_control_execution_cancellation() {
    telemetry_subscribers::init_for_testing();

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
            shared_object_1.0,
            shared_object_2.0,
            owned_object.0,
        ])
        .await;

    // Creates two authority states with the same genesis objects for the actual
    // test. One tests cancellation execution, and one tests executing cancelled
    // transaction from effect.
    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state
        .insert_genesis_objects(&genesis_objects)
        .await;
    let authority_state_2 = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state_2
        .insert_genesis_objects(&genesis_objects)
        .await;

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
            &[(shared_object_1.0, initial_debt)],
            congestion_control_parameters_1.clone(),
        ))
    });
    let congestion_control_parameters_2 = congestion_control_parameters.clone();
    register_fail_point_arg("initial_suggested_gas_price_calculator", move || {
        Some(
            new_suggested_gas_price_calculator_with_initial_values_for_test(
                &[(shared_object_1.0, initial_debt, TEST_ONLY_GAS_PRICE)],
                congestion_control_parameters_2.clone(),
                TEST_ONLY_GAS_PRICE,
            ),
        )
    });

    // Runs a transaction that touches shared_object_1, shared_object_2 and a owned
    // object.
    let (congested_tx, effects) = commit_and_execute_transaction(
        &authority_state,
        &test_setup.package,
        &test_setup.sender,
        &test_setup.sender_key,
        &test_setup.gas_object_id,
        &[
            (shared_object_1.0, shared_object_1.1),
            (shared_object_2.0, shared_object_2.1),
        ],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        TEST_ONLY_GAS_UNIT,
    )
    .await;

    let suggested_gas_price = TEST_ONLY_GAS_PRICE + 1;

    // Transaction should be cancelled with `shared_object_1` and `shared_object_2`
    // as the congested objects, and the suggested gas price should be
    // `TEST_ONLY_GAS_PRICE`.
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                congested_objects: CongestedObjects(vec![shared_object_1.0, shared_object_2.0]),
                suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![
            InputSharedObject::Cancelled(
                shared_object_1.0,
                SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price)
            ),
            InputSharedObject::Cancelled(
                shared_object_2.0,
                SequenceNumber::new_congested_with_suggested_gas_price(suggested_gas_price)
            )
        ]
    );

    // Run the same transaction in `authority_state_2`, but using the above effects
    // for the execution.
    let cert = certify_shared_obj_transaction_no_execution(&authority_state_2, congested_tx)
        .await
        .unwrap();
    authority_state_2
        .epoch_store_for_testing()
        .acquire_shared_version_assignments_from_effects(
            &VerifiedExecutableTransaction::new_from_certificate(cert.clone()),
            &effects,
            authority_state_2.get_object_cache_reader().as_ref(),
        )
        .unwrap();
    let (effects_2, execution_error) = authority_state_2.execute_for_test(&cert);

    // Should result in the same cancellation.
    assert_eq!(
        execution_error.unwrap().to_execution_status().0,
        ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
            congested_objects: CongestedObjects(vec![shared_object_1.0, shared_object_2.0]),
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
            shared_object_1.0,
            shared_object_2.0,
            owned_object.0,
        ])
        .await;

    // Creates an authority state with the genesis objects.
    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(TEST_ONLY_GAS_PRICE)
        .with_protocol_config(test_setup.protocol_config.clone())
        .build()
        .await;
    authority_state
        .insert_genesis_objects(&genesis_objects)
        .await;

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
        &[(shared_object_1.0, shared_object_1.1)],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;

    // Transaction should be a success as overshoot of 2*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_ok());

    // Check that the debt stored in consensus quarantine is correct.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
            (shared_object_1.0, shared_object_1.1),
            (shared_object_2.0, shared_object_2.1),
        ],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        TEST_ONLY_GAS_UNIT / 2,
    )
    .await;

    // Transaction should be a success as overshoot of 1.5*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_ok());
    // Check that the debt stored in consensus quarantine is correct. Both shared
    // objects should have a debt of 1.5*default_tx_gas_budget.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
        &[(shared_object_2.0, shared_object_2.1)],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
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
            error: ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                congested_objects: CongestedObjects(vec![shared_object_2.0]),
                suggested_gas_price: expected_suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![InputSharedObject::Cancelled(
            shared_object_2.0,
            SequenceNumber::new_congested_with_suggested_gas_price(expected_suggested_gas_price)
        ),]
    );

    // Check that the debt stored in consensus quarantine is correct. Shared object
    // 1 should still have a stored debt of 1.5*default_tx_gas_budget from
    // commit 2 that has carried over because because it was not updated in commit 3
    // as there was not transaction touching it. Shared object 2 should have a
    // debt of 0.5*default_tx_gas_budget from commit 3 because it was updated in
    // the consensus quarantine even though the execution was cancelled.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
        &[(shared_object_1.0, shared_object_1.1)],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        5 * TEST_ONLY_GAS_UNIT / 2,
    )
    .await;

    // Transaction should be executed successfully as overshoot of
    // 2*default_tx_gas_budget is allowed.
    assert!(effects.status().is_ok());

    // Check that the debt stored in consensus quarantine is correct. Shared object
    // 1 should now have a debt of 2*default_tx_gas_budget from commit 4 and shared
    // object 2 should still have a stored debt of 0.5*default_tx_gas_budget from
    // commit 3. This debt is effectively worth nothing in commit 5 because it will
    // be reduced by default_tx_gas_budget due to the skipped round.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
            (shared_object_1.0, shared_object_1.1),
            (shared_object_2.0, shared_object_2.1),
        ],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
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
            error: ExecutionFailureStatus::ExecutionCancelledDueToSharedObjectCongestionV2 {
                congested_objects: CongestedObjects(vec![shared_object_1.0, shared_object_2.0]),
                suggested_gas_price: expected_suggested_gas_price,
            },
            command: None
        }
    );

    // Tests shared object versions in effects are set correctly.
    assert_eq!(
        effects.input_shared_objects(),
        vec![
            InputSharedObject::Cancelled(
                shared_object_1.0,
                SequenceNumber::new_congested_with_suggested_gas_price(
                    expected_suggested_gas_price
                )
            ),
            InputSharedObject::Cancelled(
                shared_object_2.0,
                SequenceNumber::new_congested_with_suggested_gas_price(
                    expected_suggested_gas_price
                )
            )
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
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;
    // Transaction should be a success as there is no shared object involved.
    assert!(effects.status().is_ok());

    // The debt on shared object 1 should still be stored as default_tx_gas_budget
    // from commit 5 as it was not updated in commit 6. The debt on shared
    // object 2 should still be stored as 0.5*default_tx_gas_budget from commit 3.
    // Both of these debts are effectively worth nothing in commit 6 because they
    // will be reduced by default_tx_gas_budget for each skipped round.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
            (shared_object_1.0, shared_object_1.1),
            (shared_object_2.0, shared_object_2.1),
        ],
        &authority_state
            .get_object(&owned_object.0)
            .await
            .unwrap()
            .compute_object_reference(),
        3 * TEST_ONLY_GAS_UNIT,
    )
    .await;
    // Transaction should be a success as overshoot of 2*default_tx_gas_budget is
    // allowed.
    assert!(effects.status().is_ok());

    // The debt on both shared objects should should have been updated in storage to
    // 2*default_tx_gas_budget.
    let shared_object_1_debt = authority_state
        .epoch_store_for_testing()
        .load_stored_object_debts_for_testing(false, &[shared_object_1.0])
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
        .load_stored_object_debts_for_testing(false, &[shared_object_2.0])
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
