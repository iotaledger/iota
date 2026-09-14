// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc};

use futures::future;
use iota_config::node::RunWithRange;
use iota_json_rpc_types::{
    EventFilter, EventPage, IotaEvent, IotaExecutionStatus, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, TransactionFilter,
};
use iota_keys::keystore::AccountKeystore;
use iota_macros::*;
use iota_node::IotaNodeHandle;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_crypto::{
    ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, GasPayment, Identifier, ObjectId, ObjectReference, Owner, Transaction,
    TransactionDigest, TransactionKind, Version,
};
use iota_storage::{
    key_value_store::TransactionKeyValueStore, key_value_store_metrics::KeyValueStoreMetrics,
};
use iota_test_transaction_builder::{
    TestTransactionBuilder, batch_make_transfer_transactions, create_nft, delete_nft,
    increment_counter, publish_basics_package, publish_basics_package_and_make_counter,
    publish_nfts_package,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    messages_grpc::TransactionInfoRequest,
    object::{Object, ObjectRead, PastObjectRead},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::{
        ExecuteTransactionRequestType, ExecuteTransactionRequestV1, QuorumDriverResponse,
    },
    storage::{ObjectKey, ObjectStore},
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        TransactionAPI,
    },
    utils::{to_sender_signed_transaction, to_sender_signed_transaction_with_multi_signers},
};
use jsonrpsee::{core::client::ClientT, rpc_params};
use move_core_types::annotated_value::MoveStructLayout;
use test_cluster::{TestClusterBuilder, override_pcool_flow};
use tokio::{
    sync::RwLock,
    time::{Duration, sleep, timeout},
};
use tracing::info;

#[sim_test]
async fn test_full_node_follows_txes() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;

    let context = &mut test_cluster.wallet;

    // TODO: test fails on CI due to flakiness without this. Once https://github.com/iotaledger/iota/pull/7056 is
    // merged we should be able to root out the flakiness.
    sleep(Duration::from_millis(10)).await;

    let (transferred_object, _, receiver, digest, _) = transfer_coin(context).await?;

    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;

    // A small delay is necessary until the checkpoint of the transaction is
    // processed.
    sleep(Duration::from_secs(1)).await;

    // verify that the node has seen the transfer
    let object_read = fullnode.state().get_object_read(&transferred_object)?;
    let object = object_read.into_object()?;

    assert_eq!(*object.owner.address_or_object().unwrap(), receiver);

    Ok(())
}

#[sim_test]
async fn test_full_node_shared_objects() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let handle = test_cluster.spawn_new_fullnode().await;
    run_full_node_shared_objects(&test_cluster.wallet, &handle.iota_node).await
}

// The same scenario with every node on the ExecutionScheduler. The cold-sync
// variant below only covers the checkpoint-executor path, where envs come from
// certified effects, so without this the live consensus path would run under
// the default TransactionManager only.
#[sim_test]
async fn test_full_node_shared_objects_execution_scheduler() -> Result<(), anyhow::Error> {
    // Selected at node construction; set before the cluster and fullnode are
    // built. The opt-out variable is cleared too since it takes precedence.
    // Process-per-test isolation keeps this from leaking to other tests.
    std::env::set_var("ENABLE_EXECUTION_SCHEDULER", "1");
    std::env::remove_var("ENABLE_TRANSACTION_MANAGER");
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let handle = test_cluster.spawn_new_fullnode().await;
    for node in test_cluster.all_node_handles() {
        node.with(|node| {
            assert!(
                node.state().uses_execution_scheduler(),
                "every node must run the ExecutionScheduler for this test to exercise the \
                 live consensus scheduling path"
            );
        });
    }
    run_full_node_shared_objects(&test_cluster.wallet, &handle.iota_node).await
}

async fn run_full_node_shared_objects(
    context: &WalletContext,
    fullnode: &IotaNodeHandle,
) -> Result<(), anyhow::Error> {
    let sender = context
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .unwrap();
    let (package_ref, counter_ref) = publish_basics_package_and_make_counter(context).await;

    let response = increment_counter(
        context,
        sender,
        None,
        package_ref.object_id,
        counter_ref.object_id,
        counter_ref.version,
    )
    .await;
    let digest = response.digest;
    // Bounded: a lost or mismatched shared version assignment leaves the
    // transaction waiting forever; fail clearly instead of hanging.
    tokio::time::timeout(
        Duration::from_secs(60),
        fullnode
            .state()
            .get_transaction_cache_reader()
            .notify_read_executed_effects_for_testing("", &[digest]),
    )
    .await
    .expect("shared-object transaction did not execute on the fullnode");

    Ok(())
}

