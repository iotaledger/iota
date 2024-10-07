// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json::{call_args, type_args};
use iota_json_rpc_api::GovernanceReadApiClient;
use iota_json_rpc_types::DelegatedStake;
use iota_protocol_config::ProtocolConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{IotaAddress, MoveObjectType, ObjectID, TransactionDigest},
    gas_coin::GAS,
    id::UID,
    object::{Data, MoveObject, ObjectInner, Owner, OBJECT_START_VERSION},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    transaction::{Argument, CallArg, ObjectArg},
    IOTA_FRAMEWORK_ADDRESS,
};
use move_core_types::identifier::Identifier;

use crate::common::{indexer_wait_for_checkpoint, rpc_call_error_msg_matches, ApiTestSetup};

#[test]
fn get_stakes_by_id() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let validator_address = cluster
            .swarm
            .active_validators()
            .next()
            .unwrap()
            .config
            .iota_address();

        let response: Vec<DelegatedStake> = client.get_stakes(validator_address).await.unwrap();
        let stake_id = response
            .first()
            .unwrap()
            .stakes
            .first()
            .unwrap()
            .staked_iota_id;

        let response = client.get_stakes_by_ids(vec![stake_id]).await.unwrap();
        assert_eq!(response.len(), 1);

        let response = client.get_stakes_by_ids(vec![]).await.unwrap();
        assert_eq!(response.len(), 0);
    });
}

#[test]
fn get_stakes() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let validator_address = cluster
            .swarm
            .active_validators()
            .next()
            .unwrap()
            .config
            .iota_address();

        let response = client.get_stakes(validator_address).await.unwrap();
        assert_eq!(response.len(), 1);

        let response = client
            .get_stakes(IotaAddress::random_for_testing_only())
            .await
            .unwrap();
        assert_eq!(response.len(), 0);
    });
}

#[test]
fn get_timelocked_stakes_by_id() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let sender = cluster.get_address_0();
        let context = &cluster.wallet;

        let gas_price = context.get_reference_gas_price().await.unwrap();
        let mut gas_objects = context.get_all_gas_objects_owned_by_address(sender).await.unwrap();
        assert!(gas_objects.len() >= 2);
        let iota_coin_ref = gas_objects.pop().unwrap();
        println!("gas object id used: {:?}\n", iota_coin_ref.0);

        let gas_data = context.gas_objects(sender).await.unwrap();
        println!("gas_objects data: {:?}\n", gas_data);

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let iota_coin_argument = builder.obj(ObjectArg::ImmOrOwnedObject(iota_coin_ref)).expect("valid obj");

            // Step 1: Get the IOTA balance from the coin object.
            let iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("coin").unwrap(),
                Identifier::new("into_balance").unwrap(),
                vec![GAS::type_tag()],
                vec![iota_coin_argument]);

            /*
            // Step 2: Timelock the IOTA balance.
            let timelock_timestamp = builder.pure(1000).expect("valid pure");
            let timelocked_iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("timelock").unwrap(),
                Identifier::new("lock").unwrap(),
                vec![GAS::type_tag()],
                vec![iota_balance, timelock_timestamp]);
             */

            builder.transfer_arg(sender, iota_balance);

            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(sender, gas_objects.pop().unwrap(), gas_price);
        let txn = context.sign_transaction(
            &tx_builder.programmable(pt).build(),
        );

        let resp = context.execute_transaction_must_succeed(txn).await;

        println!("resp: {:?}", resp);

        /*
        let response = client
            .get_timelocked_stakes_by_ids(vec![ObjectID::random()])
            .await
            .unwrap();
        assert_eq!(response.len(), 0);
         */
    });
}

#[test]
fn get_timelocked_stakes() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let validator_address = cluster
            .swarm
            .active_validators()
            .next()
            .unwrap()
            .config
            .iota_address();

        let response = client
            .get_timelocked_stakes(validator_address)
            .await
            .unwrap();

        assert_eq!(response.len(), 0);
    });
}

#[test]
fn get_committee_info() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // Test with no specified epoch
        let indexer_response = client.get_committee_info(None).await.unwrap();

        let (epoch_id, validators) = (indexer_response.epoch, indexer_response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with specified epoch 0
        let indexer_response = client.get_committee_info(Some(0.into())).await.unwrap();

        let (epoch_id, validators) = (indexer_response.epoch, indexer_response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(1.into())).await;

        rpc_call_error_msg_matches(
            response,
            r#"{"code":-32603,"message":"Invalid argument with error: `Missing epoch Some(1)`"}"#,
        );
    });
}

#[test]
fn get_reference_gas_price() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_reference_gas_price().await.unwrap();
        assert_eq!(response, 1000.into());
    });
}

#[test]
fn get_latest_iota_system_state() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_latest_iota_system_state().await.unwrap();
        assert_eq!(response.epoch, 0);
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.system_state_version, 1);
    });
}

#[test]
fn get_validators_apy() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let response = client.get_validators_apy().await.unwrap();
        let (apys, epoch) = (response.apys, response.epoch);

        assert_eq!(epoch, 0);
        assert_eq!(apys.len(), 4);
        assert_eq!(apys.iter().find(|apy| apy.apy == 0.0).is_some(), true);
    });
}
