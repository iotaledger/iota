// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::SystemTime};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{IndexerApiClient, WriteApiClient};
use iota_json_rpc_types::{
    EventFilter, EventPage, IotaMoveValue, IotaObjectDataFilter, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockData, IotaTransactionBlockKind,
    IotaTransactionBlockResponseOptions, IotaTransactionBlockResponseQuery, IotaTransactionKind,
    ObjectsPage, TransactionFilter,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS, MOVE_STDLIB_PACKAGE_ID,
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    dynamic_field::DynamicFieldName,
    gas_coin::GAS,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, Command, ObjectArg, TransactionData},
    utils::to_sender_signed_transaction,
};
use move_core_types::{
    annotated_value::MoveValue,
    identifier::Identifier,
    language_storage::{StructTag, TypeTag},
};

use crate::common::{
    ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_latest_checkpoint,
    indexer_wait_for_object, indexer_wait_for_transaction, rpc_call_error_msg_matches,
    start_test_cluster_with_read_write_indexer,
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

        let supported_filters = vec![
            EventFilter::Sender(IotaAddress::ZERO),
            EventFilter::Transaction(TransactionDigest::ZERO),
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
            let result = client.query_events(event_filter, None, None, None).await;
            assert!(result.is_ok());
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

        let mut tx_responses = vec![];
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
                    Some(ExecuteTransactionRequestType::WaitForLocalExecution),
                )
                .await?;

            tx_responses.push(res)
        }

        let tx_res = tx_responses.pop().unwrap();

        indexer_wait_for_transaction(tx_res.digest, store, client).await;

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

        let first_page = iota_client
            .read_api()
            .query_transaction_blocks(query.clone(), None, Some(3), true)
            .await
            .unwrap();
        assert_eq!(3, first_page.data.len());
        assert!(first_page.data[0].transaction.is_some());
        assert!(first_page.data[0].effects.is_some());
        assert!(first_page.data[0].events.is_some());
        assert!(first_page.has_next_page);

        // Read the next page for the last transaction
        let next_page = iota_client
            .read_api()
            .query_transaction_blocks(query, first_page.next_cursor, None, true)
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
        let filter = TransactionFilter::FromAddress(signer);
        let move_call_query = IotaTransactionBlockResponseQuery::new_with_filter(filter);
        let res = client
            .query_transaction_blocks(move_call_query, None, Some(20), Some(true))
            .await
            .unwrap();

        assert_eq!(1, res.data.len());

        Ok(())
    })
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
            .expect("Failed to get dynamic fields");

        assert!(
            !dynamic_fields.data.is_empty(),
            "Dynamic field was not added"
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
            .expect("Failed to get dynamic field object");

        assert!(
            dynamic_fields.data.is_some(),
            "Dynamic field object was not added"
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
            TransactionFilter::TransactionKind(IotaTransactionKind::ProgrammableTransaction);
        let query = IotaTransactionBlockResponseQuery::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks(query, None, Some(1), Some(true))
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
        let filter = TransactionFilter::TransactionKind(IotaTransactionKind::Genesis);
        let query = IotaTransactionBlockResponseQuery::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks(query, None, Some(2), Some(false))
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
        let filter = TransactionFilter::TransactionKind(IotaTransactionKind::SystemTransaction);
        let query = IotaTransactionBlockResponseQuery::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks(query, None, Some(1), Some(true))
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
            TransactionFilter::TransactionKind(IotaTransactionKind::ConsensusCommitPrologueV1);
        let query = IotaTransactionBlockResponseQuery::new(Some(filter), Some(options.clone()));
        let res = client
            .query_transaction_blocks(query, None, Some(1), Some(true))
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
        let filter = TransactionFilter::TransactionKindIn(vec![
            IotaTransactionKind::ConsensusCommitPrologueV1,
            IotaTransactionKind::ProgrammableTransaction,
        ]);
        let query = IotaTransactionBlockResponseQuery::new(Some(filter), Some(options));
        let res = client
            .query_transaction_blocks(query, None, Some(2), Some(true))
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
