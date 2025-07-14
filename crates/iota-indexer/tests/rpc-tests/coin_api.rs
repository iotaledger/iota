// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::store::PgIndexerStore;
use iota_json_rpc_api::{CoinReadApiClient, TransactionBuilderClient};
use iota_json_rpc_types::{Balance, CoinPage, TransactionBlockBytes};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, IotaKeyPair, get_key_pair},
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use test_cluster::TestCluster;
use tokio::sync::OnceCell;

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer, indexer_wait_for_object,
    rpc_tests::{
        create_migrated_coin_manager_coins, create_trusted_coins,
        get_coin_metadata_fullnode_indexer, get_total_supply_fullnode_indexer, mint_trusted_coin,
    },
};

static COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME: OnceCell<(IotaAddress, IotaKeyPair, String)> =
    OnceCell::const_new();

async fn get_or_init_addr_and_custom_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
) -> &'static (IotaAddress, IotaKeyPair, String) {
    COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME
        .get_or_init(|| async {
            let (address, keypair): (_, AccountKeyPair) = get_key_pair();
            let keypair = IotaKeyPair::Ed25519(keypair);

            for _ in 0..5 {
                cluster
                    .fund_address_and_return_gas(
                        cluster.get_reference_gas_price().await,
                        Some(500_000_000),
                        address,
                    )
                    .await;
            }

            let (coin_name, _) = create_trusted_coins(cluster, address, &keypair)
                .await
                .unwrap();

            let coin_object_ref =
                mint_trusted_coin(cluster, coin_name.clone(), address, &keypair, 100_000)
                    .await
                    .unwrap();

            indexer_wait_for_object(
                indexer_client,
                coin_object_ref.object_id,
                coin_object_ref.version,
            )
            .await;

            (address, keypair, coin_name)
        })
        .await
}

#[test]
fn get_coins_basic_scenario() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_coins_fullnode_indexer(cluster, client, *owner, None, None, None).await;

        assert!(!result_indexer.data.is_empty());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_with_cursor() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;
        let all_coins = cluster
            .rpc_client()
            .get_coins(*owner, None, None, None)
            .await
            .unwrap();
        let cursor = all_coins.data[3].coin_object_id; // get some coin from the middle

        let (result_fullnode, result_indexer) =
            get_coins_fullnode_indexer(cluster, client, *owner, None, Some(cursor), None).await;

        assert!(!result_indexer.data.is_empty());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_with_limit() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_coins_fullnode_indexer(cluster, client, *owner, None, None, Some(2)).await;

        assert!(!result_indexer.data.is_empty());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_custom_coin() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) = get_coins_fullnode_indexer(
            cluster,
            client,
            *owner,
            Some(coin_name.clone()),
            None,
            None,
        )
        .await;

        assert_eq!(result_indexer.data.len(), 1);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_coins_basic_scenario() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_all_coins_fullnode_indexer(cluster, client, *owner, None, None).await;

        assert!(!result_indexer.data.is_empty());
        assert_eq!(
            result_fullnode
                .data
                .iter()
                .sorted_by_key(|coin| coin.coin_object_id)
                .collect::<Vec<_>>(),
            result_indexer
                .data
                .iter()
                .sorted_by_key(|coin| coin.coin_object_id)
                .collect::<Vec<_>>()
        );
    });
}

#[test]
fn get_all_coins_with_cursor() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let all_coins = client.get_all_coins(*owner, None, None).await.unwrap();
        assert_eq!(all_coins.data.len(), 6);
        assert!(!all_coins.has_next_page);

        let first_page_results = client.get_all_coins(*owner, None, Some(4)).await.unwrap();
        assert!(first_page_results.has_next_page);
        let second_page_results: iota_json_rpc_types::Page<iota_json_rpc_types::Coin, ObjectID> =
            client
                .get_all_coins(*owner, first_page_results.next_cursor, Some(4))
                .await
                .unwrap();
        assert!(!second_page_results.has_next_page);

        let merged_page_contents: Vec<_> = first_page_results
            .data
            .into_iter()
            .chain(second_page_results.data)
            .collect();
        assert_eq!(all_coins.data, merged_page_contents);
    });
}

