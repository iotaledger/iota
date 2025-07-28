// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    str::FromStr,
};

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, expression::SelectableHelper};
use fastcrypto::encoding::Base64;
use futures::{StreamExt, TryStreamExt, stream::FuturesUnordered};
use iota_indexer::{
    errors::IndexerError, models::transactions::TxGlobalOrder, read_only_blocking,
    schema::tx_global_order,
};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID, Identifier, TypeTag,
    base_types::{IotaAddress, ObjectID, ObjectRef},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    gas_coin::NANOS_PER_IOTA,
    object::Owner,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{CallArg, TransactionKind},
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;
use move_core_types::{identifier::IdentStr, language_storage::StructTag};

use crate::{
    coin_api::execute_move_call,
    common::{ApiTestSetup, indexer_wait_for_checkpoint, indexer_wait_for_object},
};
type TxBytes = Base64;
type Signatures = Vec<Base64>;

// Specifies the number of attempts for test cases that may fail
// nondeterministically, such as those affected by race conditions. Increasing
// this value improves the likelihood of catching errors but also increases test
// execution time.
const NON_DETERMINISTIC_TESTS_REPETITIONS: usize = 20;

async fn prepare_and_sign_object_transfer_tx(
    sender: IotaAddress,
    sender_key_pair: AccountKeyPair,
    receiver: IotaAddress,
    object_to_transfer: ObjectRef,
    gas: ObjectRef,
) -> (TxBytes, Signatures) {
    let tx_builder = TestTransactionBuilder::new(sender, gas, 1000);
    let tx_data = tx_builder.transfer(object_to_transfer, receiver).build();
    let signed_transaction = to_sender_signed_transaction(tx_data, &sender_key_pair);
    signed_transaction.to_tx_bytes_and_signatures()
}

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

        let (sender, key_pair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let object_to_transfer = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, object_to_transfer.0, object_to_transfer.1).await;

        let (tx_bytes, signatures) = prepare_and_sign_object_transfer_tx(
            sender,
            key_pair,
            receiver,
            object_to_transfer,
            gas_ref,
        )
        .await;

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
        );

        assert!(
            dry_run_tx_block_resp
                .effects
                .mutated()
                .iter()
                .any(|obj| obj.reference.object_id == object_to_transfer.0)
        );
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

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

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

        let (sender, key_pair): (_, AccountKeyPair) = get_key_pair();
        let (receiver, _): (_, AccountKeyPair) = get_key_pair();

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let object_to_transfer = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(NANOS_PER_IOTA),
                sender,
            )
            .await;
        indexer_wait_for_object(client, object_to_transfer.0, object_to_transfer.1).await;

        let object_to_transfer_id = object_to_transfer.0;

        let (tx_bytes, signatures) = prepare_and_sign_object_transfer_tx(
            sender,
            key_pair,
            receiver,
            object_to_transfer,
            gas_ref,
        )
        .await;

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
                (obj.reference.object_id == object_to_transfer_id)
                    .then_some((obj.reference.version, obj.owner))
            })
            .unwrap();

        assert_eq!(owner, Owner::AddressOwner(receiver));

        let actual_object_info = client
            .get_object(
                object_to_transfer_id,
                Some(IotaObjectDataOptions::new().with_owner()),
            )
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

        for _ in 0..NON_DETERMINISTIC_TESTS_REPETITIONS {
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
                    Some(IotaTransactionBlockResponseOptions::full_content()),
                    None,
                )
                .await?;
            assert_eq!(res.status_ok(), Some(true));
        }

        let objects = client
            .get_owned_objects(address, None, None, None)
            .await?
            .data;

        // 2 gas coins + N coins created by 'split_coin_equal'
        assert_eq!(NON_DETERMINISTIC_TESTS_REPETITIONS + 2, objects.len());
        Ok(())
    })
}