#[sim_test]
async fn test_sponsored_transaction() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let sender = test_cluster.get_address_0();
    let sponsor = test_cluster.get_address_1();
    let another_addr = test_cluster.get_address_2();

    // This makes sender send one coin to sponsor.
    // The sent coin is used as sponsor gas in the following sponsored tx.
    let (sent_coin, sender_, receiver, _, object_ref) =
        transfer_coin(&test_cluster.wallet).await.unwrap();
    assert_eq!(sender, sender_);
    assert_eq!(sponsor, receiver);
    let object_ref = test_cluster
        .wallet
        .get_object_ref(object_ref.object_id)
        .await?;
    let gas_obj = test_cluster.wallet.get_object_ref(sent_coin).await?;
    info!("updated obj ref: {:?}", object_ref);
    info!("updated gas ref: {:?}", gas_obj);

    // Construct the sponsored transaction
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(another_addr, object_ref).unwrap();
        builder.finish()
    };
    let kind = TransactionKind::new_programmable(pt);
    let tx = Transaction::new_with_gas_data(
        kind,
        sender,
        GasPayment {
            objects: vec![gas_obj],
            owner: sponsor,
            price: rgp,
            budget: rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        },
    );

    let tx = to_sender_signed_transaction_with_multi_signers(
        tx,
        vec![
            test_cluster
                .wallet
                .config()
                .keystore()
                .get_key(&sender)
                .unwrap()
                .as_keypair()?,
            test_cluster
                .wallet
                .config()
                .keystore()
                .get_key(&sponsor)
                .unwrap()
                .as_keypair()?,
        ],
    );

    test_cluster.execute_transaction(tx).await;

    assert_eq!(
        sponsor,
        test_cluster
            .wallet
            .get_object_owner(&sent_coin)
            .await
            .unwrap(),
    );
    Ok(())
}

#[sim_test]
async fn test_full_node_move_function_index() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let node = &test_cluster.fullnode_handle.iota_node;
    let sender = test_cluster.get_address_0();
    let context = &mut test_cluster.wallet;

    let (package_ref, counter_ref) = publish_basics_package_and_make_counter(context).await;
    let response = increment_counter(
        context,
        sender,
        None,
        package_ref.object_id,
        counter_ref.object_id,
        counter_ref.version,
    )
    .await;
    let digest = response.digest;

    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::MoveFunction {
                package: package_ref.object_id,
                module: Some("counter".to_string()),
                function: Some("increment".to_string()),
            }),
            None,
            None,
            false,
        )
        .await?;

    assert_eq!(txes.len(), 1);
    assert_eq!(txes[0], digest);

    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::MoveFunction {
                package: package_ref.object_id,
                module: None,
                function: None,
            }),
            None,
            None,
            false,
        )
        .await?;

    // 2 transactions in the package i.e create and increment counter
    assert_eq!(txes.len(), 2);
    assert_eq!(txes[1], digest);

    eprint!("start...");
    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::MoveFunction {
                package: package_ref.object_id,
                module: Some("counter".to_string()),
                function: None,
            }),
            None,
            None,
            false,
        )
        .await?;

    // 2 transactions in the package i.e publish and increment
    assert_eq!(txes.len(), 2);
    assert_eq!(txes[1], digest);

    Ok(())
}