#[test]
fn get_all_coins_with_limit() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let all_coins = client.get_all_coins(*owner, None, None).await.unwrap();
        let tested_limit = 2;
        let expected_data = all_coins
            .data
            .into_iter()
            .take(tested_limit)
            .collect::<Vec<_>>();

        let limited_result = client
            .get_all_coins(*owner, None, Some(tested_limit))
            .await
            .unwrap();

        assert_eq!(limited_result.data.len(), tested_limit);
        assert_eq!(expected_data, limited_result.data);
    });
}

#[test]
fn get_balance_iota_coin() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_balance_fullnode_indexer(cluster, client, *owner, None).await;

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_balance_custom_coin() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_balance_fullnode_indexer(cluster, client, *owner, Some(coin_name.to_string()))
                .await;

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_balances() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (mut result_fullnode, mut result_indexer) =
            get_all_balances_fullnode_indexer(cluster, client, *owner).await;

        result_fullnode.sort_by_key(|balance: &Balance| balance.coin_type.clone());
        result_indexer.sort_by_key(|balance: &Balance| balance.coin_type.clone());

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_balances_with_zero_iotas() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        store,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, keypair, _) = get_or_init_addr_and_custom_coins(cluster, client).await;
        let coins_dump_address = IotaAddress::random_for_testing_only();

        // first call is to make node and potentially the indexer cache the result
        // and increase chance of producing wrong result on the second call
        get_all_balances_fullnode_indexer(cluster, client, *owner).await;

        transfer_all_coins(cluster, client, store, *owner, keypair, coins_dump_address).await;

        let (mut result_fullnode, mut result_indexer) =
            get_all_balances_fullnode_indexer(cluster, client, *owner).await;

        result_fullnode.sort_by_key(|balance: &Balance| balance.coin_type.clone());
        result_indexer.sort_by_key(|balance: &Balance| balance.coin_type.clone());

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coin_metadata() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
#[ignore = "https://github.com/iotaledger/iota/issues/7014"]
fn fullnode_get_coin_metadata_with_migrated_coin_manager_coins() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        store,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (address, address_kp, _) = get_or_init_addr_and_custom_coins(cluster, client).await;
        let (coin_name, immutable_metadata_coin_name) =
            create_migrated_coin_manager_coins(cluster, client, store, *address, address_kp)
                .await
                .unwrap();

        let (result_fullnode, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_fullnode.is_some());
        assert_eq!(result_fullnode, result_indexer);

        let (result_fullnode, result_indexer) = get_coin_metadata_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;

        assert!(result_fullnode.is_some());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn indexer_get_coin_metadata_with_migrated_coin_manager_coins() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("indexer_get_coin_metadata_with_migrated_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_migrated_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (_, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        let result_indexer = result_indexer.unwrap();
        assert_eq!(result_indexer.decimals, 2);
        assert_eq!(result_indexer.name, "Trusted Coin");
        assert_eq!(result_indexer.symbol, "TRUSTED");
        assert_eq!(result_indexer.description, "Trusted Coin for test");
        assert_eq!(result_indexer.icon_url, None);
        assert!(result_indexer.id.is_some());

        let (_, result_indexer) = get_coin_metadata_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;

        assert!(result_indexer.is_some());
        let result_indexer = result_indexer.unwrap();
        assert_eq!(result_indexer.decimals, 2);
        assert_eq!(result_indexer.name, "Immutable Metadata Trusted Coin");
        assert_eq!(result_indexer.symbol, "IMM_META_TRUSTED");
        assert_eq!(
            result_indexer.description,
            "Immutable Metadata Trusted Coin for test"
        );
        assert_eq!(result_indexer.icon_url, None);
        assert!(result_indexer.id.is_none()); // Immutable data is stored in struct that doesn't have ID
    });
}

#[test]
fn get_coin_metadata_with_native_coin_manager_coins() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("get_coin_metadata_with_native_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_native_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (result_fullnode, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
        assert!(result_indexer.unwrap().id.is_some());

        let (result_fullnode, result_indexer) = get_coin_metadata_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
        assert!(result_indexer.unwrap().id.is_none()); // Immutable data is stored in struct that doesn't have ID
    });
}

