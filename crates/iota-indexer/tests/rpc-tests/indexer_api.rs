// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    str::FromStr,
    time::{Duration, SystemTime},
};

use diesel::RunQueryDsl;
use downcast::Any;
use iota_indexer::{
    errors::IndexerError,
    schema::{optimistic_transactions, tx_global_order},
    store::PgIndexerStore,
    transactional_blocking_with_retry,
};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{IndexerApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    EventFilter, EventPage, IotaMoveValue, IotaObjectDataFilter, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockData, IotaTransactionBlockKind,
    IotaTransactionBlockResponseOptions, IotaTransactionBlockResponseQuery,
    IotaTransactionBlockResponseQueryV2, IotaTransactionKind, ObjectsPage, TransactionFilter,
    TransactionFilterV2,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, MOVE_STDLIB_PACKAGE_ID,
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    dynamic_field::DynamicFieldName,
    gas_coin::GAS,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, Command, ObjectArg, TransactionData},
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use move_core_types::{
    annotated_value::MoveValue,
    identifier::Identifier,
    language_storage::{StructTag, TypeTag},
};

use crate::{
    coin_api::execute_move_call,
    common::{
        ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, execute_tx_must_succeed,
        indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint, indexer_wait_for_object,
        indexer_wait_for_transaction, rpc_call_error_msg_matches,
        start_test_cluster_with_read_write_indexer,
    },
    write_api::{create_basic_object, deploy_basics_pkg},
};

#[test]
fn query_events_no_events_descending() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let indexer_events = client
            .query_events(
                EventFilter::Sender(
                    IotaAddress::from_str(
                        "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                    )
                    .unwrap(),
                ),
                None,
                None,
                Some(true),
            )
            .await
            .unwrap();

        assert_eq!(indexer_events, EventPage::empty())
    });
}

#[test]
fn query_events_no_events_ascending() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let indexer_events = client
            .query_events(
                EventFilter::Sender(
                    IotaAddress::from_str(
                        "0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99",
                    )
                    .unwrap(),
                ),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(indexer_events, EventPage::empty())
    });
}

#[test]
fn query_events_by_sender() -> Result<(), IndexerError> {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        let mut expected_event_ids = Vec::new();
        // Generate 5 events to test pagination
        for _ in 0..5 {
            let res = execute_move_call(
                client,
                sender,
                &sender_kp,
                package_id,
                "object_basics".to_string(),
                "update".to_string(),
                type_args![].unwrap(),
                call_args!(basic_obj_1, basic_obj_2).unwrap(),
                None,
            )
            .await?;
            assert_eq!(res.status_ok(), Some(true));

            let event_id = res
                .events
                .as_ref()
                .unwrap()
                .data
                .iter()
                .exactly_one()
                .unwrap()
                .id;
            expected_event_ids.push(event_id);
        }

        // ensure all events are checkpointed
        indexer_wait_for_transaction(expected_event_ids.last().unwrap().tx_digest, store, client)
            .await;

        assert_paginated_filtered_events(
            client,
            expected_event_ids.as_slice(),
            EventFilter::Sender(sender),
            2,
        )
        .await?;

        Ok::<(), IndexerError>(())
    })
}

#[test]
fn query_events_by_tx_digest() -> Result<(), IndexerError> {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        let res = execute_move_call(
            client,
            sender,
            &sender_kp,
            package_id,
            "object_basics".to_string(),
            "update".to_string(),
            type_args![].unwrap(),
            call_args!(basic_obj_1, basic_obj_2).unwrap(),
            None,
        )
        .await?;
        assert_eq!(res.status_ok(), Some(true));
        indexer_wait_for_transaction(res.digest, store, client).await;

        let event_id = res
            .events
            .as_ref()
            .unwrap()
            .data
            .iter()
            .exactly_one()
            .unwrap()
            .id;

        let all_events = client
            .query_events(EventFilter::Transaction(res.digest), None, None, None)
            .await
            .unwrap();
        let returned_event_ids: Vec<_> = all_events.data.iter().map(|e| e.id).collect();
        assert_eq!(returned_event_ids, vec![event_id]);

        // ensure event is checkpointed
        indexer_wait_for_transaction(res.digest, store, client).await;

        let all_events = client
            .query_events(EventFilter::Transaction(res.digest), None, None, None)
            .await
            .unwrap();
        let returned_event_ids: Vec<_> = all_events.data.iter().map(|e| e.id).collect();
        assert_eq!(returned_event_ids, vec![event_id]);

        Ok::<(), IndexerError>(())
    })
}

