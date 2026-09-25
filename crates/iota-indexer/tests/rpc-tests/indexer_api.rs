// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    str::FromStr,
    time::{Duration, SystemTime},
};

use iota_indexer::{config::RetentionConfig, errors::IndexerError, pruning::pruner::PrunableTable};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{IndexerApiClient, TransactionBuilderClient, WriteApiClient};
use iota_json_rpc_types::{
    EventFilter, EventPage, IotaMoveValue, IotaObjectDataFilter, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockData, IotaTransactionBlockKind,
    IotaTransactionBlockResponseOptions, IotaTransactionBlockResponseQuery,
    IotaTransactionBlockResponseQueryV2, IotaTransactionKind, ObjectsPage, TransactionFilter,
    TransactionFilterV2,
};
use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::{
    Address, Command, Identifier, ObjectId, StructTag, Transaction, TransactionDigest, TypeTag,
};
use iota_test_transaction_builder::{TestTransactionBuilder, split_coin_equal_tx};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    dynamic_field::DynamicFieldName,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, TransactionAPI},
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use move_core_types::annotated_value::MoveValue;

use crate::{
    coin_api::execute_move_call,
    common::{
        ApiTestSetup, execute_tx_and_wait_for_indexer_checkpoint, execute_tx_must_succeed,
        indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint, indexer_wait_for_object,
        indexer_wait_for_transaction, publish_test_move_package, rpc_call_error_msg_matches,
        start_test_cluster_with_read_write_indexer, wait_for_oldest_available_checkpoint,
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
                    Address::from_str(
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

        assert_eq!(
            indexer_events,
            EventPage {
                oldest_available_checkpoint: Some(0u64.into()),
                ..EventPage::empty()
            }
        )
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
                    Address::from_str(
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

        assert_eq!(
            indexer_events,
            EventPage {
                oldest_available_checkpoint: Some(0u64.into()),
                ..EventPage::empty()
            }
        )
    });
}

/// Returns the `oldest_available_checkpoint` reported for `filter`.
async fn events_oldest_available_checkpoint(
    client: &HttpClient,
    filter: EventFilter,
) -> Option<u64> {
    client
        .query_events(filter, None, None, None)
        .await
        .expect("query_events should succeed")
        .oldest_available_checkpoint
        .map(|cp| *cp)
}

#[tokio::test]
async fn query_events_reports_oldest_available_checkpoint() {
    // Only `tx_senders` is pruned; every other table, `events` included, is
    // retained. A sender-filtered query reads `tx_senders` while a
    // package-filtered one does not, so they report different checkpoints.
    let overrides = HashMap::from([(PrunableTable::TxSenders, 1)]);
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("test_query_events_reports_oldest_available_checkpoint"),
        None,
        Some(RetentionConfig::new(100, overrides)),
    )
    .await;

    indexer_wait_for_checkpoint(store, 1).await;

    let by_sender = EventFilter::Sender(
        Address::from_str("0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99")
            .unwrap(),
    );
    let by_package = EventFilter::Package(ObjectId::from_str("0x2").unwrap());

    assert_eq!(
        wait_for_oldest_available_checkpoint(
            || events_oldest_available_checkpoint(client, by_sender.clone()),
            |cp| cp.is_some()
        )
        .await,
        Some(0)
    );
    assert_eq!(
        events_oldest_available_checkpoint(client, by_package.clone()).await,
        Some(0)
    );

    let transactions = client
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
        .unwrap();
    let tx_digest = transactions
        .data
        .first()
        .expect("the indexer has at least one transaction")
        .digest;
    assert_eq!(
        events_oldest_available_checkpoint(client, EventFilter::Transaction(tx_digest)).await,
        Some(0)
    );

    cluster.force_new_epoch().await;

    // Once `tx_senders` is pruned, the sender filter reports a checkpoint
    // above the genesis one.
    wait_for_oldest_available_checkpoint(
        || events_oldest_available_checkpoint(client, by_sender.clone()),
        |cp| cp > Some(0),
    )
    .await;

    // The package filter does not read `tx_senders`, so pruning it does not
    // change what that filter reports.
    assert_eq!(
        events_oldest_available_checkpoint(client, by_package).await,
        Some(0)
    );
}

