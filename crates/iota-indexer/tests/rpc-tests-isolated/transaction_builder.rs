// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[path = "../common/mod.rs"]
mod common;

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod transaction_builder_tests_isolated {
    use iota_indexer::store::PgIndexerStore;
    use iota_json_rpc_api::{
        CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, TransactionBuilderClient,
    };
    use iota_json_rpc_types::{
        IotaObjectDataOptions, IotaObjectResponseQuery, ObjectsPage, StakeStatus,
        TransactionBlockBytes,
    };
    use iota_protocol_config::ProtocolConfig;
    use iota_swarm_config::genesis_config::AccountConfig;
    use iota_types::{
        base_types::{IotaAddress, MoveObjectType, ObjectID},
        crypto::{AccountKeyPair, get_key_pair},
        digests::TransactionDigest,
        id::UID,
        iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
        object::{Data, MoveObject, OBJECT_START_VERSION, ObjectInner, Owner},
        timelock::{
            label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
            timelock::TimeLock,
        },
    };
    use jsonrpsee::http_client::HttpClient;
    use test_cluster::TestCluster;

    use crate::common::{
        execute_tx_and_wait_for_indexer, indexer_wait_for_checkpoint,
        indexer_wait_for_latest_checkpoint, indexer_wait_for_object,
        start_test_cluster_with_read_write_indexer,
    };

    const FUNDED_BALANCE_PER_COIN: u64 = 10_000_000_000;

    #[tokio::test]
    async fn request_add_stake() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("transaction_builder_request_add_stake"),
            None,
            None,
        )
        .await;
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let coins = create_coins_and_wait_for_indexer(cluster, client, address, 4).await;
        let gas = coins[3];
        let coins_to_stake = coins[..3].to_vec();
        let validator = get_validator(client).await;
        // subtracting some amount to see if it is possible to stake smaller amount than
        // is provided in the input coins
        let stake_amount = FUNDED_BALANCE_PER_COIN * 3 - 10_000;

        let tx_bytes: TransactionBlockBytes = client
            .request_add_stake(
                address,
                coins_to_stake,
                Some(stake_amount.into()),
                validator,
                Some(gas),
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let staked_iota = client.get_stakes(address).await.unwrap();

        assert_eq!(1, staked_iota.len());
        let staked_iota = &staked_iota[0];
        assert_eq!(validator, staked_iota.validator_address);

        assert_eq!(1, staked_iota.stakes.len());
        let stake = &staked_iota.stakes[0];
        assert!(matches!(stake.status, StakeStatus::Pending));
        assert_eq!(stake.principal, stake_amount);

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;
        let staked_iota = client.get_stakes(address).await.unwrap();
        let stake = &staked_iota[0].stakes[0];
        assert!(matches!(stake.status, StakeStatus::Active { .. }));
    }

    #[tokio::test]
    async fn request_withdraw_stake_from_active() {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("transaction_builder_request_withdraw_stake_from_active"),
            None,
            None,
        )
        .await;
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let coins = create_coins_and_wait_for_indexer(cluster, client, address, 4).await;
        let gas = coins[3];
        let coins_to_stake = coins[..3].to_vec();
        let validator = get_validator(client).await;
        // subtracting some amount to see if it is possible to stake smaller amount than
        // is provided in the input coins
        let stake_amount = FUNDED_BALANCE_PER_COIN * 3 - 10_000;

        let tx_bytes: TransactionBlockBytes = client
            .request_add_stake(
                address,
                coins_to_stake,
                Some(stake_amount.into()),
                validator,
                Some(gas),
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(store, cluster).await;
        let staked_iota = client.get_stakes(address).await.unwrap();
        let stake = &staked_iota[0].stakes[0];
        assert!(matches!(stake.status, StakeStatus::Active { .. }));

        let tx_bytes: TransactionBlockBytes = client
            .request_withdraw_stake(address, stake.staked_iota_id, Some(gas), 100_000_000.into())
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(client, cluster, store, tx_bytes, &keypair).await;

        let staked_iota = client.get_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());
    }

    #[tokio::test]
    async fn request_add_timelocked_stake() {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let (cluster, store, client, timelocked_balance) = create_cluster_with_timelocked_iota(
            address,
            "transaction_builder_request_add_timelocked_stake",
        )
        .await;
        indexer_wait_for_checkpoint(&store, 1).await;

        let coin = get_gas_object_id(&client, address).await;
        let validator = get_validator(&client).await;

        let tx_bytes: TransactionBlockBytes = client
            .request_add_timelocked_stake(
                address,
                timelocked_balance,
                validator,
                coin,
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(&client, &cluster, &store, tx_bytes, &keypair).await;

        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();

        assert_eq!(1, staked_iota.len());
        let staked_iota = &staked_iota[0];
        assert_eq!(validator, staked_iota.validator_address);

        assert_eq!(1, staked_iota.stakes.len());
        let stake = &staked_iota.stakes[0];
        assert!(matches!(stake.status, StakeStatus::Pending));

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;
        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
        let stake = &staked_iota[0].stakes[0];
        assert!(matches!(stake.status, StakeStatus::Active { .. }));
    }

    #[tokio::test]
    async fn request_withdraw_timelocked_stake_from_active() {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let (cluster, store, client, timelocked_balance) = create_cluster_with_timelocked_iota(
            address,
            "transaction_builder_request_withdraw_timelocked_stake_from_active",
        )
        .await;
        indexer_wait_for_checkpoint(&store, 1).await;

        let coin = get_gas_object_id(&client, address).await;
        let validator = get_validator(&client).await;

        let tx_bytes: TransactionBlockBytes = client
            .request_add_timelocked_stake(
                address,
                timelocked_balance,
                validator,
                coin,
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(&client, &cluster, &store, tx_bytes, &keypair).await;

        cluster.force_new_epoch().await;
        indexer_wait_for_latest_checkpoint(&store, &cluster).await;
        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
        let stake = &staked_iota[0].stakes[0];
        assert!(matches!(stake.status, StakeStatus::Active { .. }));

        let tx_bytes: TransactionBlockBytes = client
            .request_withdraw_timelocked_stake(
                address,
                stake.timelocked_staked_iota_id,
                coin,
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(&client, &cluster, &store, tx_bytes, &keypair).await;

        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());
    }

    #[tokio::test]
    async fn request_withdraw_timelocked_stake_from_pending() {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let (cluster, store, client, timelocked_balance) = create_cluster_with_timelocked_iota(
            address,
            "transaction_builder_request_withdraw_timelocked_stake_from_pending",
        )
        .await;
        indexer_wait_for_checkpoint(&store, 1).await;

        let coin = get_gas_object_id(&client, address).await;
        let validator = get_validator(&client).await;

        let tx_bytes: TransactionBlockBytes = client
            .request_add_timelocked_stake(
                address,
                timelocked_balance,
                validator,
                coin,
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(&client, &cluster, &store, tx_bytes, &keypair).await;

        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
        let stake = &staked_iota[0].stakes[0];
        assert!(matches!(stake.status, StakeStatus::Pending));

        let tx_bytes: TransactionBlockBytes = client
            .request_withdraw_timelocked_stake(
                address,
                stake.timelocked_staked_iota_id,
                coin,
                100_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer(&client, &cluster, &store, tx_bytes, &keypair).await;

        let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
        assert!(staked_iota.is_empty());
    }

    async fn get_validator(client: &HttpClient) -> IotaAddress {
        let iota_system_state = client.get_latest_iota_system_state_v2().await.unwrap();
        match iota_system_state {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => panic!("unsupported IotaSystemStateSummary"),
        }
    }

    async fn create_coins_and_wait_for_indexer(
        cluster: &TestCluster,
        indexer_client: &HttpClient,
        address: IotaAddress,
        objects_count: u32,
    ) -> Vec<ObjectID> {
        let mut coins: Vec<ObjectID> = Vec::new();
        for _ in 0..objects_count {
            let coin = cluster
                .fund_address_and_return_gas(
                    cluster.get_reference_gas_price().await,
                    Some(FUNDED_BALANCE_PER_COIN),
                    address,
                )
                .await;
            indexer_wait_for_object(indexer_client, coin.0, coin.1).await;
            coins.push(coin.0);
        }
        coins
    }

    async fn create_cluster_with_timelocked_iota(
        address: IotaAddress,
        indexer_db_name: &str,
    ) -> (TestCluster, PgIndexerStore, HttpClient, ObjectID) {
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

        let (cluster, store, client) = start_test_cluster_with_read_write_indexer(
            Some(indexer_db_name),
            Some(Box::new(move |builder| {
                builder
                    .with_accounts(
                        [AccountConfig {
                            address: Some(address),
                            gas_amounts: [1_000_000_000].into(),
                        }]
                        .into(),
                    )
                    .with_objects([timelock_iota.into()])
            })),
            None,
        )
        .await;

        let fullnode_client = cluster.rpc_client();

        let objects: ObjectsPage = fullnode_client
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new_with_options(
                    IotaObjectDataOptions::full_content(),
                )),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(2, objects.data.len());

        let timelocked_balance = objects
            .data
            .into_iter()
            .find(|o| !o.data.as_ref().unwrap().is_gas_coin())
            .unwrap()
            .object()
            .unwrap()
            .object_id;

        (cluster, store, client, timelocked_balance)
    }

    async fn get_gas_object_id(client: &HttpClient, address: IotaAddress) -> ObjectID {
        client
            .get_coins(address, None, None, None)
            .await
            .unwrap()
            .data[0]
            .coin_object_id
    }
}
