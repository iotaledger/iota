// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use iota_json::{call_args, type_args};
use iota_json_rpc::authority_state::StateRead;
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, TransactionBuilderClient,
    WriteApiClient,
};
use iota_json_rpc_types::{
    Balance, CoinPage, DelegatedStake, IotaCoinMetadata, IotaExecutionStatus,
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange, StakeStatus,
    TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_sdk::wallet_context::WalletContext;
use iota_swarm_config::genesis_config::{DEFAULT_GAS_AMOUNT, DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT};
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
    coin::{COIN_MODULE_NAME, TreasuryCap},
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
};
use jsonrpsee::http_client::HttpClient;
use test_cluster::{TestCluster, TestClusterBuilder};

async fn create_and_mint_coins(
    http_client: &HttpClient,
    address: IotaAddress,
    wallet: &WalletContext,
    amount: u64,
) -> Result<String, anyhow::Error> {
    let coins = http_client
        .get_coins(address, None, None, Some(1))
        .await
        .unwrap()
        .data;
    let gas = &coins[0];

    // Publish test coin package
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "dummy_modules_publish"]);
    let compiled_package = BuildConfig::default().build(&path).unwrap();
    let with_unpublished_deps = false;
    let compiled_modules_bytes = compiled_package.get_package_base64(with_unpublished_deps);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

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

    let tx = wallet.sign_transaction(&transaction_bytes.to_data().unwrap());
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

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

    let coin_name = format!(
        "{}::trusted_coin::TRUSTED_COIN",
        package_id.to_hex_literal()
    );
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

    let tx = wallet.sign_transaction(&transaction_bytes.to_data().unwrap());
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

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

    assert_eq!(IotaExecutionStatus::Success, *effects.unwrap().status());

    Ok(coin_name)
}

#[sim_test]
async fn get_coins() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let result: CoinPage = http_client.get_coins(address, None, None, None).await?;
    assert_eq!(5, result.data.len());
    assert!(!result.has_next_page);

    let result: CoinPage = http_client
        .get_coins(address, Some("0x2::iota::TestCoin".into()), None, None)
        .await?;
    assert_eq!(0, result.data.len());

    let result: CoinPage = http_client
        .get_coins(address, Some("0x2::iota::IOTA".into()), None, None)
        .await?;
    assert_eq!(5, result.data.len());
    assert!(!result.has_next_page);

    // Test paging
    let result: CoinPage = http_client
        .get_coins(address, Some("0x2::iota::IOTA".into()), None, Some(3))
        .await?;
    assert_eq!(3, result.data.len());
    assert!(result.has_next_page);

    let result: CoinPage = http_client
        .get_coins(
            address,
            Some("0x2::iota::IOTA".into()),
            result.next_cursor,
            Some(3),
        )
        .await?;
    assert_eq!(2, result.data.len(), "{:?}", result);
    assert!(!result.has_next_page);

    let result: CoinPage = http_client
        .get_coins(
            address,
            Some("0x2::iota::IOTA".into()),
            result.next_cursor,
            None,
        )
        .await?;
    assert_eq!(0, result.data.len(), "{:?}", result);
    assert!(!result.has_next_page);

    Ok(())
}

#[sim_test]
async fn get_balance() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let result: Balance = http_client.get_balance(address, None).await?;
    assert_eq!("0x2::iota::IOTA", result.coin_type);
    assert_eq!(
        (DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT as u64 * DEFAULT_GAS_AMOUNT) as u128,
        result.total_balance
    );
    assert_eq!(
        DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT,
        result.coin_object_count
    );
    Ok(())
}