/// Returns the `oldest_available_checkpoint` reported for `filter` by
/// `iotax_queryTransactionBlocks`.
async fn transaction_blocks_oldest_available_checkpoint(
    client: &HttpClient,
    filter: TransactionFilter,
) -> Option<u64> {
    client
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::new_with_filter(filter),
            None,
            None,
            None,
        )
        .await
        .expect("query_transaction_blocks should succeed")
        .oldest_available_checkpoint
        .map(|cp| *cp)
}

/// Returns the `oldest_available_checkpoint` reported for `filter` by the v2
/// version of `iotax_queryTransactionBlocks`.
async fn transaction_blocks_v2_oldest_available_checkpoint(
    client: &HttpClient,
    filter: TransactionFilterV2,
) -> Option<u64> {
    client
        .query_transaction_blocks_v2(
            IotaTransactionBlockResponseQueryV2::new_with_filter(filter),
            None,
            None,
            None,
        )
        .await
        .expect("query_transaction_blocks_v2 should succeed")
        .oldest_available_checkpoint
        .map(|cp| *cp)
}

#[tokio::test]
async fn query_transaction_blocks_reports_oldest_available_checkpoint() {
    // Only `tx_senders` is pruned; every other table is retained. A
    // sender-filtered query reads `tx_senders` while a kind-filtered one does
    // not, so they report different checkpoints.
    let overrides = HashMap::from([(PrunableTable::TxSenders, 1)]);
    let (cluster, store, client) = &start_test_cluster_with_read_write_indexer(
        Some("test_query_transaction_blocks_reports_oldest_available_checkpoint"),
        None,
        Some(RetentionConfig::new(100, overrides)),
    )
    .await;

    indexer_wait_for_checkpoint(store, 1).await;

    let sender =
        Address::from_str("0x9a934a2644c4ca2decbe3d126d80720429c5e31896aa756765afa23ae2cb4b99")
            .unwrap();
    let by_sender = TransactionFilter::FromAddress(sender);
    let by_kind = TransactionFilter::TransactionKind(IotaTransactionKind::ProgrammableTransaction);

    assert_eq!(
        wait_for_oldest_available_checkpoint(
            || transaction_blocks_oldest_available_checkpoint(client, by_sender.clone()),
            |cp| cp.is_some()
        )
        .await,
        Some(0)
    );
    assert_eq!(
        transaction_blocks_oldest_available_checkpoint(client, by_kind.clone()).await,
        Some(0)
    );
    assert_eq!(
        transaction_blocks_v2_oldest_available_checkpoint(
            client,
            TransactionFilterV2::FromAddress(sender)
        )
        .await,
        Some(0)
    );

    cluster.force_new_epoch().await;

    // Once `tx_senders` is pruned, the sender filter reports a checkpoint
    // above the genesis one, in both versions of the method.
    wait_for_oldest_available_checkpoint(
        || transaction_blocks_oldest_available_checkpoint(client, by_sender.clone()),
        |cp| cp > Some(0),
    )
    .await;
    wait_for_oldest_available_checkpoint(
        || {
            transaction_blocks_v2_oldest_available_checkpoint(
                client,
                TransactionFilterV2::FromAddress(sender),
            )
        },
        |cp| cp > Some(0),
    )
    .await;

    // The kind filter does not read `tx_senders`, so pruning it does not change
    // what that filter reports.
    assert_eq!(
        transaction_blocks_oldest_available_checkpoint(client, by_kind).await,
        Some(0)
    );
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

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.object_id, gas_ref.version).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_key, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();

        let mut expected_event_ids = Vec::new();
        // Generate 5 events to test pagination
        for _ in 0..5 {
            let res = execute_move_call(
                client,
                sender,
                &sender_key,
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

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.object_id, gas_ref.version).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_key, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();

        let res = execute_move_call(
            client,
            sender,
            &sender_key,
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

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.object_id, gas_ref.version).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_key, client).await;
        let basic_obj_1 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();
        let basic_obj_2 = create_basic_object(sender, &sender_key, client, &package_id)
            .await
            .unwrap();

        // Generate multiple events by calling update function multiple times
        let mut expected_event_ids = Vec::new();

        // Generate 5 events to test pagination
        for _ in 0..5 {
            let res = execute_move_call(
                client,
                sender,
                &sender_key,
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
            EventFilter::Sender(Address::ZERO),
            EventFilter::Transaction(real_tx_digest),
            EventFilter::Package(ObjectId::ZERO),
            EventFilter::MoveEventModule {
                package: ObjectId::ZERO,
                module: "x".parse().unwrap(),
            },
            EventFilter::MoveEventType("0xabcd::MyModule::Foo".parse().unwrap()),
            EventFilter::MoveModule {
                package: ObjectId::ZERO,
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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.object_id, gas_ref.version).await;
        let coin_to_split = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, coin_to_split.object_id, coin_to_split.version).await;
        let grpc_client = cluster.grpc_client();

        for _ in 0..5 {
            let tx_data = split_coin_equal_tx(
                &grpc_client,
                address,
                coin_to_split.object_id,
                2,
                Some(gas_ref.object_id),
                10_000_000,
            )
            .await;

            let signed_transaction = to_sender_signed_transaction(tx_data, &key);

            let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

            let res = client
                .execute_transaction_block(
                    tx_bytes,
                    signatures,
                    Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                    Some(ExecuteTransactionRequestType::WaitForEffectsCert.into()),
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

#[test]
fn test_query_transaction_blocks() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, key): (_, AccountPrivateKey) = get_key_pair();

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

        indexer_wait_for_object(client, gas.object_id, gas.version).await;
        indexer_wait_for_object(client, coin_1.object_id, coin_1.version).await;
        indexer_wait_for_object(client, coin_2.object_id, coin_2.version).await;

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
        let package_id = ObjectId::FRAMEWORK;
        let signer = address;

        let tx_builder = iota_client.transaction_builder().clone();
        let mut pt_builder = ProgrammableTransactionBuilder::new();

        let module = Identifier::from_static("pay");
        let function_1 = Identifier::from_static("split");
        let function_2 = Identifier::from_static("divide_and_keep");

        let iota_type_args = type_args![TypeTag::from(StructTag::new_gas())]?;
        let type_args = iota_type_args
            .into_iter()
            .map(|ty| ty.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        let iota_call_args_1 = call_args!(coin_1.object_id, 10)?;
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
        let cmd_1 = Command::new_move_call(
            package_id,
            module.to_owned(),
            function_1.to_owned(),
            type_args.clone(),
            call_args_1.clone(),
        );

        let iota_call_args_2 = call_args!(coin_2.object_id, 10)?;
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
        let cmd_2 = Command::new_move_call(
            package_id,
            module.to_owned(),
            function_2.to_owned(),
            type_args,
            call_args_2,
        );
        pt_builder.command(cmd_1);
        pt_builder.command(cmd_2);
        let pt = pt_builder.finish();

        let tx = Transaction::new_programmable(signer, vec![gas], pt, 10_000_000, 1000);

        let signed_transaction = to_sender_signed_transaction(tx, &key);

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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();
        let recipient_1 = Address::random();
        let recipient_2 = Address::random();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient_1,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, transfer_request, &key).await;
        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient_2,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &key).await;

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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();
        let recipient = Address::random();
        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        let filter = TransactionFilter::FromOrToAddress { addr: recipient };

        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        let digest_1 = execute_tx_must_succeed(client, transfer_request, &key).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient,
                Some(150_000_000.into()),
            )
            .await
            .unwrap();
        let digest_2 = execute_tx_must_succeed(client, transfer_request, &key).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient,
                Some(160_000_000.into()),
            )
            .await
            .unwrap();
        let digest_3 =
            execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &key).await;

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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();
        let recipient_1 = Address::random();
        let recipient_2 = Address::random();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient_1,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_must_succeed(client, transfer_request, &key).await;
        let transfer_request = client
            .transfer_iota(
                address,
                gas.object_id,
                5_000_000.into(),
                recipient_2,
                Some(100_000_000.into()),
            )
            .await
            .unwrap();
        execute_tx_and_wait_for_indexer_checkpoint(client, store, transfer_request, &key).await;

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
    expected_transactions_digests: &[TransactionDigest],
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
    expected_transactions_digests: &[TransactionDigest],
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
    expected_transactions_digests: &[TransactionDigest],
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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        // Create a bag object
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            let bag = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::BAG_MODULE,
                Identifier::from_static("new"),
                vec![],
                vec![],
            );

            let field_name_argument = builder.pure(0u64).expect("valid pure");
            let field_value_argument = builder.pure(0u64).expect("valid pure");

            let _ = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::BAG_MODULE,
                Identifier::from_static("add"),
                vec![TypeTag::U64, TypeTag::U64],
                vec![bag, field_name_argument, field_value_argument],
            );

            builder.transfer_arg(address, bag);
            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(address, gas, 1000);
        let tx_data = tx_builder.programmable(pt).build();
        let signed_transaction = to_sender_signed_transaction(tx_data, &key);

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
                    Some(IotaObjectDataFilter::StructType(StructTag::new_bag())),
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
            .get_dynamic_fields(bag_object_ref.object_id, None, None)
            .await
            .expect("failed to get dynamic fields");

        assert!(
            !dynamic_fields.data.is_empty(),
            "dynamic field was not added"
        );

        Ok(())
    })
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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

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
                ObjectId::FRAMEWORK,
                Identifier::OBJECT_BAG_MODULE,
                Identifier::from_static("new"),
                vec![],
                vec![],
            );

            let field_name_argument = builder.pure(0u64).expect("valid pure");
            let field_value_argument = builder
                .input(CallArg::ImmutableOrOwned(child_object))
                .unwrap();

            let _ = builder.programmable_move_call(
                ObjectId::FRAMEWORK,
                Identifier::OBJECT_BAG_MODULE,
                Identifier::from_static("add"),
                vec![
                    TypeTag::U64,
                    TypeTag::Struct(Box::new(StructTag::new_gas_coin())),
                ],
                vec![bag, field_name_argument, field_value_argument],
            );

            builder.transfer_arg(address, bag);
            builder.finish()
        };

        let tx_builder = TestTransactionBuilder::new(address, gas, 1000);
        let tx_data = tx_builder.programmable(pt).build();
        let signed_transaction = to_sender_signed_transaction(tx_data, &key);

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
                    Some(IotaObjectDataFilter::StructType(StructTag::new_object_bag())),
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
            type_tag: TypeTag::U64,
            value: IotaMoveValue::from(MoveValue::U64(0u64)).to_json_value(),
        };

        // Verify that the dynamic field was successfully added
        let dynamic_fields = client
            .get_dynamic_field_object(bag_object_ref.object_id, name)
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
        let (address, key): (_, AccountPrivateKey) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000),
                address,
            )
            .await;
        let iota_client = cluster.wallet.get_client().await.unwrap();

        indexer_wait_for_object(client, gas.object_id, gas.version).await;

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

        let package_id = ObjectId::STD;
        let module = Identifier::from_static("address");
        let function = Identifier::from_static("length");

        let mut pt_builder = ProgrammableTransactionBuilder::new();
        pt_builder.move_call(package_id, module, function, vec![], vec![])?;
        let pt = pt_builder.finish();

        let tx = Transaction::new_programmable(signer, vec![gas], pt, 10_000_000, 1_000);
        let signed_transaction = to_sender_signed_transaction(tx, &key);

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
        assert_eq!(tx_data_v1.sender, Address::ZERO);

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