#[test]
fn query_events_by_package() -> Result<(), IndexerError> {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        // Generate multiple events by calling update function multiple times
        let mut expected_event_ids = Vec::new();

        // Generate 5 events to test pagination
        for _ in 0..5 {
            let res = execute_move_call(
                client,
                sender,
                &sender_kp,
                package_id,
                "object_basics".to_string(),
                "update".to_string(),
                type_args![].unwrap(),
                call_args!(basic_obj_1, basic_obj_2).unwrap(),
                None,
            )
            .await?;
            assert_eq!(res.status_ok(), Some(true));

            let event_id = res
                .events
                .as_ref()
                .unwrap()
                .data
                .iter()
                .exactly_one()
                .unwrap()
                .id;
            expected_event_ids.push(event_id);
        }

        // ensure all events are checkpointed
        indexer_wait_for_transaction(expected_event_ids.last().unwrap().tx_digest, store, client)
            .await;

        assert_paginated_filtered_events(
            client,
            expected_event_ids.as_slice(),
            EventFilter::Package(package_id),
            2,
        )
        .await?;

        Ok::<(), IndexerError>(())
    })
}

#[test]
fn query_events_unsupported_events() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        // Get the current time in milliseconds since the UNIX epoch
        let now_millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Subtract 10 minutes from the current time
        let ten_minutes_ago = now_millis - (10 * 60 * 1000); // 600 seconds = 10 minutes

        let unsupported_filters = vec![
            EventFilter::All(vec![]),
            EventFilter::Any(vec![]),
            EventFilter::And(
                Box::new(EventFilter::Any(vec![])),
                Box::new(EventFilter::Any(vec![])),
            ),
            EventFilter::Or(
                Box::new(EventFilter::Any(vec![])),
                Box::new(EventFilter::Any(vec![])),
            ),
            EventFilter::TimeRange {
                start_time: ten_minutes_ago as u64,
                end_time: now_millis as u64,
            },
            EventFilter::MoveEventField {
                path: String::default(),
                value: serde_json::Value::Bool(true),
            },
        ];

        for event_filter in unsupported_filters {
            let result = client
                .query_events(event_filter, None, None, None)
                .await;

            assert!(rpc_call_error_msg_matches(
                result,
                r#"{"code":-32603,"message": "Indexer does not support the feature with error: `This type of EventFilter is not supported.`"}"#,
            ));
        }
    });
}

#[test]
fn query_events_supported_events() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let real_tx_digest = client
            .query_transaction_blocks(
                IotaTransactionBlockResponseQuery {
                    filter: None,
                    options: None,
                },
                None,
                Some(1),
                None,
            )
            .await
            .unwrap()
            .data[0]
            .digest;

        let supported_filters = vec![
            EventFilter::Sender(IotaAddress::ZERO),
            EventFilter::Transaction(real_tx_digest),
            EventFilter::Package(ObjectID::ZERO),
            EventFilter::MoveEventModule {
                package: ObjectID::ZERO,
                module: "x".parse().unwrap(),
            },
            EventFilter::MoveEventType("0xabcd::MyModule::Foo".parse().unwrap()),
            EventFilter::MoveModule {
                package: ObjectID::ZERO,
                module: "x".parse().unwrap(),
            },
        ];

        for event_filter in supported_filters {
            let err_str = format!("query_events should succeed for filter: {event_filter:?}");
            let result = client.query_events(event_filter, None, None, None).await;
            result.expect(&err_str);
        }
    });
}

