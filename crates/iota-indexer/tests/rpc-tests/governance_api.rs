// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, TransactionBuilderClient,
    WriteApiClient,
};
use iota_json_rpc_types::{
    CoinPage, DelegatedStake, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockResponseOptions, ObjectsPage, StakeStatus, TransactionBlockBytes,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS,
    balance::Balance,
    base_types::ObjectID,
    gas_coin::GAS,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, ObjectArg},
};
use move_core_types::{identifier::Identifier, language_storage::TypeTag};

use crate::common::{
    ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
    indexer_wait_for_transaction,
};

#[test]
fn test_staking() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let address = cluster.get_address_0();

        let objects: ObjectsPage = client
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction(),
                )),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(5, objects.data.len());

        // Check StakedIota object before test
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());

        let validator = client
            .get_latest_iota_system_state()
            .await
            .unwrap()
            .active_validators[0]
            .iota_address;

        let coin = objects.data[0].object().unwrap().object_id;
        // Delegate some IOTA
        let transaction_bytes: TransactionBlockBytes = client
            .request_add_stake(
                address,
                vec![coin],
                Some(1000000000.into()),
                validator,
                None,
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data().unwrap());

        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        // Check DelegatedStake object after epoch transition
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(address).await.unwrap();
        assert_eq!(1, staked_iota.len());
        assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
        assert!(matches!(
            staked_iota[0].stakes[0].status,
            StakeStatus::Active {
                estimated_reward: _
            }
        ));
    });
}

#[test]
fn test_unstaking() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    let indexer_client = client;

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let address = cluster.get_address_0();

        let coins: CoinPage = indexer_client
            .get_coins(address, None, None, None)
            .await
            .unwrap();
        assert_eq!(5, coins.data.len());

        // Check StakedIota object before test
        let staked_iota: Vec<DelegatedStake> = indexer_client.get_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());

        let validator = indexer_client
            .get_latest_iota_system_state()
            .await
            .unwrap()
            .active_validators[0]
            .iota_address;

        // Delegate some IOTA
        let transaction_bytes: TransactionBlockBytes = indexer_client
            .request_add_stake(
                address,
                vec![coins.data[0].coin_object_id],
                Some(1000000000.into()),
                validator,
                None,
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data().unwrap());

        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        indexer_client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        // Check DelegatedStake object
        let staked_iota: Vec<DelegatedStake> = indexer_client.get_stakes(address).await.unwrap();
        assert_eq!(1, staked_iota.len());
        assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
        assert!(matches!(
            staked_iota[0].stakes[0].status,
            StakeStatus::Active {
                estimated_reward: _
            }
        ));

        let transaction_bytes: TransactionBlockBytes = indexer_client
            .request_withdraw_stake(
                address,
                staked_iota[0].stakes[0].staked_iota_id,
                None,
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data().unwrap());

        let _ = cluster.wallet.execute_transaction_must_succeed(tx).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        let node_response = cluster
            .rpc_client()
            .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
            .await
            .unwrap();
        assert_eq!(1, node_response.len());
        assert!(matches!(
            node_response[0].stakes[0].status,
            StakeStatus::Unstaked
        ));

        let indexer_response = indexer_client
            .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
            .await
            .unwrap();
        assert_eq!(0, indexer_response.len());

        let staked_iota: Vec<DelegatedStake> = indexer_client.get_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());
    });
}

#[test]
fn test_timelocked_staking() {
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
        let mut gas_objects = context
            .get_all_gas_objects_owned_by_address(sender)
            .await
            .unwrap();
        assert!(gas_objects.len() >= 2);
        let iota_coin_ref = gas_objects.pop().unwrap();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let iota_coin_argument = builder
                .input(CallArg::Object(ObjectArg::ImmOrOwnedObject(iota_coin_ref)))
                .expect("valid obj");

            // Step 1: Get the IOTA balance from the coin object.
            let iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("coin").unwrap(),
                Identifier::new("into_balance").unwrap(),
                vec![GAS::type_tag()],
                vec![iota_coin_argument],
            );

            // Step 2: Timelock the IOTA balance.
            let timelock_timestamp = builder.input(CallArg::from(u64::MAX)).unwrap();
            let timelocked_iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("timelock").unwrap(),
                Identifier::new("lock").unwrap(),
                vec![TypeTag::Struct(Box::new(Balance::type_(GAS::type_tag())))],
                vec![iota_balance, timelock_timestamp],
            );

            // Step 3: Delegate the timelocked IOTA balance.
            let validator = client
                .get_latest_iota_system_state()
                .await
                .unwrap()
                .active_validators[0]
                .iota_address;

            let validator = builder
                .input(CallArg::Pure(bcs::to_bytes(&validator).unwrap()))
                .unwrap();
            let state = builder.input(CallArg::IOTA_SYSTEM_MUT).unwrap();

            let _ = builder.programmable_move_call(
                ObjectID::new(IOTA_SYSTEM_ADDRESS.into_bytes()),
                Identifier::new("timelocked_staking").unwrap(),
                Identifier::new("request_add_stake").unwrap(),
                vec![],
                vec![state, timelocked_iota_balance, validator],
            );

            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(sender, gas_objects.pop().unwrap(), gas_price);
        let txn = context.sign_transaction(&tx_builder.programmable(pt).build());

        let res = context.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        let response = client.get_timelocked_stakes(sender).await.unwrap();

        assert_eq!(response.len(), 1);
    });
}

