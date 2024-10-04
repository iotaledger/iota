// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

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
use iota_swarm_config::genesis_config::AccountConfig;
use iota_types::{
    base_types::{MoveObjectType, ObjectID},
    crypto::deterministic_random_account_key,
    digests::TransactionDigest,
    id::UID,
    object::{Data, MoveObject, ObjectInner, Owner, OBJECT_START_VERSION},
    quorum_driver_types::ExecuteTransactionRequestType,
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    utils::to_sender_signed_transaction,
};
use test_cluster::TestClusterBuilder;
use tokio::time::sleep;

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

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert!(staked_iota.is_empty());

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
    Ok(())
}

#[ignore]
#[sim_test]
async fn test_unstaking() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(10000)
        .build()
        .await;

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
    // Check DelegatedStake object
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);

    sleep(Duration::from_millis(10000)).await;

    let staked_iota_copy = http_client
        .get_stakes_by_ids(vec![
            staked_iota[0].stakes[0].staked_iota_id,
            staked_iota[0].stakes[1].staked_iota_id,
            staked_iota[0].stakes[2].staked_iota_id,
        ])
        .await?;

    assert!(matches!(
        &staked_iota_copy[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
    assert!(matches!(
        &staked_iota_copy[0].stakes[1].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
    assert!(matches!(
        &staked_iota_copy[0].stakes[2].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));

    let transaction_bytes: TransactionBlockBytes = http_client
        .request_withdraw_stake(
            address,
            staked_iota_copy[0].stakes[2].staked_iota_id,
            None,
            1_000_000.into(),
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

    sleep(Duration::from_millis(20000)).await;

    let staked_iota_copy = http_client
        .get_stakes_by_ids(vec![
            staked_iota[0].stakes[0].staked_iota_id,
            staked_iota[0].stakes[1].staked_iota_id,
            staked_iota[0].stakes[2].staked_iota_id,
        ])
        .await?;

    assert!(matches!(
        &staked_iota_copy[0].stakes[0].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
    assert!(matches!(
        &staked_iota_copy[0].stakes[1].status,
        StakeStatus::Active {
            estimated_reward: _
        }
    ));
    assert!(matches!(
        &staked_iota_copy[0].stakes[2].status,
        StakeStatus::Unstaked
    ));
    Ok(())
}

#[sim_test]
async fn test_timelocked_staking() -> Result<(), anyhow::Error> {
    // Create a cluster
    let (address, keypair) = deterministic_random_account_key();

    let principal = 100_000_000_000;
    let expiration_timestamp_ms = u64::MAX;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock_iota = unsafe {
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            false,
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

    let timelock_iota = unsafe {
        MoveObject::new_from_execution(
            MoveObjectType::timelocked_iota_balance(),
            false,
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
        .with_epoch_duration_ms(10000)
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

    // Sleep for 10 seconds
    sleep(Duration::from_millis(10000)).await;

    // Request the DelegatedTimelockedStake one more time
    let staked_iota_copy = http_client
        .get_timelocked_stakes_by_ids(vec![stake.timelocked_staked_iota_id])
        .await?;

    assert_eq!(1, staked_iota_copy.len());
    let staked_iota_copy = &staked_iota_copy[0];
    assert_eq!(1, staked_iota_copy.stakes.len());
    let stake_copy = &staked_iota_copy.stakes[0];

    assert_eq!(principal, stake_copy.principal);
    assert!(matches!(&stake_copy.status, StakeStatus::Active { .. }));

    // Request withdraw timelocked stake
    let transaction_bytes: TransactionBlockBytes = http_client
        .request_withdraw_timelocked_stake(
            address,
            stake_copy.timelocked_staked_iota_id,
            coin,
            10_000_000.into(),
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

    // Sleep for 20 seconds
    sleep(Duration::from_millis(20000)).await;

    // Request the DelegatedTimelockedStake one more time
    let staked_iota_copy = http_client
        .get_timelocked_stakes_by_ids(vec![stake.timelocked_staked_iota_id])
        .await?;

    assert_eq!(1, staked_iota_copy.len());
    let staked_iota_copy = &staked_iota_copy[0];
    assert_eq!(1, staked_iota_copy.stakes.len());
    let stake_copy = &staked_iota_copy.stakes[0];

    // Check the result
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
    assert!(matches!(stake_copy.status, StakeStatus::Unstaked));
    assert_eq!(
        stake.expiration_timestamp_ms,
        stake_copy.expiration_timestamp_ms
    );
    assert_eq!(stake.label, stake_copy.label);

    Ok(())
}