#[tokio::test]
async fn query_validator_epoch_info_event() {
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("query_validator_epoch_info_event"),
        None,
        None,
    )
    .await;
    indexer_wait_for_checkpoint(store, 1).await;

    cluster.force_new_epoch().await;
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let result = client.query_events(EventFilter::MoveEventType("0x0000000000000000000000000000000000000000000000000000000000000003::validator_set::ValidatorEpochInfoEventV1".parse().unwrap()), None, None, None).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().data.is_empty());

    let result = client
        .query_events(
            EventFilter::MoveEventType(
                "0x3::validator_set::ValidatorEpochInfoEventV1"
                    .parse()
                    .unwrap(),
            ),
            None,
            None,
            None,
        )
        .await;
    assert!(result.is_ok());
    assert!(!result.unwrap().data.is_empty());

    let result = client
        .query_events(
            EventFilter::MoveEventType(
                "0x0003::validator_set::ValidatorEpochInfoEventV1"
                    .parse()
                    .unwrap(),
            ),
            None,
            None,
            None,
        )
        .await;
    assert!(result.is_ok());
    assert!(!result.unwrap().data.is_empty());

    let result = client
        .query_events(
            EventFilter::MoveEventType(
                "0x1::validator_set::ValidatorEpochInfoEventV1"
                    .parse()
                    .unwrap(),
            ),
            None,
            None,
            None,
        )
        .await;
    assert!(result.is_ok());
    assert!(result.unwrap().data.is_empty());
}

#[test]
fn test_get_owned_objects() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;

        let address = cluster.get_address_0();

        let objects = client
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

        Ok(())
    })
}

#[test]
fn test_query_transaction_blocks_pagination() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;
        let coin_to_split = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, coin_to_split.0, coin_to_split.1).await;
        let iota_client = cluster.wallet.get_client().await.unwrap();

        for _ in 0..5 {
            let tx_data = iota_client
                .transaction_builder()
                .split_coin_equal(address, coin_to_split.0, 2, Some(gas_ref.0), 10_000_000)
                .await?;

            let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);

            let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

            let res = client
                .execute_transaction_block(
                    tx_bytes,
                    signatures,
                    Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                    Some(ExecuteTransactionRequestType::WaitForEffectsCert),
                )
                .await?;

            indexer_wait_for_transaction(res.digest, store, client).await;
        }

        let objects = client
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

        // 2 gas coins + 5 coins from the split
        assert_eq!(7, objects.len());

        // filter transactions by address
        let query = IotaTransactionBlockResponseQuery {
            options: Some(IotaTransactionBlockResponseOptions {
                show_input: true,
                show_effects: true,
                show_events: true,
                ..Default::default()
            }),
            filter: Some(TransactionFilter::FromAddress(address)),
        };

        let first_page = client
            .query_transaction_blocks(query.clone(), None, Some(3), Some(true))
            .await
            .unwrap();
        assert_eq!(3, first_page.data.len());
        assert!(first_page.data[0].transaction.is_some());
        assert!(first_page.data[0].effects.is_some());
        assert!(first_page.data[0].events.is_some());
        assert!(first_page.has_next_page);

        // Read the next page for the last transaction
        let next_page = client
            .query_transaction_blocks(query, first_page.next_cursor, None, Some(true))
            .await
            .unwrap();

        assert_eq!(2, next_page.data.len());
        assert!(next_page.data[0].transaction.is_some());
        assert!(next_page.data[0].effects.is_some());
        assert!(next_page.data[0].events.is_some());
        assert!(!next_page.has_next_page);

        Ok(())
    })
}

#[tokio::test]
async fn test_query_transaction_blocks_pagination_with_partial_global_order()
-> Result<(), anyhow::Error> {
    // separate test environment needed because DB is wiped during test
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("test_query_transaction_blocks_pagination_with_partial_global_order"),
        None,
        None,
    )
    .await;
    indexer_wait_for_checkpoint(store, 1).await;

    let (address, keypair): (_, AccountKeyPair) = get_key_pair();

    let gas_ref = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(500_000_000),
            address,
        )
        .await;
    indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;
    let coin_to_split = cluster
        .fund_address_and_return_gas(
            cluster.get_reference_gas_price().await,
            Some(500_000_000),
            address,
        )
        .await;
    indexer_wait_for_object(client, coin_to_split.0, coin_to_split.1).await;
    let iota_client = cluster.wallet.get_client().await.unwrap();
    let mut expected_tx_digests = vec![];

    for _ in 0..5 {
        let tx_data = iota_client
            .transaction_builder()
            .split_coin_equal(address, coin_to_split.0, 2, Some(gas_ref.0), 10_000_000)
            .await?;
        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);
        let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();
        let res = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                Some(ExecuteTransactionRequestType::WaitForEffectsCert),
            )
            .await?;
        indexer_wait_for_transaction(res.digest, store, client).await;
        expected_tx_digests.push(res.digest);
    }

    wipe_global_order_and_optimistic_tables(store); // data indexed before this point will not have global order

    for _ in 0..5 {
        let tx_data = iota_client
            .transaction_builder()
            .split_coin_equal(address, coin_to_split.0, 2, Some(gas_ref.0), 10_000_000)
            .await?;
        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);
        let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();
        let res = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                Some(ExecuteTransactionRequestType::WaitForEffectsCert),
            )
            .await?;
        indexer_wait_for_transaction(res.digest, store, client).await;
        expected_tx_digests.push(res.digest);
    }

    let filter = TransactionFilter::FromAddress(address);

    assert_paginated_filtered_transactions(client, &expected_tx_digests, filter.clone(), 2).await?;

    // wait for data to be checkpointed
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert_paginated_filtered_transactions(client, &expected_tx_digests, filter, 2).await?;

    Ok(())
}

