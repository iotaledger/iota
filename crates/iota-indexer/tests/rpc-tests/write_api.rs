// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{path::Path, str::FromStr};

use fastcrypto::encoding::Base64;
use futures::{StreamExt, TryStreamExt, stream::FuturesUnordered};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, MoveCallParams,
    ObjectChange, RPCTransactionRequestParams, TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, Identifier, TypeTag,
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, TransactionKind},
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use move_core_types::{identifier::IdentStr, language_storage::StructTag};
use test_cluster::TestCluster;

use crate::common::{ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_object};
type TxBytes = Base64;
type Signatures = Vec<Base64>;

async fn prepare_and_sign_tx(
    sender: IotaAddress,
    receiver: IotaAddress,
    cluster: &TestCluster,
    client: &HttpClient,
    obj_id: ObjectID,
    gas: ObjectID,
) -> (TxBytes, Signatures) {
    let transaction_bytes = client
        .transfer_object(sender, obj_id, Some(gas), 10_000_000.into(), receiver)
        .await
        .unwrap();

    let (tx_bytes, signatures) = cluster
        .wallet
        .sign_transaction(&transaction_bytes.to_data().unwrap())
        .to_tx_bytes_and_signatures();

    (tx_bytes, signatures)
}

async fn get_objects_to_mutate(
    cluster: &TestCluster,
    address: IotaAddress,
) -> (Vec<ObjectID>, ObjectID) {
    let owned_objects = cluster.get_owned_objects(address, None).await.unwrap();

    let gas = owned_objects.last().unwrap().object_id().unwrap();

    let object_ids = owned_objects
        .iter()
        .take(owned_objects.len() - 1)
        .map(|obj| obj.object_id().unwrap())
        .collect();

    (object_ids, gas)
}

#[ignore = "https://github.com/iotaledger/iota/issues/6120"]
#[test]
fn dry_run_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let sender = cluster.get_address_0();
        let receiver = cluster.get_address_1();

        let (objects, gas) = get_objects_to_mutate(cluster, sender).await;

        let (tx_bytes, signatures) =
            prepare_and_sign_tx(sender, receiver, cluster, client, objects[0], gas).await;

        let dry_run_tx_block_resp = client
            .dry_run_transaction_block(tx_bytes.clone())
            .await
            .unwrap();

        let indexer_tx_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(
                    IotaTransactionBlockResponseOptions::new()
                        .with_effects()
                        .with_object_changes(),
                ),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();

        assert_eq!(
            *indexer_tx_response.effects.as_ref().unwrap().status(),
            IotaExecutionStatus::Success
        );

        assert_eq!(
            indexer_tx_response.object_changes.unwrap(),
            dry_run_tx_block_resp.object_changes
        )
    });
}

#[test]
fn dev_inspect_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, _): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let (obj_id, seq_num, digest) = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, obj_id, seq_num).await;

        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .transfer_object(receiver, (obj_id, seq_num, digest))
            .unwrap();
        let ptb = builder.finish();

        let indexer_devinspect_results = client
            .dev_inspect_transaction_block(
                sender,
                Base64::from_bytes(&bcs::to_bytes(&TransactionKind::programmable(ptb)).unwrap()),
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            *indexer_devinspect_results.effects.status(),
            IotaExecutionStatus::Success
        );

        let owner = indexer_devinspect_results
            .effects
            .mutated()
            .iter()
            .find_map(|obj| (obj.reference.object_id == obj_id).then_some(obj.owner))
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        let latest_checkpoint_seq_number = client
            .get_latest_checkpoint_sequence_number()
            .await
            .unwrap();

        // Ensure that the actual object sequence number remains unchanged after the
        // checkpoint advances
        indexer_wait_for_checkpoint(store, latest_checkpoint_seq_number.into_inner() + 1).await;

        let actual_object_data = client
            .get_object(obj_id, Some(IotaObjectDataOptions::new().with_owner()))
            .await
            .unwrap()
            .data
            .unwrap();

        assert_eq!(
            actual_object_data.version, seq_num,
            "The object sequence number should not mutate"
        );
        assert_eq!(
            actual_object_data.owner.unwrap(),
            Owner::AddressOwner(sender),
            "The initial owner of the object should not change"
        );
    });
}

#[test]
fn execute_transaction_block() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let addresses = cluster.get_addresses();
        let sender = addresses[2];
        let receiver = addresses[3];

        let (objects, gas) = get_objects_to_mutate(cluster, sender).await;

        let obj_id = objects[0];

        let (tx_bytes, signatures) =
            prepare_and_sign_tx(sender, receiver, cluster, client, obj_id, gas).await;

        let indexer_tx_response = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();
        assert_eq!(indexer_tx_response.status_ok(), Some(true));

        let (seq_num, owner) = indexer_tx_response
            .effects
            .unwrap()
            .mutated()
            .iter()
            .find_map(|obj| {
                (obj.reference.object_id == obj_id).then_some((obj.reference.version, obj.owner))
            })
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        let actual_object_info = client
            .get_object(obj_id, Some(IotaObjectDataOptions::new().with_owner()))
            .await
            .unwrap();

        assert_eq!(actual_object_info.data.as_ref().unwrap().version, seq_num);
        assert_eq!(
            actual_object_info.data.unwrap().owner.unwrap(),
            Owner::AddressOwner(receiver)
        );
    });
}

