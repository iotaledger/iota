// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{SocketAddr},
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use diesel::PgConnection;
use iota_config::{
    local_ip_utils::{get_available_port, new_local_tcp_socket_for_testing},
    node::RunWithRange,
};
use iota_indexer::{
    IndexerConfig,
    errors::IndexerError,
    indexer::Indexer,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::{ReaderWriterConfig, start_test_indexer},
};
use iota_json_rpc_api::ReadApiClient;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_metrics::init_metrics;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::TransactionDigest,
};
use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    types::ErrorObject,
};
use simulacrum::Simulacrum;
use tempfile::tempdir;
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio::{runtime::Runtime, task::JoinHandle};

const DEFAULT_DB_URL: &str = "postgres://postgres:postgrespw@localhost:5432/iota_indexer";
const DEFAULT_INDEXER_IP: &str = "127.0.0.1";
const DEFAULT_INDEXER_PORT: u16 = 9005;
const DEFAULT_SERVER_PORT: u16 = 3000;

static GLOBAL_API_TEST_SETUP: OnceLock<ApiTestSetup> = OnceLock::new();

pub struct ApiTestSetup {
    pub runtime: Runtime,
    pub cluster: TestCluster,
    pub store: PgIndexerStore<PgConnection>,
    /// Indexer RPC Client
    pub client: HttpClient,
}

impl ApiTestSetup {
    pub fn get_or_init() -> &'static ApiTestSetup {
        GLOBAL_API_TEST_SETUP.get_or_init(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            let (cluster, store, client) =
                runtime.block_on(start_test_cluster_with_read_write_indexer(
                    None,
                    Some("shared_test_indexer_db".to_string()),
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

pub struct SimulacrumApiTestEnvDefinition {
    pub unique_env_name: String,
    pub env_initializer: Box<dyn Fn() -> Simulacrum>,
}

pub struct InitializedSimulacrumEnv {
    pub runtime: Runtime,
    pub sim: Arc<Simulacrum>,
    pub store: PgIndexerStore,
    /// Indexer RPC Client
    pub client: HttpClient,
}

impl SimulacrumApiTestEnvDefinition {
    pub fn get_or_init_env<'a>(
        &self,
        initialized_env_container: &'a OnceLock<InitializedSimulacrumEnv>,
    ) -> &'a InitializedSimulacrumEnv {
        initialized_env_container.get_or_init(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let sim = Arc::new((self.env_initializer)());
            let db_name = format!("simulacrum_env_db_{}", self.unique_env_name);
            let (_, store, _, client) = runtime.block_on(
                start_simulacrum_rest_api_with_read_write_indexer(sim.clone(), Some(db_name)),
            );

            InitializedSimulacrumEnv {
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
    stop_cluster_after_checkpoint_seq: Option<u64>,
    database_name: Option<String>,
) -> (TestCluster, PgIndexerStore<PgConnection>, HttpClient) {
    let temp = tempdir().unwrap().into_path();
    let mut builder = TestClusterBuilder::new().with_data_ingestion_dir(temp.clone());

    // run the cluster until the declared checkpoint sequence number
    if let Some(stop_cluster_after_checkpoint_seq) = stop_cluster_after_checkpoint_seq {
        builder = builder.with_fullnode_run_with_range(Some(RunWithRange::Checkpoint(
            stop_cluster_after_checkpoint_seq,
        )));
    };

    let cluster = builder.build().await;

    // start indexer in write mode
    let (pg_store, _pg_store_handle) = start_test_indexer(
        Some(DEFAULT_DB_URL.to_owned()),
        cluster.rpc_url().to_string(),
        ReaderWriterConfig::writer_mode(None),
        temp.clone(),
        database_name.clone(),
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

/// Wait for the indexer to catch up to the given checkpoint sequence number
///
/// Indexer starts storing data after checkpoint 0
pub async fn indexer_wait_for_checkpoint(
    pg_store: &PgIndexerStore<PgConnection>,
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
pub async fn indexer_wait_for_latest_checkpoint(
    pg_store: &PgIndexerStore<PgConnection>,
    cluster: &TestCluster,
) {
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
            if client
                .get_object(object_id, None)
                .await
                .unwrap()
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
    pg_store: &PgIndexerStore<PgConnection>,
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

fn replace_db_name(db_url: &str, new_db_name: &str) -> String {
    let pos = db_url.rfind('/').expect("Unable to find / in db_url");
    format!("{}/{}", &db_url[..pos], new_db_name)
}

/// Start an Indexer instance in `Read` mode
fn start_indexer_reader(fullnode_rpc_url: impl Into<String>, data_ingestion_path: PathBuf, database_name: Option<String>) -> u16 {
    let db_url = match database_name {
        Some(database_name) => replace_db_name(DEFAULT_DB_URL, &database_name),
        None => DEFAULT_DB_URL.to_owned(),
    };

    let port = get_available_port(DEFAULT_INDEXER_IP);
    let config = IndexerConfig {
        db_url: Some(db_url.clone()),
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

    tokio::spawn(async move {
        Indexer::start_reader::<PgConnection>(&config, &registry, db_url).await
    });
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
    database_name: Option<String>,
) -> (
    JoinHandle<()>,
    PgIndexerStore<PgConnection>,
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
        Some(DEFAULT_DB_URL.to_owned()),
        format!("http://{}", server_url),
        ReaderWriterConfig::writer_mode(None),
        data_ingestion_path,
        database_name,
    )
    .await;
    (server_handle, pg_store, pg_handle)
}

pub async fn start_simulacrum_rest_api_with_read_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    database_name: Option<String>,
) -> (
    JoinHandle<()>,
    PgIndexerStore<PgConnection>,
    JoinHandle<Result<(), IndexerError>>,
    HttpClient,
) {
    let server_url = new_local_tcp_socket_for_testing();
    let (server_handle, pg_store, pg_handle) =
        start_simulacrum_rest_api_with_write_indexer(sim, data_ingestion_path.clone(), Some(server_url), database_name.clone())
            .await;

    // start indexer in read mode
    let indexer_port = start_indexer_reader(format!("http://{}", server_url), data_ingestion_path, database_name);

    // create an RPC client by using the indexer url
    let rpc_client = HttpClientBuilder::default()
        .build(format!("http://{DEFAULT_INDEXER_IP}:{indexer_port}"))
        .unwrap();

    (server_handle, pg_store, pg_handle, rpc_client)
}