#[test]
fn test_query_transaction_blocks() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        let coin_1 = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        let coin_2 = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        let iota_client = cluster.wallet.get_client().await.unwrap();

        indexer_wait_for_object(client, gas.0, gas.1).await;
        indexer_wait_for_object(client, coin_1.0, coin_1.1).await;
        indexer_wait_for_object(client, coin_2.0, coin_2.1).await;

        let objects = client
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

        assert_eq!(objects.len(), 3);

        // make 2 move calls of same package & module, but different functions
        let package_id = ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes());
        let signer = address;

        let tx_builder = iota_client.transaction_builder().clone();
        let mut pt_builder = ProgrammableTransactionBuilder::new();

        let module = Identifier::from_str("pay")?;
        let function_1 = Identifier::from_str("split")?;
        let function_2 = Identifier::from_str("divide_and_keep")?;

        let iota_type_args = type_args![GAS::type_tag()]?;
        let type_args = iota_type_args
            .into_iter()
            .map(|ty| ty.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        let iota_call_args_1 = call_args!(coin_1.0, 10)?;
        let call_args_1 = tx_builder
            .resolve_and_checks_json_args(
                &mut pt_builder,
                package_id,
                &module,
                &function_1,
                &type_args,
                iota_call_args_1,
            )
            .await?;
        let cmd_1 = Command::move_call(
            package_id,
            module.clone(),
            function_1,
            type_args.clone(),
            call_args_1.clone(),
        );

        let iota_call_args_2 = call_args!(coin_2.0, 10)?;
        let call_args_2 = tx_builder
            .resolve_and_checks_json_args(
                &mut pt_builder,
                package_id,
                &module,
                &function_2,
                &type_args,
                iota_call_args_2,
            )
            .await?;
        let cmd_2 = Command::move_call(package_id, module, function_2, type_args, call_args_2);
        pt_builder.command(cmd_1);
        pt_builder.command(cmd_2);
        let pt = pt_builder.finish();

        let tx_data = TransactionData::new_programmable(signer, vec![gas], pt, 10_000_000, 1000);

        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);

        let response = iota_client
            .quorum_driver_api()
            .execute_transaction_block(
                signed_transaction,
                IotaTransactionBlockResponseOptions::new(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        indexer_wait_for_transaction(response.digest, store, client).await;

        // match with None function, the DB should have 2 records, but both points to
        // the same tx
        let filter = TransactionFilterV2::FromAddress(signer);
        let move_call_query = IotaTransactionBlockResponseQueryV2::new_with_filter(filter);
        let res = client
            .query_transaction_blocks_v2(move_call_query, None, Some(20), Some(true))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());

        Ok(())
    })
}

#[test]
fn test_query_transaction_blocks_from_and_to_address() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        client,
        store,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let recipient_1 = IotaAddress::random_for_testing_only();
        let recipient_2 = IotaAddress::random_for_testing_only();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient_1,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, transfer_request, &keypair).await;
        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient_2,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &keypair).await;

        let query = IotaTransactionBlockResponseQuery::new_with_filter(
            TransactionFilter::FromAndToAddress {
                from: address,
                to: recipient_1,
            },
        );
        let res = client
            .query_transaction_blocks(query, None, Some(20), Some(true))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());

        Ok(())
    })
}