#[sim_test]
async fn test_full_node_indexes() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let mut test_cluster = TestClusterBuilder::new()
        .enable_fullnode_events()
        .build()
        .await;
    let node = &test_cluster.fullnode_handle.iota_node;
    let context = &mut test_cluster.wallet;

    let (_, sender, receiver, digest, _) = transfer_coin(context).await?;

    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::FromAddress(sender)),
            None,
            None,
            false,
        )
        .await?;
    assert_eq!(txes.len(), 1);
    assert_eq!(txes[0], digest);

    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::ToAddress(receiver)),
            None,
            None,
            false,
        )
        .await?;
    assert_eq!(txes.len(), 2);
    assert_eq!(txes[1], digest);

    // Note that this is also considered a tx to the sender, because it mutated
    // one or more of the sender's objects.
    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::ToAddress(sender)),
            None,
            None,
            false,
        )
        .await?;
    assert_eq!(txes.len(), 2);
    assert_eq!(txes[1], digest);

    // No transactions have originated from the receiver
    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::FromAddress(receiver)),
            None,
            None,
            false,
        )
        .await?;
    assert_eq!(txes.len(), 0);

    // This is a poor replacement for a wait until the checkpoint is processed.
    // Unfortunately event store writes seem to add some latency so this wait is
    // needed
    sleep(Duration::from_millis(1000)).await;

    // // one event is stored, and can be looked up by digest
    // query by timestamp verifies that a timestamp is inserted, within an hour
    // let sender_balance_change = BalanceChange {
    // change_type: BalanceChangeType::Pay,
    // owner: sender,
    // coin_type: parse_struct_tag("0x2::iota::IOTA").unwrap(),
    // amount: -100000000000000,
    // };
    // let recipient_balance_change = BalanceChange {
    // change_type: BalanceChangeType::Receive,
    // owner: receiver,
    // coin_type: parse_struct_tag("0x2::iota::IOTA").unwrap(),
    // amount: 100000000000000,
    // };
    // let gas_balance_change = BalanceChange {
    // change_type: BalanceChangeType::Gas,
    // owner: sender,
    // coin_type: parse_struct_tag("0x2::iota::IOTA").unwrap(),
    // amount: (gas_used as i128).neg(),
    // };
    //
    // query all events
    // let all_events = node
    // .state()
    // .get_transaction_events(
    // EventQuery::TimeRange {
    // start_time: ts.unwrap() - HOUR_MS,
    // end_time: ts.unwrap() + HOUR_MS,
    // },
    // None,
    // 100,
    // false,
    // )
    // .await?;
    // let all_events = &all_events[all_events.len() - 3..];
    // assert_eq!(all_events.len(), 3);
    // assert_eq!(all_events[0].1.tx_digest, digest);
    // let all_events = all_events
    // .iter()
    // .map(|(_, envelope)| envelope.event.clone())
    // .collect::<Vec<_>>();
    // assert_eq!(all_events[0], gas_event.clone());
    // assert_eq!(all_events[1], sender_event.clone());
    // assert_eq!(all_events[2], recipient_event.clone());
    //
    // query by sender
    // let events_by_sender = node
    // .state()
    // .query_events(EventQuery::Sender(sender), None, 10, false)
    // .await?;
    // assert_eq!(events_by_sender.len(), 3);
    // assert_eq!(events_by_sender[0].1.tx_digest, digest);
    // let events_by_sender = events_by_sender
    // .into_iter()
    // .map(|(_, envelope)| envelope.event)
    // .collect::<Vec<_>>();
    // assert_eq!(events_by_sender[0], gas_event.clone());
    // assert_eq!(events_by_sender[1], sender_event.clone());
    // assert_eq!(events_by_sender[2], recipient_event.clone());
    //
    // query by tx digest
    // let events_by_tx = node
    // .state()
    // .query_events(EventQuery::Transaction(digest), None, 10, false)
    // .await?;
    // assert_eq!(events_by_tx.len(), 3);
    // assert_eq!(events_by_tx[0].1.tx_digest, digest);
    // let events_by_tx = events_by_tx
    // .into_iter()
    // .map(|(_, envelope)| envelope.event)
    // .collect::<Vec<_>>();
    // assert_eq!(events_by_tx[0], gas_event);
    // assert_eq!(events_by_tx[1], sender_event.clone());
    // assert_eq!(events_by_tx[2], recipient_event.clone());
    //
    // query by recipient
    // let events_by_recipient = node
    // .state()
    // .query_events(
    // EventQuery::Recipient(Owner::Address(receiver)),
    // None,
    // 100,
    // false,
    // )
    // .await?;
    // assert_eq!(events_by_recipient.last().unwrap().1.tx_digest, digest);
    // assert_eq!(events_by_recipient.last().unwrap().1.event, recipient_event);
    //
    // query by object
    // let mut events_by_object = node
    // .state()
    // .query_events(EventQuery::Object(transferred_object), None, 100, false)
    // .await?;
    // let events_by_object = events_by_object.split_off(events_by_object.len() -
    // 2); assert_eq!(events_by_object.len(), 2);
    // assert_eq!(events_by_object[0].1.tx_digest, digest);
    // let events_by_object = events_by_object
    // .into_iter()
    // .map(|(_, envelope)| envelope.event)
    // .collect::<Vec<_>>();
    // assert_eq!(events_by_object[0], sender_event.clone());
    // assert_eq!(events_by_object[1], recipient_event.clone());
    //
    // query by transaction module
    // Query by module ID
    // let events_by_module = node
    // .state()
    // .query_events(
    // EventQuery::MoveModule {
    // package: IotaFramework::ID,
    // module: "unused_input_object".to_string(),
    // },
    // None,
    // 10,
    // false,
    // )
    // .await?;
    // assert_eq!(events_by_module[0].1.tx_digest, digest);
    // let events_by_module = events_by_module
    // .into_iter()
    // .map(|(_, envelope)| envelope.event)
    // .collect::<Vec<_>>();
    // assert_eq!(events_by_module.len(), 2);
    // assert_eq!(events_by_module[0], sender_event);
    // assert_eq!(events_by_module[1], recipient_event);

    Ok(())
}

// Test for syncing a node to an authority that already has many txes.
#[sim_test]
async fn test_full_node_cold_sync() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;

    let context = &mut test_cluster.wallet;
    let _ = transfer_coin(context).await?;
    let _ = transfer_coin(context).await?;
    let _ = transfer_coin(context).await?;
    let (_transferred_object, _, _, digest, ..) = transfer_coin(context).await?;

    // Make sure the validators are quiescent before bringing up the node.
    sleep(Duration::from_millis(1000)).await;

    // Start a new fullnode that is not on the write path
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;

    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;

    let info = fullnode
        .state()
        .handle_transaction_info_request(TransactionInfoRequest {
            transaction_digest: digest,
        })
        .await?;
    // Check that it has been executed.
    info.status.into_effects_for_testing();

    Ok(())
}

// Same cold-sync scenario, but every node runs the ExecutionScheduler. The
// scheduler has otherwise never been exercised through the checkpoint-executor
// / state-sync path, where transactions are enqueued with a certified expected
// effects digest and their inputs arrive by applying synced checkpoints rather
// than from local submission. A regression where a synced transaction never
// becomes ready under the ExecutionScheduler would stall sync silently.
#[sim_test]
async fn test_full_node_cold_sync_execution_scheduler() -> Result<(), anyhow::Error> {
    // Selected at node construction; set before the cluster and fullnode are
    // built. The opt-out variable is cleared too since it takes precedence.
    // Process-per-test isolation keeps this from leaking to other tests.
    std::env::set_var("ENABLE_EXECUTION_SCHEDULER", "1");
    std::env::remove_var("ENABLE_TRANSACTION_MANAGER");
    let mut test_cluster = TestClusterBuilder::new().build().await;

    let context = &mut test_cluster.wallet;
    let _ = transfer_coin(context).await?;
    let _ = transfer_coin(context).await?;
    let _ = transfer_coin(context).await?;
    let (_transferred_object, _, _, digest, ..) = transfer_coin(context).await?;

    // Make sure the validators are quiescent before bringing up the node.
    sleep(Duration::from_millis(1000)).await;

    // Start a new fullnode that is not on the write path.
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    assert!(
        fullnode.state().uses_execution_scheduler(),
        "the synced fullnode must run the ExecutionScheduler for this test to exercise the \
         state-sync scheduling path"
    );

    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;

    let info = fullnode
        .state()
        .handle_transaction_info_request(TransactionInfoRequest {
            transaction_digest: digest,
        })
        .await?;
    // Check that it has been executed.
    info.status.into_effects_for_testing();

    Ok(())
}

