// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Real-node coverage of the RPC index store: epoch-boundary history
//! buckets and index pruning, which unit tests cannot exercise through a
//! node's actual lifecycle. Restarting the fullnode under the simulator is
//! not covered: a stopped fullnode's database locks are not released (the
//! restarted instance fails on the RocksDB `LOCK` files), which needs a
//! harness fix first; reopen semantics are pinned by the `rpc_indexes`
//! unit tests instead.

use std::{num::NonZeroUsize, time::Duration};

use iota_json_rpc_api::{CoinReadApiClient, IndexerApiClient};
use iota_json_rpc_types::{EventFilter, IotaTransactionBlockResponseQuery, TransactionFilter};
use iota_macros::sim_test;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, TransactionDigest};
use iota_swarm::memory::Swarm;
use iota_test_transaction_builder::TestTransactionBuilder;
use test_cluster::{TestCluster, TestClusterBuilder};

/// Transfers an object between the wallet's first two accounts and returns
/// the sender and the transaction digest.
async fn transfer_coin(context: &WalletContext) -> (Address, TransactionDigest) {
    let gas_price = context.get_reference_gas_price().await.unwrap();
    let accounts_and_objs = context.get_all_accounts_and_gas_objects().await.unwrap();
    let sender = accounts_and_objs[0].0;
    let receiver = accounts_and_objs[1].0;
    let gas_object = accounts_and_objs[0].1[0];
    let object_to_send = accounts_and_objs[0].1[1];
    let txn = context.sign_transaction(
        &TestTransactionBuilder::new(sender, gas_object, gas_price)
            .transfer(object_to_send, receiver)
            .build(),
    );
    let resp = context.execute_transaction_must_succeed(txn).await;
    (sender, resp.digest)
}

/// The sender's transactions of both epochs, through the fullnode's own
/// index, forward and reverse.
async fn transactions_from(
    cluster: &TestCluster,
    sender: Address,
    reverse: bool,
) -> Vec<TransactionDigest> {
    cluster
        .fullnode_handle
        .iota_node
        .state()
        .get_transactions_for_tests(
            Some(TransactionFilter::FromAddress(sender)),
            None,
            None,
            reverse,
        )
        .await
        .unwrap()
}

/// Queries chain across the epoch boundary in both directions.
#[sim_test]
async fn indexes_chain_across_epoch_buckets_on_a_live_node() {
    let cluster = TestClusterBuilder::new().build().await;

    let (sender, digest_epoch_0) = transfer_coin(&cluster.wallet).await;
    cluster.force_new_epoch().await;
    let (_, digest_epoch_1) = transfer_coin(&cluster.wallet).await;

    let forward = transactions_from(&cluster, sender, false).await;
    assert_eq!(forward, vec![digest_epoch_0, digest_epoch_1]);
    let reverse = transactions_from(&cluster, sender, true).await;
    assert_eq!(reverse, vec![digest_epoch_1, digest_epoch_0]);
}

/// Retention this test configures, in epochs. Small enough that the test can
/// advance past it, large enough that recent history survives.
const EPOCHS_TO_RETAIN: u64 = 2;

/// With `num_epochs_to_retain_for_indexes` configured, the pruner drops
/// expired epochs' history on a running node while recent history and the
/// live-state tables keep serving.
#[sim_test]
async fn index_pruning_drops_expired_epochs_on_a_live_node() {
    let cluster = TestClusterBuilder::new()
        .with_fullnode_num_epochs_to_retain_for_indexes(Some(EPOCHS_TO_RETAIN))
        .build()
        .await;

    let (sender, old_digest) = transfer_coin(&cluster.wallet).await;

    // One epoch past the retention, so epoch 0 falls out of it.
    for _ in 0..=EPOCHS_TO_RETAIN {
        cluster.force_new_epoch().await;
    }
    let (_, recent_digest) = transfer_coin(&cluster.wallet).await;

    let indexes = cluster
        .fullnode_handle
        .iota_node
        .state()
        .rpc_indexes_store
        .clone()
        .unwrap();

    // The pruner runs on its own schedule; wait for it to drop epoch 0.
    let mut pruned = false;
    for _ in 0..60 {
        if indexes.lookup_digest(&old_digest).unwrap().is_none() {
            pruned = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert!(pruned, "epoch 0's history must be pruned");

    assert!(
        indexes.lookup_digest(&recent_digest).unwrap().is_some(),
        "recent history must survive the pruning"
    );
    let txes = transactions_from(&cluster, sender, false).await;
    assert!(
        txes.contains(&recent_digest) && !txes.contains(&old_digest),
        "queries must serve the retained epochs only"
    );
}

/// A node serving the JSON-RPC API answers every index-backed endpoint,
/// so a client needs no capability probe before using it.
#[sim_test]
async fn jsonrpc_node_serves_every_index_backed_endpoint() {
    // Relies on the JSON-RPC API being on by default.
    let cluster = TestClusterBuilder::new().build().await;
    let address = cluster.get_address_0();
    let client = cluster.rpc_client();

    client
        .get_owned_objects(address, None, None, None)
        .await
        .expect("getOwnedObjects must be served");
    client
        .get_coins(address, None, None, None)
        .await
        .expect("getCoins must be served");
    client
        .get_balance(address, None)
        .await
        .expect("getBalance must be served");
    client
        .get_all_balances(address)
        .await
        .expect("getAllBalances must be served");
    client
        .query_transaction_blocks(
            IotaTransactionBlockResponseQuery::default(),
            None,
            Some(1),
            Some(false),
        )
        .await
        .expect("queryTransactionBlocks must be served");
    client
        .query_events(EventFilter::All(vec![]), None, Some(1), Some(false))
        .await
        .expect("queryEvents must be served");
}

/// A node with the JSON-RPC API off serves no JSON-RPC, but still answers
/// `/health` on its JSON-RPC address, which liveness probes rely on.
#[sim_test]
async fn node_without_jsonrpc_api_serves_only_health() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut swarm = Swarm::builder()
        .committee_size(NonZeroUsize::new(1).unwrap())
        .with_fullnode_count(1)
        .with_fullnode_enable_jsonrpc_api(false)
        .build();
    swarm.launch().await.unwrap();

    let fullnode = swarm.fullnodes().next().unwrap();
    let address = fullnode.config().json_rpc_address;
    let status_line = |request: String| async move {
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        // Read only up to the status line and keep the connection open: the
        // simulator drops a response the server sent just before closing.
        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        while !response.windows(2).any(|w| w == b"\r\n") {
            let n = stream.read(&mut buf).await.unwrap();
            assert_ne!(n, 0, "the connection closed before a status line");
            response.extend_from_slice(&buf[..n]);
        }
        String::from_utf8_lossy(&response)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string()
    };

    let health = status_line("GET /health HTTP/1.1\r\nHost: node\r\n\r\n".to_string()).await;
    assert!(health.contains(" 200 "), "/health must answer: {health}");

    let body = r#"{"jsonrpc":"2.0","id":1,"method":"iota_getChainIdentifier","params":[]}"#;
    let json_rpc = status_line(format!(
        "POST / HTTP/1.1\r\nHost: node\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    ))
    .await;
    assert!(
        !json_rpc.contains(" 200 "),
        "JSON-RPC must not be served: {json_rpc}"
    );
}
