// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{AccountKeyPair, get_key_pair},
    effects::TransactionEffectsAPI,
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
};
use move_core_types::ident_str;

use crate::{
    authority::{
        AuthorityState, authority_tests::execute_programmable_transaction,
        move_integration_tests::build_and_publish_test_package,
        test_authority_builder::TestAuthorityBuilder,
    },
    move_call,
};

/// Gas price used only for testing gas price feedback mechanism.
const GAS_PRICE_FOR_TEST: u64 = 1_000;

const GAS_UNITS_FOR_TEST: u64 = 10_000;

async fn create_shared_counter_ptb(
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

#[sim_test]
async fn gas_price_feedback_mechanism() {
    let (sender, sender_key): (IotaAddress, AccountKeyPair) = get_key_pair();

    let mut protocol_config =
        ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown);
    protocol_config.set_per_object_congestion_control_mode_for_testing(
        PerObjectCongestionControlMode::TotalTxCount,
    );

    let max_execution_duration_per_commit = 2;
    protocol_config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(
        max_execution_duration_per_commit,
    );

    protocol_config.set_max_deferral_rounds_for_congestion_control_for_testing(3);

    let authority_state = TestAuthorityBuilder::new()
        .with_reference_gas_price(GAS_PRICE_FOR_TEST)
        .with_protocol_config(protocol_config.clone())
        .build()
        .await;

    let gas_object_id = ObjectID::random();
    let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);
    authority_state
        .insert_genesis_object(gas_object.clone())
        .await;

    let package = build_and_publish_test_package(
        &authority_state,
        &sender,
        &sender_key,
        &gas_object_id,
        "gas_price_feedback",
        false,
    )
    .await;

    create_shared_counter_ptb(
        &authority_state,
        &package.0,
        &gas_object_id,
        &sender,
        &sender_key,
    )
    .await;
}
