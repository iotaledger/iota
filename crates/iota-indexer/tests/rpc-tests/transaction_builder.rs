// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    str::FromStr,
};

use iota_indexer::{
    db::{ConnectionPoolConfig, DbUrl, new_connection_pool},
    read::IndexerReader,
    store::PgIndexerStore,
    test_utils::db_url,
};
use iota_json::{call_arg, call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, ReadApiClient,
    TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaArgument, IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, MoveCallParams,
    ObjectChange, ObjectsPage, PtbInput, RPCTransactionRequestParams, StakeStatus,
    TransactionBlockBytes, TransferObjectParams,
};
use iota_move_build::BuildConfig;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_move_types::iota_framework::object::UID;
use iota_sdk_types::{
    Address, MoveStruct, ObjectData, ObjectDigest, ObjectId, Owner, StructTag, TransactionDigest,
    Version,
};
use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT};
use iota_transaction_builder::DataReader;
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    object::{MoveStructExt, OBJECT_START_VERSION, ObjectInner},
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
    utils::to_sender_signed_transaction,
};
use jsonrpsee::http_client::HttpClient;
use test_cluster::TestCluster;

use crate::common::{
    ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, execute_tx_must_succeed,
    indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint, indexer_wait_for_object,
    start_test_cluster_with_read_write_indexer,
};
const FUNDED_BALANCE_PER_COIN: u64 = 10_000_000_000;

#[test]
fn transfer_object() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let receiver = Address::random();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let gas = sender_coins[0];
        let object_to_send = sender_coins[1];

        let tx_bytes = client
            .transfer_object(
                sender,
                object_to_send,
                Some(gas),
                100_000_000.into(),
                receiver,
            )
            .await
            .unwrap();
        let signed_tx =
            to_sender_signed_transaction(tx_bytes.clone().to_data().unwrap(), &sender_key);
        let (unsigned_tx_bytes, _signatures) = signed_tx.to_tx_bytes_and_signatures();
        let dry_run_response = client
            .dry_run_transaction_block(unsigned_tx_bytes)
            .await
            .unwrap();

        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        assert_same_object_changes_ignoring_version_and_digest(
            dry_run_response.object_changes,
            tx_response.object_changes.unwrap(),
        );

        let transferred_object = client
            .get_object(object_to_send, Some(IotaObjectDataOptions::full_content()))
            .await
            .unwrap();

        assert_eq!(transferred_object.owner(), Some(Owner::Address(receiver)));
    });
}

#[test]
fn transfer_iota() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let receiver = Address::random();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 1).await;
        let gas = sender_coins[0];
        let transferred_balance = 100_000;

        let tx_bytes = client
            .transfer_iota(
                sender,
                gas,
                100_000_000.into(),
                receiver,
                Some(transferred_balance.into()),
            )
            .await
            .unwrap();
        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        let transferred = i128::from(transferred_balance);
        assert_eq!(
            balance_change_of(&tx_response, sender),
            -transferred - net_gas_usage(&tx_response)
        );
        assert_eq!(balance_change_of(&tx_response, receiver), transferred);

        let receiver_balances = get_address_balances(client, receiver).await;

        assert_eq!(receiver_balances, [transferred_balance]);
    });
}

#[test]
fn pay() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let receiver_1 = Address::random();
        let receiver_2 = Address::random();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32 + 1)
                .await;
        let total_input_coins_balance = FUNDED_BALANCE_PER_COIN * input_coins;
        let transferred_balance_1 = total_input_coins_balance / 2 - 100;
        let transferred_balance_2 = total_input_coins_balance / 2 - 700;

        let tx_bytes = client
            .pay(
                sender,
                sender_coins[0..input_coins as usize].into(),
                [receiver_1, receiver_2].into(),
                [transferred_balance_1.into(), transferred_balance_2.into()].into(),
                None, // let node find the gas automatically
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        let transferred_1 = i128::from(transferred_balance_1);
        let transferred_2 = i128::from(transferred_balance_2);
        assert_eq!(
            balance_change_of(&tx_response, sender),
            -transferred_1 - transferred_2 - net_gas_usage(&tx_response)
        );
        assert_eq!(balance_change_of(&tx_response, receiver_1), transferred_1);
        assert_eq!(balance_change_of(&tx_response, receiver_2), transferred_2);

        let receiver_1_balances = get_address_balances(client, receiver_1).await;
        let receiver_2_balances = get_address_balances(client, receiver_2).await;

        assert_eq!(receiver_1_balances, [transferred_balance_1]);
        assert_eq!(receiver_2_balances, [transferred_balance_2]);
    });
}

