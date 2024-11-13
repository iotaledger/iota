// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use diesel::PgConnection;
use iota_cluster_test::faucet::RemoteFaucetClient;
use iota_config::local_ip_utils::{get_available_port, new_local_tcp_socket_for_testing};
use iota_indexer::{
    IndexerConfig,
    db::{ConnectionPoolConfig, new_connection_pool_with_config},
    errors::IndexerError,
    indexer::Indexer,
    metrics::IndexerMetrics,
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::{ReaderWriterConfig, start_test_indexer},
};
use iota_json_rpc_api::{CoinReadApiClient, MoveUtilsClient, ReadApiClient};
use iota_json_rpc_types::{
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions, ObjectChange,
};
use iota_metrics::init_metrics;
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber},
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

const POSTGRES_URL: &str = "postgres://postgres:postgrespw@localhost:5432";
const DEFAULT_DB: &str = "iota_indexer";
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
    pub store: PgIndexerStore<PgConnection>,
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
) -> (TestCluster, PgIndexerStore<PgConnection>, HttpClient) {
    let temp = tempdir().unwrap().into_path();
    let mut builder = TestClusterBuilder::new().with_data_ingestion_dir(temp.clone());

    if let Some(builder_modifier) = builder_modifier {
        builder = builder_modifier(builder);
    };

    let cluster = builder.build().await;

    // start indexer in write mode
    let (pg_store, _pg_store_handle) = start_test_indexer(
        Some(get_indexer_db_url(None)),
        cluster.rpc_url().to_string(),
        ReaderWriterConfig::writer_mode(None),
        temp.clone(),
        database_name,
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

pub async fn connect_to_remote_node_and_indexer() -> (HttpClient, HttpClient, RemoteFaucetClient) {
    let registry = prometheus::Registry::default();
    init_metrics(&registry);

    let testnet_addresses = (
        "https://api.testnet.iota.cafe",
        "https://indexer.testnet.iota.cafe",
        "https://faucet.testnet.iota.cafe",
    );

    let local_addresses = (
        "http://localhost:9000",
        "http://localhost:9005",
        "http://localhost:9123",
    );

    let addresses = local_addresses;
    let node_rpc_client = HttpClientBuilder::default().build(addresses.0).unwrap();
    let indexer_rpc_client = HttpClientBuilder::default().build(addresses.1).unwrap();
    let faucet = RemoteFaucetClient::new(addresses.2.into());

    (node_rpc_client, indexer_rpc_client, faucet)
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
) -> i32 {
    let mut waits_count = 0;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let Ok(obj_res) = client.get_object(object_id, None).await else {
                continue;
            };

            if obj_res
                .data
                .map(|obj| obj.version >= sequence_number)
                .unwrap_or_default()
            {
                println!("Object {} exists in get_object endpoint", object_id);
                break;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
            waits_count += 1;
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to given object's sequence number");
    waits_count
}

/// Wait for the indexer to catch up to the given object sequence number
pub async fn indexer_multi_wait_for_object(
    client: &HttpClient,
    object_ids: Vec<ObjectID>,
    sequence_numbers: Vec<SequenceNumber>,
) -> i32 {
    let mut waits_count = 0;
    let ids_chunks = object_ids.chunks(50);
    let seqnums_chunks = sequence_numbers.chunks(50);
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut futures = vec![];
        for (id_chunk, seqnum_chunk) in ids_chunks.into_iter().zip(seqnums_chunks) {
            futures.push(indexer_multi_wait_for_object_chunk(
                client,
                id_chunk.into(),
                seqnum_chunk.into(),
            ));
        }
        futures::future::join_all(futures).await;
    })
    .await
    .expect("Timeout waiting for indexer to catchup to given object's sequence number");
    waits_count
}

/// Wait for the indexer to catch up to the given object sequence number
pub async fn indexer_multi_wait_for_object_chunk(
    client: &HttpClient,
    object_ids: Vec<ObjectID>,
    sequence_numbers: Vec<SequenceNumber>,
) -> i32 {
    let mut waits_count = 0;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let resp = client.multi_get_objects(object_ids.clone(), None).await;
            let Ok(obj_res) = resp else {
                println!("Err: {:#?}", resp);
                continue;
            };

            if obj_res.len() != object_ids.len() {
                println!("Wrong len: {:#?} vs {:#?}", obj_res.len(), object_ids.len());
                continue;
            }

            if obj_res
                .iter()
                .zip(object_ids.clone())
                .zip(sequence_numbers.clone())
                .all(|((obj_res, obj_id), seq_num)| {
                    obj_res.data.is_some()
                        && obj_res.data.as_ref().unwrap().object_id == obj_id
                        && obj_res.data.as_ref().unwrap().version >= seq_num
                })
            {
                break;
            } else {
                println!("Not all objs ready, waiting...",);
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
            waits_count += 1;
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to given object's sequence number");
    waits_count
}

pub async fn indexer_wait_for_transaction(
    tx_res: &IotaTransactionBlockResponse,
    client: &HttpClient,
) -> i32 {
    let mut waits_count = 0;
    let now = Instant::now();
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(tx) = client
                .get_transaction_block(
                    tx_res.digest,
                    Some(IotaTransactionBlockResponseOptions::new()),
                )
                .await
            {
                println!("Got tx block elapsed: {:#?}", now.elapsed());
                if let Some(obj_changes) = &tx_res.object_changes {
                    let now = Instant::now();
                    let mut objs_to_wait_on: Vec<(ObjectID, SequenceNumber)> = Vec::new();
                    for obj in obj_changes {
                        match obj {
                            ObjectChange::Created {
                                object_id, version, ..
                            } => {
                                objs_to_wait_on.push((*object_id, *version));
                            }
                            ObjectChange::Transferred {
                                object_id, version, ..
                            } => {
                                objs_to_wait_on.push((*object_id, *version));
                            }
                            ObjectChange::Mutated {
                                object_id, version, ..
                            } => {
                                objs_to_wait_on.push((*object_id, *version));
                            }
                            ObjectChange::Published { package_id, .. } => {
                                println!("Waiting for pkg Publish");
                                indexer_wait_for_package(*package_id, client).await;
                            }
                            ObjectChange::Deleted { .. } => {
                                panic!("Not implemented");
                            }
                            ObjectChange::Wrapped { .. } => {
                                panic!("Not implemented");
                            }
                        }
                    }

                    println!("Waiting for {} objs", objs_to_wait_on.len());
                    indexer_multi_wait_for_object(
                        client,
                        objs_to_wait_on.iter().map(|e| e.0).collect(),
                        objs_to_wait_on.iter().map(|e| e.1).collect(),
                    )
                    .await;

                    println!("Waiting on all tx effects elapsed: {:#?}", now.elapsed());
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            waits_count += 1;
        }
    })
    .await
    .expect("Timeout waiting for indexer to catchup to given transaction");
    waits_count
}

pub async fn indexer_wait_for_package(package_id: ObjectID, indexer_client: &HttpClient) -> i32 {
    let mut waits_count = 0;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(tx) = indexer_client
                .get_normalized_move_modules_by_package(package_id)
                .await
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            waits_count += 1;
        }
    })
    .await
    .expect("Timeout waiting for indexer to index given package");
    waits_count
}

pub async fn indexer_wait_for_coins(
    indexer_client: &HttpClient,
    owner: &IotaAddress,
    coins_cnt: u32,
) -> i32 {
    let mut waits_count = 0;
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let now = chrono::Utc::now().timestamp_millis() as u64;
            if let Ok(coins) = indexer_client.get_coins(*owner, None, None, None).await {
                if coins.data.len() >= coins_cnt as usize {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
            waits_count += 1;
        }
    })
    .await
    .expect("Timeout waiting for coins");
    waits_count
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

    tokio::spawn(
        async move { Indexer::start_reader::<PgConnection>(&config, &registry, db_url).await },
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

/// Set up a test indexer fetching from a REST endpoint served by the given
/// Simulacrum.
pub async fn start_simulacrum_rest_api_with_write_indexer(
    sim: Arc<Simulacrum>,
    data_ingestion_path: PathBuf,
    server_url: Option<SocketAddr>,
    database_name: Option<&str>,
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
        Some(get_indexer_db_url(None)),
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
    database_name: Option<&str>,
) -> (
    JoinHandle<()>,
    PgIndexerStore<PgConnection>,
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
