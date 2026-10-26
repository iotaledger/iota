// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests of the RPC index store on a running node: history buckets across
//! epoch boundaries, index pruning, indexing of transactions reported as
//! final, and indexing again at start the checkpoints whose index commits
//! were lost.

use std::{num::NonZeroUsize, time::Duration};

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::ledger_service::{
        GetTransactionsRequest, TransactionRequest, TransactionRequests, transaction_result,
    },
};
use iota_json_rpc_api::{CoinReadApiClient, IndexerApiClient};
use iota_json_rpc_types::{EventFilter, IotaTransactionBlockResponseQuery, TransactionFilter};
use iota_macros::sim_test;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, TransactionDigest};
use iota_swarm::memory::Swarm;
use iota_test_transaction_builder::{TestTransactionBuilder, make_transfer_iota_transaction};
use prost_types::FieldMask;
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};

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
    check_indexes_chain_across_epoch_buckets(true).await;
}

/// [`indexes_chain_across_epoch_buckets_on_a_live_node`] with the certificate
/// flow that mainnet and testnet use.
#[sim_test]
async fn indexes_chain_across_epoch_buckets_without_the_pcool_flow() {
    let _pcool_guard = override_pcool_flow(false);
    check_indexes_chain_across_epoch_buckets(false).await;
}

async fn check_indexes_chain_across_epoch_buckets(pcool_flow: bool) {
    let cluster = TestClusterBuilder::new().build().await;
    assert_pcool_flow(&cluster, pcool_flow);

    let (sender, digest_epoch_0) = transfer_coin(&cluster.wallet).await;
    cluster.force_new_epoch().await;
    let (_, digest_epoch_1) = transfer_coin(&cluster.wallet).await;

    let forward = transactions_from(&cluster, sender, false).await;
    assert_eq!(forward, vec![digest_epoch_0, digest_epoch_1]);
    let reverse = transactions_from(&cluster, sender, true).await;
    assert_eq!(reverse, vec![digest_epoch_1, digest_epoch_0]);
}

/// Checks that every validator of `cluster` runs the P-COOL flow exactly when
/// `enabled`.
fn assert_pcool_flow(cluster: &TestCluster, enabled: bool) {
    for handle in cluster.all_validator_handles() {
        handle.with(|node| {
            assert_eq!(
                node.state()
                    .epoch_store_for_testing()
                    .protocol_config()
                    .enable_pcool_flow(),
                enabled
            );
        });
    }
}

const EPOCHS_TO_RETAIN: u64 = 2;

/// The epoch boundary drops index history outside
/// `num_epochs_to_retain_for_indexes` while recent history keeps serving.
#[sim_test]
async fn index_pruning_drops_expired_epochs_on_a_live_node() {
    check_index_pruning_drops_expired_epochs(true).await;
}

/// [`index_pruning_drops_expired_epochs_on_a_live_node`] with the certificate
/// flow that mainnet and testnet use.
#[sim_test]
async fn index_pruning_drops_expired_epochs_without_the_pcool_flow() {
    let _pcool_guard = override_pcool_flow(false);
    check_index_pruning_drops_expired_epochs(false).await;
}