#[test]
fn test_query_by_recently_executed_tx_cursor() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        client,
        store,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let recipient = IotaAddress::random_for_testing_only();
        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let filter = TransactionFilter::FromOrToAddress { addr: recipient };

        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        let digest_1 = execute_tx_must_succeed(client, transfer_request, &keypair).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient,
                Some(150_000_000.into()),
            )
            .await
            .unwrap();
        let digest_2 = execute_tx_must_succeed(client, transfer_request, &keypair).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient,
                Some(160_000_000.into()),
            )
            .await
            .unwrap();
        let digest_3 =
            execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &keypair)
                .await;

        assert_paginated_filtered_transactions(
            client,
            &[digest_1, digest_2, digest_3],
            filter.clone(),
            2,
        )
        .await?;

        // wait for data to be checkpointed
        tokio::time::sleep(Duration::from_secs(2)).await;

        assert_paginated_filtered_transactions(client, &[digest_1, digest_2, digest_3], filter, 2)
            .await?;

        Ok(())
    })
}

#[test]
fn test_query_transaction_blocks_from_or_to_address() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        client,
        store,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let recipient_1 = IotaAddress::random_for_testing_only();
        let recipient_2 = IotaAddress::random_for_testing_only();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient_1,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, transfer_request, &keypair).await;
        let transfer_request = client
            .transfer_iota(
                address,
                gas.0,
                5_000_000.into(),
                recipient_2,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &keypair).await;

        let query = IotaTransactionBlockResponseQuery::new_with_filter(
            TransactionFilter::FromOrToAddress { addr: address },
        );
        let res = client
            .query_transaction_blocks(query, None, None, Some(false))
            .await
            .unwrap();
        assert_eq!(3, res.data.len());

        let query = IotaTransactionBlockResponseQuery::new_with_filter(
            TransactionFilter::FromOrToAddress { addr: recipient_1 },
        );
        let res = client
            .query_transaction_blocks(query, None, None, Some(true))
            .await
            .unwrap();
        assert_eq!(1, res.data.len());

        Ok(())
    })
}

async fn assert_paginated_filtered_transactions(
    client: &HttpClient,
    expected_transactions_digests: &[iota_types::digests::TransactionDigest],
    filter: TransactionFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    // Test querying all transactions (ascending order - default)
    let all_transactions = client
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::new_with_filter(filter.clone()),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Verify transactions are returned in ascending order
    let returned_transactions_digests: Vec<_> =
        all_transactions.data.iter().map(|e| e.digest).collect();
    assert_eq!(returned_transactions_digests, expected_transactions_digests);

    assert_paginated_transactions_ascending(
        client,
        expected_transactions_digests,
        &filter,
        page_size,
    )
    .await?;
    assert_paginated_transactions_descending(
        client,
        expected_transactions_digests,
        &filter,
        page_size,
    )
    .await?;

    Ok(())
}

async fn assert_paginated_transactions_ascending(
    client: &HttpClient,
    expected_transactions_digests: &[iota_types::digests::TransactionDigest],
    filter: &TransactionFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    let mut cursor = None;
    let mut transactions_processed = 0;
    let total_transactions = expected_transactions_digests.len();

    loop {
        let page = client
            .query_transaction_blocks(
                IotaTransactionBlockResponseQuery::new_with_filter(filter.clone()),
                cursor,
                Some(page_size),
                None,
            )
            .await
            .unwrap();

        let transactions_remaining = total_transactions - transactions_processed;
        let expected_page_size = std::cmp::min(page_size, transactions_remaining);
        let is_last_page = transactions_processed + expected_page_size >= total_transactions;

        let actual_transactions_ids: Vec<_> = page.data.iter().map(|e| e.digest).collect();
        let expected_transactions_digests_slice = &expected_transactions_digests
            [transactions_processed..transactions_processed + expected_page_size];

        assert_eq!(actual_transactions_ids, expected_transactions_digests_slice);
        assert_eq!(page.has_next_page, !is_last_page);

        if is_last_page {
            break;
        }
        cursor = page.next_cursor;
        transactions_processed += expected_page_size;
    }

    Ok(())
}