#[test]
fn pay_iota() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let receiver_1 = Address::random();
        let receiver_2 = Address::random();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32).await;
        let gas_budget = 100_000_000;
        let total_available_input_coins_balance: u64 =
            FUNDED_BALANCE_PER_COIN * input_coins - gas_budget;
        let transferred_balance_1 = total_available_input_coins_balance / 2 - 100;
        let transferred_balance_2 = total_available_input_coins_balance / 2 - 700;

        let tx_bytes = client
            .pay_iota(
                sender,
                sender_coins,
                [receiver_1, receiver_2].into(),
                [transferred_balance_1.into(), transferred_balance_2.into()].into(),
                gas_budget.into(),
            )
            .await
            .unwrap();
        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        let transferred_1 = i128::from(transferred_balance_1);
        let transferred_2 = i128::from(transferred_balance_2);
        assert_eq!(
            balance_change_of(&tx_response, sender),
            -transferred_1 - transferred_2 - net_gas_usage(&tx_response)
        );
        assert_eq!(balance_change_of(&tx_response, receiver_1), transferred_1);
        assert_eq!(balance_change_of(&tx_response, receiver_2), transferred_2);

        let receiver_1_balances = get_address_balances(client, receiver_1).await;
        let receiver_2_balances = get_address_balances(client, receiver_2).await;

        assert_eq!(receiver_1_balances, [transferred_balance_1]);
        assert_eq!(receiver_2_balances, [transferred_balance_2]);
    });
}

#[test]
fn pay_all_iota() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let receiver = Address::random();

        let input_coins: u64 = 3;
        let sender_coins =
            create_coins_and_wait_for_indexer(cluster, client, sender, input_coins as u32).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .pay_all_iota(sender, sender_coins, receiver, gas_budget.into())
            .await
            .unwrap();
        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        let total_input_balance = i128::from(FUNDED_BALANCE_PER_COIN * input_coins);
        let expected_receiver_balance = total_input_balance - net_gas_usage(&tx_response);
        assert_eq!(
            balance_change_of(&tx_response, sender),
            -total_input_balance
        );
        assert_eq!(
            balance_change_of(&tx_response, receiver),
            expected_receiver_balance
        );

        let receiver_balances = get_address_balances(client, receiver).await;

        assert_eq!(
            receiver_balances,
            [u64::try_from(expected_receiver_balance).unwrap()]
        );
    });
}

#[test]
fn move_call() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

            let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
            let gas = sender_coins[0];

            let tx_bytes = client
                .move_call(
                    sender,
                    ObjectId::FRAMEWORK,
                    "coin".to_string(),
                    "join".to_string(),
                    type_args![TypeTag::from(StructTag::new_gas())].unwrap(),
                    call_args!(sender_coins[1], sender_coins[2]).unwrap(),
                    Some(gas),
                    10_000_000.into(),
                    None,
                )
                .await
                .unwrap();
            execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

            let mut sender_balances = get_address_balances(client, sender).await;
            sender_balances.sort();

            assert_eq!(sender_balances[1], FUNDED_BALANCE_PER_COIN * 2);

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[test]
fn split_coin() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let split_amount_1 = 100_000;
        let split_amount_2 = 20_000;
        let split_amount_3 = 30_000;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .split_coin(
                sender,
                sender_coins[0],
                vec![
                    split_amount_1.into(),
                    split_amount_2.into(),
                    split_amount_3.into(),
                ],
                None,
                gas_budget.into(),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(
            sender_balances[0..3],
            [split_amount_2, split_amount_3, split_amount_1,]
        );
    });
}

#[test]
fn split_coin_equal() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .split_coin_equal(sender, sender_coins[0], 3.into(), None, gas_budget.into())
            .await
            .unwrap();
        execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(
            sender_balances[0..3],
            [3_333_333_333, 3_333_333_333, 3_333_333_334,]
        );
    });
}

#[test]
fn merge_coin() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();

        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
        let gas_budget = 100_000_000;

        let tx_bytes = client
            .merge_coin(
                sender,
                sender_coins[0],
                sender_coins[1],
                Some(sender_coins[2]),
                gas_budget.into(),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

        let mut sender_balances = get_address_balances(client, sender).await;
        sender_balances.sort();

        assert_eq!(sender_balances.len(), 2);
        assert_eq!(sender_balances[1], FUNDED_BALANCE_PER_COIN * 2);
    });
}

