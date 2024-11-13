// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, str::FromStr, time::Duration};

use iota_cluster_test::faucet::{FaucetClient, RemoteFaucetClient};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    CoinReadApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Checkpoint, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, MoveCallParams,
    ObjectChange, RPCTransactionRequestParams, TransactionBlockBytes,
};
use iota_move_build::BuildConfig;
use iota_sdk::IotaClientBuilder;
use iota_types::{
    self,
    base_types::{IotaAddress, ObjectID},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    object::Owner,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use itertools::Itertools;
use jsonrpsee::http_client::HttpClient;

use crate::common::{
    connect_to_remote_node_and_indexer, indexer_wait_for_coins, indexer_wait_for_object,
    indexer_wait_for_package, indexer_wait_for_transaction,
};

const EXPERIMENTS_CNT: u64 = 100;

#[tokio::test]
async fn test_transfer_object_fullnode() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_transfer_object_for_given_client(&node_client, &faucet, "test_transfer_object_fullnode")
        .await;
}

#[tokio::test]
async fn test_transfer_object_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_transfer_object_for_given_client(&indexer_client, &faucet, "test_transfer_object_indexer")
        .await;
}

async fn test_transfer_object_for_given_client(
    client: &HttpClient,
    faucet: &RemoteFaucetClient,
    test_name: &str,
) {
    let (sender, keypair_sender): (_, AccountKeyPair) = get_key_pair();
    let (receiver, keypair_receiver): (_, AccountKeyPair) = get_key_pair();

    faucet.request_iota_coins(sender).await;
    faucet.request_iota_coins(sender).await;
    faucet.request_iota_coins(receiver).await;

    let object_to_send = get_gas_object_id(&client, sender).await;

    for i in 0..EXPERIMENTS_CNT {
        let current_sender = if i % 2 == 0 { sender } else { receiver };
        let current_receiver = if i % 2 == 0 { receiver } else { sender };
        let current_sender_kp = if i % 2 == 0 {
            &keypair_sender
        } else {
            &keypair_receiver
        };

        let tx_bytes = client
            .transfer_object(
                current_sender,
                object_to_send,
                None,
                100_000_000.into(),
                current_receiver,
            )
            .await
            .unwrap();
        let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), current_sender_kp);
        let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
        let request_start_ts_ms = chrono::Utc::now().timestamp_millis() as i64;
        let res = client
            .execute_transaction_block(
                tx_bytes,
                signatures,
                Some(IotaTransactionBlockResponseOptions::full_content()),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap();
        measure_tx_response_time(request_start_ts_ms, &res, client, test_name).await;

        let transferred_object = client
            .get_object(object_to_send, Some(IotaObjectDataOptions::full_content()))
            .await
            .unwrap();

        assert_eq!(
            transferred_object.owner(),
            Some(Owner::AddressOwner(current_receiver))
        );
    }
}

#[tokio::test]
async fn test_increment_counter_public_fullnode() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &node_client,
        &faucet,
        false,
        1,
        "public",
        "test_increment_counter_public_fullnode",
    )
    .await;
}

#[tokio::test]
async fn test_increment_counter_public_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &indexer_client,
        &faucet,
        true,
        1,
        "public",
        "test_increment_counter_public_indexer",
    )
    .await;
}

#[tokio::test]
async fn test_increment_counter_private_fullnode() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &node_client,
        &faucet,
        false,
        1,
        "private",
        "test_increment_counter_private_fullnode",
    )
    .await;
}

#[tokio::test]
async fn test_increment_counter_private_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &indexer_client,
        &faucet,
        true,
        1,
        "private",
        "test_increment_counter_private_indexer",
    )
    .await;
}

#[tokio::test]
async fn test_numerous_increments_counter_public_fullnode() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &node_client,
        &faucet,
        false,
        1_000,
        "public",
        "test_numerous_increments_counter_public_fullnode",
    )
    .await;
}

#[tokio::test]
async fn test_numerous_increments_counter_public_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &indexer_client,
        &faucet,
        true,
        1_000,
        "public",
        "test_numerous_increments_counter_public_indexer",
    )
    .await; // Max PTB is 1024
}

#[tokio::test]
async fn test_numerous_increments_counter_private_fullnode() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &node_client,
        &faucet,
        false,
        1_000,
        "private",
        "test_numerous_increments_counter_private_fullnode",
    )
    .await;
}

#[tokio::test]
async fn test_numerous_increments_counter_private_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_increment_counter_for_given_client(
        &indexer_client,
        &faucet,
        true,
        1_000,
        "private",
        "test_numerous_increments_counter_private_indexer",
    )
    .await; // Max PTB is 1024
}

