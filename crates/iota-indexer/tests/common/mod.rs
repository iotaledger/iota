// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use diesel::{QueryDsl, RunQueryDsl};
use fastcrypto::traits::Signer;
use iota_config::local_ip_utils::{get_available_port, new_local_tcp_socket_for_testing};
use iota_grpc_server::GrpcServerHandle;
use iota_indexer::{
    config::{IotaNamesOptions, JsonRpcConfig, PruningOptions, SnapshotLagConfig},
    db::{ConnectionPoolConfig, new_connection_pool},
    errors::IndexerError,
    indexer::Indexer,
    metrics::IndexerMetrics,
    read_only_blocking,
    schema::optimistic_transactions,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::{DBInitHook, IndexerTypeConfig, create_pg_store, db_url, start_test_indexer},
};
use iota_json_rpc_api::{
    CoinReadApiClient, ReadApiClient, TransactionBuilderClient, WriteApiClient,
};
use iota_json_rpc_types::{
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
    TransactionBlockBytes,
};
use iota_metrics::init_metrics;
use iota_move_build::BuildConfig;
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{IotaKeyPair, Signature},
    digests::TransactionDigest,
    quorum_driver_types::ExecuteTransactionRequestType,
    utils::to_sender_signed_transaction,
};
use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    types::ErrorObject,
};
use simulacrum::Simulacrum;
use simulacrum_server::start_simulacrum_grpc_server;
use tempfile::tempdir;
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{
    runtime::Runtime,
    sync::{Mutex, OnceCell},
    task::JoinHandle,
};

const DEFAULT_DB: &str = "iota_indexer";
const DEFAULT_INDEXER_IP: &str = "127.0.0.1";
const DEFAULT_INDEXER_PORT: u16 = 9005;
const DEFAULT_SERVER_PORT: u16 = 3000;
pub const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

static GLOBAL_API_TEST_SETUP: OnceLock<ApiTestSetup> = OnceLock::new();
static PACKAGE_PUBLISH_LOCK: OnceCell<Arc<Mutex<i64>>> = OnceCell::const_new();

pub struct ApiTestSetup {
    pub runtime: Runtime,
    pub cluster: TestCluster,
    pub store: PgIndexerStore,
    /// Indexer RPC Client
    pub client: HttpClient,
}

impl ApiTestSetup {
    pub fn get_or_init() -> &'static ApiTestSetup {
        GLOBAL_API_TEST_SETUP.get_or_init(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            let (cluster, store, client) =
                runtime.block_on(start_test_cluster_with_read_write_indexer(
                    Some("shared_test_indexer_db"),
                    None,
                    None,
                ));

            Self {
                runtime,
                cluster,
                store,
                client,
            }
        })
    }
}

pub struct SimulacrumTestSetup {
    pub runtime: Runtime,
    pub sim: Arc<Simulacrum>,
    pub store: PgIndexerStore,
    /// Indexer RPC Client
    pub client: HttpClient,
}

impl SimulacrumTestSetup {
    pub fn get_or_init<'a>(
        unique_env_name: &str,
        env_initializer: impl Fn(PathBuf) -> Simulacrum,
        initialized_env_container: &'a OnceLock<SimulacrumTestSetup>,
    ) -> &'a SimulacrumTestSetup {
        initialized_env_container.get_or_init(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let data_ingestion_path = tempdir().unwrap().keep();

            let sim = env_initializer(data_ingestion_path.clone());
            let sim = Arc::new(sim);

            let db_name = format!("simulacrum_env_db_{unique_env_name}");
            let (_, store, _, client) =
                runtime.block_on(start_simulacrum_grpc_with_read_write_indexer(
                    sim.clone(),
                    data_ingestion_path,
                    Some(&db_name),
                ));

            SimulacrumTestSetup {
                runtime,
                sim,
                store,
                client,
            }
        })
    }
}

