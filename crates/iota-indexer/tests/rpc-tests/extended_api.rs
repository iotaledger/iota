// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    ExtendedApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    EndOfEpochInfo, EpochInfo, EpochMetrics, IotaObjectDataOptions, IotaObjectResponseQuery,
    IotaTransactionBlockResponseOptions, TransactionBlockBytes,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    gas_coin::GAS,
    quorum_driver_types::ExecuteTransactionRequestType,
    storage::ReadStore,
    IOTA_FRAMEWORK_ADDRESS,
};
use serial_test::serial;
use simulacrum::Simulacrum;
use test_cluster::TestCluster;

use crate::common::pg_integration::{
    indexer_wait_for_checkpoint, start_simulacrum_rest_api_with_read_write_indexer,
    start_test_cluster_with_read_write_indexer,
};

#[tokio::test]
#[serial]
async fn test_get_epochs() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epochs = indexer_client.get_epochs(None, None, None).await.unwrap();

    assert_eq!(epochs.data.len(), 3);
    assert_eq!(epochs.has_next_page, false);
    assert!(matches!(
        epochs.data[0],
        EpochInfo {
            epoch: 0,
            first_checkpoint_id: 0,
            epoch_total_transactions: 17, // 15 + 1 for genesis + 1 for end of epoch
            end_of_epoch_info: Some(EndOfEpochInfo {
                last_checkpoint_id: 1,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        epochs.data[1],
        EpochInfo {
            epoch: 1,
            first_checkpoint_id: 2,
            epoch_total_transactions: 28, // 17 from previous epoch + 10 + 1 for end of epoch
            end_of_epoch_info: Some(EndOfEpochInfo {
                last_checkpoint_id: 2,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        epochs.data[2],
        EpochInfo {
            epoch: 2,
            first_checkpoint_id: 3,
            epoch_total_transactions: 0, // set to 0 for ongoing epoch, is it intended?
            end_of_epoch_info: None,
            ..
        }
    ));
}

#[tokio::test]
#[serial]
async fn test_get_epochs_descending() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epochs = indexer_client
        .get_epochs(None, None, Some(true))
        .await
        .unwrap();

    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 3);
    assert_eq!(epochs.has_next_page, false);
    assert_eq!(actual_epochs_order, [2, 1, 0])
}

#[tokio::test]
#[serial]
async fn test_get_epochs_paging() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epochs = indexer_client
        .get_epochs(None, Some(2), None)
        .await
        .unwrap();
    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 2);
    assert_eq!(epochs.has_next_page, true);
    assert_eq!(epochs.next_cursor, Some(1.into()));
    assert_eq!(actual_epochs_order, [0, 1]);

    let epochs = indexer_client
        .get_epochs(Some(1.into()), Some(2), None)
        .await
        .unwrap();
    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 1);
    assert_eq!(epochs.has_next_page, false);
    assert_eq!(epochs.next_cursor, Some(2.into()));
    assert_eq!(actual_epochs_order, [2]);
}

#[tokio::test]
#[serial]
async fn test_get_epoch_metrics() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epoch_metrics = indexer_client
        .get_epoch_metrics(None, None, None)
        .await
        .unwrap();

    assert_eq!(epoch_metrics.data.len(), 3);
    assert_eq!(epoch_metrics.has_next_page, false);
    assert!(matches!(
        epoch_metrics.data[0],
        EpochMetrics {
            epoch: 0,
            first_checkpoint_id: 0,
            epoch_total_transactions: 17, // 15 + 1 for genesis + 1 for end of epoch
            end_of_epoch_info: Some(EndOfEpochInfo {
                last_checkpoint_id: 1,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        epoch_metrics.data[1],
        EpochMetrics {
            epoch: 1,
            first_checkpoint_id: 2,
            epoch_total_transactions: 28, // 17 from previous epoch + 10 + 1 for end of epoch
            end_of_epoch_info: Some(EndOfEpochInfo {
                last_checkpoint_id: 2,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        epoch_metrics.data[2],
        EpochMetrics {
            epoch: 2,
            first_checkpoint_id: 3,
            epoch_total_transactions: 0, // set to 0 for ongoing epoch, is it intended?
            end_of_epoch_info: None,
            ..
        }
    ));
}

#[tokio::test]
#[serial]
async fn test_get_epoch_metrics_descending() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epochs = indexer_client
        .get_epoch_metrics(None, None, Some(true))
        .await
        .unwrap();

    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 3);
    assert_eq!(epochs.has_next_page, false);
    assert_eq!(actual_epochs_order, [2, 1, 0])
}

#[tokio::test]
#[serial]
async fn test_get_epoch_metrics_paging() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let epochs = indexer_client
        .get_epoch_metrics(None, Some(2), None)
        .await
        .unwrap();
    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 2);
    assert_eq!(epochs.has_next_page, true);
    assert_eq!(epochs.next_cursor, Some(1.into()));
    assert_eq!(actual_epochs_order, [0, 1]);

    let epochs = indexer_client
        .get_epoch_metrics(Some(1.into()), Some(2), None)
        .await
        .unwrap();
    let actual_epochs_order = epochs
        .data
        .iter()
        .map(|epoch| epoch.epoch)
        .collect::<Vec<u64>>();

    assert_eq!(epochs.data.len(), 1);
    assert_eq!(epochs.has_next_page, false);
    assert_eq!(epochs.next_cursor, Some(2.into()));
    assert_eq!(actual_epochs_order, [2]);
}

#[tokio::test]
#[serial]
async fn test_get_current_epoch() {
    let mut sim = Simulacrum::new();
    add_test_epochs_to_simulacrum(&mut sim, &[15, 10, 5]);
    let last_checkpoint = sim.get_latest_checkpoint().unwrap();

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, last_checkpoint.sequence_number).await;

    let current_epoch = indexer_client.get_current_epoch().await.unwrap();

    assert!(matches!(
        current_epoch,
        EpochInfo {
            epoch: 2,
            first_checkpoint_id: 3,
            epoch_total_transactions: 0, // set to 0 for ongoing epoch, is it intended?
            end_of_epoch_info: None,
            ..
        }
    ));
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2368524531"]
#[tokio::test]
#[serial]
async fn test_get_network_metrics() {
    let (_, pg_store, indexer_client) = start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 10).await;

    let network_metrics = indexer_client.get_network_metrics().await.unwrap();

    println!("{:#?}", network_metrics);
}