#[sim_test]
async fn test_full_node_sync_flood() {
    do_test_full_node_sync_flood().await
}

#[sim_test(check_determinism)]
#[ignore = "https://github.com/iotaledger/iota/issues/7469"]
async fn test_full_node_sync_flood_determinism() {
    do_test_full_node_sync_flood().await
}

async fn do_test_full_node_sync_flood() {
    let mut test_cluster = TestClusterBuilder::new().build().await;

    // Start a new fullnode that is not on the write path
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;

    let test_cluster = Arc::new(RwLock::new(test_cluster));

    let mut futures = Vec::new();

    let (package_ref, counter_ref) =
        publish_basics_package_and_make_counter(&test_cluster.read().await.wallet).await;

    // Start up 5 different tasks that all spam txs at the authorities.
    for _i in 0..5 {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let test_cluster = test_cluster.clone();
        tokio::task::spawn(async move {
            let (sender, object_to_split, gas_obj) = {
                let mut test_cluster = test_cluster.write().await;
                let context = &mut test_cluster.wallet;

                let sender = context
                    .config()
                    .keystore()
                    .addresses()
                    .first()
                    .cloned()
                    .unwrap();

                let mut coins = context.gas_objects(sender).await.unwrap();
                let object_to_split = coins.swap_remove(0).1.object_ref();
                let gas_obj = coins.swap_remove(0).1.object_ref();
                (sender, object_to_split, gas_obj)
            };

            let mut owned_tx_digest = None;
            let mut shared_tx_digest = None;
            let gas_object_id = gas_obj.object_id;
            for _ in 0..10 {
                let test_cluster = test_cluster.read().await;
                let res = {
                    let tx = TestTransactionBuilder::new(
                        sender,
                        gas_obj,
                        test_cluster.get_reference_gas_price().await,
                    )
                    .split_coin(object_to_split, vec![1])
                    .build();

                    let tx = test_cluster.wallet.sign_transaction(&tx);
                    test_cluster.execute_transaction(tx).await
                };

                owned_tx_digest = Some(*res.transaction_digest());
                shared_tx_digest = Some(
                    increment_counter(
                        &test_cluster.wallet,
                        sender,
                        Some(gas_object_id),
                        package_ref.object_id,
                        counter_ref.object_id,
                        counter_ref.version,
                    )
                    .await
                    .digest,
                );
            }
            tx.send((owned_tx_digest.unwrap(), shared_tx_digest.unwrap()))
                .unwrap();
        });
        futures.push(rx);
    }

    // make sure the node syncs up to the last digest sent by each task.
    let digests: Vec<_> = future::join_all(futures)
        .await
        .iter()
        .map(|r| r.clone().unwrap())
        .flat_map(|(a, b)| std::iter::once(a).chain(std::iter::once(b)))
        .collect();
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &digests)
        .await;
}

// Test fullnode has event read jsonrpc endpoints working
#[sim_test]
async fn test_full_node_event_read_api_ok() {
    let mut test_cluster = TestClusterBuilder::new()
        .with_fullnode_rpc_port(50000)
        .enable_fullnode_events()
        .build()
        .await;

    let context = &mut test_cluster.wallet;
    let node = &test_cluster.fullnode_handle.iota_node;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let (package_id, _, publish_digest) = publish_nfts_package(context).await;

    let (_, sender, _, transfer_digest, _) = transfer_coin(context).await.unwrap();

    let txes = node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::FromAddress(sender)),
            None,
            None,
            false,
        )
        .await
        .unwrap();

    assert_eq!(txes.len(), 2);
    assert!(
        (txes[0] == publish_digest && txes[1] == transfer_digest)
            || (txes[0] == transfer_digest && txes[1] == publish_digest)
    );

    // This is a poor replacement for a wait until the checkpoint is processed.
    sleep(Duration::from_millis(1000)).await;

    let (_sender, _object_id, digest2) = create_nft(context, package_id).await;

    // Add a delay to ensure event processing is done after transaction commits.
    sleep(Duration::from_secs(5)).await;

    // query by move event struct name
    let params = rpc_params![digest2];
    let events: Vec<IotaEvent> = jsonrpc_client
        .request("iota_getEvents", params)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id.tx_digest, digest2);
}

#[sim_test]
async fn test_full_node_event_query_by_module_ok() {
    let mut test_cluster = TestClusterBuilder::new()
        .enable_fullnode_events()
        .build()
        .await;

    let context = &mut test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let (package_id, _, _) = publish_nfts_package(context).await;

    // This is a poor replacement for a wait until the checkpoint is processed.
    sleep(Duration::from_millis(1000)).await;

    let (_sender, _object_id, digest2) = create_nft(context, package_id).await;

    // Add a delay to ensure event processing is done after transaction commits.
    sleep(Duration::from_secs(5)).await;

    // query by move event module
    let params = rpc_params![EventFilter::MoveEventModule {
        package: package_id,
        module: Identifier::from_static("testnet_nft")
    }];
    let page: EventPage = jsonrpc_client
        .request("iotax_queryEvents", params)
        .await
        .unwrap();
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].id.tx_digest, digest2);
}