#[test]
fn test_consecutive_modifications_of_owned_object() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        client,
        ..
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();

        let consecutive_updates = 20;
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

        for _ in 0..consecutive_updates {
            let tx_data = client
                .split_coin_equal(
                    address,
                    coin_to_split.0,
                    2.into(),
                    Some(gas_ref.0),
                    10_000_000.into(),
                )
                .await?
                .to_data()
                .unwrap();
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

            assert_eq!(res.status_ok(), Some(true));
        }

        let objects = client
            .get_owned_objects(address, None, None, None)
            .await?
            .data;

        // 2 gas coins + N coins created by 'split_coin_equal'
        assert_eq!(consecutive_updates + 2, objects.len());
        Ok(())
    })
}

#[test]
fn test_consecutive_wrap_unwrap() -> Result<(), anyhow::Error> {
    // let _guard = telemetry_subscribers::TelemetryConfig::new()
    //     .with_env()
    //     .init();

    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
    } = ApiTestSetup::get_or_init();
    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 1).await;
        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
        let consecutive_updates = 3000;

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_basics_pkg(sender, &sender_kp, client).await;

        let package_id = res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();
        println!("Publish result: {:#?}", package_id);
        assert_eq!(res.status_ok(), Some(true));

        let upgrade_cap = res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Created { object_id, .. } => Some(object_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();
        println!("Upgrade cap: {:#?}", upgrade_cap);

        let (_, basic_obj) = create_basic_object(sender, &sender_kp, client, package_id).await?;
        println!("Basic obj: {:#?}", basic_obj);

        for n in 0..consecutive_updates {
            let (res, _) = wrap_basic_object(sender, &sender_kp, client, package_id, &basic_obj)
                .await
                .unwrap();

            assert_eq!(res.status_ok(), Some(true));
            let wrapped_obj_id = res
                .effects
                .unwrap()
                .created()
                .iter()
                .exactly_one()
                .unwrap()
                .object_id();
            println!("Wrapped obj {:#?}", wrapped_obj_id);

            let objects = client
                .get_owned_objects(sender, None, None, None)
                .await?
                .data
                .iter()
                .map(|o| o.object_id().unwrap())
                .sorted()
                .collect::<Vec<_>>();
            assert_eq!(
                objects,
                vec![wrapped_obj_id, *upgrade_cap, gas.0]
                    .into_iter()
                    .sorted()
                    .collect::<Vec<_>>()
            );

            let (res, _) =
                unwrap_basic_object(sender, &sender_kp, client, package_id, &wrapped_obj_id)
                    .await
                    .unwrap();
            assert_eq!(res.status_ok(), Some(true));

            let objects = client
                .get_owned_objects(sender, None, None, None)
                .await?
                .data
                .iter()
                .map(|o| o.object_id().unwrap())
                .sorted()
                .collect::<Vec<_>>();
            assert_eq!(
                objects,
                vec![basic_obj, *upgrade_cap, gas.0]
                    .into_iter()
                    .sorted()
                    .collect::<Vec<_>>()
            );

            println!("FINISHED PASS: {}", n);
        }
        Ok(())
    })
}

#[test]
fn test_execute_transactions_with_shared_objects() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_basics_pkg(sender, &sender_kp, client).await;

        let package_id = res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();

        let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, package_id)
            .await
            .unwrap();

        let res_1 = increment_counter(sender, &sender_kp, client, package_id, &counter_obj, None)
            .await
            .unwrap();
        assert_eq!(res_1.status_ok(), Some(true));

        // TODO: extend with subsequent call to the same object once race
        // conditions are fixed
    });
}

#[test]
fn test_parallel_indentical_requests() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_basics_pkg(sender, &sender_kp, client).await;
        assert_eq!(res.status_ok(), Some(true));

        let package_id: ObjectID = *res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();
        println!("Publish result: {:#?}", package_id);

        let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        let range = 0..10;
        let transaction_results: Vec<_> = range
            .map(|_| increment_counter(sender, &sender_kp, client, &package_id, &counter_obj, None))
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await
            .unwrap();

        let digests = transaction_results
            .iter()
            .map(|res| {
                (
                    res.digest,
                    res.effects.as_ref().unwrap().dependencies().to_vec(),
                )
            })
            .collect::<Vec<_>>(); // TODO: fix

        println!("FINISHED PASS: {:#?}", digests);
    });
}

