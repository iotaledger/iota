// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use fastcrypto::traits::Signer;
use iota_indexer::store::PgIndexerStore;
use iota_json::{IotaJsonValue, call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, IndexerApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Balance, CoinPage, IotaCoinMetadata, IotaObjectData, IotaObjectDataFilter, IotaObjectRef,
    IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, IotaTypeTag, TransactionBlockBytes,
};
use iota_keys::keystore::AccountKeystore;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, TypeTag,
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
    coin::{COIN_MODULE_NAME, CoinMetadata, TreasuryCap},
    crypto::{AccountKeyPair, IotaKeyPair, Signature, get_key_pair},
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use test_cluster::TestCluster;
use tokio::sync::OnceCell;

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, indexer_wait_for_object,
    indexer_wait_for_transaction, publish_test_move_package,
    start_test_cluster_with_read_write_indexer,
};

static COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME: OnceCell<(IotaAddress, IotaKeyPair, String)> =
    OnceCell::const_new();

/// Creates a new address with 5 IOTA coins and 1 custom coin.
async fn create_addr_and_custom_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
) -> (IotaAddress, IotaKeyPair, String) {
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

    let coin_object_ref = mint_trusted_coin(cluster, coin_name.clone(), address, &keypair, 100_000)
        .await
        .unwrap();

    indexer_wait_for_object(
        indexer_client,
        coin_object_ref.object_id,
        coin_object_ref.version,
    )
    .await;

    (address, keypair, coin_name)
}

/// Returns a shared, cached address with 5 IOTA coins and 1 custom coin.
/// The address is initialized once and reused across multiple tests.
/// WARNING: Tests using this function should NOT modify the address state.
async fn get_or_init_addr_and_custom_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
) -> &'static (IotaAddress, IotaKeyPair, String) {
    COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME
        .get_or_init(|| async { create_addr_and_custom_coins(cluster, indexer_client).await })
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
        let (owner, keypair, _) = create_addr_and_custom_coins(cluster, client).await;
        let coins_dump_address = IotaAddress::random_for_testing_only();

        // first call is to make node and potentially the indexer cache the result
        // and increase chance of producing wrong result on the second call
        get_all_balances_fullnode_indexer(cluster, client, owner).await;

        transfer_all_coins(cluster, client, store, owner, &keypair, coins_dump_address).await;

        let (mut result_fullnode, mut result_indexer) =
            get_all_balances_fullnode_indexer(cluster, client, owner).await;

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
        let (address, address_kp, _) = create_addr_and_custom_coins(cluster, client).await;
        let (coin_name, immutable_metadata_coin_name) =
            create_migrated_coin_manager_coins(cluster, client, store, address, &address_kp)
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

async fn get_coin_metadata_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    coin_type: String,
) -> (Option<IotaCoinMetadata>, Option<IotaCoinMetadata>) {
    let result_fullnode = cluster
        .rpc_client()
        .get_coin_metadata(coin_type.clone())
        .await
        .unwrap();
    let result_indexer = client.get_coin_metadata(coin_type).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn get_total_supply_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    coin_type: String,
) -> (Option<Supply>, Option<Supply>) {
    let result_fullnode = cluster
        .rpc_client()
        .get_total_supply(coin_type.clone())
        .await
        .ok();
    let result_indexer = client.get_total_supply(coin_type).await.ok();
    (result_fullnode, result_indexer)
}

async fn create_trusted_coins(
    cluster: &TestCluster,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
) -> Result<(String, String), anyhow::Error> {
    let http_client = cluster.rpc_client();

    let ((package_id, _, _), _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "dummy_modules_publish",
    )
    .await?;

    let coin_name = format!("{package_id}::trusted_coin::TRUSTED_COIN");
    let imm_coin_name =
        format!("{package_id}::immutable_metadata_trusted_coin::IMMUTABLE_METADATA_TRUSTED_COIN");

    Ok((coin_name, imm_coin_name))
}

