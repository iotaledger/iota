// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(msim))]
use std::str::FromStr;

use iota_json::{call_args, type_args};
use iota_json_rpc_api::IndexerApiClient;
use iota_json_rpc_types::{
    EventFilter, EventPage, IotaMoveValue, IotaObjectDataFilter, IotaObjectDataOptions,
    IotaObjectResponseQuery, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    IotaTransactionBlockResponseQuery, ObjectsPage, TransactionFilter,
};
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_swarm_config::genesis_config::AccountConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    collection_types::VecMap,
    crypto::deterministic_random_account_key,
    digests::TransactionDigest,
    dynamic_field::DynamicFieldName,
    gas_coin::GAS,
    id::UID,
    object::{Data, MoveObject, OBJECT_START_VERSION, ObjectInner, Owner},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    stardust::output::{Irc27Metadata, Nft},
    transaction::{CallArg, Command, ObjectArg, TransactionData},
};
use move_core_types::{
    annotated_value::MoveValue,
    identifier::Identifier,
    language_storage::{StructTag, TypeTag},
};
use test_cluster::TestClusterBuilder;
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

    let nft_move_object = {
        MoveObject::new_from_execution(
            MoveObjectType::stardust_nft(),
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

#[sim_test]
async fn query_events_no_events_descending() {
    let cluster = TestClusterBuilder::new().build().await;

    let client = cluster.rpc_client();

    let indexer_events = client
        .query_events(
            EventFilter::Sender(IotaAddress::random_for_testing_only()),
            None,
            None,
            Some(true),
        )
        .await
        .unwrap();

    assert_eq!(indexer_events, EventPage::empty())
}

#[sim_test]
async fn query_events_no_events_ascending() {
    let cluster = TestClusterBuilder::new().build().await;

    let client = cluster.rpc_client();

    let indexer_events = client
        .query_events(
            EventFilter::Sender(IotaAddress::random_for_testing_only()),
            None,
            None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(indexer_events, EventPage::empty())
}

#[sim_test]
async fn query_events() {
    let cluster = TestClusterBuilder::new().build().await;

    let client = cluster.rpc_client();

    let result = client
        .query_events(EventFilter::Sender(IotaAddress::ZERO), None, None, None)
        .await;

    let event_page = result.unwrap();

    for event in event_page.data {
        assert_eq!(event.sender, IotaAddress::ZERO);
    }
}

#[sim_test]
async fn query_events_unsupported_filters() {
    let cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(1000)
        .build()
        .await;

    let client = cluster.rpc_client();

    let unsupported_filters = vec![
        EventFilter::Any(vec![]),
        EventFilter::And(
            Box::new(EventFilter::Any(vec![])),
            Box::new(EventFilter::Any(vec![])),
        ),
        EventFilter::Or(
            Box::new(EventFilter::Any(vec![])),
            Box::new(EventFilter::Any(vec![])),
        ),
        EventFilter::MoveEventField {
            path: String::default(),
            value: true.into(),
        },
        EventFilter::Package(ObjectID::random()),
    ];

    for event_filter in unsupported_filters {
        let result = client.query_events(event_filter, None, None, None).await;

        let err = result.unwrap_err();
        let msg = err.to_string();

        assert!(msg.contains("This query type is not supported by the full node"));
    }
}

#[sim_test]
async fn test_get_owned_objects() -> Result<(), anyhow::Error> {
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

    Ok(())
}

#[sim_test]
async fn test_query_transaction_blocks_pagination() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;

    let context = &cluster.wallet;

    let mut tx_responses: Vec<IotaTransactionBlockResponse> = Vec::new();

    let client = context.get_client().await.unwrap();

    for address in cluster.get_addresses() {
        let objects = client
            .read_api()
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
        let gas_id = objects.last().unwrap().object().unwrap().object_id;

        assert_eq!(5, objects.len());

        // Make some transactions
        for obj in &objects[..objects.len() - 1] {
            let oref = obj.object().unwrap();
            let data = client
                .transaction_builder()
                .transfer_object(address, oref.object_id, Some(gas_id), 1_000_000, address)
                .await?;
            let tx = cluster.wallet.sign_transaction(&data);

            let response = client
                .quorum_driver_api()
                .execute_transaction_block(
                    tx,
                    IotaTransactionBlockResponseOptions::new(),
                    Some(ExecuteTransactionRequestType::WaitForLocalExecution),
                )
                .await
                .unwrap();

            tx_responses.push(response);
        }
    }

    // test get_recent_transactions with smaller range with address filter
    let query = IotaTransactionBlockResponseQuery {
        options: Some(IotaTransactionBlockResponseOptions {
            show_input: true,
            show_effects: true,
            show_events: true,
            ..Default::default()
        }),
        filter: Some(TransactionFilter::FromAddress(cluster.get_address_0())),
    };

    let tx = client
        .read_api()
        .query_transaction_blocks(query.clone(), None, Some(3), true)
        .await
        .unwrap();
    assert_eq!(3, tx.data.len());
    assert!(tx.data[0].transaction.is_some());
    assert!(tx.data[0].effects.is_some());
    assert!(tx.data[0].events.is_some());
    assert!(tx.has_next_page);

    // Read the next page for the last transaction
    let next_page = client
        .read_api()
        .query_transaction_blocks(query, tx.next_cursor, None, true)
        .await
        .unwrap();

    assert_eq!(1, next_page.data.len());

    // test get all transactions paged without address filter
    let first_page = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::default(),
            None,
            Some(5),
            false,
        )
        .await
        .unwrap();
    assert_eq!(5, first_page.data.len());
    assert!(first_page.has_next_page);

    let second_page = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::default(),
            first_page.next_cursor,
            None,
            false,
        )
        .await
        .unwrap();
    assert!(second_page.data.len() > 5);

    let mut all_txs = first_page.data.clone();
    all_txs.extend(second_page.data);

    // test get 10 transactions paged
    let latest = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::default(),
            None,
            Some(10),
            false,
        )
        .await
        .unwrap();
    assert_eq!(10, latest.data.len());
    assert_eq!(Some(all_txs[9].digest), latest.next_cursor);
    assert_eq!(all_txs[0..10], latest.data);
    assert!(latest.has_next_page);

    // test get from address txs in ascending order
    let address_txs_asc = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::new_with_filter(TransactionFilter::FromAddress(
                cluster.get_address_0(),
            )),
            None,
            None,
            false,
        )
        .await
        .unwrap();
    assert_eq!(4, address_txs_asc.data.len());

    // test get from address txs in descending order
    let address_txs_desc = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::new_with_filter(TransactionFilter::FromAddress(
                cluster.get_address_0(),
            )),
            None,
            None,
            true,
        )
        .await
        .unwrap();
    assert_eq!(4, address_txs_desc.data.len());

    // test get from address txs in both ordering are the same.
    let mut data_asc = address_txs_asc.data;
    data_asc.reverse();
    assert_eq!(data_asc, address_txs_desc.data);

    // test get_recent_transactions
    let tx = client
        .read_api()
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::default(),
            None,
            Some(20),
            true,
        )
        .await
        .unwrap();
    assert_eq!(20, tx.data.len());

    Ok(())
}