/// Start a [`TestCluster`][`test_cluster::TestCluster`] with a `Read` &
/// `Write` indexer. Set `epochs_to_keep` (> 0) to enable indexer pruning.
pub async fn start_test_cluster_with_read_write_indexer(
    database_name: impl Into<Option<&str>>,
    builder_modifier: Option<Box<dyn FnOnce(TestClusterBuilder) -> TestClusterBuilder>>,
    pruning_options: Option<PruningOptions>,
) -> (TestCluster, PgIndexerStore, HttpClient) {
    let database_name = database_name.into();
    let mut builder = TestClusterBuilder::new().with_fullnode_enable_grpc_api(true);

    if let Some(builder_modifier) = builder_modifier {
        builder = builder_modifier(builder);
    };

    let cluster = builder.build().await;

    // start indexer in write mode
    let (pg_store, _pg_store_handle, _) = start_test_indexer(
        db_url(database_name.unwrap_or(DEFAULT_DB)),
        // reset the existing db
        true,
        None,
        cluster.grpc_url(),
        IndexerTypeConfig::writer_mode(None, pruning_options),
        None,
    )
    .await;

    // start indexer in read mode
    let indexer_port = start_indexer_reader(cluster.grpc_url(), database_name);

    // create an RPC client by using the indexer url
    let rpc_client = HttpClientBuilder::default()
        .build(format!("http://{DEFAULT_INDEXER_IP}:{indexer_port}"))
        .unwrap();

    (cluster, pg_store, rpc_client)
}