#[sim_test]
async fn test_full_node_transaction_orchestrator_basic() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;
    let metrics = KeyValueStoreMetrics::new_for_tests();
    let kv_store = Arc::new(TransactionKeyValueStore::new(
        "rocksdb",
        metrics,
        fullnode.state(),
    ));

    let context = &mut test_cluster.wallet;
    let transaction_orchestrator = fullnode.with(|node| {
        node.transaction_orchestrator()
            .expect("Fullnode should have transaction orchestrator toggled on.")
    });
    let mut rx = fullnode.with(|node| {
        node.subscribe_to_transaction_orchestrator_effects()
            .expect("Fullnode should have transaction orchestrator toggled on.")
    });

    let txn_count = 4;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    // Test WaitForLocalExecution
    let txn = txns.swap_remove(0);
    let digest = *txn.digest();
    let res = transaction_orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1::new(txn),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to execute transaction {digest:?}: {e:?}"));

    let (
        tx,
        QuorumDriverResponse {
            effects_cert: certified_txn_effects,
            events: txn_events,
            ..
        },
    ) = rx.recv().await.unwrap().unwrap();
    let (response, is_executed_locally) = res;
    assert_eq!(*tx.digest(), digest);
    assert_eq!(
        response.effects.effects.digest(),
        *certified_txn_effects.digest()
    );
    assert!(is_executed_locally);
    assert_eq!(
        response.events.unwrap_or_default().digest(),
        txn_events.unwrap_or_default().digest()
    );
    // verify that the node has sequenced and executed the txn
    fullnode.state().get_executed_transaction_and_effects(digest, kv_store.clone()).await
        .unwrap_or_else(|e| panic!("Fullnode does not know about the txn {digest:?} that was executed with WaitForLocalExecution: {e:?}"));

    // Test WaitForEffectsCert
    let txn = txns.swap_remove(0);
    let digest = *txn.digest();
    let res = transaction_orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1::new(txn),
            ExecuteTransactionRequestType::WaitForEffectsCert,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to execute transaction {digest:?}: {e:?}"));

    let (
        tx,
        QuorumDriverResponse {
            effects_cert: certified_txn_effects,
            events: txn_events,
            ..
        },
    ) = rx.recv().await.unwrap().unwrap();
    let (response, is_executed_locally) = res;
    assert_eq!(*tx.digest(), digest);
    assert_eq!(
        response.effects.effects.digest(),
        *certified_txn_effects.digest()
    );
    assert_eq!(
        txn_events.unwrap_or_default().digest(),
        response.events.unwrap_or_default().digest()
    );
    assert!(!is_executed_locally);
    fullnode
        .state()
        .get_transaction_cache_reader()
        .notify_read_executed_effects_for_testing("", &[digest])
        .await;
    fullnode.state().get_executed_transaction_and_effects(digest, kv_store).await
        .unwrap_or_else(|e| panic!("Fullnode does not know about the txn {digest:?} that was executed with WaitForEffectsCert: {e:?}"));

    Ok(())
}

/// Test a validator node does not have transaction orchestrator
#[tokio::test]
async fn test_validator_node_has_no_transaction_orchestrator() {
    let test_cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;
    let node_handle = test_cluster.swarm.validator_node_handles().pop().unwrap();
    node_handle.with(|node| {
        assert!(node.transaction_orchestrator().is_none());
        assert!(
            node.subscribe_to_transaction_orchestrator_effects()
                .is_err()
        );
    });
}

#[sim_test]
async fn test_execute_tx_with_serialized_signature() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    context
        .config_mut()
        .keystore_mut()
        .add_key(None, SimpleKeypair::from(Secp256k1PrivateKey::random()))?;
    context
        .config_mut()
        .keystore_mut()
        .add_key(None, SimpleKeypair::from(Ed25519PrivateKey::random()))?;

    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let txn_count = 4;
    let txns = batch_make_transfer_transactions(context, txn_count).await;
    for txn in txns {
        let tx_digest = txn.digest();
        let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
        let params = rpc_params![
            tx_bytes,
            signatures,
            IotaTransactionBlockResponseOptions::new(),
            ExecuteTransactionRequestType::WaitForLocalExecution
        ];
        let response: IotaTransactionBlockResponse = jsonrpc_client
            .request("iota_executeTransactionBlock", params)
            .await
            .unwrap();

        let IotaTransactionBlockResponse {
            digest,
            confirmed_local_execution,
            ..
        } = response;
        assert_eq!(digest, *tx_digest);
        assert!(confirmed_local_execution.unwrap());
    }
    Ok(())
}