#[sim_test]
async fn get_metadata() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();

    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
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
        .await?
        .data;

    let gas = objects.first().unwrap().object().unwrap();

    // Publish test coin package
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", "dummy_modules_publish"]);
    let compiled_package = BuildConfig::new_for_testing().build(&path)?;
    let compiled_modules_bytes =
        compiled_package.get_package_base64(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let transaction_bytes: TransactionBlockBytes = http_client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            Some(gas.object_id),
            100_000_000.into(),
        )
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response = http_client
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
        .await?;

    let object_changes = tx_response.object_changes.unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|e| {
            if let ObjectChange::Published { package_id, .. } = e {
                Some(package_id)
            } else {
                None
            }
        })
        .unwrap();

    let result: IotaCoinMetadata = http_client
        .get_coin_metadata(format!("{package_id}::trusted_coin::TRUSTED_COIN"))
        .await?
        .unwrap();

    assert_eq!("TRUSTED", result.symbol);
    assert_eq!("Trusted Coin for test", result.description);
    assert_eq!("Trusted Coin", result.name);
    assert_eq!(2, result.decimals);

    Ok(())
}

#[sim_test]
async fn get_total_supply() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();

    let total_amount = 100000;
    let coin_name = create_and_mint_coins(
        http_client,
        cluster.get_address_0(),
        &cluster.wallet,
        total_amount,
    )
    .await
    .unwrap();

    let result: Supply = http_client.get_total_supply(coin_name).await?;
    assert_eq!(total_amount, result.value);

    Ok(())
}

#[sim_test]
async fn staking_multiple_coins() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins: CoinPage = http_client.get_coins(address, None, None, None).await?;
    assert_eq!(5, coins.data.len());

    let genesis_coin_amount = coins.data[0].balance;

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert!(staked_iota.is_empty());

    let iota_system_state = http_client.get_latest_iota_system_state_v2().await?;
    let validator = match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => panic!("unsupported IotaSystemStateSummary"),
    };

    // Delegate some IOTA
    let transaction_bytes: TransactionBlockBytes = http_client
        .request_add_stake(
            address,
            vec![
                coins.data[0].coin_object_id,
                coins.data[1].coin_object_id,
                coins.data[2].coin_object_id,
            ],
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

    let dryrun_response = http_client
        .dry_run_transaction_block(tx_bytes.clone())
        .await?;

    let executed_response = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_balance_changes()
                    .with_input(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    // Check coin balance changes on dry run
    assert_eq!(
        dryrun_response.balance_changes,
        executed_response.balance_changes.unwrap()
    );

    // Check that inputs for dry run match the executed transaction
    assert_eq!(
        dryrun_response.input,
        executed_response.transaction.unwrap().data
    );

    // Check DelegatedStake object
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert_eq!(1, staked_iota.len());
    assert_eq!(1000000000, staked_iota[0].stakes[0].principal);
    assert!(matches!(
        staked_iota[0].stakes[0].status,
        StakeStatus::Pending
    ));

    // Coins should be merged into one and returned to the sender.
    let coins: CoinPage = http_client.get_coins(address, None, None, None).await?;
    assert_eq!(3, coins.data.len());

    // Find the new coin
    let new_coin = coins
        .data
        .iter()
        .find(|coin| coin.balance > genesis_coin_amount)
        .unwrap();
    assert_eq!((genesis_coin_amount * 3) - 1000000000, new_coin.balance);

    Ok(())
}

#[sim_test]
async fn get_all_coins() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let rpc_all_coins: CoinPage = http_client
        .get_all_coins(address, None, None)
        .await
        .unwrap();

    assert!(!rpc_all_coins.data.is_empty(), "Should have some coins");
    assert!(
        !rpc_all_coins.has_next_page,
        "Should not have next page initially"
    );

    let fullnode_coins = cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            let coin_cursor = (String::from_utf8([0u8].to_vec()).unwrap(), ObjectID::ZERO);
            node.state()
                .get_owned_coins(address, coin_cursor, 100, false)
        })
        .unwrap();

    assert_eq!(rpc_all_coins.data.len(), fullnode_coins.len());
    assert_eq!(fullnode_coins, rpc_all_coins.data);
}