async fn check_index_pruning_drops_expired_epochs(pcool_flow: bool) {
    let cluster = TestClusterBuilder::new()
        .with_fullnode_num_epochs_to_retain_for_indexes(Some(EPOCHS_TO_RETAIN))
        .build()
        .await;
    assert_pcool_flow(&cluster, pcool_flow);

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

    // Expiry runs asynchronously after the epoch boundary.
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

/// A transaction reported under `WaitForLocalExecution` is already visible to
/// the index-backed reads.
#[sim_test]
async fn index_backed_reads_see_a_transaction_reported_as_executed_locally() {
    let cluster = TestClusterBuilder::new().build().await;
    let client = cluster.rpc_client();

    // Repeated to cover more than one checkpoint.
    for _ in 0..5 {
        let recipient = Address::random();
        let txn = make_transfer_iota_transaction(&cluster.wallet, Some(recipient), Some(9)).await;
        cluster.wallet.execute_transaction_must_succeed(txn).await;

        let owned = client
            .get_owned_objects(recipient, None, None, None)
            .await
            .expect("getOwnedObjects must be served");
        assert_eq!(
            owned.data.len(),
            1,
            "the transferred coin must be indexed by the time the execution is reported"
        );
    }
}

/// The same holds on the certificate-based flow, where the quorum driver
/// serves `WaitForLocalExecution`.
#[sim_test]
async fn index_backed_reads_see_a_transaction_reported_by_the_quorum_driver() {
    let _pcool_guard = override_pcool_flow(false);
    let cluster = TestClusterBuilder::new().build().await;
    let client = cluster.rpc_client();

    // Repeated to cover more than one checkpoint.
    for _ in 0..5 {
        let recipient = Address::random();
        let txn = make_transfer_iota_transaction(&cluster.wallet, Some(recipient), Some(9)).await;
        let response = cluster.wallet.execute_transaction_must_succeed(txn).await;
        assert_eq!(
            response.confirmed_local_execution,
            Some(true),
            "the quorum driver path must confirm local execution"
        );

        let owned = client
            .get_owned_objects(recipient, None, None, None)
            .await
            .expect("getOwnedObjects must be served");
        assert_eq!(
            owned.data.len(),
            1,
            "the transferred coin must be indexed by the time the execution is reported"
        );
    }
}

/// A node serving the JSON-RPC API answers every index-backed endpoint.
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

/// A transaction still held by the ledger reports its checkpoint over gRPC
/// after the RPC index has pruned its epoch.
#[sim_test]
async fn transaction_checkpoint_survives_a_shorter_index_window() {
    let cluster = TestClusterBuilder::new()
        .with_fullnode_num_epochs_to_retain_for_indexes(Some(1))
        // The ledger must outlive the index window.
        .disable_fullnode_pruning()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let (_sender, digest) = transfer_coin(&cluster.wallet).await;
    let indexes = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().rpc_indexes_store.clone().unwrap());

    // Advance past the index retention.
    for _ in 0..=1 {
        cluster.force_new_epoch().await;
    }
    let current_epoch = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch());
    tokio::task::spawn_blocking({
        let indexes = indexes.clone();
        move || indexes.prune(current_epoch)
    })
    .await
    .unwrap()
    .unwrap();

    let checkpoint = cluster
        .fullnode_handle
        .iota_node
        .with(|node| {
            node.state()
                .get_checkpoint_cache()
                .try_get_transaction_perpetual_checkpoint(&digest)
        })
        .unwrap();
    let (_epoch, checkpoint_seq) = checkpoint.unwrap_or_else(|| {
        panic!("the ledger must still answer which checkpoint confirmed {digest}")
    });

    assert_eq!(
        indexes.lookup_digest(&digest).unwrap(),
        None,
        "the pruned index must no longer place {digest} in the query order"
    );

    let mut ledger_client = cluster.grpc_client().ledger_service_client();
    let request = GetTransactionsRequest::default()
        .with_requests(TransactionRequests::default().with_requests(vec![
            TransactionRequest::default().with_digest(
                iota_grpc_types::v1::types::Digest::default().with_digest(digest.bytes().to_vec()),
            ),
        ]))
        .with_read_mask(FieldMask::from_paths(["checkpoint"]));
    let mut responses = ledger_client
        .get_transactions(request)
        .await
        .expect("gRPC must serve the transaction")
        .into_inner();
    let response = responses
        .next()
        .await
        .expect("gRPC must return a response")
        .expect("gRPC must return a response");
    let Some(transaction_result::Result::ExecutedTransaction(transaction)) = response
        .transaction_results
        .first()
        .and_then(|result| result.result.as_ref())
    else {
        panic!("gRPC must return {digest} as an executed transaction");
    };
    assert_eq!(
        transaction.checkpoint,
        Some(checkpoint_seq),
        "gRPC must report the checkpoint of {digest} once its index bucket is gone"
    );
}

/// The fullnode's highest executed checkpoint.
#[cfg(msim)]
fn highest_executed_checkpoint(fullnode: &test_cluster::FullNodeHandle) -> u64 {
    fullnode.iota_node.with(|node| {
        node.state()
            .get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()
            .unwrap()
            .unwrap_or_default()
    })
}

