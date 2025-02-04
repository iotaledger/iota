// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, time::Duration};

use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, TransactionBuilderClient,
    WriteApiClient,
};
use iota_json_rpc_types::{
    CoinPage, DelegatedStake, DelegatedTimelockedStake, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockResponseOptions, ObjectsPage, StakeStatus,
    TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_swarm_config::genesis_config::{
    AccountConfig, ValidatorGenesisConfig, ValidatorGenesisConfigBuilder,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{MoveObjectType, ObjectID},
    crypto::deterministic_random_account_key,
    digests::TransactionDigest,
    governance::MIN_VALIDATOR_JOINING_STAKE_NANOS,
    id::UID,
    iota_system_state::IotaSystemStateTrait,
    object::{Data, MoveObject, OBJECT_START_VERSION, ObjectInner, Owner},
    quorum_driver_types::ExecuteTransactionRequestType,
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    utils::to_sender_signed_transaction,
};
use rand::rngs::OsRng;
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::time::sleep;

/// Execute a sequence of transactions to add a validator, including adding
/// candidate, adding stake and activate the validator.
/// It does not however trigger reconfiguration yet.
async fn execute_add_validator_transactions(
    test_cluster: &TestCluster,
    new_validator: &ValidatorGenesisConfig,
) {
    let pending_active_count = test_cluster.fullnode_handle.iota_node.with(|node| {
        let system_state = node
            .state()
            .get_iota_system_state_object_for_testing()
            .unwrap();
        system_state
            .get_pending_active_validators(node.state().get_object_store().as_ref())
            .unwrap()
            .len()
    });

    let cur_validator_candidate_count = test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_iota_system_state_object_for_testing()
            .unwrap()
            .into_iota_system_state_summary()
            .validator_candidates_size
    });
    let address = (&new_validator.account_key_pair.public()).into();
    let gas = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(address)
        .await
        .unwrap()
        .unwrap();

    let tx =
        TestTransactionBuilder::new(address, gas, test_cluster.get_reference_gas_price().await)
            .call_request_add_validator_candidate(
                &new_validator.to_validator_info_with_random_name().into(),
            )
            .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(tx).await;

    // Check that the candidate can be found in the candidate table now.
    test_cluster.fullnode_handle.iota_node.with(|node| {
        let system_state = node
            .state()
            .get_iota_system_state_object_for_testing()
            .unwrap();
        let system_state_summary = system_state.into_iota_system_state_summary();
        assert_eq!(
            system_state_summary.validator_candidates_size,
            cur_validator_candidate_count + 1
        );
    });

    let address = (&new_validator.account_key_pair.public()).into();
    let stake_coin = test_cluster
        .wallet
        .gas_for_owner_budget(
            address,
            MIN_VALIDATOR_JOINING_STAKE_NANOS,
            Default::default(),
        )
        .await
        .unwrap()
        .1
        .object_ref();
    let gas = test_cluster
        .wallet
        .gas_for_owner_budget(address, 0, BTreeSet::from([stake_coin.0]))
        .await
        .unwrap()
        .1
        .object_ref();

    let rgp = test_cluster.get_reference_gas_price().await;
    let stake_tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_staking(stake_coin, address)
        .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(stake_tx).await;

    let gas = test_cluster.wallet.get_object_ref(gas.0).await.unwrap();
    let tx = TestTransactionBuilder::new(address, gas, rgp)
        .call_request_add_validator()
        .build_and_sign(&new_validator.account_key_pair);
    test_cluster.execute_transaction(tx).await;

    // Check that we can get the pending validator from 0x5.
    test_cluster.fullnode_handle.iota_node.with(|node| {
        let system_state = node
            .state()
            .get_iota_system_state_object_for_testing()
            .unwrap();
        let pending_active_validators = system_state
            .get_pending_active_validators(node.state().get_object_store().as_ref())
            .unwrap();
        assert_eq!(pending_active_validators.len(), pending_active_count + 1);
        assert_eq!(
            pending_active_validators[pending_active_validators.len() - 1].iota_address,
            address
        );
    });
}

