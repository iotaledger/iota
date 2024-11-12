// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::OnceLock};

use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    ExtendedApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockResponseOptions,
    TransactionBlockBytes,
};
use iota_types::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::{IotaAddress, ObjectID},
    gas_coin::GAS,
    quorum_driver_types::ExecuteTransactionRequestType,
    storage::ReadStore,
};
use simulacrum::Simulacrum;
use test_cluster::TestCluster;

use crate::common::{ApiTestSetup, SimulacrumTestSetup, indexer_wait_for_checkpoint};

static EXTENDED_API_SHARED_SIMULACRUM_INITIALIZED_ENV: OnceLock<SimulacrumTestSetup> =
    OnceLock::new();

fn get_or_init_shared_extended_api_simulacrum_env() -> &'static SimulacrumTestSetup {
    SimulacrumTestSetup::get_or_init(
        "extended_api",
        |data_ingestion_path| {
            let mut sim = Simulacrum::new();
            sim.set_data_ingestion_path(data_ingestion_path);

            execute_simulacrum_transactions(&mut sim, 15);
            add_checkpoints(&mut sim, 300);
            sim.advance_epoch();

            execute_simulacrum_transactions(&mut sim, 10);
            add_checkpoints(&mut sim, 300);
            sim.advance_epoch();

            execute_simulacrum_transactions(&mut sim, 5);
            add_checkpoints(&mut sim, 300);

            sim
        },
        &EXTENDED_API_SHARED_SIMULACRUM_INITIALIZED_ENV,
    )
}

#[test]
fn get_epochs() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epochs = client.get_epochs(None, None, None).await.unwrap();

        assert_eq!(epochs.data.len(), 3);
        assert!(!epochs.has_next_page);

        let end_of_epoch_info = epochs.data[0].end_of_epoch_info.as_ref().unwrap();
        assert_eq!(epochs.data[0].epoch, 0);
        assert_eq!(epochs.data[0].first_checkpoint_id, 0);
        assert_eq!(epochs.data[0].epoch_total_transactions, 17);
        assert_eq!(end_of_epoch_info.last_checkpoint_id, 301);

        let end_of_epoch_info = epochs.data[1].end_of_epoch_info.as_ref().unwrap();
        assert_eq!(epochs.data[1].epoch, 1);
        assert_eq!(epochs.data[1].first_checkpoint_id, 302);
        assert_eq!(epochs.data[1].epoch_total_transactions, 11);
        assert_eq!(end_of_epoch_info.last_checkpoint_id, 602);

        assert_eq!(epochs.data[2].epoch, 2);
        assert_eq!(epochs.data[2].first_checkpoint_id, 603);
        assert_eq!(epochs.data[2].epoch_total_transactions, 0);
        assert!(epochs.data[2].end_of_epoch_info.is_none());
    });
}

#[test]
fn get_epochs_descending() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epochs = client.get_epochs(None, None, Some(true)).await.unwrap();

        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 3);
        assert!(!epochs.has_next_page);
        assert_eq!(actual_epochs_order, [2, 1, 0])
    });
}

#[test]
fn get_epochs_paging() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epochs = client.get_epochs(None, Some(2), None).await.unwrap();
        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 2);
        assert!(epochs.has_next_page);
        assert_eq!(epochs.next_cursor, Some(1.into()));
        assert_eq!(actual_epochs_order, [0, 1]);

        let epochs = client
            .get_epochs(Some(1.into()), Some(2), None)
            .await
            .unwrap();
        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 1);
        assert!(!epochs.has_next_page);
        assert_eq!(epochs.next_cursor, Some(2.into()));
        assert_eq!(actual_epochs_order, [2]);
    });
}

#[test]
fn get_epoch_metrics() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epoch_metrics = client.get_epoch_metrics(None, None, None).await.unwrap();

        assert_eq!(epoch_metrics.data.len(), 3);
        assert!(!epoch_metrics.has_next_page);

        let end_of_epoch_info = epoch_metrics.data[0].end_of_epoch_info.as_ref().unwrap();
        assert_eq!(epoch_metrics.data[0].epoch, 0);
        assert_eq!(epoch_metrics.data[0].first_checkpoint_id, 0);
        assert_eq!(epoch_metrics.data[0].epoch_total_transactions, 17);
        assert_eq!(end_of_epoch_info.last_checkpoint_id, 301);

        let end_of_epoch_info = epoch_metrics.data[1].end_of_epoch_info.as_ref().unwrap();
        assert_eq!(epoch_metrics.data[1].epoch, 1);
        assert_eq!(epoch_metrics.data[1].first_checkpoint_id, 302);
        assert_eq!(epoch_metrics.data[1].epoch_total_transactions, 11);
        assert_eq!(end_of_epoch_info.last_checkpoint_id, 602);

        assert_eq!(epoch_metrics.data[2].epoch, 2);
        assert_eq!(epoch_metrics.data[2].first_checkpoint_id, 603);
        assert_eq!(epoch_metrics.data[2].epoch_total_transactions, 0);
        assert!(epoch_metrics.data[2].end_of_epoch_info.is_none());
    });
}

