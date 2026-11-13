// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use iota_indexer::store::PgIndexerStore;
use iota_json::{IotaJsonValue, call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, QUERY_MAX_RESULT_LIMIT,
    TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Balance, Coin, CoinPage, IotaCoinMetadata, IotaObjectData, IotaObjectDataFilter,
    IotaObjectResponseQuery, IotaSystemStateSummary, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, IotaTypeTag,
    OwnedObjectCursor, StakeStatus, TransactionBlockBytes,
};
use iota_keys::keystore::AccountKeystore;
use iota_sdk_crypto::{Signer, simple::SimpleKeypair};
use iota_sdk_types::{
    Address, Identifier, ObjectId, ObjectReference, StructTag, TypeTag, crypto::SimpleSignature,
};
use iota_types::{
    balance::Supply,
    crypto::{AccountPrivateKey, get_key_pair},
    gas_coin::NANOS_PER_IOTA,
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use test_cluster::TestCluster;
use tokio::sync::OnceCell;

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, indexer_wait_for_object,
    indexer_wait_for_transaction, publish_test_move_package,
    start_test_cluster_with_read_write_indexer,
};

static COMMON_TESTING_ADDR_AND_CUSTOM_COIN_NAME: OnceCell<(Address, SimpleKeypair, String)> =
    OnceCell::const_new();

/// Creates a new address with 5 IOTA coins and 1 custom coin.
async fn create_addr_and_custom_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
) -> (Address, SimpleKeypair, String) {
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
    let keypair = SimpleKeypair::from(key);

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
) -> &'static (Address, SimpleKeypair, String) {
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
        assert_eq!(result_fullnode.has_next_page, result_indexer.has_next_page);
        assert_eq!(
            sorted_by_object_id(&result_fullnode),
            sorted_by_object_id(&result_indexer),
            "both stores must list the same coins"
        );
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

        // Each store orders its coins its own way and only resumes from its
        // own cursors, so each is paged on its own.
        let fullnode = cluster.rpc_client();
        let mut listings = Vec::new();
        for store in [fullnode, client] {
            let all_coins = store.get_coins(*owner, None, None, None).await.unwrap();
            assert!(!all_coins.has_next_page);
            let first_page = store.get_coins(*owner, None, None, Some(3)).await.unwrap();
            assert!(first_page.has_next_page);
            let second_page = store
                .get_coins(*owner, None, first_page.next_cursor, None)
                .await
                .unwrap();
            assert!(!second_page.has_next_page);

            let merged: Vec<_> = first_page
                .data
                .into_iter()
                .chain(second_page.data)
                .collect();
            assert_eq!(all_coins.data, merged, "the pages must cover the listing");
            listings.push(all_coins);
        }
        assert_eq!(
            sorted_by_object_id(&listings[0]),
            sorted_by_object_id(&listings[1]),
            "both stores must list the same coins"
        );
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

        let tested_limit = 2;
        for store in [cluster.rpc_client(), client] {
            let all_coins = store.get_coins(*owner, None, None, None).await.unwrap();
            let limited = store
                .get_coins(*owner, None, None, Some(tested_limit))
                .await
                .unwrap();
            assert_eq!(limited.data.len(), tested_limit);
            assert_eq!(
                limited.data,
                all_coins.data[..tested_limit],
                "a limited page must be the start of the store's listing"
            );
        }
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
        assert_same_coin_page(&result_fullnode, &result_indexer);
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
        let second_page_results: iota_json_rpc_types::CoinPage = client
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
        let coins_dump_address = Address::random();

        // first call is to make node and potentially the indexer cache the result
        // and increase chance of producing wrong result on the second call
        get_all_balances_fullnode_indexer(cluster, client, owner).await;

        transfer_all_coins(cluster, client, store, owner, &keypair, coins_dump_address).await;

        let (mut result_fullnode, mut result_indexer) =
            get_all_balances_fullnode_indexer(cluster, client, owner).await;

        result_fullnode.sort_by_key(|balance: &Balance| balance.coin_type.clone());
        result_indexer.sort_by_key(|balance: &Balance| balance.coin_type.clone());

        assert_eq!(result_fullnode, result_indexer);
        assert!(
            result_indexer
                .iter()
                .all(|balance| balance.coin_type != "0x2::iota::IOTA"),
            "no IOTA balance should remain after sending away all IOTA coins: {result_indexer:?}"
        );
    });
}