#[test]
fn batch_transaction() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
            let receiver = Address::random();

            let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 3).await;
            let gas = sender_coins[0];
            let coin_to_split = sender_coins[1];
            let coin_to_transfer: ObjectId = sender_coins[2];
            let amount_to_split = FUNDED_BALANCE_PER_COIN / 2 - 123_000;
            let amount_to_leave = FUNDED_BALANCE_PER_COIN - amount_to_split;

            let tx_bytes: TransactionBlockBytes = client
                .batch_transaction(
                    sender,
                    vec![
                        RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                            package_object_id: ObjectId::FRAMEWORK,
                            module: "pay".to_string(),
                            function: "split".to_string(),
                            type_arguments: type_args![TypeTag::from(StructTag::new_gas())]?,
                            arguments: call_args!(coin_to_split, amount_to_split)?
                                .into_iter()
                                .map(PtbInput::CallArg)
                                .collect(),
                        }),
                        RPCTransactionRequestParams::TransferObjectRequestParams(
                            TransferObjectParams {
                                recipient: receiver,
                                object_id: coin_to_transfer,
                            },
                        ),
                    ],
                    Some(gas),
                    10_000_000.into(),
                    None,
                )
                .await?;
            execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

            let mut sender_balances = get_address_balances(client, sender).await;
            let receiver_balances = get_address_balances(client, receiver).await;
            sender_balances.sort();

            assert_eq!(sender_balances.len(), 3);
            assert_eq!(sender_balances[0..2], [amount_to_split, amount_to_leave]);
            assert_eq!(receiver_balances, [FUNDED_BALANCE_PER_COIN]);

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[test]
fn batch_transaction_with_result() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
            let receiver = Address::random();

            let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 2).await;
            let gas = sender_coins[0];
            let coin_to_split = sender_coins[1];
            let amount_to_split = 10;

            let tx_bytes: TransactionBlockBytes = client
                .batch_transaction(
                    sender,
                    vec![
                        RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                            package_object_id: ObjectId::FRAMEWORK,
                            module: "coin".to_string(),
                            function: "split".to_string(),
                            type_arguments: type_args![StructTag::new_gas()]?,
                            arguments: call_args!(coin_to_split, amount_to_split)?
                                .into_iter()
                                .map(PtbInput::CallArg)
                                .collect(),
                        }),
                        RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
                            package_object_id: ObjectId::FRAMEWORK,
                            module: "transfer".to_string(),
                            function: "public_transfer".to_string(),
                            type_arguments: type_args![StructTag::new_gas_coin()]?,
                            arguments: vec![
                                PtbInput::PtbRef(IotaArgument::Result(0)),
                                PtbInput::CallArg(call_arg!(receiver)?),
                            ],
                        }),
                    ],
                    Some(gas),
                    10_000_000.into(),
                    None,
                )
                .await?;
            execute_tx_must_succeed(client, tx_bytes, &sender_key).await;

            let split_coin_balance = client
                .get_coins(sender, None, None, None)
                .await?
                .data
                .into_iter()
                .find(|coin| coin.coin_object_id == coin_to_split)
                .unwrap()
                .balance;
            let receiver_balances = get_address_balances(client, receiver).await;

            assert_eq!(
                split_coin_balance,
                FUNDED_BALANCE_PER_COIN - amount_to_split
            );
            assert_eq!(receiver_balances, [amount_to_split]);

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[test]
fn publish() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let sender_coins = create_coins_and_wait_for_indexer(cluster, client, sender, 1).await;

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.extend(["tests", "data", "dummy_modules_publish"]);
        let compiled_package = BuildConfig::new_for_testing().build(&path).unwrap();
        let compiled_modules_bytes =
            compiled_package.get_package_base64(/* with_unpublished_deps */ false);
        let module_count = compiled_modules_bytes.len();
        let dependencies = compiled_package.get_dependency_storage_package_ids();

        let tx_bytes = client
            .publish(
                sender,
                compiled_modules_bytes,
                dependencies,
                Some(sender_coins[0]),
                100_000_000.into(),
            )
            .await
            .unwrap();
        let tx_response = execute_tx_with_changes(client, tx_bytes, &sender_key).await;

        let published_modules = tx_response
            .object_changes
            .unwrap()
            .into_iter()
            .filter_map(|change| match change {
                ObjectChange::Published { modules, .. } => Some(modules),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(published_modules.len(), 1);
        assert_eq!(published_modules[0].len(), module_count);
    });
}

