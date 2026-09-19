// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Real-node coverage of the RPC index store: epoch-boundary history
//! buckets and index pruning, which unit tests cannot exercise through a
//! node's actual lifecycle. Restarting the fullnode under the simulator is
//! not covered: a stopped fullnode's database locks are not released (the
//! restarted instance fails on the RocksDB `LOCK` files), which needs a
//! harness fix first; reopen semantics are pinned by the `rpc_indexes`
//! unit tests instead.

use std::time::Duration;

use iota_core::authority::authority_store_pruner::MIN_EPOCHS_TO_RETAIN_FOR_INDEXES;
use iota_json_rpc_types::TransactionFilter;
use iota_macros::sim_test;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{Address, TransactionDigest};
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

/// With `num_epochs_to_retain_for_indexes` configured, the pruner drops
/// expired epochs' history on a running node while recent history and the
/// live-state tables keep serving.
#[sim_test]
async fn index_pruning_drops_expired_epochs_on_a_live_node() {
    let cluster = TestClusterBuilder::new()
        .with_fullnode_num_epochs_to_retain_for_indexes(Some(MIN_EPOCHS_TO_RETAIN_FOR_INDEXES))
        .build()
        .await;

    let (sender, old_digest) = transfer_coin(&cluster.wallet).await;

    // One epoch past the retention, so epoch 0 falls out of it.
    for _ in 0..=MIN_EPOCHS_TO_RETAIN_FOR_INDEXES {
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