async fn assert_paginated_transactions_descending(
    client: &HttpClient,
    expected_transactions_digests: &[iota_types::digests::TransactionDigest],
    filter: &TransactionFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    let mut cursor = None;
    let mut transactions_processed = 0;
    let total_transactions = expected_transactions_digests.len();

    // In descending order, we expect transactions in reverse chronological order
    let expected_desc_transactions: Vec<_> = expected_transactions_digests
        .iter()
        .rev()
        .cloned()
        .collect();

    loop {
        let page = client
            .query_transaction_blocks(
                IotaTransactionBlockResponseQuery::new_with_filter(filter.clone()),
                cursor,
                Some(page_size),
                Some(true),
            )
            .await
            .unwrap();

        let transactions_remaining = total_transactions - transactions_processed;
        let expected_page_size = std::cmp::min(page_size, transactions_remaining);
        let is_last_page = transactions_processed + expected_page_size >= total_transactions;

        let actual_transactions_ids: Vec<_> = page.data.iter().map(|e| e.digest).collect();
        let expected_transactions_digests_slice = &expected_desc_transactions
            [transactions_processed..transactions_processed + expected_page_size];

        assert_eq!(actual_transactions_ids, expected_transactions_digests_slice);
        assert_eq!(page.has_next_page, !is_last_page);

        if is_last_page {
            break;
        }
        cursor = page.next_cursor;
        transactions_processed += expected_page_size;
    }

    Ok(())
}

#[test]
fn test_get_dynamic_fields() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        // Create a bag object
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::from_str("bag")?,
                Identifier::from_str("new")?,
                vec![],
                vec![],
            );

            let field_name_argument = builder.pure(0u64).expect("valid pure");
            let field_value_argument = builder.pure(0u64).expect("valid pure");

            let _ = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::from_str("bag")?,
                Identifier::from_str("add")?,
                vec![TypeTag::U64, TypeTag::U64],
                vec![bag, field_name_argument, field_value_argument],
            );

            builder.transfer_arg(address, bag);
            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(address, gas, 1000);
        let tx_data = tx_builder.programmable(pt).build();
        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);

        let res = cluster
            .wallet
            .execute_transaction_must_succeed(signed_transaction)
            .await;

        // Wait for the transaction to be executed
        indexer_wait_for_transaction(res.digest, store, client).await;

        // Find the bag object
        let objects: ObjectsPage = client
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::StructType(StructTag {
                        address: IOTA_FRAMEWORK_ADDRESS,
                        module: Identifier::from_str("bag")?,
                        name: Identifier::from_str("Bag")?,
                        type_params: Vec::new(),
                    })),
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

        let bag_object_ref = objects.data.first().unwrap().object().unwrap().object_ref();

        // Verify that the dynamic field was successfully added
        let dynamic_fields = client
            .get_dynamic_fields(bag_object_ref.0, None, None)
            .await
            .expect("failed to get dynamic fields");

        assert!(
            !dynamic_fields.data.is_empty(),
            "dynamic field was not added"
        );

        Ok(())
    })
}

fn wipe_global_order_and_optimistic_tables(store: &PgIndexerStore) {
    let pool = store.blocking_cp();

    transactional_blocking_with_retry!(
        &pool,
        |conn| { diesel::dsl::delete(tx_global_order::table).execute(conn) },
        Duration::from_secs(10)
    )
    .unwrap();

    transactional_blocking_with_retry!(
        &pool,
        |conn| { diesel::dsl::delete(optimistic_transactions::table).execute(conn) },
        Duration::from_secs(10)
    )
    .unwrap();
}

#[test]
fn test_get_dynamic_field_objects() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        let child_object = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;

        // Create a object bag object
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::from_str("object_bag")?,
                Identifier::from_str("new")?,
                vec![],
                vec![],
            );

            let field_name_argument = builder.pure(0u64).expect("valid pure");
            let field_value_argument = builder
                .input(CallArg::Object(ObjectArg::ImmOrOwnedObject(child_object)))
                .unwrap();

            let _ = builder.programmable_move_call(
                ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes()),
                Identifier::from_str("object_bag")?,
                Identifier::from_str("add")?,
                vec![
                    TypeTag::U64,
                    TypeTag::Struct(Box::new(StructTag {
                        address: IOTA_FRAMEWORK_ADDRESS,
                        module: Identifier::from_str("coin")?,
                        name: Identifier::from_str("Coin")?,
                        type_params: vec![GAS::type_tag()],
                    })),
                ],
                vec![bag, field_name_argument, field_value_argument],
            );

            builder.transfer_arg(address, bag);
            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(address, gas, 1000);
        let tx_data = tx_builder.programmable(pt).build();
        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);

        let res = cluster
            .wallet
            .execute_transaction_must_succeed(signed_transaction)
            .await;

        // Wait for the transaction to be executed
        indexer_wait_for_transaction(res.digest, store, client).await;

        // Find the bag object
        let objects: ObjectsPage = client
            .get_owned_objects(
                address,
                Some(IotaObjectResponseQuery::new(
                    Some(IotaObjectDataFilter::StructType(StructTag {
                        address: IOTA_FRAMEWORK_ADDRESS,
                        module: Identifier::from_str("object_bag")?,
                        name: Identifier::from_str("ObjectBag")?,
                        type_params: Vec::new(),
                    })),
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

        let bag_object_ref = objects.data.first().unwrap().object().unwrap().object_ref();

        let name = DynamicFieldName {
            type_: TypeTag::U64,
            value: IotaMoveValue::from(MoveValue::U64(0u64)).to_json_value(),
        };

        // Verify that the dynamic field was successfully added
        let dynamic_fields = client
            .get_dynamic_field_object(bag_object_ref.0, name)
            .await
            .expect("failed to get dynamic field object");

        assert!(
            dynamic_fields.data.is_some(),
            "dynamic field object was not added"
        );

        Ok(())
    })
}

