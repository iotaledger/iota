// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, net::SocketAddr, time::Duration};

use diesel::connection::SimpleConnection;
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_metrics::init_metrics;
use tokio::task::JoinHandle;
use tracing::info;

use crate::{
    db::{new_pg_connection_pool_with_config, reset_database, PgConnectionPoolConfig},
    errors::IndexerError,
    indexer::Indexer,
    processors::objects_snapshot_processor::SnapshotLagConfig,
    store::PgIndexerStore,
    IndexerConfig, IndexerMetrics,
};

pub enum ReaderWriterConfig {
    Reader { reader_mode_rpc_url: String },
    Writer { snapshot_config: SnapshotLagConfig },
}

impl ReaderWriterConfig {
    pub fn reader_mode(reader_mode_rpc_url: String) -> Self {
        Self::Reader {
            reader_mode_rpc_url,
        }
    }

    pub fn writer_mode(snapshot_config: Option<SnapshotLagConfig>) -> Self {
        Self::Writer {
            snapshot_config: snapshot_config.unwrap_or_default(),
        }
    }
}

pub async fn start_test_indexer(
    db_url: Option<String>,
    rpc_url: String,
    reader_writer_config: ReaderWriterConfig,
) -> (PgIndexerStore, JoinHandle<Result<(), IndexerError>>) {
    start_test_indexer_impl(db_url, rpc_url, reader_writer_config, None).await
}

pub async fn start_test_indexer_impl(
    db_url: Option<String>,
    rpc_url: String,
    reader_writer_config: ReaderWriterConfig,
    new_database: Option<String>,
) -> (PgIndexerStore, JoinHandle<Result<(), IndexerError>>) {
    let db_url = db_url.unwrap_or_else(|| {
        let pg_host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
        let pg_port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "32770".into());
        let pw = env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "postgrespw".into());
        format!("postgres://postgres:{pw}@{pg_host}:{pg_port}")
    });

    let store = create_pg_store(db_url.clone(), new_database);
    // dynamically set ports instead of all to 9000
    let base_port = rpc_url
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse::<u16>()
        .unwrap();

    // Default writer mode
    let mut config = IndexerConfig {
        db_url: Some(db_url.clone()),
        rpc_client_url: rpc_url,
        reset_db: true,
        fullnode_sync_worker: true,
        rpc_server_worker: false,
        rpc_server_port: base_port + 1,
        ..Default::default()
    };

    let registry = prometheus::Registry::default();

    init_metrics(&registry);

    let indexer_metrics = IndexerMetrics::new(&registry);

    let handle = match reader_writer_config {
        ReaderWriterConfig::Reader {
            reader_mode_rpc_url,
        } => {
            let reader_mode_rpc_url = reader_mode_rpc_url
                .parse::<SocketAddr>()
                .expect("Unable to parse fullnode address");
            config.fullnode_sync_worker = false;
            config.rpc_server_worker = true;
            config.rpc_server_url = reader_mode_rpc_url.ip().to_string();
            config.rpc_server_port = reader_mode_rpc_url.port();
            tokio::spawn(async move { Indexer::start_reader(&config, &registry, db_url).await })
        }
        ReaderWriterConfig::Writer { snapshot_config } => {
            if config.reset_db {
                reset_database(&mut store.blocking_cp().get().unwrap(), true).unwrap();
            }
            let store_clone = store.clone();

            tokio::spawn(async move {
                Indexer::start_writer_with_config(
                    &config,
                    store_clone,
                    indexer_metrics,
                    snapshot_config,
                )
                .await
            })
        }
    };

    (store, handle)
}

pub fn create_pg_store(db_url: String, new_database: Option<String>) -> PgIndexerStore {
    // Reduce the connection pool size to 10 for testing
    // to prevent maxing out
    info!("Setting DB_POOL_SIZE to 10");
    std::env::set_var("DB_POOL_SIZE", "10");

    // Set connection timeout for tests to 1 second
    let mut pool_config = PgConnectionPoolConfig::default();
    pool_config.set_connection_timeout(Duration::from_secs(1));

    let registry = prometheus::Registry::default();

    init_metrics(&registry);

    let indexer_metrics = IndexerMetrics::new(&registry);

    let mut parsed_url = db_url.clone();
    if let Some(new_database) = new_database {
        // Switch to default to create a new database
        let (default_db_url, _) = replace_db_name(&parsed_url, "postgres");

        // Open in default mode
        let blocking_pool =
            new_pg_connection_pool_with_config(&default_db_url, Some(5), pool_config).unwrap();
        let mut default_conn = blocking_pool.get().unwrap();

        // Delete the old db if it exists
        default_conn
            .batch_execute(&format!("DROP DATABASE IF EXISTS {}", new_database))
            .unwrap();

        // Create the new db
        default_conn
            .batch_execute(&format!("CREATE DATABASE {}", new_database))
            .unwrap();
        parsed_url = replace_db_name(&parsed_url, &new_database).0;
    }

    let blocking_pool =
        new_pg_connection_pool_with_config(&parsed_url, Some(5), pool_config).unwrap();
    PgIndexerStore::new(blocking_pool, indexer_metrics.clone())
}