#[sim_test]
async fn get_stakes_with_new_validator() {
    // Create the keypair for the new validator candidate
    let new_validator = ValidatorGenesisConfigBuilder::new().build(&mut OsRng);
    let address = (&new_validator.account_key_pair.public()).into();

    let mut test_cluster = TestClusterBuilder::new()
        .with_validator_candidates([address])
        .with_num_validators(4)
        .build()
        .await;

    let client = test_cluster.rpc_client().clone();

    assert_eq!(test_cluster.committee().epoch, 0);

    let stakes = client.get_stakes(address).await.unwrap();
    assert!(stakes.is_empty());

    // 1. Add validator as candidate
    // 2. Stake the required tokens
    // 3. Submit staking transaction
    // 4. Request to join validator set
    execute_add_validator_transactions(&test_cluster, &new_validator).await;

    // We just added the validator, it's not active yet, it will be on epoch change.
    let stakes = test_cluster.rpc_client().get_stakes(address).await.unwrap();
    assert!(matches!(stakes[0].stakes[0].status, StakeStatus::Pending));

    test_cluster.force_new_epoch().await;

    assert_eq!(test_cluster.committee().epoch, 1);

    // Check that a new validator has joined the committee.
    test_cluster.fullnode_handle.iota_node.with(|node| {
        assert_eq!(
            node.state()
                .epoch_store_for_testing()
                .committee()
                .num_members(),
            5
        );
    });

    // after epoch change the new validator is active and part of the committee
    let stakes = client.get_stakes(address).await.unwrap();
    assert!(matches!(
        stakes[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: 0
        }
    ));

    // Starts the validator node process
    let new_validator_handle = test_cluster.spawn_new_validator(new_validator).await;
    test_cluster.wait_for_epoch_all_nodes(1).await;

    new_validator_handle.with(|node| {
        assert!(
            node.state()
                .is_validator(&node.state().epoch_store_for_testing())
        );
    });

    test_cluster.force_new_epoch().await;

    let stakes = client.get_stakes(address).await.unwrap();
    assert!(matches!(stakes[0].stakes[0].status, StakeStatus::Active {
        estimated_reward
    } if estimated_reward > 0));
}

#[sim_test]
async fn test_staking() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects: ObjectsPage = http_client
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
        .await?;
    assert_eq!(5, objects.data.len());

    let validator = http_client
        .get_latest_iota_system_state()
        .await?
        .active_validators[0]
        .iota_address;

    let coin = objects.data[0].object()?.object_id;
    // Delegate some IOTA
    let transaction_bytes: TransactionBlockBytes = http_client
        .request_add_stake(
            address,
            vec![coin],
            Some(1000000000.into()),
            validator,
            None,
            100_000_000.into(),
        )
        .await?;
    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);

    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    // Check DelegatedStake object
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Pending
    ));
    let staked_iota_copy = http_client
        .get_stakes_by_ids(vec![staked_iota[0].stakes[0].staked_iota_id])
        .await?;
    assert_eq!(
        staked_iota[0].stakes[0].staked_iota_id,
        staked_iota_copy[0].stakes[0].staked_iota_id
    );

    cluster.force_new_epoch().await;

    // Check DelegatedStake object after epoch transition
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    Ok(())
}

