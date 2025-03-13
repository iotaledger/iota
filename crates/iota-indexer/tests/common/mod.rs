// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use iota_config::local_ip_utils::{get_available_port, new_local_tcp_socket_for_testing};
use iota_indexer::{
    IndexerConfig,
    errors::IndexerError,
    handlers::objects_snapshot_handler::SnapshotLagConfig,
    indexer::Indexer,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::{IndexerTypeConfig, start_test_indexer},
};
use iota_json_rpc_api::ReadApiClient;
use iota_json_rpc_types::{IotaTransactionBlockResponseOptions, TransactionBlockBytes};
use iota_metrics::init_metrics;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    crypto::AccountKeyPair,
    digests::TransactionDigest,
    utils::to_sender_signed_transaction,
};
use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    types::ErrorObject,
};
use simulacrum::Simulacrum;
use tempfile::tempdir;
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{runtime::Runtime, task::JoinHandle};

const POSTGRES_URL: &str = "postgres://postgres:postgrespw@localhost:5432";
const DEFAULT_DB: &str = "iota_indexer";
const DEFAULT_INDEXER_IP: &str = "127.0.0.1";
const DEFAULT_INDEXER_PORT: u16 = 9005;
const DEFAULT_SERVER_PORT: u16 = 3000;

static GLOBAL_API_TEST_SETUP: OnceLock<ApiTestSetup> = OnceLock::new();

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

            let (cluster, store, client) = runtime.block_on(
                start_test_cluster_with_read_write_indexer(Some("shared_test_indexer_db"), None),
            );

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
            let data_ingestion_path = tempdir().unwrap().into_path();

            let sim = env_initializer(data_ingestion_path.clone());
            let sim = Arc::new(sim);

            let db_name = format!("simulacrum_env_db_{}", unique_env_name);
            let (_, store, _, client) =
                runtime.block_on(start_simulacrum_rest_api_with_read_write_indexer(
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
/// `Write` indexer
pub async fn start_test_cluster_with_read_write_indexer(
    database_name: Option<&str>,
    builder_modifier: Option<Box<dyn FnOnce(TestClusterBuilder) -> TestClusterBuilder>>,
) -> (TestCluster, PgIndexerStore, HttpClient) {
    let temp = tempdir().unwrap().into_path();
    let mut builder = TestClusterBuilder::new();

    if let Some(builder_modifier) = builder_modifier {
        builder = builder_modifier(builder);
    };

    let cluster = builder.build().await;

    // start indexer in write mode
    let (pg_store, _pg_store_handle) = start_test_indexer(
        get_indexer_db_url(database_name),
        // reset the existing db
        true,
        cluster.rpc_url().to_string(),
        IndexerTypeConfig::writer_mode(None),
        None,
    )
    .await;

    // start indexer in read mode
    let indexer_port = start_indexer_reader(cluster.rpc_url().to_owned(), temp, database_name);

    // create an RPC client by using the indexer url
    let rpc_client = HttpClientBuilder::default()
        .build(format!("http://{DEFAULT_INDEXER_IP}:{indexer_port}"))
        .unwrap();

    (cluster, pg_store, rpc_client)
}

fn get_indexer_db_url(database_name: Option<&str>) -> String {
    database_name.map_or_else(
        || format!("{POSTGRES_URL}/{DEFAULT_DB}"),
        |db_name| format!("{POSTGRES_URL}/{db_name}"),
    )
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
    .expect("Timeout waiting for indexer to catchup to checkpoint");
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

/// Wait for the indexer to catch up to the given object sequence number
pub async fn indexer_wait_for_object(
    client: &HttpClient,
    object_id: ObjectID,
    sequence_number: SequenceNumber,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let Ok(obj_res) = client.get_object(object_id, None).await else {
                continue;
            };

            if obj_res
                .data
                .map(|obj| obj.version == sequence_number)
                .unwrap_or_default()
            {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to given object's sequence number");
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
    .expect("Timeout waiting for indexer to catchup to given transaction");
}

pub async fn execute_tx_and_wait_for_indexer(
    indexer_client: &HttpClient,
    cluster: &TestCluster,
    store: &PgIndexerStore,
    tx_bytes: TransactionBlockBytes,
    keypair: &AccountKeyPair,
) {
    let txn = to_sender_signed_transaction(tx_bytes.to_data().unwrap(), keypair);
    let res = cluster.wallet.execute_transaction_must_succeed(txn).await;
    indexer_wait_for_transaction(res.digest, store, indexer_client).await;
}

/// Start an Indexer instance in `Read` mode
fn start_indexer_reader(
    fullnode_rpc_url: impl Into<String>,
    data_ingestion_path: PathBuf,
    database_name: Option<&str>,
) -> u16 {
    let db_url = get_indexer_db_url(database_name);
    let port = get_available_port(DEFAULT_INDEXER_IP);
    let config = IndexerConfig {
        db_url: Some(db_url.clone().into()),
        rpc_client_url: fullnode_rpc_url.into(),
        reset_db: true,
        rpc_server_worker: true,
        rpc_server_url: DEFAULT_INDEXER_IP.to_owned(),
        rpc_server_port: port,
        data_ingestion_path: Some(data_ingestion_path),
        ..Default::default()
    };

    let registry = prometheus::Registry::default();
    init_metrics(&registry);

    tokio::spawn(async move { Indexer::start_reader(&config, &registry, db_url).await });
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

/// Set up a test indexer fetching from a REST endpoint served by the given
/// Simulacrum.
pub async fn start_simulacrum_rest_api_with_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    server_url: Option<SocketAddr>,
    database_name: Option<&str>,
) -> (
    JoinHandle<()>,
    PgIndexerStore,
    JoinHandle<Result<(), IndexerError>>,
) {
    let server_url = server_url.unwrap_or_else(new_local_tcp_socket_for_testing);
    let server_handle = tokio::spawn(async move {
        iota_rest_api::RestService::new_without_version(sim)
            .start_service(server_url)
            .await;
    });
    // Starts indexer
    let (pg_store, pg_handle) = start_test_indexer(
        get_indexer_db_url(database_name),
        true,
        format!("http://{}", server_url),
        IndexerTypeConfig::writer_mode(Some(SnapshotLagConfig {
            snapshot_min_lag: 5,
            sleep_duration: 0,
        })),
        Some(data_ingestion_path),
    )
    .await;
    (server_handle, pg_store, pg_handle)
}

pub async fn start_simulacrum_rest_api_with_read_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    database_name: Option<&str>,
) -> (
    JoinHandle<()>,
    PgIndexerStore,
    JoinHandle<Result<(), IndexerError>>,
    HttpClient,
) {
    let simulacrum_server_url = new_local_tcp_socket_for_testing();
    let (server_handle, pg_store, pg_handle) = start_simulacrum_rest_api_with_write_indexer(
        sim,
        data_ingestion_path.clone(),
        Some(simulacrum_server_url),
        database_name,
    )
    .await;

    // start indexer in read mode
    let indexer_port = start_indexer_reader(
        format!("http://{}", simulacrum_server_url),
        data_ingestion_path,
        database_name,
    );

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
                .get_latest_object_snapshot_checkpoint_sequence_number()
                .await
                .unwrap();
            cp_opt.is_none() || (cp_opt.unwrap() < checkpoint_sequence_number)
        } {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to checkpoint for objects snapshot");
    Ok(())
}