async fn test_increment_counter_for_given_client(
    client: &HttpClient,
    faucet: &RemoteFaucetClient,
    wait_for_subsequent_txs: bool,
    increment_n_times: u64,
    counter_type: &str,
    test_name: &str,
) {
    let (sender, keypair_sender): (_, AccountKeyPair) = get_key_pair();

    faucet.request_iota_coins(sender).await;
    faucet.request_iota_coins(sender).await;

    indexer_wait_for_coins(client, &sender, 2).await;

    let res = deploy_basics_pkg(sender, &keypair_sender, &client).await;

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

    let waits_count = indexer_wait_for_package(*package_id, client).await;
    // if !wait_for_subsequent_txs {
    //     assert_eq!(waits_count, 0);
    // }

    for i in 0..EXPERIMENTS_CNT {
        let current_sender = sender;
        let current_sender_kp = &keypair_sender;

        let (tx_digest, counter_obj) = create_counter_object(
            current_sender,
            current_sender_kp,
            &client,
            package_id,
            counter_type,
        )
        .await;
        // println!("Call result: {:#?}", counter_obj);

        let waits_count = indexer_wait_for_object(client, counter_obj, 1.into()).await;
        // if !wait_for_subsequent_txs {
        //     assert_eq!(waits_count, 0); // sometimes it happens that even fullnode
        // needs to wait a bit here }

        let (res, request_start_ts_ms) = increment_counter_n_times_batch(
            current_sender,
            current_sender_kp,
            &client,
            package_id,
            &counter_obj,
            increment_n_times,
        )
        .await
        .unwrap();
        measure_tx_response_time(request_start_ts_ms, &res, client, test_name).await;
    }
}

#[tokio::test]
async fn test_hit_the_hive_public_node() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_hit_the_hive(
        &node_client,
        &faucet,
        true,
        2_000,
        "public",
        "test_hit_the_hive_public_node",
    )
    .await;
}

#[tokio::test]
async fn test_hit_the_hive_public_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_hit_the_hive(
        &indexer_client,
        &faucet,
        true,
        2_000,
        "public",
        "test_hit_the_hive_public_indexer",
    )
    .await;
}

#[tokio::test]
async fn test_hit_the_hive_private_node() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_hit_the_hive(
        &node_client,
        &faucet,
        true,
        2_000,
        "private",
        "test_hit_the_hive_private_node",
    )
    .await;
}

#[tokio::test]
async fn test_hit_the_hive_private_indexer() {
    let (node_client, indexer_client, faucet) = connect_to_remote_node_and_indexer().await;

    test_hit_the_hive(
        &indexer_client,
        &faucet,
        true,
        2_000,
        "private",
        "test_hit_the_hive_private_indexer",
    )
    .await;
}

async fn test_hit_the_hive(
    client: &HttpClient,
    faucet: &RemoteFaucetClient,
    wait_for_subsequent_txs: bool,
    hit_strength: u64,
    hive_type: &str,
    test_name: &str,
) {
    let (sender, keypair_sender): (_, AccountKeyPair) = get_key_pair();

    faucet.request_iota_coins(sender).await;
    faucet.request_iota_coins(sender).await;
    faucet.request_iota_coins(sender).await;
    // faucet.request_iota_coins(sender).await;
    // faucet.request_iota_coins(sender).await;
    // faucet.request_iota_coins(sender).await;
    // faucet.request_iota_coins(sender).await;

    indexer_wait_for_coins(client, &sender, 3).await;

    let res = deploy_basics_pkg(sender, &keypair_sender, &client).await;

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

    let waits_count = indexer_wait_for_package(*package_id, client).await;
    // if !wait_for_subsequent_txs {
    //     assert_eq!(waits_count, 0);
    // }

    for i in 0..EXPERIMENTS_CNT {
        let current_sender = sender;
        let current_sender_kp = &keypair_sender;

        let (tx_digest, hive_obj) = create_bee_hive(
            current_sender,
            current_sender_kp,
            &client,
            package_id,
            hive_type,
        )
        .await;
        // println!("Call result: {:#?}", counter_obj);

        if wait_for_subsequent_txs {
            indexer_wait_for_object(client, hive_obj, 1.into()).await;
        }

        let (res, request_start_ts_ms) = hit_the_hive(
            current_sender,
            current_sender_kp,
            &client,
            package_id,
            &hive_obj,
            hit_strength,
        )
        .await
        .unwrap();
        measure_tx_response_time(request_start_ts_ms, &res, client, test_name).await;
    }
}

async fn measure_tx_response_time(
    request_start_ts_ms: i64,
    res: &IotaTransactionBlockResponse,
    client: &HttpClient,
    test_name: &str,
) {
    if !res.status_ok().unwrap() {
        println!("{:#?}", res);
        panic!("Request failed");
    }
    let wait_start_ts = chrono::Utc::now().timestamp_millis() as i64;

    indexer_wait_for_transaction(res, &client).await;
    let wait_end_ts = chrono::Utc::now().timestamp_millis() as i64;

    let (tx, cp) = wait_for_checkpoint_in_tx(client, res.digest).await;

    let tx_ts = tx.timestamp_ms.unwrap();
    assert_eq!(cp.timestamp_ms, tx_ts);
    println!(
        "Test: {} request_start_ts: {:?} Tx_cp: {:?} cp_ts: {} checkpoint created after: {:?} started waiting after: {:?} Wait Elapsed: {:.2?} Total request time: {}",
        test_name,
        request_start_ts_ms,
        tx.checkpoint.unwrap(),
        tx_ts,
        tx_ts as i64 - request_start_ts_ms,
        wait_start_ts - request_start_ts_ms,
        wait_end_ts - wait_start_ts,
        wait_end_ts - request_start_ts_ms
    );
}