#[test]
fn test_parallel_shared_object_updates() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();

        let rgp = cluster.get_reference_gas_price().await;

        let range = 0..10;
        let separate_gas_for_each_request: Vec<_> = futures::stream::iter(range)
            .then(|_| async {
                cluster
                    .fund_address_and_return_gas(rgp, Some(10_000_000_000), sender)
                    .await
                    .0
            })
            .collect()
            .await;

        let gas = cluster
            .fund_address_and_return_gas(rgp, Some(10_000_000_000), sender)
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_basics_pkg(sender, &sender_kp, client).await;
        assert_eq!(res.status_ok(), Some(true));

        let package_id: ObjectID = *res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();
        println!("Publish result: {:#?}", package_id);

        let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        println!("Starting concurrent requests\n\n\n\n");
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        println!("Slept\n\n\n\n");

        let transaction_results: Vec<_> = separate_gas_for_each_request
            .iter()
            .map(|gas| {
                increment_counter(
                    sender,
                    &sender_kp,
                    client,
                    &package_id,
                    &counter_obj,
                    Some(*gas),
                )
            })
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await
            .unwrap();

        let digests = transaction_results
            .iter()
            .map(|res| {
                (
                    res.digest,
                    res.effects.as_ref().unwrap().dependencies().to_vec(),
                )
            })
            .collect::<Vec<_>>(); // TODO: fix

        println!("FINISHED PASS: {:#?}", digests);
    });
}

#[test]
fn test_repeatedly_update_display() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async {
        let consecutive_updates = 150;
        indexer_wait_for_checkpoint(store, 1).await;

        let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();

        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas.0, gas.1).await;

        let res = deploy_bear_pkg(sender, &sender_kp, client).await;
        assert_eq!(res.status_ok(), Some(true));

        let package_id = res
            .object_changes
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .exactly_one()
            .unwrap();
        println!("Publish result: {:#?}", package_id);

        let display_obj_id = ObjectID::from_hex_literal(
            res.events.unwrap().data[0].parsed_json.as_object().unwrap()["id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        println!("Display object: {:#?}", display_obj_id);

        let (res, _) = create_new_bear(sender, &sender_kp, client, package_id, "bear name")
            .await
            .unwrap();
        assert_eq!(res.status_ok(), Some(true));

        let bear_id = res
            .effects
            .unwrap()
            .created()
            .iter()
            .exactly_one()
            .unwrap()
            .object_id();

        println!("Bear object: {:#?}", bear_id);

        let bear_type_tag = TypeTag::Struct(Box::new(StructTag {
            address: (*package_id).into(),
            name: IdentStr::new("DemoBear").unwrap().into(),
            module: IdentStr::new("demo_bear").unwrap().into(),
            type_params: Vec::new(),
        }));

        for n in 0..consecutive_updates {
            let new_bear_description = format!("Bear description {n}");

            let (res, _) = update_display_object(
                sender,
                &sender_kp,
                client,
                &display_obj_id,
                bear_type_tag.clone(),
                "description",
                &new_bear_description,
            )
            .await
            .unwrap();
            assert_eq!(res.status_ok(), Some(true));

            let (res, _) = bump_display_object_version(
                sender,
                &sender_kp,
                client,
                &display_obj_id,
                bear_type_tag.clone(),
            )
            .await
            .unwrap();
            assert_eq!(res.status_ok(), Some(true));

            let res = client
                .get_object(bear_id, Some(IotaObjectDataOptions::new().with_display()))
                .await
                .unwrap();

            // println!("{:#?}", res);

            let actual_description =
                res.data.unwrap().display.unwrap().data.unwrap()["description"].clone();

            assert_eq!(actual_description, new_bear_description);
        }
    });
}

async fn update_display_object(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    display_object_id: &ObjectID,
    display_obj_type_tag: TypeTag,
    name_to_update: &str,
    new_value: &str,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "display".to_string();
    let function = "edit".to_string();

    let tx_bytes: TransactionBlockBytes = {
        let rpc_params = RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
            package_object_id: IOTA_FRAMEWORK_PACKAGE_ID,
            module: module.clone(),
            function: function.clone(),
            type_arguments: type_args![display_obj_type_tag].unwrap(),
            arguments: call_args!(
                display_object_id,
                name_to_update.to_string(),
                new_value.to_string()
            )
            .unwrap(),
        });
        client
            .batch_transaction(address, vec![rpc_params], None, 3_000_000_000.into(), None)
            .await
            .unwrap()
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok((res, request_start_ts_ms))
}

