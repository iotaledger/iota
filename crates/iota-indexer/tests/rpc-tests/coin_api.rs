// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use iota_indexer::store::PgIndexerStore;
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{CoinReadApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    Balance, CoinPage, IotaCoinMetadata, IotaExecutionStatus, IotaObjectRef,
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, ObjectChange, TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
    coin::{COIN_MODULE_NAME, TreasuryCap},
    crypto::{AccountKeyPair, get_key_pair},
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use jsonrpsee::http_client::HttpClient;
use test_cluster::TestCluster;
use tokio::sync::OnceCell;

use crate::common::{ApiTestSetup, indexer_wait_for_object};

static COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME: OnceCell<(IotaAddress, String)> =
    OnceCell::const_new();

async fn once_prepare_addr_with_iota_and_custom_coins(
    cluster: &TestCluster,
    store: &PgIndexerStore,
) -> (IotaAddress, String) {
    COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME
        .get_or_init(|| async {
            let (address, keypair): (_, AccountKeyPair) = get_key_pair();

            for _ in 0..5 {
                cluster
                    .fund_address_and_return_gas(
                        cluster.get_reference_gas_price().await,
                        Some(500_000_000),
                        address,
                    )
                    .await;
            }

            let (coin_name, coin_object_ref) =
                create_and_mint_trusted_coin(cluster, address, keypair, 100_000)
                    .await
                    .unwrap();

            indexer_wait_for_object(store, coin_object_ref.object_id, coin_object_ref.version)
                .await;

            (address, coin_name)
        })
        .await
        .clone()
}

#[test]
fn get_coins_basic_scenario() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_coins_fullnode_indexer(cluster, client, owner, None, None, None).await;

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_with_cursor() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;
        let all_coins = cluster
            .rpc_client()
            .get_coins(owner, None, None, None)
            .await
            .unwrap();
        let cursor = all_coins.data[3].coin_object_id; // get some coin from the middle

        let (result_fullnode, result_indexer) =
            call_get_coins_fullnode_indexer(cluster, client, owner, None, Some(cursor), None).await;

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_with_limit() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_coins_fullnode_indexer(cluster, client, owner, None, None, Some(2)).await;

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coins_custom_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, coin_name) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_coins_fullnode_indexer(cluster, client, owner, Some(coin_name), None, None)
                .await;

        assert_eq!(result_indexer.data.len(), 1);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_coins_basic_scenario() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_all_coins_fullnode_indexer(cluster, client, owner, None, None).await;

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/3588"]
#[test]
fn get_all_coins_with_cursor() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        store,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let all_coins = cluster
            .rpc_client()
            .get_coins(owner, None, None, None)
            .await
            .unwrap();
        let cursor = all_coins.data[3].coin_object_id; // get some coin from the middle

        let (result_fullnode_all, result_indexer_all) =
            call_get_all_coins_fullnode_indexer(cluster, client, owner, None, None).await;

        let (result_fullnode, result_indexer) =
            call_get_all_coins_fullnode_indexer(cluster, client, owner, Some(cursor), None).await;

        println!("Fullnode all: {:#?}", result_fullnode_all);
        println!("Indexer all: {:#?}", result_indexer_all);
        println!("Fullnode: {:#?}", result_fullnode);
        println!("Indexer: {:#?}", result_indexer);
        println!("Cursor: {:#?}", cursor);

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_coins_with_limit() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_all_coins_fullnode_indexer(cluster, client, owner, None, Some(2)).await;

        assert!(result_indexer.data.len() > 0);
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_balance_iota_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_balance_fullnode_indexer(cluster, client, owner, None).await;

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_balance_custom_coin() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, coin_name) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_balance_fullnode_indexer(cluster, client, owner, Some(coin_name.to_string()))
                .await;

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_all_balances() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (mut result_fullnode, mut result_indexer) =
            call_get_all_balances_fullnode_indexer(cluster, client, owner).await;

        result_fullnode.sort_by_key(|balance: &Balance| balance.coin_type.clone());
        result_indexer.sort_by_key(|balance: &Balance| balance.coin_type.clone());

        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_coin_metadata() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, coin_name) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_coin_metadata_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert!(result_indexer.is_some());
        assert_eq!(result_fullnode, result_indexer);
    });
}