/// Waits until `fullnode` has executed `checkpoint`.
#[cfg(msim)]
async fn wait_for_executed_checkpoint(fullnode: &test_cluster::FullNodeHandle, checkpoint: u64) {
    tokio::time::timeout(Duration::from_secs(120), async {
        while highest_executed_checkpoint(fullnode) < checkpoint {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the fullnode must execute checkpoint {checkpoint}"));
}

/// The digests of the transactions matching `filter`, oldest first.
#[cfg(msim)]
async fn indexed_transactions(
    fullnode: &test_cluster::FullNodeHandle,
    filter: TransactionFilter,
) -> Vec<TransactionDigest> {
    let mut digests = Vec::new();
    let mut cursor = None;
    loop {
        let page = fullnode
            .rpc_client
            .query_transaction_blocks(
                IotaTransactionBlockResponseQuery::new_with_filter(filter.clone()),
                cursor,
                None,
                Some(false),
            )
            .await
            .unwrap();
        digests.extend(page.data.iter().map(|response| response.digest));
        if !page.has_next_page {
            return digests;
        }
        cursor = page.next_cursor;
    }
}

/// Every index-backed answer the fullnode gives about `addresses`, as JSON:
/// balances, owned objects, coins, and the transactions sent and received.
#[cfg(msim)]
async fn index_backed_answers(
    fullnode: &test_cluster::FullNodeHandle,
    addresses: &[Address],
) -> Vec<serde_json::Value> {
    let client = &fullnode.rpc_client;
    let mut answers = Vec::new();
    for &address in addresses {
        let balances = client.get_all_balances(address).await.unwrap();
        answers.push(serde_json::to_value(balances).unwrap());
        let mut cursor = None;
        loop {
            let page = client
                .get_owned_objects(address, None, cursor, None)
                .await
                .unwrap();
            answers.push(serde_json::to_value(&page.data).unwrap());
            if !page.has_next_page {
                break;
            }
            cursor = page.next_cursor;
        }
        let mut cursor = None;
        loop {
            let page = client.get_coins(address, None, cursor, None).await.unwrap();
            answers.push(serde_json::to_value(&page.data).unwrap());
            if !page.has_next_page {
                break;
            }
            cursor = page.next_cursor;
        }
        for filter in [
            TransactionFilter::FromAddress(address),
            TransactionFilter::ToAddress(address),
        ] {
            let digests = indexed_transactions(fullnode, filter).await;
            answers.push(serde_json::to_value(digests).unwrap());
        }
    }
    answers
}

/// A fullnode that lost the index commits of its last checkpoints, as a
/// power loss can lose them, indexes those checkpoints again when it starts,
/// an epoch change among them, and then answers every index-backed read like
/// the fullnode that lost nothing.
///
/// Simulator only: the fail point that loses the commits is compiled there.
#[cfg(msim)]
#[sim_test]
async fn lost_index_commits_are_indexed_again_at_start() {
    use test_cluster::FullNodeHandle;

    const LOSE_COMMIT: &str = "rpc-index-lose-commit";

    let mut cluster = TestClusterBuilder::new().build().await;
    // The wallet reads through the cluster's fullnode, which loses nothing.
    let lossy = cluster.spawn_new_fullnode().await;
    let lossy_name = lossy.iota_node.with(|node| node.state().name);
    let lossy_sim_id = lossy
        .iota_node
        .with(|_| iota_simulator::current_simnode_id());
    let addresses = cluster.wallet.get_addresses();

    transfer_coin(&cluster.wallet).await;
    iota_macros::register_fail_point_if(LOSE_COMMIT, move || {
        iota_simulator::current_simnode_id() == lossy_sim_id
    });
    let (sender, lost_in_first_epoch) = transfer_coin(&cluster.wallet).await;
    cluster.force_new_epoch().await;
    let (_, lost_in_second_epoch) = transfer_coin(&cluster.wallet).await;
    let payment = make_transfer_iota_transaction(&cluster.wallet, None, Some(1_000)).await;
    cluster
        .wallet
        .execute_transaction_must_succeed(payment)
        .await;
    wait_for_executed_checkpoint(
        &lossy,
        highest_executed_checkpoint(&cluster.fullnode_handle),
    )
    .await;
    let indexed = indexed_transactions(&lossy, TransactionFilter::FromAddress(sender)).await;
    assert!(
        !indexed.contains(&lost_in_first_epoch) && !indexed.contains(&lost_in_second_epoch),
        "the fullnode must have lost the index commits"
    );
    iota_macros::clear_fail_point(LOSE_COMMIT);

    drop(lossy);
    cluster.stop_node(&lossy_name);
    cluster.start_node(&lossy_name).await;
    let lossy = {
        let node = cluster.swarm.node(&lossy_name).unwrap();
        FullNodeHandle::new(node.get_node_handle().unwrap(), node.json_rpc_address()).await
    };

    transfer_coin(&cluster.wallet).await;
    wait_for_executed_checkpoint(
        &lossy,
        highest_executed_checkpoint(&cluster.fullnode_handle),
    )
    .await;
    assert_eq!(
        index_backed_answers(&lossy, &addresses).await,
        index_backed_answers(&cluster.fullnode_handle, &addresses).await
    );
}
