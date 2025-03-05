// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_api::{GovernanceReadApiClient, TransactionBuilderClient};
use iota_json_rpc_types::{
    DelegatedStake, DelegatedTimelockedStake, StakeStatus, TransactionBlockBytes,
};
use iota_protocol_config::ProtocolVersion;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, IOTA_SYSTEM_ADDRESS,
    balance::Balance,
    base_types::ObjectID,
    crypto::{AccountKeyPair, get_key_pair},
    gas_coin::GAS,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, ObjectArg},
    utils::to_sender_signed_transaction,
};
use move_core_types::{identifier::Identifier, language_storage::TypeTag};

use crate::common::{
    ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
    indexer_wait_for_object, indexer_wait_for_transaction,
    start_test_cluster_with_read_write_indexer,
};

#[test]
fn test_staking() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (cluster, store, client) =
            &start_test_cluster_with_read_write_indexer(Some("test_staking"), None).await;

        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let iota_coin_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, iota_coin_ref.0, iota_coin_ref.1).await;

        // Check StakedIota object before test
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
        assert!(staked_iota.is_empty());

        let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
        let validator = match iota_system_state {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => panic!("unsupported IotaSystemStateSummary"),
        };

        // Delegate some IOTA
        let transaction_bytes: TransactionBlockBytes = client
            .request_add_stake(
                sender,
                vec![iota_coin_ref.0],
                Some(1000000000.into()),
                validator,
                Some(gas.0),
                100_000_000.into(),
            )
            .await
            .unwrap();

        let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &keypair);

        let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        // Check DelegatedStake object after epoch transition
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
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
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (cluster, store, client) =
            &start_test_cluster_with_read_write_indexer(Some("test_unstaking"), None).await;

        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let iota_coin_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, iota_coin_ref.0, iota_coin_ref.1).await;

        // Check StakedIota object before test
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
        assert!(staked_iota.is_empty());

        let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
        let validator = match iota_system_state {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => panic!("unsupported IotaSystemStateSummary"),
        };

        // Delegate some IOTA
        let transaction_bytes: TransactionBlockBytes = client
            .request_add_stake(
                sender,
                vec![iota_coin_ref.0],
                Some(1000000000.into()),
                validator,
                Some(gas.0),
                100_000_000.into(),
            )
            .await
            .unwrap();

        let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &keypair);

        let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        // Check DelegatedStake object
        let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
        assert_eq!(1, staked_iota.len());
        assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
        assert!(matches!(
            staked_iota[0].stakes[0].status,
            StakeStatus::Active {
                estimated_reward: _
            }
        ));

        let transaction_bytes: TransactionBlockBytes = client
            .request_withdraw_stake(
                sender,
                staked_iota[0].stakes[0].staked_iota_id,
                Some(gas.0),
                100_000_000.into(),
            )
            .await
            .unwrap();

        let txn = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &keypair);

        let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        let indexer_response = client
            .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
            .await
            .unwrap();
        assert_eq!(0, indexer_response.len());

        let staked_iota: Vec<DelegatedStake> = client.get_stakes(sender).await.unwrap();
        assert!(staked_iota.is_empty());
    });
}

#[test]
fn test_timelocked_staking() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (cluster, store, client) =
            &start_test_cluster_with_read_write_indexer(Some("test_timelocked_staking"), None)
                .await;

        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let iota_coin_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, iota_coin_ref.0, iota_coin_ref.1).await;

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
            let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
            let validator = match iota_system_state {
                IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
                IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
                _ => panic!("unsupported IotaSystemStateSummary"),
            };

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

        let context = &cluster.wallet;
        let gas_price = context.get_reference_gas_price().await.unwrap();

        let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
        let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &keypair);

        let res = context.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        let staked_iota: Vec<DelegatedTimelockedStake> =
            client.get_timelocked_stakes(sender).await.unwrap();

        assert_eq!(staked_iota.len(), 1);
        assert_eq!(10000000000, staked_iota[0].stakes[0].principal);
        assert!(matches!(
            staked_iota[0].stakes[0].status,
            StakeStatus::Active {
                estimated_reward: _
            }
        ));
    });
}

#[test]
fn test_timelocked_unstaking() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (cluster, store, client) =
            &start_test_cluster_with_read_write_indexer(Some("test_timelocked_unstaking"), None)
                .await;

        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let iota_coin_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, iota_coin_ref.0, iota_coin_ref.1).await;

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
            let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
            let validator = match iota_system_state {
                IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
                IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
                _ => panic!("unsupported IotaSystemStateSummary"),
            };

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

        let context = &cluster.wallet;
        let gas_price = context.get_reference_gas_price().await.unwrap();

        let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
        let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &keypair);

        let res = context.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        let staked_iota = client.get_timelocked_stakes(sender).await.unwrap();

        assert_eq!(staked_iota.len(), 1);
        assert_eq!(10000000000, staked_iota[0].stakes[0].principal);
        assert!(matches!(
            staked_iota[0].stakes[0].status,
            StakeStatus::Active {
                estimated_reward: _
            }
        ));

        let timelocked_stake_id = staked_iota[0].stakes[0].timelocked_staked_iota_id;
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

        let gas = cluster.wallet.get_object_ref(gas.0).await.unwrap();
        let tx_builder = TestTransactionBuilder::new(sender, gas, gas_price);
        let txn = to_sender_signed_transaction(tx_builder.programmable(pt).build(), &keypair);

        let res = context.execute_transaction_must_succeed(txn).await;
        indexer_wait_for_transaction(res.digest, store, client).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;

        let res = client.get_timelocked_stakes(sender).await.unwrap();
        assert_eq!(res.len(), 0);

        let res = client
            .get_timelocked_stakes_by_ids(vec![timelocked_stake_id])
            .await
            .unwrap();

        assert_eq!(res.len(), 0);
    });
}

#[test]
fn get_latest_iota_system_state_v2() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // ensure that the system state is updated
        cluster.force_new_epoch().await;
        let system_state = client.get_latest_iota_system_state_v2().await.unwrap();
        let IotaSystemStateSummary::V2(system_state_v2) = system_state else {
            panic!("expected IotaSystemStateSummaryV2");
        };
        assert_eq!(
            system_state_v2.protocol_version,
            ProtocolVersion::MAX.as_u64()
        );
        assert_eq!(system_state_v2.system_state_version, 2);
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
        let response = client.get_committee_info(None).await.unwrap();

        assert_eq!(response.validators.len(), 4);

        // Test with specified epoch 0
        let response = client.get_committee_info(Some(0.into())).await.unwrap();

        let (epoch_id, validators) = (response.epoch, response.validators);

        assert!(epoch_id == 0);
        assert_eq!(validators.len(), 4);

        // Test with non-existent epoch
        let response = client.get_committee_info(Some(u64::MAX.into())).await;

        assert!(response.is_err());
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

        let apys = client.get_validators_apy().await.unwrap().apys;

        assert_eq!(apys.len(), 4);
        assert!(apys.iter().any(|apy| apy.apy == 0.0));
    });
}