#[test]
fn get_coins_filters_by_coin_type() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let unknown_type = client
            .get_coins(*owner, Some("0x2::iota::TestCoin".into()), None, None)
            .await
            .unwrap();
        assert!(unknown_type.data.is_empty());
        assert!(!unknown_type.has_next_page);

        let default_type = client.get_coins(*owner, None, None, None).await.unwrap();
        let iota_type = client
            .get_coins(*owner, Some("0x2::iota::IOTA".into()), None, None)
            .await
            .unwrap();
        assert_eq!(iota_type.data.len(), 5);
        assert!(!iota_type.has_next_page);
        assert_eq!(
            default_type.data, iota_type.data,
            "the coin type defaults to IOTA"
        );
    });
}

#[test]
fn get_all_coins_limit_zero() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, _, _) = get_or_init_addr_and_custom_coins(cluster, client).await;

        let all_coins = client.get_all_coins(*owner, None, Some(0)).await.unwrap();

        assert!(
            !all_coins.data.is_empty(),
            "a zero limit means the maximum page size, not an empty page"
        );
        assert!(all_coins.data.len() <= *QUERY_MAX_RESULT_LIMIT);
    });
}

/// A cursor carries its own position, so a page resumes after it even when the
/// coin it named has been destroyed, not merely transferred away.
#[test]
fn get_all_coins_with_a_cursor_whose_object_is_gone() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        store,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (owner, keypair) = fund_new_address(cluster, client, 5, NANOS_PER_IOTA).await;

        let all = client.get_all_coins(owner, None, None).await.unwrap();
        assert_eq!(all.data.len(), 5);
        let cursor_coin = all.data[0].coin_object_id;
        let next_coin = all.data[1].coin_object_id;
        // Merging into `next_coin` would change its own row too, so use a later one.
        let primary_coin = all.data[2].coin_object_id;
        let gas_coin = all.data[4].coin_object_id;

        let first = client.get_all_coins(owner, None, Some(1)).await.unwrap();
        let cursor = first.next_cursor.expect("a page follows the first coin");

        let merge = client
            .merge_coin(
                owner,
                primary_coin,
                cursor_coin,
                Some(gas_coin),
                10_000_000.into(),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer_checkpoint(client, store, merge, &keypair).await;

        let resumed = client
            .get_all_coins(owner, Some(cursor), None)
            .await
            .expect("a cursor whose coin is gone still names a position");
        let ids: Vec<_> = resumed.data.iter().map(|c| c.coin_object_id).collect();
        assert!(
            !ids.contains(&cursor_coin),
            "the destroyed cursor coin must not come back",
        );
        assert!(
            ids.contains(&next_coin),
            "the row after the cursor must not be skipped with it: {ids:?}",
        );
    });
}

#[test]
fn request_add_stake_with_multiple_coins() {
    let ApiTestSetup {
        runtime,
        client,
        cluster,
        store,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let coin_amount = NANOS_PER_IOTA;
        let stake_amount = NANOS_PER_IOTA;
        let (owner, keypair) = fund_new_address(cluster, client, 5, coin_amount).await;

        let coins = client.get_coins(owner, None, None, None).await.unwrap();
        assert_eq!(coins.data.len(), 5);
        assert!(client.get_stakes(owner).await.unwrap().is_empty());

        let validator = match client.get_latest_iota_system_state_v2().await.unwrap() {
            IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
            IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
            _ => unimplemented!(
                "a new IotaSystemStateSummary enum variant was added and needs to be handled"
            ),
        };

        let transaction_bytes: TransactionBlockBytes = client
            .request_add_stake(
                owner,
                vec![
                    coins.data[0].coin_object_id,
                    coins.data[1].coin_object_id,
                    coins.data[2].coin_object_id,
                ],
                Some(stake_amount.into()),
                validator,
                Some(coins.data[3].coin_object_id),
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx = to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &keypair);
        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

        let dry_run_response = client
            .dry_run_transaction_block(tx_bytes.clone())
            .await
            .unwrap();
        let executed_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(
                    IotaTransactionBlockResponseOptions::new()
                        .with_balance_changes()
                        .with_input(),
                ),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            dry_run_response.balance_changes,
            executed_response.balance_changes.unwrap(),
            "the dry run must predict the balance changes"
        );
        assert_eq!(
            dry_run_response.input,
            executed_response.transaction.unwrap().data,
            "the dry run must report the executed transaction's input"
        );
        indexer_wait_for_transaction(executed_response.digest, store, client).await;

        let stakes = client.get_stakes(owner).await.unwrap();
        assert_eq!(stakes.len(), 1);
        assert_eq!(stakes[0].stakes[0].principal, stake_amount);
        assert!(matches!(stakes[0].stakes[0].status, StakeStatus::Pending));

        // The staked coins are merged into one, and the rest returned to the
        // sender.
        let coins = client.get_coins(owner, None, None, None).await.unwrap();
        assert_eq!(coins.data.len(), 3);
        assert!(
            coins
                .data
                .iter()
                .any(|coin| coin.balance == 3 * coin_amount - stake_amount),
            "the remainder of the staked coins must come back as one coin: {coins:?}"
        );
    });
}

