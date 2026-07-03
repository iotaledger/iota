// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
#![cfg(not(msim))]

//! Shared harness for CLI integration tests that run against the production
//! Strategy-Y topology: the wallet reads over an indexer (JSON-RPC) and
//! executes over the node (gRPC), since the node no longer serves JSON-RPC.
//!
//! Requires a real Postgres (the indexer store), so callers run on real tokio
//! and are excluded from the deterministic simulator (`cargo simtest`).

use std::time::Duration;

use iota_config::{
    Config, IOTA_CLIENT_CONFIG, PersistedConfig, local_ip_utils::get_available_port,
};
use iota_indexer::{
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::{IndexerTypeConfig, db_url, start_test_indexer},
};
use iota_sdk::{
    iota_client_config::{IotaClientConfig, IotaEnv},
    wallet_context::WalletContext,
};
use iota_types::digests::TransactionDigest;
use test_cluster::{TestCluster, TestClusterBuilder};

/// Build the given cluster with an indexer (writer + JSON-RPC reader) and
/// re-point its wallet's active env: reads → indexer, execution → node gRPC.
///
/// `db_name` must be unique per test so concurrently-running tests don't share
/// a Postgres database.
pub async fn cluster_with_indexer_backed_wallet(
    db_name: &str,
    builder: TestClusterBuilder,
) -> (TestCluster, PgIndexerStore) {
    let mut cluster = builder.build().await;
    let node_grpc_url = cluster.fullnode_handle.grpc_url.clone();

    // Indexer writer: stream checkpoints from the fullnode gRPC into Postgres.
    let (pg_store, _writer, _writer_token) = start_test_indexer(
        db_url(db_name),
        true,
        None,
        node_grpc_url.clone(),
        IndexerTypeConfig::writer_mode(None),
        None,
    )
    .await;

    // Indexer reader: serve JSON-RPC, proxying execution to the fullnode gRPC.
    let reader_addr = format!("127.0.0.1:{}", get_available_port("127.0.0.1"));
    let (_reader, _reader_handle, _reader_token) = start_test_indexer(
        db_url(db_name),
        false,
        None,
        node_grpc_url.clone(),
        IndexerTypeConfig::reader_mode(reader_addr.clone()),
        None,
    )
    .await;
    let indexer_rpc_url = format!("http://{reader_addr}");

    let config_path = cluster.swarm.dir().join(IOTA_CLIENT_CONFIG);
    let mut config: IotaClientConfig = PersistedConfig::read(&config_path).unwrap();
    config.set_env(IotaEnv::new("localnet", indexer_rpc_url).with_grpc(node_grpc_url));
    config.set_active_env(Some("localnet".to_string()));
    config.persisted(&config_path).save().unwrap();

    cluster.wallet = WalletContext::new(&config_path)
        .unwrap()
        .with_grpc_client(cluster.fullnode_handle.grpc_client.clone());

    (cluster, pg_store)
}

/// Wait for the indexer to catch up to the node's latest executed checkpoint,
/// so CLI reads that follow an execution observe the new state.
pub async fn wait_for_indexer(pg_store: &PgIndexerStore, cluster: &TestCluster) {
    let target = cluster
        .highest_executed_checkpoint_seq_number()
        .expect("fullnode has not executed any checkpoint yet");

    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(Some(cp)) = pg_store.get_latest_checkpoint_sequence_number().await {
                if cp >= target {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for indexer to catch up");
}

/// Wait for the fullnode to execute `digest`, so a later node-gRPC read (e.g.
/// object owner) observes the transaction. Needed when the CLI executes via the
/// indexer (which returns before the fullnode syncs the checkpoint) and a
/// subsequent command reads node state over gRPC.
pub async fn wait_for_transaction_on_node(cluster: &TestCluster, digest: TransactionDigest) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let executed = cluster
                .fullnode_handle
                .iota_node
                .with(|node| node.state().is_tx_already_executed(&digest));
            if executed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("timeout waiting for fullnode to execute transaction");
}
