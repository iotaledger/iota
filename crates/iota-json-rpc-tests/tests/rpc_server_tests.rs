// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::str::FromStr;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, ReadApiClient,
    TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Balance, CoinPage, DelegatedStake, DelegatedTimelockedStake, IotaCoinMetadata,
    IotaExecutionStatus, IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, ObjectChange, ObjectsPage, StakeStatus,
    TransactionBlockBytes,
};
use iota_macros::sim_test;
use iota_move_build::BuildConfig;
use iota_protocol_config::ProtocolConfig;
use iota_swarm_config::genesis_config::{
    AccountConfig, DEFAULT_GAS_AMOUNT, DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT,
};
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Supply,
    base_types::{IotaAddress, MoveObjectType, ObjectID, SequenceNumber},
    coin::{COIN_MODULE_NAME, TreasuryCap},
    collection_types::VecMap,
    crypto::deterministic_random_account_key,
    digests::{ObjectDigest, TransactionDigest},
    gas_coin::GAS,
    id::UID,
    object::{Data, MoveObject, OBJECT_START_VERSION, ObjectInner, Owner},
    parse_iota_struct_tag,
    quorum_driver_types::ExecuteTransactionRequestType,
    stardust::output::{Irc27Metadata, Nft},
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    utils::to_sender_signed_transaction,
};
use test_cluster::TestClusterBuilder;
use tokio::time::sleep;

#[sim_test]
async fn test_get_objects() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();

    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new(),
            )),
            None,
            None,
        )
        .await?;
    assert_eq!(5, objects.data.len());

    // Multiget objectIDs test
    let object_digests = objects
        .data
        .iter()
        .map(|o| o.object().unwrap().object_id)
        .collect();

    let object_resp = http_client.multi_get_objects(object_digests, None).await?;
    assert_eq!(5, object_resp.len());
    Ok(())
}

#[tokio::test]
async fn test_get_package_with_display_should_not_fail() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let response = http_client
        .get_object(
            ObjectID::from(IOTA_FRAMEWORK_ADDRESS),
            Some(IotaObjectDataOptions::new().with_display()),
        )
        .await;
    assert!(response.is_ok());
    let response: IotaObjectResponse = response?;
    assert!(
        response
            .into_object()
            .unwrap()
            .display
            .unwrap()
            .data
            .is_none()
    );
    Ok(())
}

#[sim_test]
async fn test_public_transfer_object() -> Result<(), anyhow::Error> {
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

    let obj = objects.clone().first().unwrap().object().unwrap().object_id;
    let gas = objects.clone().last().unwrap().object().unwrap().object_id;

    let transaction_bytes: TransactionBlockBytes = http_client
        .transfer_object(address, obj, Some(gas), 1_000_000.into(), address)
        .await?;

    let tx = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data()?);
    let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
    let tx_bytes1 = tx_bytes.clone();
    let dryrun_response = http_client.dry_run_transaction_block(tx_bytes).await?;

    let tx_response: IotaTransactionBlockResponse = http_client
        .execute_transaction_block(
            tx_bytes1,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_object_changes(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    assert_same_object_changes_ignoring_version_and_digest(
        dryrun_response.object_changes,
        tx_response.object_changes.unwrap(),
    );
    Ok(())
}

fn assert_same_object_changes_ignoring_version_and_digest(
    expected: Vec<ObjectChange>,
    actual: Vec<ObjectChange>,
) {
    fn collect_changes_mask_version_and_digest(
        changes: Vec<ObjectChange>,
    ) -> BTreeMap<ObjectID, ObjectChange> {
        changes
            .into_iter()
            .map(|mut change| {
                let object_id = change.object_id();
                // ignore the version and digest for comparison
                change.mask_for_test(SequenceNumber::MAX, ObjectDigest::MAX);
                (object_id, change)
            })
            .collect()
    }
    let expected = collect_changes_mask_version_and_digest(expected);
    let actual = collect_changes_mask_version_and_digest(actual);
    assert!(expected.keys().all(|id| actual.contains_key(id)));
    assert!(actual.keys().all(|id| expected.contains_key(id)));
    for (id, exp) in &expected {
        let act = actual.get(id).unwrap();
        assert_eq!(act, exp);
    }
}

#[sim_test]
async fn test_publish() -> Result<(), anyhow::Error> {
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
        .await?;
    let gas = objects.data.first().unwrap().object().unwrap();

    let compiled_package =
        BuildConfig::new_for_testing().build(Path::new("../../examples/move/basics"))?;
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
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 6);
    Ok(())
}