/// Funds a new address with `count` IOTA coins of `amount` each, and waits for
/// the indexer to see them.
async fn fund_new_address(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    count: usize,
    amount: u64,
) -> (Address, SimpleKeypair) {
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
    for _ in 0..count {
        let coin_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(amount),
                address,
            )
            .await;
        indexer_wait_for_object(indexer_client, coin_ref.object_id, coin_ref.version).await;
    }
    (address, SimpleKeypair::from(key))
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

#[tokio::test]
async fn indexer_get_coin_metadata_with_migrated_coin_manager_coins() {
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
}

#[tokio::test]
async fn get_coin_metadata_with_native_coin_manager_coins() {
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

#[tokio::test]
async fn indexer_get_total_supply_with_migrated_coin_manager_coins() {
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
}

#[tokio::test]
async fn get_total_supply_with_native_coin_manager_coins() {
    // disable full node pruning: node needs historical object versions to assemble
    // the checkpoint contents the indexer needs to ingest.
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("get_total_supply_with_native_coin_manager_coins"),
        Some(Box::new(|builder| builder.disable_fullnode_pruning())),
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

/// The coins of `page` in object-id order, for comparing two stores that order
/// their coins differently.
fn sorted_by_object_id(page: &CoinPage) -> Vec<&Coin> {
    page.data
        .iter()
        .sorted_by_key(|coin| coin.coin_object_id)
        .collect()
}

/// Compares two pages of the same coins; of the cursors, only the object id is
/// compared, as the full node's also carries a position in its owner index,
/// and only with a next page, as only the indexer returns one on the last.
fn assert_same_coin_page(fullnode: &CoinPage, indexer: &CoinPage) {
    assert_eq!(fullnode.data, indexer.data);
    assert_eq!(fullnode.has_next_page, indexer.has_next_page);
    if fullnode.has_next_page {
        assert_eq!(
            fullnode.next_cursor.map(|cursor| cursor.object_id()),
            indexer.next_cursor.map(|cursor| cursor.object_id()),
            "both cursors must name the same coin",
        );
    }
}

async fn get_coins_fullnode_indexer(
    cluster: &TestCluster,
    client: &HttpClient,
    owner: Address,
    coin_type: Option<String>,
    cursor: Option<OwnedObjectCursor>,
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
    owner: Address,
    cursor: Option<OwnedObjectCursor>,
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
    owner: Address,
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
    owner: Address,
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
        .map(Into::into)
        .ok();
    let result_indexer = client
        .get_total_supply(coin_type)
        .await
        .map(Into::into)
        .ok();
    (result_fullnode, result_indexer)
}

async fn create_trusted_coins(
    cluster: &TestCluster,
    address: Address,
    account_keypair: &SimpleKeypair,
) -> Result<(String, String), anyhow::Error> {
    let http_client = cluster.rpc_client();

    let (package_object_ref, _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "dummy_modules_publish",
    )
    .await?;

    let coin_name = format!(
        "{}::trusted_coin::TRUSTED_COIN",
        package_object_ref.object_id
    );
    let imm_coin_name = format!(
        "{}::immutable_metadata_trusted_coin::IMMUTABLE_METADATA_TRUSTED_COIN",
        package_object_ref.object_id
    );

    Ok((coin_name, imm_coin_name))
}

pub async fn execute_move_call(
    client: &HttpClient,
    address: Address,
    account_keypair: &impl Signer<SimpleSignature>,
    package_object_id: ObjectId,
    module: String,
    function: String,
    type_arguments: Vec<IotaTypeTag>,
    arguments: Vec<IotaJsonValue>,
    gas: Option<ObjectId>,
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
                    .with_events()
                    .with_object_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution.into()),
        )
        .await
        .unwrap())
}

