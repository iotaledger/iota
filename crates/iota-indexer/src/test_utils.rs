// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

use std::{env, net::SocketAddr, time::Duration};

use diesel::connection::SimpleConnection;
use mysten_metrics::init_metrics;
use iota_json_rpc_types::IotaTransactionBlockResponse;
use tokio::task::JoinHandle;
use tracing::info;

use crate::{
    db::{new_pg_connection_pool_with_config, reset_database, PgConnectionPoolConfig},
    errors::IndexerError,
    handlers::objects_snapshot_processor::SnapshotLagConfig,
    indexer::Indexer,
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
    // Reduce the connection pool size to 10 for testing
    // to prevent maxing out
    info!("Setting DB_POOL_SIZE to 10");
    std::env::set_var("DB_POOL_SIZE", "10");

    let db_url = db_url.unwrap_or_else(|| {
        let pg_host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".into());
        let pg_port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "32770".into());
        let pw = env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "postgrespw".into());
        format!("postgres://postgres:{pw}@{pg_host}:{pg_port}")
    });

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

    // Set connection timeout for tests to 1 second
    let mut pool_config = PgConnectionPoolConfig::default();
    pool_config.set_connection_timeout(Duration::from_secs(1));

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

    let mut parsed_url = config.get_db_url().unwrap();

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
    let store = PgIndexerStore::new(blocking_pool.clone(), indexer_metrics.clone());

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
                reset_database(&mut blocking_pool.get().unwrap(), true).unwrap();
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
