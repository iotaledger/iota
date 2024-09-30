// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(dead_code)]
#[cfg(feature = "pg_integration")]
pub mod pg_integration {
    use std::{
        net::SocketAddr,
        sync::{Arc, OnceLock},
        time::Duration,
    };

    use iota_config::node::RunWithRange;
    use iota_indexer::{
        errors::IndexerError,
        indexer::Indexer,
        store::{indexer_store::IndexerStore, PgIndexerStore},
        test_utils::{start_test_indexer, ReaderWriterConfig},
        IndexerConfig,
    };
    use iota_metrics::init_metrics;
    use iota_types::storage::ReadStore;
    use jsonrpsee::{
        http_client::{HttpClient, HttpClientBuilder},
        types::ErrorObject,
    };
    use simulacrum::Simulacrum;
    use test_cluster::{TestCluster, TestClusterBuilder};
    use tokio::{runtime::Runtime, task::JoinHandle};

    const DEFAULT_DB_URL: &str = "postgres://postgres:postgrespw@localhost:5432/iota_indexer";
    const DEFAULT_INDEXER_IP: &str = "127.0.0.1";
    const DEFAULT_INDEXER_PORT: u16 = 9005;
    const DEFAULT_SERVER_PORT: u16 = 3000;

    static GLOBAL_INDEXER_RPC_CLIENT: OnceLock<HttpClient> = OnceLock::new();
    static GLOBAL_NODE_RPC_CLIENT: OnceLock<HttpClient> = OnceLock::new();
    static GLOBAL_TEST_CLUSTER_WITH_INDEXER: OnceLock<ApiTestSetup> = OnceLock::new();

    pub struct ApiTestSetup {
        pub runtime: Runtime,
        pub cluster: TestCluster,
        pub store: PgIndexerStore,
        /// Indexer RPC Client
        pub client: HttpClient,
    }

    pub fn get_global_indexer_rpc_client() -> &'static HttpClient {
        GLOBAL_INDEXER_RPC_CLIENT.get_or_init(|| {
            // create an RPC client by using the indexer url
            HttpClientBuilder::default()
                .build("http://0.0.0.0:9124")
                .unwrap()
        })
    }
    pub fn get_global_node_rpc_client() -> &'static HttpClient {
        GLOBAL_NODE_RPC_CLIENT.get_or_init(|| {
            // create an RPC client by using the node url
            HttpClientBuilder::default()
                .build("http://0.0.0.0:9000")
                .unwrap()
        })
    }

    pub fn setup_api_tests() -> &'static ApiTestSetup {
        GLOBAL_TEST_CLUSTER_WITH_INDEXER.get_or_init(|| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            let (cluster, store, client) =
                runtime.block_on(start_test_cluster_with_read_write_indexer(None));

            ApiTestSetup {
                runtime,
                cluster,
                store,
                client,
            }
        })
    }

    /// Start a [`TestCluster`][`test_cluster::TestCluster`] with a `Read` &
    /// `Write` indexer
    pub async fn start_test_cluster_with_read_write_indexer(
        stop_cluster_after_checkpoint_seq: Option<u64>,
    ) -> (TestCluster, PgIndexerStore, HttpClient) {
        let mut builder = TestClusterBuilder::new();

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
        )
        .await;

        // start indexer in read mode
        start_indexer_reader(cluster.rpc_url().to_owned());

        // create an RPC client by using the indexer url
        let rpc_client = HttpClientBuilder::default()
            .build(format!(
                "http://{DEFAULT_INDEXER_IP}:{DEFAULT_INDEXER_PORT}"
            ))
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
                    .get_latest_tx_checkpoint_sequence_number()
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

    /// Start an Indexer instance in `Read` mode
    fn start_indexer_reader(fullnode_rpc_url: impl Into<String>) {
        let config = IndexerConfig {
            db_url: Some(DEFAULT_DB_URL.to_owned()),
            rpc_client_url: fullnode_rpc_url.into(),
            reset_db: true,
            rpc_server_worker: true,
            rpc_server_url: DEFAULT_INDEXER_IP.to_owned(),
            rpc_server_port: DEFAULT_INDEXER_PORT,
            ..Default::default()
        };

        let registry = prometheus::Registry::default();
        init_metrics(&registry);

        tokio::spawn(async move {
            Indexer::start_reader(&config, &registry, DEFAULT_DB_URL.to_owned()).await
        });
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
    ) -> (
        JoinHandle<()>,
        PgIndexerStore,
        JoinHandle<Result<(), IndexerError>>,
    ) {
        let server_url: SocketAddr = format!("127.0.0.1:{}", DEFAULT_SERVER_PORT)
            .parse()
            .unwrap();

        let server_handle = tokio::spawn(async move {
            let chain_id = (*sim
                .get_checkpoint_by_sequence_number(0)
                .unwrap()
                .unwrap()
                .digest())
            .into();

            iota_rest_api::RestService::new_without_version(sim, chain_id)
                .start_service(server_url, Some("/rest".to_owned()))
                .await;
        });
        // Starts indexer
        let (pg_store, pg_handle) = start_test_indexer(
            Some(DEFAULT_DB_URL.to_owned()),
            format!("http://{}", server_url),
            ReaderWriterConfig::writer_mode(None),
        )
        .await;
        (server_handle, pg_store, pg_handle)
    }
}