async fn mint_trusted_coin(
    cluster: &TestCluster,
    coin_name: String,
    address: Address,
    account_keypair: &SimpleKeypair,
    amount: u64,
) -> Result<ObjectReference, anyhow::Error> {
    let http_client = cluster.rpc_client();

    let result: Supply = http_client
        .get_total_supply(coin_name.clone())
        .await
        .unwrap()
        .into();
    assert_eq!(0, result.value);

    let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
    let treasury_cap_type = StructTag::new_treasury_cap(coin_type);
    let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
        .await
        .object_id;

    let tx_response = execute_move_call(
        http_client,
        address,
        account_keypair,
        ObjectId::FRAMEWORK,
        Identifier::COIN_MODULE.to_string(),
        "mint_and_transfer".into(),
        type_args![coin_name.clone()].unwrap(),
        call_args![treasury_cap, amount, address].unwrap(),
        None,
    )
    .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    let created_coin_obj_ref = tx_response.effects.unwrap().created()[0].reference;

    Ok(created_coin_obj_ref)
}

async fn create_migrated_coin_manager_coins(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    pg_store: &PgIndexerStore,
    address: Address,
    account_keypair: &SimpleKeypair,
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
    let (package_object_ref, _) = publish_test_move_package(
        http_client,
        address,
        account_keypair,
        "migrate_to_coin_manager",
    )
    .await?;

    {
        let coin_type = parse_iota_struct_tag(&coin_name).unwrap();
        let treasury_cap_type = StructTag::new_treasury_cap(coin_type.clone());
        let treasury_cap = get_single_owned_object_by_type(http_client, address, treasury_cap_type)
            .await
            .object_id;

        let coin_metadata_type = StructTag::new_coin_metadata(coin_type.clone());
        let coin_metadata =
            get_single_owned_object_by_type(http_client, address, coin_metadata_type)
                .await
                .object_id;

        let guardian_type = StructTag::new(
            package_object_ref.object_id,
            Identifier::from_static("coin_manager_coin"),
            Identifier::from_static("Guardian"),
            vec![TypeTag::Struct(Box::new(StructTag::new(
                package_object_ref.object_id,
                Identifier::from_static("coin_manager_coin"),
                Identifier::from_static("COIN_MANAGER_COIN"),
                vec![],
            )))],
        );
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_coin_name = format!(
            "{}::coin_manager_coin::COIN_MANAGER_COIN",
            package_object_ref.object_id
        );
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_object_ref.object_id,
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
        let treasury_cap_type = StructTag::new_treasury_cap(imm_coin_type.clone());
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

        let guardian_type = StructTag::new(
            package_object_ref.object_id,
            Identifier::from_static("immutable_metadata_coin_manager_coin"),
            Identifier::from_static("Guardian"),
            vec![TypeTag::Struct(Box::new(StructTag::new(
                package_object_ref.object_id,
                Identifier::from_static("immutable_metadata_coin_manager_coin"),
                Identifier::from_static("IMMUTABLE_METADATA_COIN_MANAGER_COIN"),
                vec![],
            )))],
        );
        let guardian = get_single_owned_object_by_type(http_client, address, guardian_type)
            .await
            .object_id;

        let coin_manager_immutable_metadata_coin_name = format!(
            "{}::immutable_metadata_coin_manager_coin::IMMUTABLE_METADATA_COIN_MANAGER_COIN",
            package_object_ref.object_id
        );
        let tx_response = execute_move_call(
            http_client,
            address,
            account_keypair,
            package_object_ref.object_id,
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
            imm_coin_type.address().into(),
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
    address: Address,
    account_keypair: &SimpleKeypair,
) -> Result<(String, String), anyhow::Error> {
    let http_client = cluster.rpc_client();

    let (package_object_ref, tx_response) =
        publish_test_move_package(http_client, address, account_keypair, "coin_manager_coins")
            .await?;
    indexer_wait_for_transaction(tx_response.digest, pg_store, indexer_client).await;

    let coin_name = format!("{}::normal_coin::NORMAL_COIN", package_object_ref.object_id);
    let immutable_metadata_coin_name = format!(
        "{}::immutable_metadata_coin::IMMUTABLE_METADATA_COIN",
        package_object_ref.object_id
    );
    Ok((coin_name, immutable_metadata_coin_name))
}

async fn get_single_owned_object_by_type(
    http_client: &HttpClient,
    address: Address,
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
    from_address: Address,
    keypair: &SimpleKeypair,
    to_address: Address,
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