#[test]
fn get_total_supply() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (_, coin_name) = once_prepare_addr_with_iota_and_custom_coins(cluster, store).await;

        let (result_fullnode, result_indexer) =
            call_get_total_supply_fullnode_indexer(cluster, client, coin_name.to_string()).await;

        assert_eq!(result_fullnode, result_indexer);
    });
}

async fn call_get_coins_fullnode_indexer(
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

async fn call_get_all_coins_fullnode_indexer(
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

async fn call_get_balance_fullnode_indexer(
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

async fn call_get_all_balances_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: IotaAddress,
) -> (Vec<Balance>, Vec<Balance>) {
    let result_fullnode = cluster.rpc_client().get_all_balances(owner).await.unwrap();
    let result_indexer = client.get_all_balances(owner).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn call_get_coin_metadata_fullnode_indexer(
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

async fn call_get_total_supply_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    coin_type: String,
) -> (Supply, Supply) {
    let result_fullnode = cluster
        .rpc_client()
        .get_total_supply(coin_type.clone())
        .await
        .unwrap();
    let result_indexer = client.get_total_supply(coin_type).await.unwrap();
    (result_fullnode, result_indexer)
}

async fn create_and_mint_trusted_coin(
    cluster: &TestCluster,
    address: IotaAddress,
    account_keypair: AccountKeyPair,
    amount: u64,
) -> Result<(String, IotaObjectRef), anyhow::Error> {
    let http_client = cluster.rpc_client();
    let coins = http_client
        .get_coins(address, None, None, Some(1))
        .await
        .unwrap()
        .data;
    let gas = &coins[0];

    // Publish test coin package
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "dummy_modules_publish"]);
    let compiled_package = BuildConfig::default().build(path).unwrap();
    let with_unpublished_deps = false;
    let compiled_modules_bytes = compiled_package.get_package_base64(with_unpublished_deps);
    let dependencies = compiled_package.get_dependency_original_package_ids();

    let transaction_bytes: TransactionBlockBytes = http_client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            Some(gas.coin_object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_object_changes()
                    .with_events(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let object_changes = tx_response.object_changes.as_ref().unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(package_id),
            _ => None,
        })
        .unwrap();

    let coin_name = format!("{package_id}::trusted_coin::TRUSTED_COIN");
    let result: Supply = http_client
        .get_total_supply(coin_name.clone())
        .await
        .unwrap();

    assert_eq!(0, result.value);

    let object_changes = tx_response.object_changes.as_ref().unwrap();
    let treasury_cap = object_changes
        .iter()
        .filter_map(|change| match change {
            ObjectChange::Created {
                object_id,
                object_type,
                ..
            } => Some((object_id, object_type)),
            _ => None,
        })
        .find_map(|(object_id, object_type)| {
            let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
            (&TreasuryCap::type_(coin_type) == object_type).then_some(object_id)
        })
        .unwrap();

    let transaction_bytes: TransactionBlockBytes = http_client
        .move_call(
            address,
            IOTA_FRAMEWORK_ADDRESS.into(),
            COIN_MODULE_NAME.to_string(),
            "mint_and_transfer".into(),
            type_args![coin_name.clone()].unwrap(),
            call_args![treasury_cap, amount, address].unwrap(),
            Some(gas.coin_object_id),
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let tx_response = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let IotaTransactionBlockResponse { effects, .. } = tx_response;

    assert_eq!(
        IotaExecutionStatus::Success,
        *effects.as_ref().unwrap().status()
    );

    let created_coin_obj_ref = effects.unwrap().created()[0].reference.clone();

    Ok((coin_name, created_coin_obj_ref))
}