pub async fn execute_move_call(
    client: &HttpClient,
    address: IotaAddress,
    account_keypair: &dyn Signer<Signature>,
    package_object_id: ObjectID,
    module: String,
    function: String,
    type_arguments: Vec<IotaTypeTag>,
    arguments: Vec<IotaJsonValue>,
    gas: Option<ObjectID>,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let transaction_bytes: TransactionBlockBytes = client
        .move_call(
            address,
            package_object_id,
            module,
            function,
            type_arguments,
            arguments,
            gas,
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    Ok(client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_events(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap())
}

async fn mint_trusted_coin(
    cluster: &TestCluster,
    coin_name: String,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
    amount: u64,
) -> Result<IotaObjectRef, anyhow::Error> {
    let http_client = cluster.rpc_client();

    let result: Supply = http_client
        .get_total_supply(coin_name.clone())
        .await
        .unwrap();
    assert_eq!(0, result.value);

    let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
    let treasury_cap_type = TreasuryCap::type_(coin_type);
    let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
        .await
        .object_id;

    let tx_response = execute_move_call(
        http_client,
        address,
        account_keypair,
        IOTA_FRAMEWORK_ADDRESS.into(),
        COIN_MODULE_NAME.to_string(),
        "mint_and_transfer".into(),
        type_args![coin_name.clone()].unwrap(),
        call_args![treasury_cap, amount, address].unwrap(),
        None,
    )
    .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let created_coin_obj_ref = tx_response.effects.unwrap().created()[0].reference.clone();

    Ok(created_coin_obj_ref)
}

async fn create_migrated_coin_manager_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    pg_store: &PgIndexerStore,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
) -> Result<(String, String), anyhow::Error> {
    let (coin_name, immutable_metadata_coin_name) =
        create_trusted_coins(cluster, address, account_keypair).await?;
    mint_trusted_coin(
        cluster,
        coin_name.clone(),
        address,
        account_keypair,
        100_000,
    )
    .await
    .unwrap();

    let http_client = cluster.rpc_client();
    let ((package_id, _, _), _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "migrate_to_coin_manager",
    )
    .await?;

    {
        let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
        let treasury_cap_type = TreasuryCap::type_(coin_type.clone());
        let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
            .await
            .object_id;

        let coin_metadata_type = CoinMetadata::type_(coin_type.clone());
        let coin_metadata =
            get_single_owned_object_by_type(http_client, address, coin_metadata_type)
                .await
                .object_id;

        let guardian_type = StructTag {
            address: *package_id,
            module: Identifier::new("coin_manager_coin").unwrap(),
            name: Identifier::new("Guardian").unwrap(),
            type_params: vec![TypeTag::Struct(Box::new(StructTag {
                address: *package_id,
                module: Identifier::new("coin_manager_coin").unwrap(),
                name: Identifier::new("COIN_MANAGER_COIN").unwrap(),
                type_params: vec![],
            }))],
        };
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_coin_name = format!("{package_id}::coin_manager_coin::COIN_MANAGER_COIN");
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_id,
            "coin_manager_coin".to_string(),
            "migrate_to_manager".into(),
            type_args![coin_name, coin_manager_coin_name].unwrap(),
            call_args![guardian, treasury_cap, coin_metadata].unwrap(),
            None,
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;
    }

    {
        let imm_coin_type = parse_iota_struct_tag(&immutable_metadata_coin_name).unwrap();
        let treasury_cap_type = TreasuryCap::type_(imm_coin_type.clone());
        let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
            .await
            .object_id;

        let coin_metadata = http_client
            .get_coin_metadata(immutable_metadata_coin_name.clone())
            .await
            .unwrap()
            .unwrap()
            .id
            .unwrap();

        let guardian_type = StructTag {
            address: *package_id,
            module: Identifier::new("immutable_metadata_coin_manager_coin").unwrap(),
            name: Identifier::new("Guardian").unwrap(),
            type_params: vec![TypeTag::Struct(Box::new(StructTag {
                address: *package_id,
                module: Identifier::new("immutable_metadata_coin_manager_coin").unwrap(),
                name: Identifier::new("IMMUTABLE_METADATA_COIN_MANAGER_COIN").unwrap(),
                type_params: vec![],
            }))],
        };
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_immutable_metadata_coin_name = format!(
            "{package_id}::immutable_metadata_coin_manager_coin::IMMUTABLE_METADATA_COIN_MANAGER_COIN"
        );
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_id,
            "immutable_metadata_coin_manager_coin".to_string(),
            "migrate_to_manager".into(),
            type_args![
                immutable_metadata_coin_name,
                coin_manager_immutable_metadata_coin_name
            ]
            .unwrap(),
            call_args![guardian, treasury_cap, coin_metadata].unwrap(),
            None,
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;

        // Hide metadata of immutable coin, so that Node/Indexer can not use it.
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            imm_coin_type.address.into(),
            "immutable_metadata_trusted_coin".to_string(),
            "hide_metadata".into(),
            type_args![immutable_metadata_coin_name].unwrap(),
            call_args![coin_metadata].unwrap(),
            None,
        )
        .await?;
        assert_eq!(tx_response.status_ok(), Some(true));
        indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;
    }

    Ok((coin_name, immutable_metadata_coin_name))
}

async fn create_native_coin_manager_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    pg_store: &PgIndexerStore,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
) -> Result<(String, String), anyhow::Error> {
    let http_client = cluster.rpc_client();

    let ((package_id, _, _), tx_response) =
        publish_test_move_package(http_client, address, account_keypair, "coin_manager_coins")
            .await?;
    indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;

    let coin_name = format!("{package_id}::normal_coin::NORMAL_COIN");
    let immutable_metadata_coin_name =
        format!("{package_id}::immutable_metadata_coin::IMMUTABLE_METADATA_COIN");
    Ok((coin_name, immutable_metadata_coin_name))
}

async fn get_single_owned_object_by_type(
    http_client: &HttpClient,
    address: IotaAddress,
    struct_tag: StructTag,
) -> IotaObjectData {
    http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(struct_tag)),
                None,
            )),
            None,
            None,
        )
        .await
        .unwrap()
        .data
        .pop()
        .unwrap()
        .data
        .unwrap()
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

    execute_tx_and_wait_for_indexer_checkpoint(indexer_client, store, tx_bytes, keypair).await;
}
