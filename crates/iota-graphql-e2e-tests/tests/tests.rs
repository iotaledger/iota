// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(unused_imports)]
#![allow(unused_variables)]

use std::{
    hash::{Hash, Hasher},
    path::Path,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use iota_graphql_rpc::{
    config::ConnectionConfig,
    test_infra::cluster::{DEFAULT_INTERNAL_DATA_SOURCE_PORT, ExecutorCluster, serve_executor},
};
use iota_transactional_test_runner::{
    args::IotaInitArgs,
    create_adapter,
    offchain_state::{OffchainStateReader, TestResponse},
    run_tasks_with_adapter,
    test_adapter::{IotaTestAdapter, PRE_COMPILED},
};
pub const TEST_DIR: &str = "tests";

pub struct OffchainReaderForAdapter {
    cluster: Arc<ExecutorCluster>,
}

#[async_trait]
impl OffchainStateReader for OffchainReaderForAdapter {
    async fn wait_for_objects_snapshot_catchup(&self, base_timeout: Duration) {
        self.cluster
            .wait_for_objects_snapshot_catchup(base_timeout)
            .await
    }

    async fn wait_for_checkpoint_catchup(&self, checkpoint: u64, base_timeout: Duration) {
        self.cluster
            .wait_for_checkpoint_catchup(checkpoint, base_timeout)
            .await
    }

    async fn wait_for_pruned_checkpoint(&self, checkpoint: u64, base_timeout: Duration) {
        self.cluster
            .wait_for_checkpoint_pruned(checkpoint, base_timeout)
            .await
    }

    async fn execute_graphql(
        &self,
        query: String,
        show_usage: bool,
    ) -> Result<TestResponse, anyhow::Error> {
        let result = self
            .cluster
            .graphql_client
            .execute_to_graphql(query, show_usage, vec![], vec![])
            .await?;

        Ok(TestResponse {
            http_headers: Some(result.http_headers_without_date()),
            response_body: result.response_body_json_pretty(),
            service_version: result.graphql_version().ok(),
        })
    }
}

datatest_stable::harness!(run_test, TEST_DIR, r".*\.(mvir|move)$");

#[cfg_attr(not(msim), tokio::main)]
#[cfg_attr(msim, msim::main)]
async fn run_test(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(feature = "pg_integration") {
        // start the adapter first to start the executor (simulacrum)
        let (output, mut adapter) =
            create_adapter::<IotaTestAdapter>(path, Some(Arc::new(PRE_COMPILED.clone()))).await?;

        // In another crate like `iota-mvr-graphql-e2e-tests`, this would be the place
        // to translate from `offchain_config` to something compatible with the
        // indexer and graphql flavor of choice.
        let offchain_config = adapter.offchain_config.as_ref().unwrap();

        // Hash the file path to create custom unique DB name
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.to_path_buf().hash(&mut hasher);
        let hash = hasher.finish();
        let db_name = format!("iota_graphql_test_{}", hash);

        // Use the hash as a seed to generate a random port number
        let base_port = hash as u16 % 8192;

        let graphql_port = 20000 + base_port;
        let graphql_prom_port = graphql_port + 1;
        let internal_data_port = graphql_prom_port + 1;
        let cluster = serve_executor(
            ConnectionConfig::ci_integration_test_cfg_with_db_name(
                db_name,
                graphql_port,
                graphql_prom_port,
            ),
            internal_data_port,
            adapter.read_replica.as_ref().unwrap().clone(),
            Some(offchain_config.snapshot_config.clone()),
            offchain_config.epochs_to_keep,
            offchain_config.data_ingestion_path.clone(),
        )
        .await;

        let cluster_arc = Arc::new(cluster);

        adapter.with_offchain_reader(Box::new(OffchainReaderForAdapter {
            cluster: cluster_arc.clone(),
        }));

        run_tasks_with_adapter(path, adapter, output).await?;

        match Arc::try_unwrap(cluster_arc) {
            Ok(cluster) => cluster.cleanup_resources().await,
            Err(_) => panic!("Still other Arc references!"),
        }
    }
    Ok(())
}