#[tokio::test]
#[serial]
async fn test_get_move_call_metrics() {
    let (cluster, pg_store, indexer_client) =
        start_test_cluster_with_read_write_indexer(None).await;

    execute_move_fn(&cluster).await.unwrap();

    let latest_checkpoint_sn = cluster
        .rpc_client()
        .get_latest_checkpoint_sequence_number()
        .await
        .unwrap();
    indexer_wait_for_checkpoint(&pg_store, latest_checkpoint_sn.into_inner()).await;

    let move_call_metrics = indexer_client.get_move_call_metrics().await.unwrap();

    // TODO: Why is the move call not included in the stats?
    assert_eq!(move_call_metrics.rank_3_days.len(), 0);
    assert_eq!(move_call_metrics.rank_7_days.len(), 0);
    assert_eq!(move_call_metrics.rank_30_days.len(), 0);
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2368524531"]
#[tokio::test]
#[serial]
async fn test_get_latest_address_metrics() {
    let (_, pg_store, indexer_client) = start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 10).await;

    let address_metrics = indexer_client.get_latest_address_metrics().await.unwrap();

    println!("{:#?}", address_metrics);
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2368524531"]
#[tokio::test]
#[serial]
async fn test_get_checkpoint_address_metrics() {
    let (_, pg_store, indexer_client) = start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 10).await;

    let address_metrics = indexer_client
        .get_checkpoint_address_metrics(0)
        .await
        .unwrap();

    println!("{:#?}", address_metrics);
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2368524531"]
#[tokio::test]
#[serial]
async fn test_get_all_epoch_address_metrics() {
    let (_, pg_store, indexer_client) = start_test_cluster_with_read_write_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 10).await;

    let address_metrics = indexer_client
        .get_all_epoch_address_metrics(None)
        .await
        .unwrap();

    println!("{:#?}", address_metrics);
}

#[tokio::test]
#[serial]
async fn test_get_total_transactions() {
    let mut sim = Simulacrum::new();
    execute_simulacrum_transactions(&mut sim, 5);

    let latest_checkpoint = sim.create_checkpoint();
    let total_transactions_count = latest_checkpoint.network_total_transactions;

    let (_, pg_store, _, indexer_client) =
        start_simulacrum_rest_api_with_read_write_indexer(Arc::new(sim)).await;
    indexer_wait_for_checkpoint(&pg_store, latest_checkpoint.sequence_number).await;

    let transactions_cnt = indexer_client.get_total_transactions().await.unwrap();
    assert_eq!(transactions_cnt.into_inner(), total_transactions_count);
    assert_eq!(transactions_cnt.into_inner(), 6);
}

fn add_test_epochs_to_simulacrum(mut sim: &mut Simulacrum, tx_counts_in_epochs: &[u32]) {
    if let [counts_in_finished_epochs @ .., current_epoch_tx_count] = tx_counts_in_epochs {
        for tx_count in counts_in_finished_epochs {
            execute_simulacrum_transactions(&mut sim, *tx_count);
            sim.advance_epoch(false);
        }

        execute_simulacrum_transactions(&mut sim, *current_epoch_tx_count);
        sim.create_checkpoint();
    } else {
        return;
    }
}

async fn execute_move_fn(cluster: &TestCluster) -> Result<(), anyhow::Error> {
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
    assert!(tx_response.status_ok().unwrap_or(false));
    Ok(())
}

fn execute_simulacrum_transaction(sim: &mut Simulacrum) {
    let transfer_recipient = IotaAddress::random_for_testing_only();
    let (transaction, _) = sim.transfer_txn(transfer_recipient);
    sim.execute_transaction(transaction.clone()).unwrap();
}

fn execute_simulacrum_transactions(sim: &mut Simulacrum, transactions_count: u32) {
    for _ in 0..transactions_count {
        execute_simulacrum_transaction(sim);
    }
}