#[sim_test]
async fn test_move_call() -> Result<(), anyhow::Error> {
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
    let coin = &objects[1].object()?;

    // now do the call
    let package_id = ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes());
    let module = "pay".to_string();
    let function = "split".to_string();

    let transaction_bytes: TransactionBlockBytes = http_client
        .move_call(
            address,
            package_id,
            module,
            function,
            type_args![GAS::type_tag()]?,
            call_args!(coin.object_id, 10)?,
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
    matches!(tx_response, IotaTransactionBlockResponse {effects, ..} if effects.as_ref().unwrap().created().len() == 1);
    Ok(())
}

#[sim_test]
async fn test_get_object_info() -> Result<(), anyhow::Error> {
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

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let result = http_client
            .get_object(
                oref.object_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
            .await?;
        assert!(
            matches!(result, IotaObjectResponse { data: Some(object), .. } if oref.object_id == object.object_id && object.owner.unwrap().get_owner_address()? == address)
        );
    }
    Ok(())
}

#[sim_test]
async fn test_get_object_data_with_content() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let http_client = cluster.rpc_client();
    let address = cluster.get_address_0();
    let objects = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new_with_options(
                IotaObjectDataOptions::new().with_content().with_owner(),
            )),
            None,
            None,
        )
        .await?
        .data;

    for obj in objects {
        let oref = obj.into_object().unwrap();
        let result = http_client
            .get_object(
                oref.object_id,
                Some(IotaObjectDataOptions::new().with_content().with_owner()),
            )
            .await?;
        assert!(
            matches!(result, IotaObjectResponse { data: Some(object), .. } if oref.object_id == object.object_id && object.owner.unwrap().get_owner_address()? == address)
        );
    }
    Ok(())
}

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

#[sim_test]
async fn test_nft_display_object() -> Result<(), anyhow::Error> {
    // Create a cluster
    let (address, _) = deterministic_random_account_key();

    let nft = Nft {
        id: UID::new(ObjectID::random()),
        legacy_sender: Some(IotaAddress::ZERO),
        metadata: Some(String::from("metadata value").into_bytes()),
        tag: Some(String::from("tag value").into_bytes()),
        immutable_issuer: Some(
            IotaAddress::from_str(
                "0x1000000000000000002000000000000003000000000000040000000000000005",
            )
            .unwrap(),
        ),
        immutable_metadata: Irc27Metadata {
            version: String::from("version value"),
            media_type: String::from("media type value"),
            uri: String::from("url value").try_into().unwrap(),
            name: String::from("name value"),
            collection_name: Some(String::from("collection name value")),
            royalties: VecMap { contents: vec![] },
            issuer_name: Some(String::from("issuer name value")),
            description: Some(String::from("description value")),
            attributes: VecMap { contents: vec![] },
            non_standard_fields: VecMap { contents: vec![] },
        },
    };

    let nft_move_object = unsafe {
        MoveObject::new_from_execution(
            MoveObjectType::stardust_nft(),
            true,
            OBJECT_START_VERSION,
            bcs::to_bytes(&nft).unwrap(),
            &ProtocolConfig::get_for_min_version(),
        )
        .unwrap()
    };
    let nft_object = ObjectInner {
        owner: Owner::AddressOwner(address),
        data: Data::Move(nft_move_object),
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
        .with_objects([nft_object.into()])
        .build()
        .await;

    // Check owned objects
    let http_client = cluster.rpc_client();

    let objects: ObjectsPage = http_client
        .get_owned_objects(
            address,
            Some(IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(Nft::tag())),
                Some(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction()
                        .with_display(),
                ),
            )),
            None,
            None,
        )
        .await?;
    assert_eq!(1, objects.data.len());

    // Check the Nft display
    let nft_display = objects.data[0]
        .data
        .as_ref()
        .unwrap()
        .display
        .as_ref()
        .unwrap()
        .data
        .as_ref()
        .unwrap();

    assert_eq!(8, nft_display.len());

    assert_eq!(nft_display["collection_name"], "collection name value");
    assert_eq!(nft_display["creator"], "issuer name value");
    assert_eq!(nft_display["description"], "description value");
    assert_eq!(nft_display["image_url"], "url value");
    assert_eq!(
        nft_display["immutable_issuer"],
        "0x1000000000000000002000000000000003000000000000040000000000000005"
    );
    assert_eq!(nft_display["media_type"], "media type value");
    assert_eq!(nft_display["name"], "name value");
    assert_eq!(nft_display["version"], "version value");

    Ok(())
}