#[sim_test]
async fn test_query_transaction_blocks() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let context = &cluster.wallet;
    let client = context.get_client().await.unwrap();

    let address = cluster.get_address_0();
    let objects = client
        .read_api()
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

    // make 2 move calls of same package & module, but different functions
    let package_id = ObjectID::new(IOTA_FRAMEWORK_ADDRESS.into_bytes());
    let coin = objects.first().unwrap();
    let coin_2 = &objects[1];
    let signer = cluster.wallet.active_address().unwrap();

    let tx_builder = client.transaction_builder().clone();
    let mut pt_builder = ProgrammableTransactionBuilder::new();
    let gas = objects.last().unwrap().object().unwrap().object_ref();

    let module = Identifier::from_str("pay")?;
    let function_1 = Identifier::from_str("split")?;
    let function_2 = Identifier::from_str("divide_and_keep")?;

    let iota_type_args = type_args![GAS::type_tag()]?;
    let type_args = iota_type_args
        .into_iter()
        .map(|ty| ty.try_into())
        .collect::<Result<Vec<_>, _>>()?;

    let iota_call_args_1 = call_args!(coin.data.clone().unwrap().object_id, 10)?;
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

    let iota_call_args_2 = call_args!(coin_2.data.clone().unwrap().object_id, 10)?;
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
    let signed_data = cluster.wallet.sign_transaction(&tx_data);
    let _response = client
        .quorum_driver_api()
        .execute_transaction_block(
            signed_data,
            IotaTransactionBlockResponseOptions::new(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    // match with None function, the DB should have 2 records, but both points to
    // the same tx
    let filter = TransactionFilter::MoveFunction {
        package: package_id,
        module: Some("pay".to_string()),
        function: None,
    };
    let move_call_query = IotaTransactionBlockResponseQuery::new_with_filter(filter);
    let tx = client
        .read_api()
        .query_transaction_blocks(move_call_query, None, Some(20), true)
        .await
        .unwrap();
    // verify that only 1 tx is returned and no
    // IotaRpcInputError::ContainsDuplicates error
    assert_eq!(1, tx.data.len());
    Ok(())
}

#[sim_test]
async fn test_get_dynamic_fields() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let context = &cluster.wallet;
    let rpc_client = cluster.rpc_client();
    let signer = cluster.get_address_0();

    // Get the first object owned by the signer (to act as parent UID object)
    let address = cluster.get_address_0();
    let objects = context
        .get_all_gas_objects_owned_by_address(address)
        .await
        .unwrap();

    let gas = objects.last().unwrap(); // Get gas object

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

    let tx_builder = TestTransactionBuilder::new(signer, *gas, 1000);
    let txn = context.sign_transaction(&tx_builder.programmable(pt).build());
    let _ = context.execute_transaction_must_succeed(txn).await;

    // Wait for the transaction to be executed
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Find the bag object
    let objects: ObjectsPage = rpc_client
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
    let dynamic_fields = rpc_client
        .get_dynamic_fields(bag_object_ref.0, None, None)
        .await
        .expect("Failed to get dynamic fields");

    assert!(
        !dynamic_fields.data.is_empty(),
        "Dynamic field was not added"
    );

    Ok(())
}