fn replace_db_name(db_url: &str, new_db_name: &str) -> (String, String) {
    let pos = db_url.rfind('/').expect("Unable to find / in db_url");
    let old_db_name = &db_url[pos + 1..];

    (
        format!("{}/{}", &db_url[..pos], new_db_name),
        old_db_name.to_string(),
    )
}

pub async fn force_delete_database(db_url: String) {
    // Replace the database name with the default `postgres`, which should be the
    // last string after `/` This is necessary because you can't drop a database
    // while being connected to it. Hence switch to the default `postgres`
    // database to drop the active database.
    let (default_db_url, db_name) = replace_db_name(&db_url, "postgres");
    // Set connection timeout for tests to 1 second
    let mut pool_config = PgConnectionPoolConfig::default();
    pool_config.set_connection_timeout(Duration::from_secs(1));

    let blocking_pool =
        new_pg_connection_pool_with_config(&default_db_url, Some(5), pool_config).unwrap();
    blocking_pool
        .get()
        .unwrap()
        .batch_execute(&format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", db_name))
        .unwrap();
}

#[derive(Clone)]
pub struct IotaTransactionBlockResponseBuilder<'a> {
    response: IotaTransactionBlockResponse,
    full_response: &'a IotaTransactionBlockResponse,
}

impl<'a> IotaTransactionBlockResponseBuilder<'a> {
    pub fn new(full_response: &'a IotaTransactionBlockResponse) -> Self {
        Self {
            response: IotaTransactionBlockResponse::default(),
            full_response,
        }
    }

    pub fn with_input(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            transaction: self.full_response.transaction.clone(),
            ..self.response
        };
        self
    }

    pub fn with_raw_input(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            raw_transaction: self.full_response.raw_transaction.clone(),
            ..self.response
        };
        self
    }

    pub fn with_effects(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            effects: self.full_response.effects.clone(),
            ..self.response
        };
        self
    }

    pub fn with_events(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            events: self.full_response.events.clone(),
            ..self.response
        };
        self
    }

    pub fn with_balance_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            balance_changes: self.full_response.balance_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn with_object_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            object_changes: self.full_response.object_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn with_input_and_changes(mut self) -> Self {
        self.response = IotaTransactionBlockResponse {
            transaction: self.full_response.transaction.clone(),
            balance_changes: self.full_response.balance_changes.clone(),
            object_changes: self.full_response.object_changes.clone(),
            ..self.response
        };
        self
    }

    pub fn build(self) -> IotaTransactionBlockResponse {
        IotaTransactionBlockResponse {
            transaction: self.response.transaction,
            raw_transaction: self.response.raw_transaction,
            effects: self.response.effects,
            events: self.response.events,
            balance_changes: self.response.balance_changes,
            object_changes: self.response.object_changes,
            // Use full response for any fields that aren't showable
            ..self.full_response.clone()
        }
    }
}

#[cfg(feature = "pg_integration")]
pub mod pg_integration {
    use std::{net::SocketAddr, sync::Arc, time::Duration};

    use iota_config::node::RunWithRange;
    use iota_metrics::init_metrics;
    use iota_types::storage::ReadStore;
    use jsonrpsee::{
        http_client::{HttpClient, HttpClientBuilder},
        types::ErrorObject,
    };
    use simulacrum::Simulacrum;
    use test_cluster::{TestCluster, TestClusterBuilder};
    use tokio::task::JoinHandle;

    use crate::{
        errors::IndexerError,
        indexer::Indexer,
        store::{indexer_store::IndexerStore, PgIndexerStore},
        test_utils::{start_test_indexer, ReaderWriterConfig},
        IndexerConfig,
    };

    const DEFAULT_DB_URL: &str = "postgres://postgres:postgrespw@localhost:5432/iota_indexer";
    const DEFAULT_INDEXER_IP: &str = "127.0.0.1";
    const DEFAULT_INDEXER_PORT: u16 = 9005;
    const DEFAULT_SERVER_PORT: u16 = 3000;

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
    pub async fn indexer_wait_for_checkpoint(
        pg_store: &PgIndexerStore,
        checkpoint_sequence_number: u64,
    ) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while {
                let cp_opt = pg_store
                    .get_latest_tx_checkpoint_sequence_number()
                    .await
                    .unwrap();
                cp_opt.is_none() || (cp_opt.unwrap() < checkpoint_sequence_number)
            } {
                tokio::time::sleep(Duration::from_secs(1)).await;
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