#[sim_test]
async fn test_full_node_transaction_orchestrator_rpc_ok() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;
    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;

    let txn_count = 4;
    let mut txns = batch_make_transfer_transactions(context, txn_count).await;
    assert!(
        txns.len() >= txn_count,
        "Expect at least {txn_count} txns. Do we generate enough gas objects during genesis?",
    );

    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();

    // Test request with ExecuteTransactionRequestType::WaitForLocalExecution
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new(),
        ExecuteTransactionRequestType::WaitForLocalExecution
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        digest,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(&digest, tx_digest);
    assert!(confirmed_local_execution.unwrap());

    let _response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_getTransactionBlock", rpc_params![*tx_digest])
        .await
        .unwrap();

    // Test request with ExecuteTransactionRequestType::WaitForEffectsCert
    // Use the same txn which should return local finalized effects
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new().with_effects(),
        ExecuteTransactionRequestType::WaitForEffectsCert
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        effects,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(effects.unwrap().transaction_digest(), tx_digest);
    assert!(confirmed_local_execution.unwrap());

    // Test request with ExecuteTransactionRequestType::WaitForEffectsCert
    // Use a different txn to avoid the case where the txn effects are already
    // cached locally
    let txn = txns.swap_remove(0);
    let tx_digest = txn.digest();

    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let params = rpc_params![
        tx_bytes,
        signatures,
        IotaTransactionBlockResponseOptions::new().with_effects(),
        ExecuteTransactionRequestType::WaitForEffectsCert
    ];
    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_executeTransactionBlock", params)
        .await
        .unwrap();

    let IotaTransactionBlockResponse {
        effects,
        confirmed_local_execution,
        ..
    } = response;
    assert_eq!(effects.unwrap().transaction_digest(), tx_digest);
    assert!(!confirmed_local_execution.unwrap());

    Ok(())
}

async fn get_obj_read_from_node(
    node: &IotaNodeHandle,
    object_id: ObjectId,
) -> Result<(ObjectReference, Object, Option<MoveStructLayout>), anyhow::Error> {
    if let ObjectRead::Exists(obj_ref, object, layout) = node.state().get_object_read(&object_id)? {
        Ok((obj_ref, object, layout))
    } else {
        anyhow::bail!("Can't find object {object_id} on fullnode.")
    }
}

async fn get_past_obj_read_from_node(
    node: &IotaNodeHandle,
    object_id: ObjectId,
    seq_num: Version,
) -> Result<(ObjectReference, Object, Option<MoveStructLayout>), anyhow::Error> {
    if let PastObjectRead::VersionFound(obj_ref, object, layout) =
        node.state().get_past_object_read(&object_id, seq_num)?
    {
        Ok((obj_ref, object, layout))
    } else {
        anyhow::bail!("Can't find object {object_id} with seq {seq_num} on fullnode.")
    }
}

#[sim_test]
async fn test_get_objects_read() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let test_cluster = TestClusterBuilder::new()
        // The past objects that are expected to be read from the node can be pruned in 10 seconds.
        // This causes test instability, so pruning is disabled in this test.
        .disable_fullnode_pruning()
        .build()
        .await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let node = &test_cluster.fullnode_handle.iota_node;
    let package_id = publish_nfts_package(&test_cluster.wallet).await.0;

    // Create the object
    let (sender, object_id, _) = create_nft(&test_cluster.wallet, package_id).await;

    let recipient = test_cluster.get_address_1();
    assert_ne!(sender, recipient);

    let (object_ref_v1, object_v1, _) = get_obj_read_from_node(node, object_id).await?;

    // Transfer the object from sender to recipient
    let gas_ref = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(sender)
        .await
        .unwrap()
        .unwrap();
    let nft_transfer_tx = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_ref, rgp)
            .transfer(object_ref_v1, recipient)
            .build(),
    );
    test_cluster.execute_transaction(nft_transfer_tx).await;
    sleep(Duration::from_secs(1)).await;

    let (object_ref_v2, object_v2, _) = get_obj_read_from_node(node, object_id).await?;
    assert_ne!(object_ref_v2, object_ref_v1);

    // Transfer some IOTA to recipient
    transfer_coin(&test_cluster.wallet)
        .await
        .expect("Failed to transfer coins to recipient");

    // Delete the object
    let response = delete_nft(&test_cluster.wallet, recipient, package_id, object_ref_v2).await;
    assert_eq!(
        *response.effects.unwrap().status(),
        IotaExecutionStatus::Success
    );
    sleep(Duration::from_secs(1)).await;

    // Now test get_object_read
    let object_ref_v3 = match node.state().get_object_read(&object_id)? {
        ObjectRead::Deleted(obj_ref) => obj_ref,
        other => anyhow::bail!("Expect object {object_id} deleted but got {other}."),
    };

    let read_ref_v3 = match node
        .state()
        .get_past_object_read(&object_id, object_ref_v3.version)?
    {
        PastObjectRead::ObjectDeleted(obj_ref) => obj_ref,
        other => anyhow::bail!("Expect object {object_id} deleted but got {other}."),
    };
    assert_eq!(object_ref_v3, read_ref_v3);

    let (read_ref_v2, read_obj_v2, _) =
        get_past_obj_read_from_node(node, object_id, object_ref_v2.version).await?;
    assert_eq!(read_ref_v2, object_ref_v2);
    assert_eq!(read_obj_v2, object_v2);
    assert_eq!(read_obj_v2.owner, Owner::Address(recipient));

    let (read_ref_v1, read_obj_v1, _) =
        get_past_obj_read_from_node(node, object_id, object_ref_v1.version).await?;
    assert_eq!(read_ref_v1, object_ref_v1);
    assert_eq!(read_obj_v1, object_v1);
    assert_eq!(read_obj_v1.owner, Owner::Address(sender));

    let too_high_version = Version::lamport_increment([object_ref_v3.version]).unwrap();

    match node
        .state()
        .get_past_object_read(&object_id, too_high_version)?
    {
        PastObjectRead::VersionTooHigh {
            object_id: obj_id,
            asked_version,
            latest_version,
        } => {
            assert_eq!(obj_id, object_id);
            assert_eq!(asked_version, too_high_version);
            assert_eq!(latest_version, object_ref_v3.version);
        }
        other => {
            anyhow::bail!("Expect SequenceNumberTooHigh for object {object_id} but got {other}.")
        }
    };

    Ok(())
}