#[test]
fn test_query_transaction_blocks_tx_kind_filter() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        let iota_client = cluster.wallet.get_client().await.unwrap();

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let objects = client
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

        assert_eq!(objects.len(), 1);

        let signer = address;

        let package_id = MOVE_STDLIB_PACKAGE_ID;
        let module = Identifier::from_str("address")?;
        let function = Identifier::from_str("length")?;

        let mut pt_builder = ProgrammableTransactionBuilder::new();
        pt_builder.move_call(package_id, module, function, vec![], vec![])?;
        let pt = pt_builder.finish();

        let tx_data = TransactionData::new_programmable(signer, vec![gas], pt, 10_000_000, 1_000);
        let signed_transaction = to_sender_signed_transaction(tx_data, &keypair);

        let response = iota_client
            .quorum_driver_api()
            .execute_transaction_block(
                signed_transaction,
                IotaTransactionBlockResponseOptions::new(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        indexer_wait_for_transaction(response.digest, store, client).await;

        let options = IotaTransactionBlockResponseOptions::new().with_input();

        // Test `ProgrammableTransaction` transaction kind filter
        let filter =
            TransactionFilterV2::TransactionKind(IotaTransactionKind::ProgrammableTransaction);
        let query = IotaTransactionBlockResponseQueryV2::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks_v2(query, None, Some(1), Some(true))
            .await
            .unwrap();
        assert_eq!(1, res.data.len());

        let IotaTransactionBlockData::V1(tx_data_v1) = &res
            .data
            .first()
            .as_ref()
            .unwrap()
            .transaction
            .as_ref()
            .unwrap()
            .data;
        assert!(matches!(
            tx_data_v1.transaction,
            IotaTransactionBlockKind::ProgrammableTransaction(_)
        ));

        // Test `Genesis` transaction kind filter
        let filter = TransactionFilterV2::TransactionKind(IotaTransactionKind::Genesis);
        let query = IotaTransactionBlockResponseQueryV2::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks_v2(query, None, Some(2), Some(false))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());
        assert!(!res.has_next_page);

        let IotaTransactionBlockData::V1(tx_data_v1) = &res
            .data
            .first()
            .as_ref()
            .unwrap()
            .transaction
            .as_ref()
            .unwrap()
            .data;
        assert!(matches!(
            tx_data_v1.transaction,
            IotaTransactionBlockKind::Genesis(_)
        ));

        // Test `SystemTransaction` transaction kind filter
        let filter = TransactionFilterV2::TransactionKind(IotaTransactionKind::SystemTransaction);
        let query = IotaTransactionBlockResponseQueryV2::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks_v2(query, None, Some(1), Some(true))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());
        assert!(res.has_next_page);

        let IotaTransactionBlockData::V1(tx_data_v1) = &res
            .data
            .first()
            .as_ref()
            .unwrap()
            .transaction
            .as_ref()
            .unwrap()
            .data;
        assert_eq!(tx_data_v1.sender, IotaAddress::ZERO);

        // Test `ConsensusCommitPrologueV1` transaction kind filter
        let filter =
            TransactionFilterV2::TransactionKind(IotaTransactionKind::ConsensusCommitPrologueV1);
        let query = IotaTransactionBlockResponseQueryV2::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks_v2(query, None, Some(1), Some(true))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());
        assert!(res.has_next_page);

        let IotaTransactionBlockData::V1(tx_data_v1) = &res
            .data
            .first()
            .as_ref()
            .unwrap()
            .transaction
            .as_ref()
            .unwrap()
            .data;
        assert!(matches!(
            tx_data_v1.transaction,
            IotaTransactionBlockKind::ConsensusCommitPrologueV1(_)
        ));

        // Test `TransactionKindIn` filter
        let filter = TransactionFilterV2::TransactionKindIn(vec![
            IotaTransactionKind::ConsensusCommitPrologueV1,
            IotaTransactionKind::ProgrammableTransaction,
        ]);
        let query = IotaTransactionBlockResponseQueryV2::new(Some(filter), Some(options));
        let res = client
            .query_transaction_blocks_v2(query, None, Some(2), Some(true))
            .await
            .unwrap();

        assert_eq!(2, res.data.len());
        assert!(res.has_next_page);

        for tb_res in res.data.iter() {
            let IotaTransactionBlockData::V1(tx_data_v1) =
                &tb_res.transaction.as_ref().unwrap().data;
            assert!(matches!(
                tx_data_v1.transaction,
                IotaTransactionBlockKind::ConsensusCommitPrologueV1(_)
                    | IotaTransactionBlockKind::ProgrammableTransaction(_)
            ));
        }

        Ok(())
    })
}