async fn bump_display_object_version(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    display_object_id: &ObjectID,
    display_obj_type_tag: TypeTag,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "display".to_string();
    let function = "update_version".to_string();

    let tx_bytes: TransactionBlockBytes = {
        let rpc_params = RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
            package_object_id: IOTA_FRAMEWORK_PACKAGE_ID,
            module: module.clone(),
            function: function.clone(),
            type_arguments: type_args![display_obj_type_tag].unwrap(),
            arguments: call_args!(display_object_id).unwrap(),
        });
        client
            .batch_transaction(address, vec![rpc_params], None, 3_000_000_000.into(), None)
            .await
            .unwrap()
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok((res, request_start_ts_ms))
}

async fn create_new_bear(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    name: &str,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "demo_bear".to_string();
    let function = "new".to_string();

    let gas = client
        .get_all_coins(address, None, None)
        .await
        .unwrap()
        .data[0]
        .object_ref();

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let name_arg = builder.input(CallArg::Pure(bcs::to_bytes(name).unwrap()))?;
        let bear = builder.programmable_move_call(
            *package_id,
            Identifier::from_str(&module)?,
            Identifier::from_str(&function)?,
            vec![],
            vec![name_arg],
        );

        builder.transfer_arg(address, bear);
        builder.finish()
    };

    let tx_builder = TestTransactionBuilder::new(address, gas, 1000);
    let tx_data = tx_builder.programmable(pt).build();
    let signed_transaction = to_sender_signed_transaction(tx_data, addres_kp);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok((res, request_start_ts_ms))
}

async fn create_counter_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, ObjectID), anyhow::Error> {
    let module = "counter".to_string();
    let tx_bytes: TransactionBlockBytes = client
        .move_call(
            address,
            *package_id,
            module.clone(),
            "create".to_string(),
            type_args![].unwrap(),
            call_args!().unwrap(),
            None,
            10_000_000.into(),
            None,
        )
        .await?;
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    let counter_obj_id = res
        .effects
        .as_ref()
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();
    Ok((res, counter_obj_id))
}

async fn create_basic_object(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
) -> Result<(TransactionDigest, ObjectID), anyhow::Error> {
    let module = "object_basics".to_string();

    let tx_bytes = client
        .move_call(
            address,
            *package_id,
            module.clone(),
            "create".to_string(),
            type_args![].unwrap(),
            call_args!(0, address).unwrap(),
            None,
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    let counter_obj_id = res
        .effects
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();
    Ok((res.digest, counter_obj_id))
}

async fn increment_counter(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    counter_id: &ObjectID,
    gas: Option<ObjectID>,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let module = "counter".to_string();
    let function = "increment".to_string();
    let tx_bytes = client
        .move_call(
            address,
            *package_id,
            module.clone(),
            function.clone(),
            type_args![].unwrap(),
            call_args!(counter_id).unwrap(),
            gas,
            10_000_000.into(),
            None,
        )
        .await
        .unwrap();
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok(res)
}

async fn wrap_basic_object(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    object_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "object_basics".to_string();
    let function = "wrap".to_string();

    let tx_bytes: TransactionBlockBytes = {
        let rpc_params = RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
            package_object_id: *package_id,
            module: module.clone(),
            function: function.clone(),
            type_arguments: type_args![].unwrap(),
            arguments: call_args!(object_id).unwrap(),
        });
        client
            .batch_transaction(address, vec![rpc_params], None, 3_000_000_000.into(), None)
            .await
            .unwrap()
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok((res, request_start_ts_ms))
}

async fn unwrap_basic_object(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    object_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "object_basics".to_string();
    let function = "unwrap".to_string();

    let tx_bytes: TransactionBlockBytes = {
        let rpc_params = RPCTransactionRequestParams::MoveCallRequestParams(MoveCallParams {
            package_object_id: *package_id,
            module: module.clone(),
            function: function.clone(),
            type_arguments: type_args![].unwrap(),
            arguments: call_args!(object_id).unwrap(),
        });
        client
            .batch_transaction(address, vec![rpc_params], None, 3_000_000_000.into(), None)
            .await
            .unwrap()
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis();
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();
    Ok((res, request_start_ts_ms))
}

async fn deploy_basics_pkg(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
) -> IotaTransactionBlockResponse {
    deploy_package(address, address_kp, client, "../../examples/move/basics").await
}

async fn deploy_bear_pkg(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
) -> IotaTransactionBlockResponse {
    deploy_package(
        address,
        addres_kp,
        client,
        "../../examples/trading/contracts/demo",
    )
    .await
}

async fn deploy_package(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    pkg_path: &str,
) -> IotaTransactionBlockResponse {
    let compiled_package = BuildConfig::new_for_testing()
        .build(Path::new(pkg_path))
        .unwrap();
    let compiled_modules_bytes =
        compiled_package.get_package_base64(/* with_unpublished_deps */ false);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let tx_bytes: TransactionBlockBytes = client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            None,
            100_000_000.into(),
        )
        .await
        .unwrap();

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), address_kp);

    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap()
}