#[sim_test]
async fn test_get_dynamic_field_object() -> Result<(), anyhow::Error> {
    let cluster = TestClusterBuilder::new().build().await;
    let context = &cluster.wallet;
    let rpc_client = cluster.rpc_client();
    let signer = cluster.get_address_0();

    // Get the first object owned by the signer (to act as parent UID object)
    let address = cluster.get_address_0();
    let objects = context
        .get_all_gas_objects_owned_by_address(address)
        .await
        .unwrap();

    let child_object = &objects[0]; // Get child object
    let gas = objects.last().unwrap(); // Get gas object

    // Create a bag object
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
            .input(CallArg::Object(ObjectArg::ImmOrOwnedObject(*child_object)))
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

    let tx_builder = TestTransactionBuilder::new(signer, *gas, 1000);
    let txn = context.sign_transaction(&tx_builder.programmable(pt).build());
    let _ = context.execute_transaction_must_succeed(txn).await;

    // Wait for the transaction to be executed
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Find the bag object
    let objects: ObjectsPage = rpc_client
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
    let dynamic_fields = rpc_client
        .get_dynamic_field_object(bag_object_ref.0, name)
        .await
        .expect("Failed to get dynamic field object");

    assert!(
        dynamic_fields.data.is_some(),
        "Dynamic field object was not added"
    );

    Ok(())
}