#[sim_test]
async fn get_all_coins_with_multiple_coin_types() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let total_amount = 100000;
    let coin_name = create_and_mint_coins(http_client, address, &cluster.wallet, total_amount)
        .await
        .unwrap();

    let rpc_all_coins: CoinPage = http_client
        .get_all_coins(address, None, None)
        .await
        .unwrap();

    assert!(
        rpc_all_coins
            .data
            .iter()
            .any(|coin| coin.coin_type.contains("0x2::iota::IOTA"))
    );
    assert!(
        rpc_all_coins
            .data
            .iter()
            .any(|coin| coin.coin_type.contains(&coin_name))
    );

    assert!(!rpc_all_coins.data.is_empty(), "Should have some coins");
    assert!(
        !rpc_all_coins.has_next_page,
        "Should not have next page initially"
    );

    let fullnode_coins = cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            let coin_cursor = (String::from_utf8([0u8].to_vec()).unwrap(), ObjectID::ZERO);
            node.state()
                .get_owned_coins(address, coin_cursor, 100, false)
        })
        .unwrap();

    assert_eq!(rpc_all_coins.data.len(), fullnode_coins.len());
    assert_eq!(fullnode_coins, rpc_all_coins.data);
}

#[sim_test]
async fn get_all_coins_with_limit() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let result_with_limit: CoinPage = http_client
        .get_all_coins(address, None, Some(2))
        .await
        .unwrap();
    assert_eq!(2, result_with_limit.data.len(), "Should return 2 coins");
    assert!(result_with_limit.has_next_page, "Should have next page");

    let next_page: CoinPage = http_client
        .get_all_coins(address, result_with_limit.next_cursor, Some(2))
        .await
        .unwrap();
    assert!(!next_page.data.is_empty(), "Next page should have coins");

    // Ensure no duplicate coins between pages
    let first_page_ids = result_with_limit
        .data
        .iter()
        .map(|c| &c.coin_object_id)
        .collect::<Vec<&ObjectID>>();
    let second_page_ids: Vec<_> = next_page
        .data
        .iter()
        .map(|c| &c.coin_object_id)
        .collect::<Vec<&ObjectID>>();

    assert!(
        first_page_ids
            .iter()
            .all(|id| !second_page_ids.contains(id)),
        "No coin should appear in both pages"
    );
}

#[sim_test]
async fn get_all_coins_with_cursor_and_limit() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let rpc_all_coins: CoinPage = http_client
        .get_all_coins(address, None, None)
        .await
        .unwrap();
    assert!(!rpc_all_coins.data.is_empty(), "Should have some coins");
    assert!(
        !rpc_all_coins.has_next_page,
        "Should not have next page when fetching all"
    );

    let mut collected_coins = Vec::new();
    let mut cursor = None;
    loop {
        let page: CoinPage = http_client
            .get_all_coins(address, cursor, Some(2))
            .await
            .unwrap();
        collected_coins.extend(page.data);
        if !page.has_next_page {
            break;
        }
        cursor = page.next_cursor;
    }

    assert_eq!(
        rpc_all_coins.data.len(),
        collected_coins.len(),
        "Paginated results should match total coins"
    );
    assert_eq!(rpc_all_coins.data, collected_coins);
}

#[sim_test]
async fn get_all_coins_with_cursor_boundaries() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let full_result: CoinPage = http_client
        .get_all_coins(address, None, None)
        .await
        .unwrap();

    assert!(!full_result.data.is_empty());

    let last_coin = full_result.data.last().unwrap();
    let result: CoinPage = http_client
        .get_all_coins(address, Some(last_coin.coin_object_id), None)
        .await
        .unwrap();
    assert!(
        result.data.is_empty(),
        "Should return no coins when cursor is at the last coin"
    );

    let first_coin = full_result.data.first().unwrap();
    let result: CoinPage = http_client
        .get_all_coins(address, Some(first_coin.coin_object_id), None)
        .await
        .unwrap();
    assert_eq!(
        full_result.data.len() - 1,
        result.data.len(),
        "Should return all coins except the first one"
    );
}

#[sim_test]
async fn get_all_coins_invalid_cursor() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let invalid_cursor_result = http_client
        .get_all_coins(address, Some(ObjectID::ZERO), None)
        .await;

    assert!(
        invalid_cursor_result.is_err(),
        "Should error with invalid cursor"
    );
}

