// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use diesel::connection::SimpleConnection;
use iota_json_rpc_types::IotaTransactionBlockResponse;
use iota_metrics::init_metrics;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    IndexerMetrics,
    config::{IngestionConfig, IotaNamesOptions, PruningOptions, SnapshotLagConfig},
    db::{ConnectionPool, ConnectionPoolConfig, PoolConnection, new_connection_pool},
    errors::IndexerError,
    indexer::Indexer,
    store::{PgIndexerAnalyticalStore, PgIndexerStore},
};

/// Type to create hooks to alter initial indexer DB state in tests.
/// Those hooks are meant to be called after DB reset (if it occurs) and before
/// indexer is started.
///
/// Example:
///
/// ```ignore
/// let emulate_insertion_order_set_earlier_by_optimistic_indexing: DBInitHook =
///     Box::new(move |pg_store: &PgIndexerStore| {
///         transactional_blocking_with_retry!(
///             &pg_store.blocking_cp(),
///             |conn| {
///                 insert_or_ignore_into!(
///                     tx_insertion_order::table,
///                     (
///                         tx_insertion_order::dsl::tx_digest.eq(digest.inner().to_vec()),
///                         tx_insertion_order::dsl::insertion_order.eq(123),
///                     ),
///                     conn
///                 );
///                 Ok::<(), IndexerError>(())
///             },
///             Duration::from_secs(60)
///         )
///             .unwrap()
///     });
///
/// let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
///     Arc::new(sim),
///     data_ingestion_path,
///     None,
///     Some("indexer_ingestion_tests_db"),
///     Some(emulate_insertion_order_set_earlier_by_optimistic_indexing),
/// )
/// .await;
/// ```
pub type DBInitHook = Box<dyn FnOnce(&PgIndexerStore) + Send>;

pub enum IndexerTypeConfig {
    Reader {
        reader_mode_rpc_url: String,
    },
    Writer {
        snapshot_config: SnapshotLagConfig,
        pruning_options: PruningOptions,
    },
    AnalyticalWorker,
}

impl IndexerTypeConfig {
    pub fn reader_mode(reader_mode_rpc_url: String) -> Self {
        Self::Reader {
            reader_mode_rpc_url,
        }
    }

    pub fn writer_mode(
        snapshot_config: Option<SnapshotLagConfig>,
        epochs_to_keep: Option<u64>,
    ) -> Self {
        Self::Writer {
            snapshot_config: snapshot_config.unwrap_or_default(),
            pruning_options: PruningOptions { epochs_to_keep },
        }
    }
}

pub async fn start_test_indexer(
    db_url: String,
    reset_db: bool,
    db_init_hook: Option<DBInitHook>,
    rpc_url: String,
    reader_writer_config: IndexerTypeConfig,
    data_ingestion_path: Option<PathBuf>,
) -> (
    PgIndexerStore,
    JoinHandle<Result<(), IndexerError>>,
    CancellationToken,
) {
    let token = CancellationToken::new();
    let (store, handle) = start_test_indexer_impl(
        db_url,
        reset_db,
        db_init_hook,
        rpc_url,
        reader_writer_config,
        data_ingestion_path,
        token.clone(),
    )
    .await;
    (store, handle, token)
}

