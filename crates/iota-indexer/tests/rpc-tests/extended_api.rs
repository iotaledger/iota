// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::OnceLock, time::Duration};

use diesel::{PgConnection, QueryDsl, RunQueryDsl, dsl::min, prelude::ExpressionMethods};
use iota_indexer::{
    config::RetentionConfig,
    metrics::IndexerMetrics,
    models::watermarks::StoredWatermark,
    processors::{
        address_metrics_processor::AddressMetricsProcessor,
        move_call_metrics_processor::MoveCallMetricsProcessor,
        network_metrics_processor::NetworkMetricsProcessor,
    },
    schema::{address_metrics, move_calls, tx_count_metrics, watermarks},
    store::{PgIndexerAnalyticalStore, PgIndexerStore},
};
use iota_json::{call_args, type_args};
use iota_json_rpc_api::{
    ExtendedApiClient, IndexerApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaObjectDataOptions, IotaObjectResponseQuery, IotaTransactionBlockResponseOptions,
    TransactionBlockBytes,
};
use iota_sdk_types::{Address, ObjectId, StructTag};
use iota_types::{quorum_driver_types::ExecuteTransactionRequestType, storage::ReadStore};
use prometheus_filtered::Registry;
use simulacrum::Simulacrum;
use test_cluster::TestCluster;

use crate::common::{
    ApiTestSetup, SimulacrumTestSetup, indexer_wait_for_checkpoint,
    indexer_wait_for_checkpoint_pruned, indexer_wait_for_latest_checkpoint, retry_with_timeout,
    start_test_cluster_with_read_write_indexer,
};

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
            sim.advance_epoch(false);

            execute_simulacrum_transactions(&mut sim, 10);
            add_checkpoints(&mut sim, 300);
            sim.advance_epoch(false);

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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
        let last_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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

        println!("{network_metrics:#?}");
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

        println!("{address_metrics:#?}");
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

        println!("{address_metrics:#?}");
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

        println!("{address_metrics:#?}");
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
        let latest_checkpoint = sim.try_get_latest_checkpoint().unwrap();
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
    let package_id = ObjectId::FRAMEWORK;
    let module = "pay".to_string();
    let function = "split".to_string();

    let transaction_bytes: TransactionBlockBytes = http_client
        .move_call(
            address,
            package_id,
            module,
            function,
            type_args![TypeTag::from(StructTag::new_gas())]?,
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
            Some(ExecuteTransactionRequestType::WaitForLocalExecution.into()),
        )
        .await?;
    assert!(tx_response.status_ok().unwrap_or(false));
    Ok(())
}

fn execute_simulacrum_transaction(sim: &mut Simulacrum) {
    let transfer_recipient = Address::random();
    let (transaction, _) = sim.transfer_txn(transfer_recipient);
    sim.execute_transaction(transaction).unwrap();
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

/// Runs a read-only query against the test database.
async fn query<T: Send + 'static>(
    store: &PgIndexerStore,
    query: impl FnOnce(&mut PgConnection) -> diesel::QueryResult<T> + Send + 'static,
) -> T {
    let pool = store.blocking_cp();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().expect("failed to get a connection");
        query(&mut conn).expect("query failed")
    })
    .await
    .expect("failed to join blocking task")
}

/// Prunes the genesis epoch, then checks that the network, address and move
/// call processors still write metrics, starting at the first checkpoint left
/// in the database.
///
/// Batch size is one so the first batch falls inside the pruned range. A
/// processor that starts from genesis instead of the lower bound then never
/// writes a row.
#[tokio::test]
async fn analytics_resume_from_the_first_available_checkpoint_on_a_pruned_database() {
    let (cluster, store, _client) = &start_test_cluster_with_read_write_indexer(
        Some("test_analytics_on_pruned_database"),
        None,
        Some(RetentionConfig::new(1, Default::default())),
    )
    .await;

    indexer_wait_for_checkpoint(store, 1).await;
    cluster.force_new_epoch().await;
    indexer_wait_for_checkpoint_pruned(store, 0).await;

    execute_move_fn(cluster).await.unwrap();
    indexer_wait_for_latest_checkpoint(store, cluster).await;

    let analytical_store = PgIndexerAnalyticalStore::new(store.blocking_cp());
    let metrics = IndexerMetrics::new(&Registry::new());

    let mut network_processor =
        NetworkMetricsProcessor::new(analytical_store.clone(), metrics.clone());
    network_processor.min_network_metrics_processor_batch_size = 1;
    network_processor.max_network_metrics_processor_batch_size = 1;
    let network_task = tokio::spawn(async move { network_processor.start().await });

    let mut address_processor =
        AddressMetricsProcessor::new(analytical_store.clone(), metrics.clone());
    address_processor.address_processor_batch_size = 1;
    address_processor.address_processor_parallelism = 1;
    let address_task = tokio::spawn(async move { address_processor.start().await });

    let mut move_call_processor = MoveCallMetricsProcessor::new(analytical_store, metrics);
    move_call_processor.move_call_processor_batch_size = 1;
    move_call_processor.move_call_processor_parallelism = 1;
    let move_call_task = tokio::spawn(async move { move_call_processor.start().await });

    let first_tx_count_checkpoint = retry_with_timeout(Duration::from_secs(60), || async move {
        query(store, |conn| {
            tx_count_metrics::table
                .select(min(tx_count_metrics::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
        })
        .await
    })
    .await
    .expect("timeout waiting for a tx count metrics row");
    let first_address_metrics_checkpoint =
        retry_with_timeout(Duration::from_secs(60), || async move {
            query(store, |conn| {
                address_metrics::table
                    .select(min(address_metrics::checkpoint))
                    .first::<Option<i64>>(conn)
            })
            .await
        })
        .await
        .expect("timeout waiting for an address metrics row");
    let first_move_call_checkpoint = retry_with_timeout(Duration::from_secs(60), || async move {
        query(store, |conn| {
            move_calls::table
                .select(min(move_calls::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
        })
        .await
    })
    .await
    .expect("timeout waiting for a move calls row");

    network_task.abort();
    address_task.abort();
    move_call_task.abort();

    // The pruner has finished with the genesis epoch by now and nothing else
    // is pruned, so the watermark is the first checkpoint still in the database.
    let checkpoints_watermark = query(store, |conn| {
        watermarks::table
            .filter(watermarks::entity.eq("checkpoints"))
            .first::<StoredWatermark>(conn)
    })
    .await;
    let first_available_checkpoint = checkpoints_watermark.min_available_cp;
    assert!(first_available_checkpoint > 0);

    // A checkpoint without transactions gets no tx count row, so the first row
    // may sit a little above the first available checkpoint, never below it.
    assert!(
        first_tx_count_checkpoint >= first_available_checkpoint,
        "tx count metrics start at checkpoint {first_tx_count_checkpoint}, \
             before the first available checkpoint {first_available_checkpoint}"
    );
    assert!(
        first_address_metrics_checkpoint >= first_available_checkpoint,
        "address metrics start at checkpoint {first_address_metrics_checkpoint}, \
             before the first available checkpoint {first_available_checkpoint}"
    );
    assert!(
        first_move_call_checkpoint >= first_available_checkpoint,
        "move calls start at checkpoint {first_move_call_checkpoint}, \
             before the first available checkpoint {first_available_checkpoint}"
    );
}