/// Wait for the indexer to catch up to the given checkpoint sequence number
///
/// Indexer starts storing data after checkpoint 0
pub async fn indexer_wait_for_checkpoint(
    pg_store: &PgIndexerStore,
    checkpoint_sequence_number: u64,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while {
            let cp_opt = pg_store
                .get_latest_checkpoint_sequence_number()
                .await
                .unwrap();
            cp_opt.is_none() || (cp_opt.unwrap() < checkpoint_sequence_number)
        } {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to catchup to checkpoint");
}

/// Wait for the indexer to catch up to the latest node checkpoint sequence
/// number. Indexer starts storing data after checkpoint 0
pub async fn indexer_wait_for_latest_checkpoint(pg_store: &PgIndexerStore, cluster: &TestCluster) {
    let latest_checkpoint = cluster
        .iota_client()
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await
        .unwrap();

    indexer_wait_for_checkpoint(pg_store, latest_checkpoint).await;
}

async fn wait_for_object(
    client: &HttpClient,
    object_id: ObjectID,
    sequence_number: SequenceNumber,
) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let Ok(obj_res) = client.get_object(object_id, None).await else {
                tokio::time::sleep(Duration::from_millis(20)).await;
                continue;
            };

            if obj_res
                .data
                .map(|obj| obj.version == sequence_number)
                .unwrap_or_default()
            {
                break;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await?;
    Ok(())
}

/// Wait for the indexer to catch up to the given object sequence number
pub async fn indexer_wait_for_object(
    client: &HttpClient,
    object_id: ObjectID,
    sequence_number: SequenceNumber,
) {
    wait_for_object(client, object_id, sequence_number)
        .await
        .expect("timeout waiting for indexer to catchup to given object's sequence number");
}

pub async fn node_wait_for_object(
    cluster: &TestCluster,
    object_id: ObjectID,
    sequence_number: SequenceNumber,
) {
    wait_for_object(cluster.rpc_client(), object_id, sequence_number)
        .await
        .expect("timeout waiting for node to catchup to given object's sequence number");
}

pub async fn get_optimistic_transactions_count(pg_store: &PgIndexerStore) -> u64 {
    let blocking_cp = pg_store.blocking_cp();
    tokio::task::spawn_blocking(move || {
        read_only_blocking!(&blocking_cp, |conn| {
            optimistic_transactions::table
                .count()
                .get_result::<i64>(conn)
        })
    })
    .await
    .unwrap()
    .unwrap() as u64
}

pub async fn indexer_wait_for_optimistic_transactions_count(
    pg_store: &PgIndexerStore,
    expected_transactions_count: u64,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let optimistic_transactions_count = get_optimistic_transactions_count(pg_store).await;
            if optimistic_transactions_count == expected_transactions_count {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to prune optimistic transactions");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // check once again, to ensure match was not accidental
    let optimistic_transactions_count = get_optimistic_transactions_count(pg_store).await;
    assert_eq!(optimistic_transactions_count, expected_transactions_count);
}

/// Wait for the indexer to prune the given checkpoint number
pub async fn indexer_wait_for_checkpoint_pruned(
    pg_store: &PgIndexerStore,
    checkpoint_sequence_number: u64,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let (min, _max) = pg_store
                .get_available_checkpoint_range()
                .await
                .expect("failed to get available checkpoint range");

            if min > checkpoint_sequence_number {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to prune checkpoint");
}

pub async fn indexer_wait_for_transaction(
    tx_digest: TransactionDigest,
    pg_store: &PgIndexerStore,
    indexer_client: &HttpClient,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(tx) = indexer_client
                .get_transaction_block(tx_digest, Some(IotaTransactionBlockResponseOptions::new()))
                .await
            {
                if let Some(checkpoint) = tx.checkpoint {
                    indexer_wait_for_checkpoint(pg_store, checkpoint).await;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to catchup to given transaction");
}

pub async fn execute_tx_and_wait_for_indexer_checkpoint(
    indexer_client: &HttpClient,
    store: &PgIndexerStore,
    tx_bytes: TransactionBlockBytes,
    keypair: &dyn Signer<Signature>,
) -> TransactionDigest {
    let digest = execute_tx_must_succeed(indexer_client, tx_bytes, keypair).await;
    indexer_wait_for_transaction(digest, store, indexer_client).await;
    digest
}

pub async fn execute_tx_must_succeed(
    indexer_client: &HttpClient,
    tx_bytes: TransactionBlockBytes,
    keypair: &dyn Signer<Signature>,
) -> TransactionDigest {
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), keypair);
    let (tx_bytes, signatures) = txn.to_tx_bytes_and_signatures();
    let indexer_tx_response = indexer_client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(IotaTransactionBlockResponseOptions::new().with_effects()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        indexer_tx_response.status_ok(),
        Some(true),
        "transaction failed: {indexer_tx_response:?}"
    );
    *txn.digest()
}

/// Start an Indexer instance in `Read` mode
fn start_indexer_reader(fullnode_rpc_url: impl Into<String>, database_name: Option<&str>) -> u16 {
    let db_url = db_url(database_name.unwrap_or(DEFAULT_DB));
    let port = get_available_port(DEFAULT_INDEXER_IP);

    let config = JsonRpcConfig {
        iota_names_options: IotaNamesOptions::default(),
        historic_fallback_options: Default::default(),
        rpc_address: SocketAddr::new(DEFAULT_INDEXER_IP.parse().unwrap(), port),
        rpc_client_url: fullnode_rpc_url.into(),
    };

    let pool = new_connection_pool(
        &db_url,
        &ConnectionPoolConfig {
            pool_size: 5,
            ..Default::default()
        },
    )
    .expect("creating new connection pool should succeed");

    let registry = prometheus::Registry::default();
    init_metrics(&registry);
    let metrics = IndexerMetrics::new(&registry);

    let store = create_pg_store(&db_url, false);

    tokio::spawn(
        async move { Indexer::start_reader(&config, store, &registry, pool, metrics).await },
    );
    port
}

/// Check if provided error message does match with
/// the [`jsonrpsee::core::ClientError::Call`] Error variant
pub fn rpc_call_error_msg_matches<T>(
    result: Result<T, jsonrpsee::core::ClientError>,
    raw_msg: &str,
) -> bool {
    let err_obj: ErrorObject = serde_json::from_str(raw_msg).unwrap();

    result.is_err_and(|err| match err {
        jsonrpsee::core::ClientError::Call(owned_obj) => {
            owned_obj.message() == ErrorObject::into_owned(err_obj).message()
        }
        _ => false,
    })
}

/// Set up a test indexer fetching from a gRPC endpoint served by the given
/// Simulacrum.
pub async fn start_simulacrum_grpc_with_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    server_url: Option<SocketAddr>,
    database_name: Option<&str>,
    db_init_hook: Option<DBInitHook>,
) -> (
    GrpcServerHandle,
    PgIndexerStore,
    JoinHandle<Result<(), IndexerError>>,
) {
    let address = server_url.unwrap_or_else(new_local_tcp_socket_for_testing);

    let config = iota_config::node::GrpcApiConfig {
        address,
        ..Default::default()
    };

    let server_handle = start_simulacrum_grpc_server(sim, config, Default::default())
        .await
        .unwrap();

    // Starts indexer
    let (pg_store, pg_handle, _) = start_test_indexer(
        db_url(database_name.unwrap_or(DEFAULT_DB)),
        true,
        db_init_hook,
        format!("http://{address}"),
        IndexerTypeConfig::writer_mode(
            Some(SnapshotLagConfig {
                snapshot_min_lag: 5,
                sleep_duration: 0,
            }),
            None,
        ),
        Some(data_ingestion_path),
    )
    .await;
    (server_handle, pg_store, pg_handle)
}

pub async fn start_simulacrum_grpc_with_read_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    database_name: Option<&str>,
) -> (
    GrpcServerHandle,
    PgIndexerStore,
    JoinHandle<Result<(), IndexerError>>,
    HttpClient,
) {
    let simulacrum_server_url = new_local_tcp_socket_for_testing();
    let (server_handle, pg_store, pg_handle) = start_simulacrum_grpc_with_write_indexer(
        sim,
        data_ingestion_path.clone(),
        Some(simulacrum_server_url),
        database_name,
        None,
    )
    .await;

    // start indexer in read mode
    let indexer_port =
        start_indexer_reader(format!("http://{simulacrum_server_url}"), database_name);

    // create an RPC client by using the indexer url
    let rpc_client = HttpClientBuilder::default()
        .build(format!("http://{DEFAULT_INDEXER_IP}:{indexer_port}"))
        .unwrap();

    (server_handle, pg_store, pg_handle, rpc_client)
}

/// Wait for the indexer to catch up to the given checkpoint sequence number for
/// objects snapshot.
pub async fn wait_for_objects_snapshot(
    pg_store: &PgIndexerStore,
    checkpoint_sequence_number: u64,
) -> Result<(), IndexerError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        while {
            let cp_opt = pg_store
                .get_latest_object_snapshot_watermark()
                .await
                .unwrap()
                .map(|watermark| watermark.checkpoint_hi_inclusive);
            cp_opt.is_none() || (cp_opt.unwrap() < checkpoint_sequence_number)
        } {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to catchup to checkpoint for objects snapshot");
    Ok(())
}

pub async fn publish_test_move_package(
    client: &HttpClient,
    address: IotaAddress,
    account_keypair: &IotaKeyPair,
    test_package_name: &str,
) -> Result<(ObjectRef, IotaTransactionBlockResponse), anyhow::Error> {
    let _lock = PACKAGE_PUBLISH_LOCK
        .get_or_init(async || Arc::new(tokio::sync::Mutex::new(0)))
        .await
        .lock()
        .await;

    let coins = client
        .get_coins(address, None, None, Some(1))
        .await
        .unwrap()
        .data;
    let gas = &coins[0];

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "data", test_package_name]);

    let compiled_package = BuildConfig::new_for_testing().build(&path).unwrap();
    let with_unpublished_deps = false;
    let compiled_modules_bytes = compiled_package.get_package_base64(with_unpublished_deps);
    let dependencies = compiled_package.get_dependency_storage_package_ids();

    let transaction_bytes: TransactionBlockBytes = client
        .publish(
            address,
            compiled_modules_bytes,
            dependencies,
            Some(gas.coin_object_id),
            100_000_000.into(),
        )
        .await
        .unwrap();

    let signed_transaction =
        to_sender_signed_transaction(transaction_bytes.to_data().unwrap(), account_keypair);
    let (tx_bytes, signatures) = signed_transaction.to_tx_bytes_and_signatures();

    let tx_response: IotaTransactionBlockResponse = client
        .execute_transaction_block(
            tx_bytes,
            signatures,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_object_changes()
                    .with_events(),
            ),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .unwrap();

    let object_changes = tx_response.object_changes.as_ref().unwrap();
    let package_object_ref = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { .. } => Some(change.object_ref()),
            _ => None,
        })
        .unwrap();

    Ok((package_object_ref, tx_response))
}