#[test]
fn test_consecutive_wrap_unwrap() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        store,
        cluster,
        client,
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

        let (res, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;

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

        let basic_obj = create_basic_object(sender, &sender_kp, client, &package_id).await?;

        for _ in 0..NON_DETERMINISTIC_TESTS_REPETITIONS {
            let (res, wrapped_obj_id) =
                wrap_basic_object(sender, &sender_kp, client, &package_id, &basic_obj)
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
                vec![wrapped_obj_id, *upgrade_cap, gas_ref.0]
                    .into_iter()
                    .sorted()
                    .collect::<Vec<_>>()
            );

            let res = unwrap_basic_object(sender, &sender_kp, client, &package_id, &wrapped_obj_id)
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
                vec![basic_obj, *upgrade_cap, gas_ref.0]
                    .into_iter()
                    .sorted()
                    .collect::<Vec<_>>()
            );
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

        let gas_ref = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(10_000_000_000),
                sender,
            )
            .await;

        indexer_wait_for_object(client, gas_ref.0, gas_ref.1).await;

        let (_, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;

        let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
            .await
            .unwrap();

        let res_1 = increment_counter(sender, &sender_kp, client, &package_id, &counter_obj, None)
            .await
            .unwrap();
        assert_eq!(res_1.status_ok(), Some(true));

        let res_2 = increment_counter(sender, &sender_kp, client, &package_id, &counter_obj, None)
            .await
            .unwrap();
        assert_eq!(res_2.status_ok(), Some(true));

        assert_ne!(res_1.digest, res_2.digest);
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

    runtime
        .block_on(async {
            indexer_wait_for_checkpoint(store, 1).await;

            let (sender, sender_kp): (_, AccountKeyPair) = get_key_pair();
            let rgp = cluster.get_reference_gas_price().await;
            let range = 0..NON_DETERMINISTIC_TESTS_REPETITIONS;
            let gas_objs: Vec<_> = range
                .map(|_| cluster.fund_address_and_return_gas(rgp, Some(10_000_000_000), sender))
                .collect::<FuturesUnordered<_>>()
                .collect::<Vec<_>>()
                .await;

            for gas in gas_objs.iter() {
                indexer_wait_for_object(client, gas.0, gas.1).await;
            }

            let (res, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
            assert_eq!(res.status_ok(), Some(true));

            let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
                .await
                .unwrap();

            let transaction_results: Vec<_> = gas_objs
                .iter()
                .map(|gas| {
                    increment_counter(
                        sender,
                        &sender_kp,
                        client,
                        &package_id,
                        &counter_obj,
                        Some(gas.0),
                    )
                })
                .collect::<FuturesUnordered<_>>()
                .try_collect()
                .await
                .unwrap();

            // Now we need to check if transaction ordering in the DB follows the ordering
            // of transactions imposed by TX dependencies
            {
                let transaction_dependencies = transaction_results
                    .iter()
                    .map(|res| {
                        (
                            res.digest,
                            HashSet::from_iter(res.effects.as_ref().unwrap().dependencies()),
                        )
                    })
                    .collect::<HashMap<_, _>>();

                let executed_transactions_digests =
                    transaction_dependencies.keys().collect::<HashSet<_>>();
                let executed_transactions_digests_to_load = executed_transactions_digests
                    .iter()
                    .map(|digest| digest.inner().to_vec())
                    .collect::<HashSet<_>>();

                let mut stored_global_orders = read_only_blocking!(&store.blocking_cp(), |conn| {
                    tx_global_order::table
                        .filter(
                            tx_global_order::tx_digest
                                .eq_any(executed_transactions_digests_to_load),
                        )
                        .select(TxGlobalOrder::as_select())
                        .load::<TxGlobalOrder>(conn)
                })
                .unwrap();
                stored_global_orders.sort_by(|a, b| {
                    (
                        a.global_sequence_number,
                        a.optimistic_sequence_number.unwrap(),
                    )
                        .cmp(&(
                            b.global_sequence_number,
                            b.optimistic_sequence_number.unwrap(),
                        ))
                });

                let mut seen_digests: HashSet<TransactionDigest> = HashSet::new();
                for stored_global_order in stored_global_orders.iter() {
                    let tx_digest =
                        TransactionDigest::try_from(&stored_global_order.tx_digest[..]).unwrap();
                    let tx_deps = &transaction_dependencies[&tx_digest];
                    let relevant_deps: HashSet<_> = tx_deps
                        .intersection(&executed_transactions_digests)
                        .cloned()
                        .cloned()
                        .collect();
                    assert!(
                        relevant_deps.is_subset(&seen_digests),
                        "Tx: {:?} should have bigger order than it's deps: {:?}",
                        tx_digest,
                        relevant_deps,
                    );
                    seen_digests.insert(tx_digest);
                }
            }

            Ok::<(), IndexerError>(())
        })
        .unwrap();
}

#[test]
fn test_repeated_tx_execution() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async {
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

            let (res, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
            assert_eq!(res.status_ok(), Some(true));

            let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
                .await
                .unwrap();

            let transaction_bytes: TransactionBlockBytes = client
                .move_call(
                    sender,
                    package_id,
                    "counter".to_string(),
                    "increment".to_string(),
                    type_args![].unwrap(),
                    call_args!(counter_obj).unwrap(),
                    Some(gas_ref.0),
                    10_000_000.into(),
                    None,
                )
                .await
                .unwrap();
            let signed_transaction =
                to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &sender_kp);
            let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

            let res_1 = client
                .execute_transaction_block(
                    tx_bytes.clone(),
                    signatures.clone(),
                    Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                    Some(ExecuteTransactionRequestType::WaitForLocalExecution),
                )
                .await
                .unwrap();

            let res_2 = client
                .execute_transaction_block(
                    tx_bytes,
                    signatures,
                    Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                    None,
                )
                .await
                .unwrap();

            assert_eq!(res_1.status_ok(), Some(true));
            assert_eq!(res_2.status_ok(), Some(true));
            assert_eq!(res_1.digest, res_2.digest);

            Ok::<(), IndexerError>(())
        })
        .unwrap();
}