/// Starts an indexer reader or writer for testing depending on the
/// `reader_writer_config`.
pub async fn start_test_indexer_impl(
    db_url: String,
    reset_db: bool,
    db_init_hook: Option<DBInitHook>,
    rpc_url: String,
    reader_writer_config: IndexerTypeConfig,
    data_ingestion_path: Option<PathBuf>,
    cancel: CancellationToken,
) -> (PgIndexerStore, JoinHandle<Result<(), IndexerError>>) {
    let store = create_pg_store(&db_url, reset_db);
    if reset_db {
        crate::db::reset_database(&mut store.blocking_cp().get().unwrap()).unwrap();
    }
    if let Some(db_init_hook) = db_init_hook {
        db_init_hook(&store);
    }

    let registry = prometheus::Registry::default();
    let handle = match reader_writer_config {
        IndexerTypeConfig::Reader {
            reader_mode_rpc_url,
        } => {
            let config = crate::config::JsonRpcConfig {
                iota_names_options: IotaNamesOptions::default(),
                rpc_address: reader_mode_rpc_url.parse().unwrap(),
                rpc_client_url: rpc_url,
            };
            let pool = store.blocking_cp();
            tokio::spawn(async move { Indexer::start_reader(&config, &registry, pool).await })
        }
        IndexerTypeConfig::Writer {
            snapshot_config,
            pruning_options,
        } => {
            let store_clone = store.clone();
            let mut ingestion_config = IngestionConfig::default();
            ingestion_config.sources.remote_store_url = data_ingestion_path
                .is_none()
                .then_some(format!("{rpc_url}/api/v1").parse().unwrap());
            ingestion_config.sources.data_ingestion_path = data_ingestion_path;
            ingestion_config.sources.rpc_client_url = Some(rpc_url.parse().unwrap());

            init_metrics(&registry);
            let indexer_metrics = IndexerMetrics::new(&registry);

            tokio::spawn(async move {
                Indexer::start_writer_with_config(
                    &ingestion_config,
                    store_clone,
                    indexer_metrics,
                    snapshot_config,
                    pruning_options,
                    cancel,
                )
                .await
            })
        }
        IndexerTypeConfig::AnalyticalWorker => {
            let store = PgIndexerAnalyticalStore::new(store.blocking_cp());

            init_metrics(&registry);
            let indexer_metrics = IndexerMetrics::new(&registry);

            tokio::spawn(
                async move { Indexer::start_analytical_worker(store, indexer_metrics).await },
            )
        }
    };

    (store, handle)
}

/// Manage a test database for integration tests.
pub struct TestDatabase {
    pub url: String,
    db_name: String,
    connection: PoolConnection,
    pool_config: ConnectionPoolConfig,
}

impl TestDatabase {
    pub fn new(db_url: String) -> Self {
        // Reduce the connection pool size to 5 for testing
        // to prevent maxing out
        let pool_config = ConnectionPoolConfig {
            pool_size: 5,
            ..Default::default()
        };

        let db_name = db_url.split('/').next_back().unwrap().into();
        let (default_url, _) = replace_db_name(&db_url, "postgres");
        let blocking_pool = new_connection_pool(&default_url, &pool_config).unwrap();
        let connection = blocking_pool.get().unwrap();
        Self {
            url: db_url,
            db_name,
            connection,
            pool_config,
        }
    }

    /// Drop the database in the server if it exists.
    pub fn drop_if_exists(&mut self) {
        self.connection
            .batch_execute(&format!("DROP DATABASE IF EXISTS {}", self.db_name))
            .unwrap();
    }

    /// Create the database in the server.
    pub fn create(&mut self) {
        self.connection
            .batch_execute(&format!("CREATE DATABASE {}", self.db_name))
            .unwrap();
    }

    /// Drop and recreate the database in the server.
    pub fn recreate(&mut self) {
        self.drop_if_exists();
        self.create();
    }

    /// Create a new connection pool to the database.
    pub fn to_connection_pool(&self) -> ConnectionPool {
        new_connection_pool(&self.url, &self.pool_config).unwrap()
    }

    pub fn reset_db(&mut self) {
        crate::db::reset_database(&mut self.to_connection_pool().get().unwrap()).unwrap();
    }
}

pub fn create_pg_store(db_url: &str, reset_database: bool) -> PgIndexerStore {
    let registry = prometheus::Registry::default();
    init_metrics(&registry);
    let indexer_metrics = IndexerMetrics::new(&registry);

    let mut test_db = TestDatabase::new(db_url.to_string());
    if reset_database {
        test_db.recreate();
    }

    PgIndexerStore::new(test_db.to_connection_pool(), indexer_metrics.clone())
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
    let mut pool_config = ConnectionPoolConfig::default();
    pool_config.set_pool_size(1);

    let blocking_pool = new_connection_pool(&default_db_url, &pool_config).unwrap();
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