async fn assert_paginated_filtered_events(
    client: &HttpClient,
    expected_event_ids: &[iota_types::event::EventID],
    filter: EventFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    // Test querying all events (ascending order - default)
    let all_events = client
        .query_events(filter.clone(), None, None, None)
        .await
        .unwrap();

    // Verify events are returned in ascending order
    let returned_event_ids: Vec<_> = all_events.data.iter().map(|e| e.id).collect();
    assert_eq!(returned_event_ids, expected_event_ids);

    assert_paginated_events_ascending(client, expected_event_ids, &filter, page_size).await?;
    assert_paginated_events_descending(client, expected_event_ids, &filter, page_size).await?;

    Ok(())
}

async fn assert_paginated_events_ascending(
    client: &HttpClient,
    expected_event_ids: &[iota_types::event::EventID],
    filter: &EventFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    let mut cursor = None;
    let mut events_processed = 0;
    let total_events = expected_event_ids.len();

    loop {
        let page = client
            .query_events(filter.clone(), cursor, Some(page_size), None)
            .await
            .unwrap();

        let events_remaining = total_events - events_processed;
        let expected_page_size = std::cmp::min(page_size, events_remaining);
        let is_last_page = events_processed + expected_page_size >= total_events;

        let actual_event_ids: Vec<_> = page.data.iter().map(|e| e.id).collect();
        let expected_event_ids_slice =
            &expected_event_ids[events_processed..events_processed + expected_page_size];

        assert_eq!(actual_event_ids, expected_event_ids_slice);
        assert_eq!(page.has_next_page, !is_last_page);

        if is_last_page {
            break;
        }
        cursor = page.next_cursor;
        events_processed += expected_page_size;
    }

    Ok(())
}

async fn assert_paginated_events_descending(
    client: &HttpClient,
    expected_event_ids: &[iota_types::event::EventID],
    filter: &EventFilter,
    page_size: usize,
) -> Result<(), IndexerError> {
    let mut cursor = None;
    let mut events_processed = 0;
    let total_events = expected_event_ids.len();

    // In descending order, we expect events in reverse chronological order
    let expected_desc_events: Vec<_> = expected_event_ids.iter().rev().cloned().collect();

    loop {
        let page = client
            .query_events(filter.clone(), cursor, Some(page_size), Some(true))
            .await
            .unwrap();

        let events_remaining = total_events - events_processed;
        let expected_page_size = std::cmp::min(page_size, events_remaining);
        let is_last_page = events_processed + expected_page_size >= total_events;

        let actual_event_ids: Vec<_> = page.data.iter().map(|e| e.id).collect();
        let expected_event_ids_slice =
            &expected_desc_events[events_processed..events_processed + expected_page_size];

        assert_eq!(actual_event_ids, expected_event_ids_slice);
        assert_eq!(page.has_next_page, !is_last_page);

        if is_last_page {
            break;
        }
        cursor = page.next_cursor;
        events_processed += expected_page_size;
    }

    Ok(())
}