#[test]
fn test_parallel_repeated_tx_execution() {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime
        .block_on(async {
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

            let (res, package_id) = deploy_basics_pkg(sender, &sender_kp, client).await;
            assert_eq!(res.status_ok(), Some(true));

            let (_, counter_obj) = create_counter_object(sender, &sender_kp, client, &package_id)
                .await
                .unwrap();

            let transaction_bytes: TransactionBlockBytes = client
                .move_call(
                    sender,
                    package_id,
                    "counter".to_string(),
                    "increment".to_string(),
                    type_args![].unwrap(),
                    call_args!(counter_obj).unwrap(),
                    Some(gas_ref.0),
                    10_000_000.into(),
                    None,
                )
                .await
                .unwrap();
            let signed_transaction =
                to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), &sender_kp);
            let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

            let range = 0..NON_DETERMINISTIC_TESTS_REPETITIONS;
            let transaction_results: Vec<_> = range
                .map(|_| {
                    client.execute_transaction_block(
                        tx_bytes.clone(),
                        signatures.clone(),
                        Some(IotaTransactionBlockResponseOptions::new().with_effects()),
                        None,
                    )
                })
                .collect::<FuturesUnordered<_>>()
                .try_collect()
                .await
                .unwrap();

            assert!(
                transaction_results
                    .iter()
                    .all(|res| res.status_ok() == Some(true))
            );

            let tx_digest = transaction_results[0].digest;
            assert!(
                transaction_results
                    .iter()
                    .all(|res| res.digest == tx_digest)
            );

            Ok::<(), IndexerError>(())
        })
        .unwrap();
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

        let (res, package_id) = deploy_bear_pkg(sender, &sender_kp, client).await;
        let display_obj_id = ObjectID::from_hex_literal(
            res.events.unwrap().data[0].parsed_json.as_object().unwrap()["id"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

        let (_, bear_id) = create_new_bear(sender, &sender_kp, client, &package_id, "bear name")
            .await
            .unwrap();

        let bear_type_tag = TypeTag::Struct(Box::new(StructTag {
            address: (*package_id),
            name: IdentStr::new("DemoBear").unwrap().into(),
            module: IdentStr::new("demo_bear").unwrap().into(),
            type_params: Vec::new(),
        }));

        for n in 0..NON_DETERMINISTIC_TESTS_REPETITIONS {
            let new_bear_description = format!("Bear description {n}");

            let res = update_display_object(
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

            let res = bump_display_object_version(
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

            let actual_description =
                res.data.unwrap().display.unwrap().data.unwrap()["description"].clone();

            assert_eq!(actual_description, new_bear_description);
        }
    });
}

pub(crate) async fn create_basic_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
) -> Result<ObjectID, anyhow::Error> {
    let res = execute_move_call(
        client,
        address,
        address_kp,
        *package_id,
        "object_basics".to_string(),
        "create".to_string(),
        type_args![].unwrap(),
        call_args!(0, address).unwrap(),
        None,
    )
    .await?;

    let basic_obj_id = res
        .effects
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();
    Ok(basic_obj_id)
}