#[tokio::test]
async fn data_reader_get_owned_objects_paginates_without_dropping_objects()
-> Result<(), anyhow::Error> {
    let db_name = "transaction_builder_data_reader_get_owned_objects_pagination";
    let (cluster, store, _client) =
        start_test_cluster_with_read_write_indexer(Some(db_name), None, None).await;
    indexer_wait_for_latest_checkpoint(&store, &cluster).await;
    let address = cluster.get_address_0();

    let pool = new_connection_pool(
        &DbUrl::from(db_url(db_name).as_str()),
        &ConnectionPoolConfig::default(),
    )?;
    let reader = IndexerReader::new_without_watermark_cache(pool);
    let gas_coins = |cursor, limit| {
        reader.get_owned_objects(
            address,
            StructTag::new_gas_coin(),
            cursor,
            limit,
            IotaObjectDataOptions::new(),
        )
    };

    let full_page = gas_coins(None, None).await?;
    assert!(!full_page.has_next_page);
    assert!(full_page.next_cursor.is_none());
    assert_eq!(full_page.data.len(), DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT);

    let page_size = 2;
    let expected_pages = DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT.div_ceil(page_size);
    assert!(expected_pages > 1, "the walk must cross a page boundary");
    let mut paginated = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        assert!(
            pages < expected_pages,
            "cursor did not advance to completion within {expected_pages} pages"
        );
        let page = gas_coins(cursor, Some(page_size)).await?;
        pages += 1;
        for object in &page.data {
            let object_id = object.object_id()?;
            assert!(
                seen.insert(object_id),
                "object {object_id} was returned by more than one page"
            );
        }
        paginated.extend(page.data);

        if !page.has_next_page {
            assert!(page.next_cursor.is_none());
            break;
        }
        cursor = page.next_cursor;
    }

    let full_ids = full_page
        .data
        .iter()
        .map(|object| object.object_id())
        .collect::<Result<Vec<_>, _>>()?;
    let paginated_ids = paginated
        .iter()
        .map(|object| object.object_id())
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(paginated_ids, full_ids);
    assert_eq!(pages, expected_pages);

    let exact_page = gas_coins(None, Some(DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT)).await?;
    assert!(!exact_page.has_next_page);
    assert!(exact_page.next_cursor.is_none());

    Ok(())
}

#[tokio::test]
async fn request_add_stake() -> Result<(), anyhow::Error> {
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("transaction_builder_request_add_stake"),
        None,
        None,
    )
    .await;
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
    execute_tx_must_succeed(client, tx_bytes, &key).await;

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

    Ok(())
}

#[test]
fn request_withdraw_stake_from_pending() {
    let ApiTestSetup {
        runtime,
        store: _,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async move {
            let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
            execute_tx_must_succeed(client, tx_bytes, &key).await;

            let staked_iota = client.get_stakes(address).await.unwrap();
            let stake = &staked_iota[0].stakes[0];
            assert!(matches!(stake.status, StakeStatus::Pending));

            let tx_bytes: TransactionBlockBytes = client
                .request_withdraw_stake(
                    address,
                    stake.staked_iota_id,
                    Some(gas),
                    100_000_000.into(),
                )
                .await
                .unwrap();
            execute_tx_must_succeed(client, tx_bytes, &key).await;

            let staked_iota = client.get_stakes(address).await.unwrap();
            assert!(staked_iota.is_empty());

            Ok::<(), anyhow::Error>(())
        })
        .unwrap();
}

#[tokio::test]
async fn request_withdraw_stake_from_active() -> Result<(), anyhow::Error> {
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("transaction_builder_request_withdraw_stake_from_active"),
        None,
        None,
    )
    .await;
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
    execute_tx_must_succeed(client, tx_bytes, &key).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;
    let staked_iota = client.get_stakes(address).await.unwrap();
    let stake = &staked_iota[0].stakes[0];
    assert!(matches!(stake.status, StakeStatus::Active { .. }));

    let tx_bytes: TransactionBlockBytes = client
        .request_withdraw_stake(address, stake.staked_iota_id, Some(gas), 100_000_000.into())
        .await
        .unwrap();
    execute_tx_must_succeed(client, tx_bytes, &key).await;

    let staked_iota = client.get_stakes(address).await.unwrap();
    assert!(staked_iota.is_empty());

    Ok(())
}

#[tokio::test]
async fn request_add_timelocked_stake() -> Result<(), anyhow::Error> {
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
    execute_tx_must_succeed(&client, tx_bytes, &key).await;

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

    Ok(())
}

#[tokio::test]
async fn request_withdraw_timelocked_stake_from_pending() -> Result<(), anyhow::Error> {
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
    execute_tx_and_wait_for_indexer_checkpoint(cluster.rpc_client(), &store, tx_bytes, &key).await;

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
    execute_tx_and_wait_for_indexer_checkpoint(&client, &store, tx_bytes, &key).await;

    let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
    assert!(staked_iota.is_empty());

    Ok(())
}