#[test]
fn get_coin_metadata_with_nonexistent_coin() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;
        let nonexistent_coin = format!("{coin_name}_some_suffix");

        let (result_fullnode, result_indexer) =
            get_coin_metadata_fullnode_indexer(cluster, client, nonexistent_coin).await;

        assert!(result_fullnode.is_none());
        assert!(result_indexer.is_none());
    });
}

#[test]
fn get_total_supply() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let (result_fullnode, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn indexer_get_total_supply_with_migrated_coin_manager_coins() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("indexer_get_total_supply_with_migrated_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_migrated_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (_, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;
        assert_eq!(result_indexer, Some(Supply { value: 100_000 }));

        let (_, result_indexer) = get_total_supply_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
    });
}

#[test]
fn get_total_supply_with_native_coin_manager_coins() {
    let ApiTestSetup { runtime, .. } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
            Some("get_total_supply_with_native_coin_manager_coins"),
            None,
            None,
        )
        .await;

        let address = cluster.wallet.active_address().unwrap();
        let address_kp = cluster
            .wallet
            .config()
            .keystore()
            .get_key(&address)
            .unwrap();
        let (coin_name, immutable_metadata_coin_name) = create_native_coin_manager_coins(
            cluster,
            client,
            store,
            address,
            address_kp.as_keypair().unwrap(),
        )
        .await
        .unwrap();

        let (result_fullnode, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
        assert_eq!(result_fullnode, result_indexer);

        let (result_fullnode, result_indexer) = get_total_supply_fullnode_indexer(
            cluster,
            client,
            immutable_metadata_coin_name.to_string(),
        )
        .await;
        assert_eq!(result_indexer, Some(Supply { value: 0 }));
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_total_supply_with_nonexistent_coin() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, _, coin_name) = get_or_init_addr_and_custom_coins(cluster, client).await;
        let nonexistent_coin = format!("{coin_name}_some_suffix");

        let (result_fullnode, result_indexer) =
            get_total_supply_fullnode_indexer(cluster, client, nonexistent_coin).await;

        assert!(result_fullnode.is_none());
        assert!(result_indexer.is_none());
    });
}

async fn get_coins_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: IotaAddress,
    coin_type: Option<String>,
    cursor: Option<ObjectID>,
    limit: Option<usize>,
) -> (CoinPage, CoinPage) {
    let result_fullnode = cluster
        .rpc_client()
        .get_coins(owner, coin_type.clone(), cursor, limit)
        .await
        .unwrap();
    let result_indexer = client
        .get_coins(owner, coin_type, cursor, limit)
        .await
        .unwrap();
    (result_fullnode, result_indexer)
}

async fn get_all_coins_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: IotaAddress,
    cursor: Option<ObjectID>,
    limit: Option<usize>,
) -> (CoinPage, CoinPage) {
    let result_fullnode = cluster
        .rpc_client()
        .get_all_coins(owner, cursor, limit)
        .await
        .unwrap();
    let result_indexer = client.get_all_coins(owner, cursor, limit).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn get_balance_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: IotaAddress,
    coin_type: Option<String>,
) -> (Balance, Balance) {
    let result_fullnode = cluster
        .rpc_client()
        .get_balance(owner, coin_type.clone())
        .await
        .unwrap();
    let result_indexer = client.get_balance(owner, coin_type).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn get_all_balances_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: IotaAddress,
) -> (Vec<Balance>, Vec<Balance>) {
    let result_fullnode = cluster.rpc_client().get_all_balances(owner).await.unwrap();
    let result_indexer = client.get_all_balances(owner).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn transfer_all_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    store: &PgIndexerStore,
    from_address: IotaAddress,
    keypair: &IotaKeyPair,
    to_address: IotaAddress,
) {
    let coins: Vec<_> = cluster
        .rpc_client()
        .get_coins(from_address, None, None, None)
        .await
        .unwrap()
        .data
        .iter()
        .map(|coin| coin.coin_object_id)
        .collect();

    let tx_bytes: TransactionBlockBytes = indexer_client
        .pay_all_iota(from_address, coins, to_address, 10_000_000.into())
        .await
        .unwrap();

    execute_tx_and_wait_for_indexer(indexer_client, cluster, store, tx_bytes, keypair).await;
}