#[test]
fn get_epoch_metrics_descending() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epochs = client
            .get_epoch_metrics(None, None, Some(true))
            .await
            .unwrap();

        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 3);
        assert!(!epochs.has_next_page);
        assert_eq!(actual_epochs_order, [2, 1, 0]);
    });
}

#[test]
fn get_epoch_metrics_paging() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let epochs = client.get_epoch_metrics(None, Some(2), None).await.unwrap();
        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 2);
        assert!(epochs.has_next_page);
        assert_eq!(epochs.next_cursor, Some(1.into()));
        assert_eq!(actual_epochs_order, [0, 1]);

        let epochs = client
            .get_epoch_metrics(Some(1.into()), Some(2), None)
            .await
            .unwrap();
        let actual_epochs_order = epochs
            .data
            .iter()
            .map(|epoch| epoch.epoch)
            .collect::<Vec<u64>>();

        assert_eq!(epochs.data.len(), 1);
        assert!(!epochs.has_next_page);
        assert_eq!(epochs.next_cursor, Some(2.into()));
        assert_eq!(actual_epochs_order, [2]);
    });
}

#[test]
fn get_current_epoch() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let last_checkpoint = sim.get_latest_checkpoint().unwrap();
        indexer_wait_for_checkpoint(store, last_checkpoint.sequence_number).await;

        let current_epoch = client.get_current_epoch().await.unwrap();

        assert_eq!(current_epoch.epoch, 2);
        assert_eq!(current_epoch.first_checkpoint_id, 603);
        assert_eq!(current_epoch.epoch_total_transactions, 0);
        assert!(current_epoch.end_of_epoch_info.is_none());
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2371642744"]
#[test]
fn get_network_metrics() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 10).await;

        let network_metrics = client.get_network_metrics().await.unwrap();

        println!("{:#?}", network_metrics);
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2371642744"]
#[test]
fn get_move_call_metrics() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        cluster,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        execute_move_fn(cluster).await.unwrap();

        let latest_checkpoint_sn = cluster
            .rpc_client()
            .get_latest_checkpoint_sequence_number()
            .await
            .unwrap();
        indexer_wait_for_checkpoint(store, latest_checkpoint_sn.into_inner()).await;

        let move_call_metrics = client.get_move_call_metrics().await.unwrap();

        // TODO: Why is the move call not included in the stats?
        assert_eq!(move_call_metrics.rank_3_days.len(), 0);
        assert_eq!(move_call_metrics.rank_7_days.len(), 0);
        assert_eq!(move_call_metrics.rank_30_days.len(), 0);
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2371642744"]
#[test]
fn get_latest_address_metrics() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 10).await;

        let address_metrics = client.get_latest_address_metrics().await.unwrap();

        println!("{:#?}", address_metrics);
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2371642744"]
#[test]
fn get_checkpoint_address_metrics() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 10).await;

        let address_metrics = client.get_checkpoint_address_metrics(0).await.unwrap();

        println!("{:#?}", address_metrics);
    });
}

#[ignore = "https://github.com/iotaledger/iota/issues/2197#issuecomment-2371642744"]
#[test]
fn get_all_epoch_address_metrics() {
    let ApiTestSetup {
        runtime,
        store,
        client,
        ..
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        indexer_wait_for_checkpoint(store, 10).await;

        let address_metrics = client.get_all_epoch_address_metrics(None).await.unwrap();

        println!("{:#?}", address_metrics);
    });
}

#[test]
fn get_total_transactions() {
    let SimulacrumTestSetup {
        runtime,
        sim,
        store,
        client,
    } = get_or_init_shared_extended_api_simulacrum_env();

    runtime.block_on(async move {
        let latest_checkpoint = sim.get_latest_checkpoint().unwrap();
        let total_transactions_count = latest_checkpoint.network_total_transactions;
        indexer_wait_for_checkpoint(store, latest_checkpoint.sequence_number).await;

        let transactions_cnt = client.get_total_transactions().await.unwrap();
        assert_eq!(transactions_cnt.into_inner(), total_transactions_count);
        assert_eq!(transactions_cnt.into_inner(), 33);
    });
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

fn add_checkpoints(sim: &mut Simulacrum, checkpoints_count: i32) {
    // Main use of this function is to create more checkpoints than the current
    // processing batch size, to circumvent the issue described in
    // https://github.com/iotaledger/iota/issues/2197#issuecomment-2376432709
    for _ in 0..checkpoints_count {
        sim.create_checkpoint();
    }
}