#[tokio::test]
async fn request_withdraw_timelocked_stake_from_active() -> Result<(), anyhow::Error> {
    let (address, key): (_, AccountPrivateKey) = get_key_pair();
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
    execute_tx_must_succeed(&client, tx_bytes, &key).await;

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
    execute_tx_must_succeed(&client, tx_bytes, &key).await;

    let staked_iota = client.get_timelocked_stakes(address).await.unwrap();
    assert!(staked_iota.is_empty());

    Ok(())
}

/// Executes `tx_bytes` signed by `key`, asserts it succeeded and returns the
/// response with effects, object changes and balance changes.
async fn execute_tx_with_changes(
    client: &HttpClient,
    tx_bytes: TransactionBlockBytes,
    key: &AccountPrivateKey,
) -> IotaTransactionBlockResponse {
    let signed_tx = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), key);
    let (tx_bytes, signatures) = signed_tx.to_tx_bytes_and_signatures();
    let tx_response = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_effects()
                    .with_object_changes()
                    .with_balance_changes(),
            ),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        tx_response.status_ok(),
        Some(true),
        "transaction failed: {tx_response:?}"
    );
    tx_response
}

fn net_gas_usage(tx_response: &IotaTransactionBlockResponse) -> i128 {
    tx_response
        .effects
        .as_ref()
        .unwrap()
        .gas_cost_summary()
        .net_gas_usage()
        .into()
}

fn balance_change_of(tx_response: &IotaTransactionBlockResponse, owner: Address) -> i128 {
    let changes = tx_response
        .balance_changes
        .as_ref()
        .unwrap()
        .iter()
        .filter(|change| change.owner == Owner::Address(owner))
        .collect::<Vec<_>>();
    assert_eq!(changes.len(), 1, "expected one balance change for {owner}");
    changes[0].amount
}

fn assert_same_object_changes_ignoring_version_and_digest(
    expected: Vec<ObjectChange>,
    actual: Vec<ObjectChange>,
) {
    fn by_object_id_without_version_and_digest(
        changes: Vec<ObjectChange>,
    ) -> BTreeMap<ObjectId, ObjectChange> {
        changes
            .into_iter()
            .map(|mut change| {
                let object_id = change.object_id();
                change.mask_for_test(Version::MAX_VALID_EXCL, ObjectDigest::MAX);
                (object_id, change)
            })
            .collect()
    }
    assert_eq!(
        by_object_id_without_version_and_digest(expected),
        by_object_id_without_version_and_digest(actual)
    );
}

async fn get_address_balances(indexer_client: &HttpClient, address: Address) -> Vec<u64> {
    indexer_client
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data
        .iter()
        .map(|coin| coin.balance)
        .collect()
}

async fn create_coins_and_wait_for_indexer(
    cluster: &TestCluster,
    indexer_client: &HttpClient,
    address: Address,
    objects_count: u32,
) -> Vec<ObjectId> {
    let mut coins: Vec<ObjectId> = Vec::new();
    for _ in 0..objects_count {
        let coin = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(FUNDED_BALANCE_PER_COIN),
                address,
            )
            .await;
        indexer_wait_for_object(indexer_client, coin.object_id, coin.version).await;
        coins.push(coin.object_id);
    }
    coins
}

async fn create_cluster_with_timelocked_iota(
    address: Address,
    indexer_db_name: &str,
) -> (TestCluster, PgIndexerStore, HttpClient, ObjectId) {
    let principal = 100_000_000_000;
    let expiration_timestamp_ms = u64::MAX;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock_iota = {
        MoveStruct::new_from_execution(
            StructTag::new_timelocked_gas_balance(),
            OBJECT_START_VERSION,
            TimeLock::<iota_types::balance::Balance>::new(
                UID::new(ObjectId::random()),
                iota_types::balance::Balance::new(principal),
                expiration_timestamp_ms,
                label.clone(),
            )
            .to_bcs_bytes(),
            &ProtocolConfig::get_for_min_version(),
            // system_mutation
            false,
        )
        .unwrap()
    };
    let timelock_iota = ObjectInner {
        owner: Owner::Address(address),
        data: ObjectData::Struct(timelock_iota),
        previous_transaction: TransactionDigest::GENESIS_MARKER,
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

async fn get_validator(client: &HttpClient) -> Address {
    let iota_system_state = client
        .get_latest_iota_system_state_v2()
        .await
        .unwrap()
        .into();
    match iota_system_state {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].iota_address,
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].iota_address,
        _ => unimplemented!(
            "a new IotaSystemStateSummary enum variant was added and needs to be handled"
        ),
    }
}

async fn get_gas_object_id(client: &HttpClient, address: Address) -> ObjectId {
    client
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data[0]
        .coin_object_id
}