#[test]
fn test_timelocked_unstaking() {
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
        let mut gas_objects = context
            .get_all_gas_objects_owned_by_address(sender)
            .await
            .unwrap();
        assert!(gas_objects.len() >= 2);
        let iota_coin_ref = gas_objects.pop().unwrap();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let iota_coin_argument = builder
                .input(CallArg::Object(ObjectArg::ImmOrOwnedObject(iota_coin_ref)))
                .expect("valid obj");

            // Step 1: Get the IOTA balance from the coin object.
            let iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("coin").unwrap(),
                Identifier::new("into_balance").unwrap(),
                vec![GAS::type_tag()],
                vec![iota_coin_argument],
            );

            // Step 2: Timelock the IOTA balance.
            let timelock_timestamp = builder.input(CallArg::from(u64::MAX)).unwrap();
            let timelocked_iota_balance = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::new("timelock").unwrap(),
                Identifier::new("lock").unwrap(),
                vec![TypeTag::Struct(Box::new(Balance::type_(GAS::type_tag())))],
                vec![iota_balance, timelock_timestamp],
            );

            // Step 3: Delegate the timelocked IOTA balance.
            let validator = client
                .get_latest_iota_system_state()
                .await
                .unwrap()
                .active_validators[0]
                .iota_address;

            let validator = builder
                .input(CallArg::Pure(bcs::to_bytes(&validator).unwrap()))
                .unwrap();
            let state = builder.input(CallArg::IOTA_SYSTEM_MUT).unwrap();

            let _ = builder.programmable_move_call(
                ObjectID::new(IOTA_SYSTEM_ADDRESS.into_bytes()),
                Identifier::new("timelocked_staking").unwrap(),
                Identifier::new("request_add_stake").unwrap(),
                vec![],
                vec![state, timelocked_iota_balance, validator],
            );

            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(sender, gas_objects.pop().unwrap(), gas_price);
        let txn = context.sign_transaction(&tx_builder.programmable(pt).build());

        let res = context.execute_transaction_must_succeed(txn).await;

        indexer_wait_for_transaction(res.digest, store, client).await;
        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        let response = client.get_timelocked_stakes(sender).await.unwrap();

        assert_eq!(response.len(), 1);

        let timelocked_stake_id = response[0].stakes[0].timelocked_staked_iota_id;
        let timelocked_stake_id_ref = cluster
            .wallet
            .get_object_ref(timelocked_stake_id)
            .await
            .unwrap();

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();

            let timelocked_stake_id_argument = builder
                .input(CallArg::Object(ObjectArg::ImmOrOwnedObject(
                    timelocked_stake_id_ref,
                )))
                .expect("valid obj");

            let state = builder.input(CallArg::IOTA_SYSTEM_MUT).unwrap();

            let _ = builder.programmable_move_call(
                ObjectID::new(IOTA_SYSTEM_ADDRESS.into_bytes()),
                Identifier::new("timelocked_staking").unwrap(),
                Identifier::new("request_withdraw_stake").unwrap(),
                vec![],
                vec![state, timelocked_stake_id_argument],
            );

            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(sender, gas_objects.pop().unwrap(), gas_price);

        let txn = context.sign_transaction(&tx_builder.programmable(pt).build());

        let res = context.execute_transaction_must_succeed(txn).await;

        indexer_wait_for_transaction(res.digest, store, client).await;
        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, &cluster).await;

        let res = client.get_timelocked_stakes(sender).await.unwrap();
        assert_eq!(res.len(), 0);

        let res = cluster
            .rpc_client()
            .get_timelocked_stakes_by_ids(vec![timelocked_stake_id])
            .await
            .unwrap();

        assert_eq!(res.len(), 1);

        assert!(matches!(res[0].stakes[0].status, StakeStatus::Unstaked));

        let res = client
            .get_timelocked_stakes_by_ids(vec![timelocked_stake_id])
            .await
            .unwrap();

        assert_eq!(res.len(), 0);
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
fn get_committee_info() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // Test with no specified epoch
        let response = client.get_committee_info(None).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with specified epoch 0
        let response = client.get_committee_info(Some(0.into())).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(1.into())).await;

        assert!(response.is_err());

        // Sleep for 5 seconds
        cluster.force_new_epoch().await;

        // Test with specified epoch 1
        let response = client.get_committee_info(Some(1.into())).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 1);
        assert_eq!(validators.len(), 4);
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