// This test case depends om the test execution model. The test pass when all
// tests are executed with `nextest` or `simtest` because of the one test per
// process design.
//
// When using `cargo test` it might fail because of the
// `QUERY_MAX_RESULT_LIMIT` is initialized upon first access (as a Singleton).
// This behavior complicates testing, as subsequent tests that rely on this data
// will be affected by the initial test's configuration. Moreover, some test
// cases may need different values for `QUERY_MAX_RESULT_LIMIT`, further
// complicating the testing process.

#[sim_test]
async fn get_all_coins_limit_zero_with_env_var() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    std::env::set_var("RPC_QUERY_MAX_RESULT_LIMIT", "0");

    let rpc_all_coins = http_client
        .get_all_coins(address, None, Some(0))
        .await
        .unwrap();

    assert!(rpc_all_coins.data.is_empty());
}

#[sim_test]
async fn get_all_coins_limit_zero() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let rpc_all_coins = http_client
        // It will default to DEFAULT_RPC_QUERY_MAX_RESULT_LIMIT=50 if "RPC_QUERY_MAX_RESULT_LIMIT"
        // env var is not set
        .get_all_coins(address, None, Some(0))
        .await
        .unwrap();

    assert!(!rpc_all_coins.data.is_empty());
    assert!(rpc_all_coins.data.len() <= 50);
}

#[sim_test]
async fn get_all_balances() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    // Get all balances
    let balances: Vec<Balance> = http_client.get_all_balances(address).await.unwrap();
    assert!(!balances.is_empty(), "Should have some balances");

    // Check if IOTA balance exists and is correct
    let iota_balance = balances
        .iter()
        .find(|b| b.coin_type == "0x2::iota::IOTA")
        .expect("IOTA balance should exist");

    assert_eq!(
        (DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT as u64 * DEFAULT_GAS_AMOUNT) as u128,
        iota_balance.total_balance,
        "IOTA balance should match expected amount"
    );

    let total_amount = 100000;
    let coin_name = create_and_mint_coins(http_client, address, &cluster.wallet, total_amount)
        .await
        .unwrap();

    let updated_balances: Vec<Balance> = http_client.get_all_balances(address).await.unwrap();
    assert!(
        updated_balances.len() > balances.len(),
        "Should have more balance entries after minting new coin"
    );

    let new_coin_balance = updated_balances
        .iter()
        .find(|b| b.coin_type.contains(&coin_name))
        .expect("New coin balance should exist");

    assert_eq!(
        total_amount as u128, new_coin_balance.total_balance,
        "New coin should have correct balance"
    );
}

#[sim_test]
async fn get_all_balances_after_zeroing_coins_count() {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let other_address = cluster.get_address_1();

    // Get all balances
    let balances: Vec<Balance> = http_client.get_all_balances(address).await.unwrap();
    assert!(!balances.is_empty(), "Should have some balances");

    // Check if IOTA balance exists
    let iota_balance = balances
        .iter()
        .find(|b| b.coin_type == "0x2::iota::IOTA")
        .expect("IOTA balance should exist");

    assert!(
        iota_balance.coin_object_count > 0,
        "There should be more than 0 IOTA coins"
    );

    transfer_all_coins(&cluster, http_client, address, other_address)
        .await
        .unwrap();

    let updated_balances: Vec<Balance> = http_client.get_all_balances(address).await.unwrap();
    assert!(
        updated_balances.is_empty(),
        "Should have no balances after sending away all IOTA coins"
    );
}

async fn transfer_all_coins(
    cluster: &TestCluster,
    http_client: &HttpClient,
    from_address: IotaAddress,
    to_address: IotaAddress,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let coins = http_client
        .get_coins(from_address, None, None, None)
        .await
        .unwrap()
        .data
        .iter()
        .map(|coin| coin.coin_object_id)
        .collect();

    let transaction_bytes: TransactionBlockBytes = http_client
        .pay_all_iota(from_address, coins, to_address, 10_000_000.into())
        .await?;

    execute_tx(cluster, http_client, transaction_bytes).await
}

async fn execute_tx(
    cluster: &TestCluster,
    http_client: &HttpClient,
    transaction_bytes: TransactionBlockBytes,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_object_changes()
                    .with_balance_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    assert_eq!(tx_response.status_ok(), Some(true));

    Ok(tx_response)
}