// Object fast path should be disabled and unused.
#[sim_test]
async fn test_pass_back_no_object() -> Result<(), anyhow::Error> {
    let _pcool_guard = override_pcool_flow(false);
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let rgp = test_cluster.get_reference_gas_price().await;
    let fullnode = test_cluster.spawn_new_fullnode().await.iota_node;

    let context = &mut test_cluster.wallet;

    let sender = context
        .config()
        .keystore()
        .addresses()
        .first()
        .cloned()
        .unwrap();

    // TODO: this is publishing the wrong package - we should be publishing the one
    // in `iota-core/src/unit_tests/data` instead.
    let package_ref = publish_basics_package(context).await;

    let gas_obj = context
        .get_one_gas_object_owned_by_address(sender)
        .await
        .unwrap()
        .unwrap();

    let transaction_orchestrator = fullnode.with(|node| {
        node.transaction_orchestrator()
            .expect("Fullnode should have transaction orchestrator toggled on.")
    });
    let mut rx = fullnode.with(|node| {
        node.subscribe_to_transaction_orchestrator_effects()
            .expect("Fullnode should have transaction orchestrator toggled on.")
    });

    let tx = Transaction::new_move_call(
        sender,
        package_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("use_clock"),
        // type_args
        vec![],
        gas_obj,
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
        rgp,
    )
    .unwrap();
    let tx = to_sender_signed_transaction(
        tx,
        context
            .config()
            .keystore()
            .get_key(&sender)
            .unwrap()
            .as_keypair()?,
    );

    let digest = *tx.digest();
    let _res = transaction_orchestrator
        .execute_transaction_block(
            ExecuteTransactionRequestV1::new(tx),
            ExecuteTransactionRequestType::WaitForLocalExecution,
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to execute transaction {digest:?}: {e:?}"));
    println!("res: {_res:?}");

    let (
        _tx,
        QuorumDriverResponse {
            effects_cert: _certified_txn_effects,
            events: _txn_events,
            ..
        },
    ) = rx.recv().await.unwrap().unwrap();
    Ok(())
}

#[sim_test]
async fn test_access_old_object_pruned() {
    // This test checks that when we ask a validator to handle a transaction that
    // uses an old object that's already been pruned, it's able to return an
    // non-retriable error ObjectVersionUnavailableForConsumption, instead of
    // the retriable error ObjectNotFound.
    let test_cluster = TestClusterBuilder::new().build().await;
    let tx_builder = test_cluster.test_transaction_builder().await;
    let sender = tx_builder.sender();
    let gas_object = tx_builder.gas_object();
    let effects = test_cluster
        .sign_and_execute_transaction(&tx_builder.transfer_iota(None, sender).build())
        .await;
    let new_gas_version = effects.gas_object().reference.version;
    test_cluster.force_new_epoch().await;
    // Construct a new transaction that uses the old gas object reference.
    let tx = test_cluster.sign_transaction(
        &test_cluster
            .test_transaction_builder_with_gas_object(sender, gas_object)
            .await
            // Make sure we are doing something different from the first transaction.
            // Otherwise we would just end up with the same digest.
            .transfer_iota(Some(1), sender)
            .build(),
    );
    for validator in test_cluster.swarm.active_validators() {
        validator
            .get_node_handle()
            .unwrap()
            .with_async(|node| async {
                let state = node.state();
                // Make sure the old version of the object is already pruned.
                assert!(
                    state
                        .database_for_testing()
                        .get_object_by_key(&gas_object.object_id, gas_object.version)
                        .is_none()
                );
                // Relocation alone would already have taken that version out of
                // the live table. It is gone from the historic bucket as well,
                // which nothing here drives: the epoch boundary crossed above
                // expired the bucket of the epoch the transfer ran in, at these
                // validators' default retention of zero historic epochs.
                assert!(
                    state
                        .get_historic_objects()
                        .get(&ObjectKey(gas_object.object_id, gas_object.version))
                        .unwrap()
                        .is_none()
                );
                let epoch_store = state.epoch_store_for_testing();
                assert_eq!(
                    state
                        .handle_transaction(
                            &epoch_store,
                            epoch_store.verify_transaction(tx.clone()).unwrap()
                        )
                        .await
                        .unwrap_err(),
                    IotaError::UserInput {
                        error: UserInputError::ObjectVersionUnavailableForConsumption {
                            provided_obj_ref: gas_object,
                            current_version: new_gas_version,
                        }
                    }
                );
            })
            .await;
    }

    // Check that fullnode would return the same error.
    let result = test_cluster.wallet.execute_transaction_may_fail(tx).await;
    assert!(
        result.unwrap_err().to_string().contains(
            &UserInputError::ObjectVersionUnavailableForConsumption {
                provided_obj_ref: gas_object,
                current_version: new_gas_version,
            }
            .to_string()
        )
    )
}

async fn transfer_coin(
    context: &WalletContext,
) -> Result<
    (
        ObjectId,
        Address,
        Address,
        TransactionDigest,
        ObjectReference,
    ),
    anyhow::Error,
> {
    let gas_price = context.get_reference_gas_price().await?;
    let accounts_and_objs = context.get_all_accounts_and_gas_objects().await.unwrap();
    let sender = accounts_and_objs[0].0;
    let receiver = accounts_and_objs[1].0;
    let gas_object = accounts_and_objs[0].1[0];
    let object_to_send = accounts_and_objs[0].1[1];
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .transfer(object_to_send, receiver)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    Ok((
        object_to_send.object_id,
        sender,
        receiver,
        resp.digest,
        gas_object,
    ))
}

#[sim_test]
async fn test_full_node_run_with_range_checkpoint() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let stop_after_checkpoint_seq = 5;
    let want_run_with_range = Some(RunWithRange::Checkpoint(stop_after_checkpoint_seq));
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(10_000)
        .with_fullnode_run_with_range(want_run_with_range)
        .build()
        .await;

    // wait for node to signal that we reached and processed our desired epoch
    let got_run_with_range = test_cluster.wait_for_run_with_range_shutdown_signal().await;

    // ensure we got the expected RunWithRange on shutdown channel
    assert_eq!(got_run_with_range, want_run_with_range);

    // ensure the highest synced checkpoint matches
    assert!(test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            == Some(stop_after_checkpoint_seq)
    }));

    // sleep some time to ensure we don't see further ccheckpoints executed
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    // verify again execution has not progressed beyond expectations
    assert!(test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            == Some(stop_after_checkpoint_seq)
    }));

    // we dont want transaction orchestrator enabled when run_with_range != None
    assert!(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.transaction_orchestrator())
            .is_none()
    );
    Ok(())
}