async fn wait_for_checkpoint_in_tx(
    client: &HttpClient,
    tx_digest: TransactionDigest,
) -> (IotaTransactionBlockResponse, Checkpoint) {
    loop {
        let tx = loop {
            let tx = client
                .get_transaction_block(tx_digest, Some(IotaTransactionBlockResponseOptions::new()))
                .await;
            match tx {
                Ok(res) => {
                    break res;
                }
                Err(..) => {
                    println!("Couldn't get tx from API");
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        };

        let cp = match tx.checkpoint {
            Some(tx_cp) => client.get_checkpoint(tx_cp.into()).await.ok(),
            None => None,
        };

        match cp {
            Some(cp) => return (tx, cp),
            None => tokio::time::sleep(Duration::from_millis(1)).await,
        }
    }
}

async fn deploy_basics_pkg(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
) -> IotaTransactionBlockResponse {
    deploy_package(address, addres_kp, client, "../../examples/move/basics").await
}

async fn deploy_package(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    pkg_path: &str,
) -> IotaTransactionBlockResponse {
    // let path =
    // "/home/tomxey/repo/iota/examples/move/basics/sources/counter.move";
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
    res
}

async fn create_counter_object(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    counter_type: &str,
) -> (TransactionDigest, ObjectID) {
    let module = "counter".to_string();
    let function = format!("create_{}", counter_type).to_string();

    let tx_bytes: TransactionBlockBytes = loop {
        let txb = client
            .move_call(
                address,
                *package_id,
                module.clone(),
                function.clone(),
                type_args![].unwrap(),
                call_args!().unwrap(),
                None,
                10_000_000.into(),
                None,
            )
            .await;
        match txb {
            Ok(res) => {
                break res;
            }
            Err(..) => {
                println!("Couldn't construct tx bytes, retrying");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    };
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
    (res.digest, counter_obj_id)
}

async fn increment_counter_n_times_batch(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    counter_id: &ObjectID,
    increment_n_times: u64,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "counter".to_string();
    let function = "increment".to_string();

    let tx_bytes: TransactionBlockBytes = loop {
        let rpc_params = {
            let mut v = vec![];
            for _ in 0..increment_n_times {
                v.push(RPCTransactionRequestParams::MoveCallRequestParams(
                    MoveCallParams {
                        package_object_id: *package_id,
                        module: module.clone(),
                        function: function.clone(),
                        type_arguments: type_args![].unwrap(),
                        arguments: call_args!(counter_id).unwrap(),
                    },
                ));
            }
            v
        };
        let txb = client
            .batch_transaction(address, rpc_params, None, 3_000_000_000.into(), None)
            .await;
        match txb {
            Ok(res) => {
                break res;
            }
            Err(..) => {
                println!("Couldn't construct tx bytes, retrying");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis() as i64;
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

async fn create_bee_hive(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    hive_type: &str,
) -> (TransactionDigest, ObjectID) {
    let module = "bee_hive".to_string();
    let function = format!("create_{hive_type}_hive").to_string();

    let tx_bytes: TransactionBlockBytes = loop {
        let txb = client
            .move_call(
                address,
                *package_id,
                module.clone(),
                function.clone(),
                type_args![].unwrap(),
                call_args!().unwrap(),
                None,
                10_000_000.into(),
                None,
            )
            .await;
        match txb {
            Ok(res) => {
                break res;
            }
            Err(..) => {
                println!("Couldn't construct tx bytes, retrying");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    };

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
    let hive_obj_id = res
        .effects
        .unwrap()
        .created()
        .iter()
        .exactly_one()
        .unwrap()
        .object_id();
    (res.digest, hive_obj_id)
}

async fn hit_the_hive(
    address: IotaAddress,
    addres_kp: &AccountKeyPair,
    client: &HttpClient,
    package_id: &ObjectID,
    hive_id: &ObjectID,
    hit_strength: u64,
) -> Result<(IotaTransactionBlockResponse, i64), anyhow::Error> {
    let module = "bee_hive".to_string();
    let function = "hit_the_hive".to_string();

    let tx_bytes: TransactionBlockBytes = loop {
        let txb = client
            .move_call(
                address,
                *package_id,
                module.clone(),
                function.clone(),
                type_args![].unwrap(),
                call_args!(hive_id, hit_strength).unwrap(),
                None,
                3_000_000_000.into(),
                None,
            )
            .await;
        match txb {
            Ok(res) => {
                break res;
            }
            Err(..) => {
                println!("Couldn't construct tx bytes, retrying");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    };

    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), addres_kp);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();

    let request_start_ts_ms = chrono::Utc::now().timestamp_millis() as i64;
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

async fn get_gas_object_id(client: &HttpClient, address: IotaAddress) -> ObjectID {
    client
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data[0]
        .coin_object_id
}
