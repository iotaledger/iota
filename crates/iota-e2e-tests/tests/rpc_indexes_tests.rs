// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Real-node coverage of the RPC index store: epoch-boundary history
//! buckets, index pruning, and the promise that a transaction reported as
//! final, or delivered to a subscriber, is indexed. Unit tests cannot
//! exercise these through the real lifecycle of a node. Restarting the fullnode
//! under the simulator is not covered: a stopped fullnode's database locks are
//! not released (the restarted instance fails on the RocksDB `LOCK` files),
//! which needs a harness fix first; reopen semantics are pinned by the
//! `rpc_indexes` unit tests instead.

use std::{collections::BTreeMap, num::NonZeroUsize, time::Duration};

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::ledger_service::{
        GetTransactionsRequest, TransactionRequest, TransactionRequests, transaction_result,
    },
};
use iota_json_rpc_api::{CoinReadApiClient, IndexerApiClient};
use iota_json_rpc_types::{
    EventFilter, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseQuery,
    TransactionFilter,
};
use iota_macros::sim_test;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, TransactionDigest};
use iota_swarm::memory::Swarm;
use iota_test_transaction_builder::{
    TestTransactionBuilder, create_nft, make_transfer_iota_transaction, publish_nfts_package,
};
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

/// History buckets are created per epoch by the live indexing path; queries
/// must chain across the epoch boundary in both directions on a real node.
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