#[sim_test]
async fn test_full_node_run_with_range_epoch() -> Result<(), anyhow::Error> {
    telemetry_subscribers::init_for_testing();
    let stop_after_epoch = 2;
    let want_run_with_range = Some(RunWithRange::Epoch(stop_after_epoch));
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(10_000)
        .with_fullnode_run_with_range(want_run_with_range)
        .build()
        .await;

    // wait for node to signal that we reached and processed our desired epoch
    let got_run_with_range = test_cluster.wait_for_run_with_range_shutdown_signal().await;

    // ensure we get the shutdown signal
    assert_eq!(got_run_with_range, want_run_with_range);

    // ensure we end up at epoch + 1
    // this is because we execute the target epoch, reconfigure, and then send
    // shutdown signal at epoch + 1
    assert!(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.current_epoch_for_testing() == stop_after_epoch + 1)
    );

    // epoch duration is 10s for testing, lets sleep long enough that epoch would
    // normally progress
    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;

    // ensure we are still at epoch + 1
    assert!(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.current_epoch_for_testing() == stop_after_epoch + 1)
    );

    // we dont want transaction orchestrator enabled when run_with_range != None
    assert!(
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.transaction_orchestrator())
            .is_none()
    );

    Ok(())
}

/// Balance and object changes are assembled from the transaction's input
/// pre-images, read by exact version. The commit of the transaction's own
/// checkpoint moves those versions out of the live objects table, so the read
/// has to reach the historic buckets to still answer.
#[sim_test]
async fn transaction_changes_resolve_after_relocation() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let (_, _, _, digest, _) = transfer_coin(&test_cluster.wallet).await.unwrap();

    let jsonrpc_client = &test_cluster.fullnode_handle.rpc_client;
    let options = IotaTransactionBlockResponseOptions::new()
        .with_balance_changes()
        .with_object_changes();

    // The versions the transaction superseded are relocated by the batch that
    // commits its checkpoint, which is done once that checkpoint counts as
    // executed.
    let sequence_number = timeout(Duration::from_secs(60), async {
        loop {
            let response: IotaTransactionBlockResponse = jsonrpc_client
                .request(
                    "iota_getTransactionBlock",
                    rpc_params![digest, options.clone()],
                )
                .await
                .unwrap();
            if let Some(sequence_number) = response.checkpoint {
                break sequence_number;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for the transaction to be checkpointed");
    test_cluster
        .wait_for_checkpoint(sequence_number, None)
        .await;

    let response: IotaTransactionBlockResponse = jsonrpc_client
        .request("iota_getTransactionBlock", rpc_params![digest, options])
        .await
        .unwrap();
    assert!(
        response.errors.is_empty(),
        "response errors: {:?}",
        response.errors
    );
    assert!(response.balance_changes.is_some());
    assert!(response.object_changes.is_some());
}

// This test checks that the fullnode is able to resolve events emitted from a
// transaction that references the structs defined in the package published by
// the transaction itself, without local execution.
#[sim_test]
async fn publish_init_events_without_local_execution() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move_test_code");
    let tx_data = test_cluster
        .test_transaction_builder()
        .await
        .publish(path)
        .build();
    let tx = test_cluster.sign_transaction(&tx_data);
    let client = test_cluster.wallet.get_client().await.unwrap();
    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            tx,
            IotaTransactionBlockResponseOptions::new().with_events(),
            Some(ExecuteTransactionRequestType::WaitForEffectsCert),
        )
        .await
        .unwrap();
    assert_eq!(response.events.unwrap().data.len(), 1);
}