#[test]
fn query_transaction_blocks_move_function_rejects_non_identifier() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let package = ObjectId::FRAMEWORK;

        let invalid_payloads = [
            "coin' OR 1=1) --",
            "coin'; SELECT 1",
            "coin)",
            "coin%",
            "coin--",
            "",
        ];

        for invalid_payload in invalid_payloads {
            let module_filter = TransactionFilterV2::MoveFunction {
                package,
                module: Some(invalid_payload.to_string()),
                function: None,
            };
            let res = client
                .query_transaction_blocks_v2(
                    IotaTransactionBlockResponseQueryV2::new_with_filter(module_filter),
                    None,
                    Some(20),
                    Some(true),
                )
                .await;
            assert!(
                res.is_err(),
                "module payload {invalid_payload:?} was not rejected"
            );
            let err = res.unwrap_err().to_string();
            assert!(
                err.contains("Invalid module name"),
                "module payload {invalid_payload:?} failed with an unexpected error: {err}"
            );

            let function_filter = TransactionFilterV2::MoveFunction {
                package,
                module: Some("coin".to_string()),
                function: Some(invalid_payload.to_string()),
            };
            let res = client
                .query_transaction_blocks_v2(
                    IotaTransactionBlockResponseQueryV2::new_with_filter(function_filter),
                    None,
                    Some(20),
                    Some(true),
                )
                .await;
            assert!(
                res.is_err(),
                "function payload {invalid_payload:?} was not rejected"
            );
            let err = res.unwrap_err().to_string();
            assert!(
                err.contains("Invalid function name"),
                "function payload {invalid_payload:?} failed with an unexpected error: {err}"
            );
        }

        let valid_filter = TransactionFilterV2::MoveFunction {
            package,
            module: Some("coin".to_string()),
            function: Some("split".to_string()),
        };
        client
            .query_transaction_blocks_v2(
                IotaTransactionBlockResponseQueryV2::new_with_filter(valid_filter),
                None,
                Some(20),
                Some(true),
            )
            .await
            .expect("valid MoveFunction filter must succeed");
    })
}