/// With `num_epochs_to_retain_for_indexes` configured, the epoch boundary
/// drops expired epochs' history on a running node while recent history and
/// the live-state tables keep serving.
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

    // Expiry runs at the epoch boundary; wait for it to drop epoch 0.
    let mut pruned = false;
    for _ in 0..60 {
        if indexes.lookup_digest(&old_digest).unwrap().is_none() {
            pruned = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert!(pruned, "epoch 0's history must be pruned");

    // Recent history and the queries stay up.
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

/// A transaction that is reported under `WaitForLocalExecution` is visible
/// to the index-backed reads. A client can query its outputs immediately. It
/// does not need to poll.
#[sim_test]
async fn index_backed_reads_see_a_transaction_reported_as_executed_locally() {
    let cluster = TestClusterBuilder::new().build().await;
    let client = cluster.rpc_client();

    // Repeated, so that the check covers more than one checkpoint.
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

/// The same promise holds on the certificate-based flow, which mainnet and
/// testnet use. There, `WaitForLocalExecution` is served by the quorum driver
/// and waits for the checkpoint separately.
#[sim_test]
async fn index_backed_reads_see_a_transaction_reported_by_the_quorum_driver() {
    let _pcool_guard = override_pcool_flow(false);
    let cluster = TestClusterBuilder::new().build().await;
    let client = cluster.rpc_client();

    // Repeated, so that the check covers more than one checkpoint.
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

/// A transaction subscription delivers a transaction only after its
/// checkpoint is indexed. The subscriber can then query the index-backed
/// reads immediately. Transactions arrive in checkpoint order.
#[sim_test]
async fn subscribers_are_notified_after_indexing_in_checkpoint_order() {
    let cluster = TestClusterBuilder::new().build().await;
    let state = cluster.fullnode_handle.iota_node.state();
    let client = cluster.rpc_client();
    let sender = cluster.get_address_0();
    let mut notifications = Box::pin(
        state
            .subscription_handler
            .subscribe_transactions(TransactionFilter::FromAddress(sender)),
    );

    let mut recipients = BTreeMap::new();
    for _ in 0..3 {
        let recipient = Address::random();
        let gas_price = cluster.wallet.get_reference_gas_price().await.unwrap();
        let gas_object = cluster
            .wallet
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap();
        let txn = cluster.wallet.sign_transaction(
            &TestTransactionBuilder::new(sender, gas_object, gas_price)
                .transfer_iota(Some(9), recipient)
                .build(),
        );
        let resp = cluster.wallet.execute_transaction_must_succeed(txn).await;
        recipients.insert(resp.digest, recipient);
    }

    let mut last_checkpoint = 0;
    let epoch_store = state.epoch_store_for_testing();
    for _ in 0..recipients.len() {
        let effects = tokio::time::timeout(Duration::from_secs(60), notifications.next())
            .await
            .expect("a notification must arrive")
            .expect("the stream must stay open");
        let digest = *effects.transaction_digest();
        let recipient = recipients
            .remove(&digest)
            .unwrap_or_else(|| panic!("unexpected notification for {digest}"));

        let checkpoint = state
            .get_transaction_checkpoint_for_tests(&digest, &epoch_store)
            .unwrap()
            .expect("a notified transaction must be checkpointed")
            .sequence_number;
        assert!(
            checkpoint >= last_checkpoint,
            "notifications must arrive in checkpoint order"
        );
        last_checkpoint = checkpoint;

        let owned = client
            .get_owned_objects(recipient, None, None, None)
            .await
            .expect("getOwnedObjects must be served");
        assert_eq!(
            owned.data.len(),
            1,
            "the transferred coin must be indexed by the time the subscriber is notified"
        );
    }
}

/// An event subscription delivers an event only after its checkpoint is
/// indexed, with the Move contents parsed through the committed package and
/// with the checkpoint timestamp, the same as `getEvents` reports.
#[sim_test]
async fn event_subscribers_receive_parsed_events_after_indexing() {
    let cluster = TestClusterBuilder::new().build().await;
    let state = cluster.fullnode_handle.iota_node.state();
    let client = cluster.rpc_client();
    let (package_id, _, _) = publish_nfts_package(&cluster.wallet).await;

    let mut notifications = Box::pin(
        state
            .subscription_handler
            .subscribe_events(EventFilter::Package(package_id)),
    );
    let (sender, nft_id, digest) = create_nft(&cluster.wallet, package_id).await;

    let event = tokio::time::timeout(Duration::from_secs(60), notifications.next())
        .await
        .expect("a notification must arrive")
        .expect("the stream must stay open");
    assert_eq!(event.id.tx_digest, digest);
    assert_eq!(event.sender, sender);
    assert!(
        event.parsed_json.is_object(),
        "the event contents must be parsed through the package layout"
    );

    let epoch_store = state.epoch_store_for_testing();
    let checkpoint = state
        .get_transaction_checkpoint_for_tests(&digest, &epoch_store)
        .unwrap()
        .expect("a notified transaction must be checkpointed");
    assert_eq!(event.timestamp_ms, Some(checkpoint.timestamp_ms));

    let owned = client
        .get_owned_objects(sender, None, None, None)
        .await
        .expect("getOwnedObjects must be served");
    assert!(
        owned.data.iter().any(|o| o.object_id().unwrap() == nft_id),
        "the minted NFT must be indexed by the time the subscriber is notified"
    );
}

/// A node serving the JSON-RPC API answers every index-backed endpoint,
/// so a client needs no capability probe before using it.
#[sim_test]
async fn jsonrpc_node_serves_every_index_backed_endpoint() {
    // The JSON-RPC API is on by default; `TestClusterBuilder` has no knob to
    // turn it off, so this exercises that default rather than setting it.
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

/// A node with the JSON-RPC API off mounts no HTTP server on its JSON-RPC
/// address, the way a node with the gRPC API off serves no gRPC.
#[sim_test]
async fn node_without_jsonrpc_api_mounts_no_http_server() {
    let mut swarm = Swarm::builder()
        .committee_size(NonZeroUsize::new(1).unwrap())
        .with_fullnode_count(1)
        .with_fullnode_enable_jsonrpc_api(false)
        .build();
    swarm.launch().await.unwrap();

    let fullnode = swarm.fullnodes().next().unwrap();
    let address = fullnode.config().json_rpc_address;
    assert!(
        tokio::net::TcpStream::connect(address).await.is_err(),
        "nothing must listen on the JSON-RPC address when the API is off"
    );
}

/// A transaction still held by the ledger reports its checkpoint over gRPC
/// even when the RPC index retains fewer epochs than the ledger does — the
/// finality answer lives with the transaction, not with the query indexes.
#[sim_test]
async fn transaction_checkpoint_survives_a_shorter_index_window() {
    let cluster = TestClusterBuilder::new()
        .with_fullnode_num_epochs_to_retain_for_indexes(Some(1))
        // The ledger must outlive the index window for the test to say
        // anything, so keep every transaction rather than leaving that to
        // which epochs the boundary happens to have expired.
        .disable_fullnode_pruning()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let (_sender, digest) = transfer_coin(&cluster.wallet).await;
    let indexes = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().rpc_indexes_store.clone().unwrap());

    // Advance past the index retention so the transaction's epoch bucket is
    // dropped, while the ledger still holds the transaction itself.
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

    // The query index is what the shorter window costs: the transaction can
    // no longer be found by query, while remaining fetchable by digest.
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