async fn wrap_basic_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    object_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, ObjectID), anyhow::Error> {
    let res = execute_move_call(
        client,
        address,
        address_kp,
        *package_id,
        "object_basics".to_string(),
        "wrap".to_string(),
        type_args![].unwrap(),
        call_args!(object_id).unwrap(),
        None,
    )
    .await?;

    let wrapped_obj_id = res
        .effects
        .as_ref()
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();

    Ok((res, wrapped_obj_id))
}

async fn unwrap_basic_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    object_id: &ObjectID,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    execute_move_call(
        client,
        address,
        address_kp,
        *package_id,
        "object_basics".to_string(),
        "unwrap".to_string(),
        type_args![].unwrap(),
        call_args!(object_id).unwrap(),
        None,
    )
    .await
}

async fn update_display_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    display_object_id: &ObjectID,
    display_obj_type_tag: TypeTag,
    name_to_update: &str,
    new_value: &str,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    execute_move_call(
        client,
        address,
        address_kp,
        IOTA_FRAMEWORK_PACKAGE_ID,
        "display".to_string(),
        "edit".to_string(),
        type_args![display_obj_type_tag].unwrap(),
        call_args!(
            display_object_id,
            name_to_update.to_string(),
            new_value.to_string()
        )
        .unwrap(),
        None,
    )
    .await
}

async fn bump_display_object_version(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    display_object_id: &ObjectID,
    display_obj_type_tag: TypeTag,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    execute_move_call(
        client,
        address,
        address_kp,
        IOTA_FRAMEWORK_PACKAGE_ID,
        "display".to_string(),
        "update_version".to_string(),
        type_args![display_obj_type_tag].unwrap(),
        call_args!(display_object_id).unwrap(),
        None,
    )
    .await
}

async fn create_counter_object(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
) -> Result<(IotaTransactionBlockResponse, ObjectID), anyhow::Error> {
    let res = execute_move_call(
        client,
        address,
        address_kp,
        *package_id,
        "counter".to_string(),
        "create".to_string(),
        type_args![].unwrap(),
        call_args!().unwrap(),
        None,
    )
    .await?;

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

async fn increment_counter(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    counter_id: &ObjectID,
    gas: Option<ObjectID>,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    execute_move_call(
        client,
        address,
        address_kp,
        *package_id,
        "counter".to_string(),
        "increment".to_string(),
        type_args![].unwrap(),
        call_args!(counter_id).unwrap(),
        gas,
    )
    .await
}

async fn create_new_bear(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    name: &str,
) -> Result<(IotaTransactionBlockResponse, ObjectID), anyhow::Error> {
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
    let signed_transaction = to_sender_signed_transaction(tx_data, address_kp);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let bear_id = res
        .effects
        .as_ref()
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();

    Ok((res, bear_id))
}

pub(crate) async fn deploy_basics_pkg(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
) -> (IotaTransactionBlockResponse, ObjectID) {
    deploy_package(address, address_kp, client, "../../examples/move/basics").await
}

async fn deploy_bear_pkg(
    address: IotaAddress,
    address_kp: &AccountKeyPair,
    client: &HttpClient,
) -> (IotaTransactionBlockResponse, ObjectID) {
    deploy_package(
        address,
        address_kp,
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
) -> (IotaTransactionBlockResponse, ObjectID) {
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
    let res = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::full_content()),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let package_id = *res
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

    (res, package_id)
}
