// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{AccountKeyPair, get_key_pair},
    effects::TransactionEffectsAPI,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg, Transaction},
};
use move_core_types::ident_str;
use rand::seq::SliceRandom;

use crate::{
    authority::{
        AuthorityState,
        authority_tests::{
            build_programmable_transaction, certify_transaction, execute_programmable_transaction,
            send_batch_consensus_no_execution,
        },
        move_integration_tests::build_and_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    move_call,
};

/// Reference gas price used in gas price feedback mechanism tests.
const REFERENCE_GAS_PRICE_FOR_TESTS: u64 = 1_000;

const GAS_UNITS_FOR_TEST: u64 = 10_000;

struct GasPriceFeedbackTester {
    authority_state: Arc<AuthorityState>,
    sender: IotaAddress,
    sender_key: AccountKeyPair,
    gas_object_ids: Vec<ObjectID>,
    package: ObjectRef,
    shared_counter_1: ObjectRef,
    shared_counter_2: ObjectRef,
}

impl GasPriceFeedbackTester {
    async fn new(
        max_deferral_rounds_for_congestion_control: u64,
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
        max_execution_duration_per_commit: u64,
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
        protocol_config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(
            max_execution_duration_per_commit,
        );
        protocol_config.set_min_checkpoint_interval_ms_for_testing(1000);

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
            sender,
            sender_key,
            gas_object_ids,
            package,
            shared_counter_1,
            shared_counter_2,
        }
    }

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

        let effects = execute_programmable_transaction(
            authority_state,
            gas_object_id,
            sender,
            sender_key,
            pt,
            GAS_UNITS_FOR_TEST,
        )
        .await
        .unwrap();

        assert!(
            effects.status().is_ok(),
            "Execution error {:?}",
            effects.status()
        );
        assert_eq!(effects.created().len(), 1);

        effects.created()[0].0
    }

    async fn build_increment_both_counters_tx(
        &self,
        gas_object_id: &ObjectID,
        // gas_price: u64
    ) -> Transaction {
        let mut txn_builder = ProgrammableTransactionBuilder::new();

        let arg1 = txn_builder
            .obj(ObjectArg::SharedObject {
                id: self.shared_counter_1.0,
                initial_shared_version: self.shared_counter_1.1,
                mutable: true,
            })
            .unwrap();

        let arg2 = txn_builder
            .obj(ObjectArg::SharedObject {
                id: self.shared_counter_2.0,
                initial_shared_version: self.shared_counter_2.1,
                mutable: true,
            })
            .unwrap();

        move_call! {
            txn_builder,
            (self.package.0)::gas_price_feedback::increment_both(arg1, arg2)
        };

        let pt = txn_builder.finish();

        build_programmable_transaction(
            &self.authority_state,
            gas_object_id,
            &self.sender,
            &self.sender_key,
            pt,
            GAS_UNITS_FOR_TEST,
        )
        .await
        .unwrap()
    }
}

#[tokio::test]
async fn gas_price_feedback_mechanism() {
    let max_deferral_rounds_for_congestion_control = 1;
    let per_object_congestion_control_mode = PerObjectCongestionControlMode::TotalTxCount;
    let max_execution_duration_per_commit = 1;
    let num_gas_objects = 20;

    let tester = GasPriceFeedbackTester::new(
        max_deferral_rounds_for_congestion_control,
        per_object_congestion_control_mode,
        max_execution_duration_per_commit,
        num_gas_objects,
    )
    .await;

    // Prepare certificates
    let mut certificates = vec![];
    for gas_object_id in tester.gas_object_ids.iter() {
        let transaction = tester.build_increment_both_counters_tx(gas_object_id).await;

        certificates.push(
            certify_transaction(&tester.authority_state, transaction)
                .await
                .unwrap(),
        );
    }
    certificates.shuffle(&mut rand::thread_rng());
    assert_eq!(certificates.len(), num_gas_objects);

    let scheduled_certificates =
        send_batch_consensus_no_execution(&tester.authority_state, &certificates, true).await;
    assert_eq!(
        scheduled_certificates.len(),
        max_execution_duration_per_commit as usize
    );

    tester.authority_state.transaction_manager().enqueue(
        scheduled_certificates.clone(),
        &tester.authority_state.epoch_store_for_testing(),
    );

    for cert in scheduled_certificates {
        let effects = tester
            .authority_state
            .get_transaction_cache_reader()
            .notify_read_executed_effects(&[*cert.digest()])
            .await
            .map(|mut r| r.pop().expect("must return correct number of effects"))
            .unwrap();
        assert!(effects.status().is_ok());
    }
}
