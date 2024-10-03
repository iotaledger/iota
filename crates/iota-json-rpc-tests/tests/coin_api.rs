// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
#[cfg(not(msim))]
use std::str::FromStr;

use iota_json::{call_args, type_args};
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
use iota_swarm_config::genesis_config::{DEFAULT_GAS_AMOUNT, DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT};
use iota_types::{
    balance::Supply,
    coin::{TreasuryCap, COIN_MODULE_NAME},
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    IOTA_FRAMEWORK_ADDRESS,
};
use test_cluster::TestClusterBuilder;

#[sim_test]
async fn test_get_coins() -> Result<(), anyhow::Error> {
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
async fn test_get_balance() -> Result<(), anyhow::Error> {
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
async fn test_get_metadata() -> Result<(), anyhow::Error> {
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
async fn test_get_total_supply() -> Result<(), anyhow::Error> {
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
    let compiled_package = BuildConfig::default().build(&path)?;
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
        .await?;

    let object_changes = tx_response.object_changes.as_ref().unwrap();
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

    let coin_name = format!("{package_id}::trusted_coin::TRUSTED_COIN");
    let result: Supply = http_client.get_total_supply(coin_name.clone()).await?;

    assert_eq!(0, result.value);

    let object_changes = tx_response.object_changes.as_ref().unwrap();
    let treasury_cap = object_changes
        .iter()
        .find_map(|e| {
            if let ObjectChange::Created {
                object_id,
                object_type,
                ..
            } = e
            {
                if &TreasuryCap::type_(parse_iota_struct_tag(&coin_name).unwrap()) == object_type {
                    Some(object_id)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();

    // Mint 100000 coin

    let transaction_bytes: TransactionBlockBytes = http_client
        .move_call(
            address,
            IOTA_FRAMEWORK_ADDRESS.into(),
            COIN_MODULE_NAME.to_string(),
            "mint_and_transfer".into(),
            type_args![coin_name]?,
            call_args![treasury_cap, 100000, address]?,
            Some(gas.object_id),
            10_000_000.into(),
            None,
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
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    let IotaTransactionBlockResponse { effects, .. } = tx_response;

    assert_eq!(IotaExecutionStatus::Success, *effects.unwrap().status());

    let result: Supply = http_client.get_total_supply(coin_name.clone()).await?;
    assert_eq!(100000, result.value);

    Ok(())
}

#[sim_test]
async fn test_staking_multiple_coins() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let coins: CoinPage = http_client.get_coins(address, None, None, None).await?;
    assert_eq!(5, coins.data.len());

    let genesis_coin_amount = coins.data[0].balance;

    // Check StakedIota object before test
    let staked_iota: Vec<DelegatedStake> = http_client.get_stakes(address).await?;
    assert!(staked_iota.is_empty());

    let validator = http_client
        .get_latest_iota_system_state()
        .await?
        .active_validators[0]
        .iota_address;
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