#[sim_test]
#[ignore = "https://github.com/iotaledger/iota/issues/5085"]
async fn test_unstaking() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins: CoinPage = http_client.get_coins(address, None, None, None).await?;
    assert_eq!(5, coins.data.len());

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert!(staked_iota.is_empty());

    let validator = http_client
        .get_latest_iota_system_state()
        .await?
        .active_validators[0]
        .iota_address;

    // Delegate some IOTA
    for i in 0..3 {
        let transaction_bytes: TransactionBlockBytes = http_client
            .request_add_stake(
                address,
                vec![coins.data[i].coin_object_id],
                Some(1000000000.into()),
                validator,
                None,
                100_000_000.into(),
            )
            .await?;
        let tx = cluster
            .wallet
            .sign_transaction(&transaction_bytes.to_data()?);

        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        http_client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await?;
    }

    cluster.force_new_epoch().await;

    // Check DelegatedStake object
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    assert!(matches!(
        staked_iota[0].stakes[1].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    assert!(matches!(
        staked_iota[0].stakes[2].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    let transaction_bytes: TransactionBlockBytes = http_client
        .request_withdraw_stake(
            address,
            staked_iota[0].stakes[2].staked_iota_id,
            None,
            100_000_000.into(),
        )
        .await?;
    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);

    let _ = cluster.wallet.execute_transaction_must_succeed(tx).await;

    cluster.force_new_epoch().await;

    let staked_iota_copy: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota_copy.len());
    assert_eq!(2, staked_iota_copy[0].stakes.len());

    assert!(matches!(
        staked_iota_copy[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    assert!(matches!(
        staked_iota_copy[0].stakes[1].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    let unstake = http_client
        .get_stakes_by_ids(vec![staked_iota[0].stakes[2].staked_iota_id])
        .await?;

    assert_eq!(1, unstake.len());

    assert!(matches!(unstake[0].stakes[0].status, StakeStatus::Unstaked));

    Ok(())
}

#[sim_test]
async fn test_timelocked_staking() -> Result<(), anyhow::Error> {
    // Create a cluster
    let (address, keypair) = deterministic_random_account_key();

    let principal = 100_000_000_000;
    let expiration_timestamp_ms = u64::MAX;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock_iota = {
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            OBJECT_START_VERSION,
            TimeLock::<iota_types::balance::Balance>::new(
                UID::new(ObjectID::random()),
                iota_types::balance::Balance::new(principal),
                expiration_timestamp_ms,
                label.clone(),
            )
            .to_bcs_bytes(),
            &ProtocolConfig::get_for_min_version(),
        )
        .unwrap()
    };
    let timelock_iota = ObjectInner {
        owner: Owner::AddressOwner(address),
        data: Data::Move(timelock_iota),
        previous_transaction: TransactionDigest::genesis_marker(),
        storage_rebate: 0,
    };

    let cluster = TestClusterBuilder::new()
        .with_accounts(
            [AccountConfig {
                address: Some(address),
                gas_amounts: [100_000_000].into(),
            }]
            .into(),
        )
        .with_objects([timelock_iota.into()])
        .build()
        .await;

    // Check owned objects
    let http_client = cluster.rpc_client();

    let objects: ObjectsPage = http_client
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
        .await?;
    assert_eq!(2, objects.data.len());

    let (coin, timelock): (Vec<_>, Vec<_>) = objects
        .data
        .into_iter()
        .partition(|o| o.data.as_ref().unwrap().is_gas_coin());
    let coin = coin[0].object()?.object_id;
    let timelocked_balance = timelock[0].object()?.object_id;

    // Check TimelockedStakedIota object before test
    let staked_iota: Vec<DelegatedTimelockedStake> =
        http_client.get_timelocked_stakes(address).await?;
    assert!(staked_iota.is_empty());

    // Delegate some timelocked IOTA
    let validator = http_client
        .get_latest_iota_system_state()
        .await?
        .active_validators[0]
        .iota_address;

    let transaction_bytes: TransactionBlockBytes = http_client
        .request_add_timelocked_stake(
            address,
            timelocked_balance,
            validator,
            coin,
            100_000_000.into(),
        )
        .await?;

    let signed_transaction = to_sender_signed_transaction(transaction_bytes.to_data()?, &keypair);

    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    // Check DelegatedTimelockedStake object
    let staked_iota: Vec<DelegatedTimelockedStake> =
        http_client.get_timelocked_stakes(address).await?;

    assert_eq!(1, staked_iota.len());
    let staked_iota = &staked_iota[0];
    assert_eq!(1, staked_iota.stakes.len());
    let stake = &staked_iota.stakes[0];

    assert_eq!(validator, staked_iota.validator_address);
    assert_eq!(principal, stake.principal);
    assert!(matches!(stake.status, StakeStatus::Pending));
    assert_eq!(expiration_timestamp_ms, stake.expiration_timestamp_ms);
    assert_eq!(label, stake.label);

    // Request the DelegatedTimelockedStake one more time
    let staked_iota_copy = http_client
        .get_timelocked_stakes_by_ids(vec![stake.timelocked_staked_iota_id])
        .await?;

    assert_eq!(1, staked_iota_copy.len());
    let staked_iota_copy = &staked_iota_copy[0];
    assert_eq!(1, staked_iota_copy.stakes.len());
    let stake_copy = &staked_iota_copy.stakes[0];

    // Check both of objects
    assert_eq!(
        staked_iota.validator_address,
        staked_iota_copy.validator_address
    );
    assert_eq!(staked_iota.staking_pool, staked_iota_copy.staking_pool);
    assert_eq!(
        stake.timelocked_staked_iota_id,
        stake_copy.timelocked_staked_iota_id
    );
    assert_eq!(stake.stake_request_epoch, stake_copy.stake_request_epoch);
    assert_eq!(stake.stake_active_epoch, stake_copy.stake_active_epoch);
    assert_eq!(stake.principal, stake_copy.principal);
    assert!(matches!(stake_copy.status, StakeStatus::Pending));
    assert_eq!(
        stake.expiration_timestamp_ms,
        stake_copy.expiration_timestamp_ms
    );
    assert_eq!(stake.label, stake_copy.label);

    Ok(())
}

#[sim_test]
async fn test_timelocked_unstaking() -> Result<(), anyhow::Error> {
    // Create a cluster
    let (address, keypair) = deterministic_random_account_key();

    let principal = 100_000_000_000;
    let expiration_timestamp_ms = u64::MAX;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock_iota = {
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            OBJECT_START_VERSION,
            TimeLock::<iota_types::balance::Balance>::new(
                UID::new(ObjectID::random()),
                iota_types::balance::Balance::new(principal),
                expiration_timestamp_ms,
                label.clone(),
            )
            .to_bcs_bytes(),
            &ProtocolConfig::get_for_min_version(),
        )
        .unwrap()
    };
    let timelock_iota = ObjectInner {
        owner: Owner::AddressOwner(address),
        data: Data::Move(timelock_iota),
        previous_transaction: TransactionDigest::genesis_marker(),
        storage_rebate: 0,
    };

    let cluster = TestClusterBuilder::new()
        .with_accounts(
            [AccountConfig {
                address: Some(address),
                gas_amounts: [500_000_000].into(),
            }]
            .into(),
        )
        .with_objects([timelock_iota.into()])
        .build()
        .await;

    // Check owned objects
    let http_client = cluster.rpc_client();

    let objects: ObjectsPage = http_client
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
        .await?;
    assert_eq!(2, objects.data.len());

    let (coin, timelock): (Vec<_>, Vec<_>) = objects
        .data
        .into_iter()
        .partition(|o| o.data.as_ref().unwrap().is_gas_coin());
    let coin = coin[0].object()?.object_id;
    let timelocked_balance = timelock[0].object()?.object_id;

    // Check TimelockedStakedIota object before test
    let staked_iota: Vec<DelegatedTimelockedStake> =
        http_client.get_timelocked_stakes(address).await?;
    assert!(staked_iota.is_empty());

    // Delegate some timelocked IOTA
    let validator = http_client
        .get_latest_iota_system_state()
        .await?
        .active_validators[0]
        .iota_address;

    let transaction_bytes: TransactionBlockBytes = http_client
        .request_add_timelocked_stake(
            address,
            timelocked_balance,
            validator,
            coin,
            100_000_000.into(),
        )
        .await?;

    let signed_transaction = to_sender_signed_transaction(transaction_bytes.to_data()?, &keypair);

    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    cluster.force_new_epoch().await;

    let staked_iota: Vec<DelegatedTimelockedStake> =
        http_client.get_timelocked_stakes(address).await?;

    assert_eq!(1, staked_iota.len());
    let staked_iota = &staked_iota[0];
    assert_eq!(1, staked_iota.stakes.len());
    let stake = &staked_iota.stakes[0];

    assert_eq!(principal, stake.principal);
    assert!(matches!(&stake.status, StakeStatus::Active { .. }));

    // Request withdraw timelocked stake
    let transaction_bytes: TransactionBlockBytes = http_client
        .request_withdraw_timelocked_stake(
            address,
            stake.timelocked_staked_iota_id,
            coin,
            100_000_000.into(),
        )
        .await?;
    let signed_transaction = to_sender_signed_transaction(transaction_bytes.to_data()?, &keypair);

    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    cluster.force_new_epoch().await;

    let staked_iota: Vec<DelegatedTimelockedStake> =
        http_client.get_timelocked_stakes(address).await?;
    assert_eq!(0, staked_iota.len());

    let timed_out_stake = http_client
        .get_timelocked_stakes_by_ids(vec![stake.timelocked_staked_iota_id])
        .await?;

    assert_eq!(1, timed_out_stake.len());

    assert!(matches!(
        timed_out_stake[0].stakes[0].status,
        StakeStatus::Unstaked
    ));

    Ok(())
}

#[sim_test]
async fn get_committee_info() {
    let cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(2000)
        .build()
        .await;

    let client = cluster.rpc_client();

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
    sleep(Duration::from_millis(5000)).await;

    // Test with specified epoch 1
    let response = client.get_committee_info(Some(1.into())).await.unwrap();

    let (epoch_id, validators) = (response.epoch, response.validators);

    assert!(epoch_id == 1);
    assert_eq!(validators.len(), 4);
}

#[sim_test]
async fn get_reference_gas_price() {
    let cluster = TestClusterBuilder::new().build().await;

    let client = cluster.rpc_client();

    let response = client.get_reference_gas_price().await.unwrap();
    assert_eq!(response, 1000.into());
}

#[sim_test]
async fn get_validators_apy() {
    let cluster = TestClusterBuilder::new().build().await;

    let client = cluster.rpc_client();

    let response = client.get_validators_apy().await.unwrap();
    let (apys, epoch) = (response.apys, response.epoch);

    assert_eq!(epoch, 0);
    assert_eq!(apys.len(), 4);
    assert!(apys.iter().any(|apy| apy.apy == 0.0));
}