/// The types in the `type_filter` test package differ only in ways that an
/// unsecaped `LIKE` prefix match cannot tell apart, so a filter for one of them
/// must not return the others.
#[test]
fn get_owned_objects_matches_struct_type_exactly() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, key): (_, AccountPrivateKey) = get_key_pair();
        let keypair = SimpleKeypair::from(key);
        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        let (package_ref, publish_resp) =
            publish_test_move_package(client, address, &keypair, "type_filter")
                .await
                .expect("publishing the test package should succeed");
        indexer_wait_for_transaction(publish_resp.digest, store, client).await;

        let package_id = package_ref.object_id;
        let struct_tag = |name: &str| {
            StructTag::from_str(&format!("{package_id}::type_filter::{name}"))
                .expect("valid struct tag")
        };
        let owned_types = async |filter: IotaObjectDataFilter| {
            let objects: ObjectsPage = client
                .get_owned_objects(
                    address,
                    Some(IotaObjectResponseQuery::new(
                        Some(filter),
                        Some(IotaObjectDataOptions::new().with_type()),
                    )),
                    None,
                    None,
                )
                .await
                .expect("querying owned objects should succeed");

            objects
                .data
                .iter()
                .map(|object| {
                    object
                        .object()
                        .expect("object data")
                        .object_type()
                        .expect("object type")
                        .to_string()
                })
                .sorted()
                .collect_vec()
        };

        let my_type = format!("{package_id}::type_filter::My_Type");
        let my_x_type = format!("{package_id}::type_filter::MyXType");
        let my_type_extra = format!("{package_id}::type_filter::My_TypeExtra");

        // `_` is the `LIKE` wildcard for any single character, and `MyXType`
        // and `My_TypeExtra` both share a prefix with `My_Type`.
        assert_eq!(
            owned_types(IotaObjectDataFilter::StructType(struct_tag("My_Type"))).await,
            vec![my_type.clone()]
        );

        assert_eq!(
            owned_types(IotaObjectDataFilter::MatchAny(vec![
                IotaObjectDataFilter::StructType(struct_tag("My_Type")),
                IotaObjectDataFilter::StructType(struct_tag("MyXType")),
            ]))
            .await,
            vec![my_type.clone(), my_x_type.clone()]
                .into_iter()
                .sorted()
                .collect_vec()
        );

        let without_my_type = owned_types(IotaObjectDataFilter::MatchNone(vec![
            IotaObjectDataFilter::StructType(struct_tag("My_Type")),
        ]))
        .await;
        assert!(!without_my_type.contains(&my_type));
        assert!(without_my_type.contains(&my_x_type));
        assert!(without_my_type.contains(&my_type_extra));

        // A filter that carries type parameters only matches that exact
        // instantiation.
        let gas_coin = "0x2::coin::Coin<0x2::iota::IOTA>";
        assert!(
            !owned_types(IotaObjectDataFilter::StructType(
                StructTag::from_str(gas_coin).expect("valid struct tag")
            ))
            .await
            .is_empty()
        );
        assert!(
            owned_types(IotaObjectDataFilter::StructType(
                StructTag::from_str("0x2::coin::Coin<0x2::iota::NOT_IOTA>")
                    .expect("valid struct tag")
            ))
            .await
            .is_empty()
        );

        // Matching any of no types matches nothing.
        assert!(
            owned_types(IotaObjectDataFilter::MatchAny(vec![]))
                .await
                .is_empty()
        );
    })
}
