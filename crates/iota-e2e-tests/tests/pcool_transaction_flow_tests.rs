// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end characterization tests for the P-COOL (certificate-less /
//! post-consensus owned-object locking) transaction flow.
//!
//! They cover the two subsystems that cooperate after consensus, end-to-end and
//! through the production RPC path:
//!
//! 1. **P-COOL post-consensus conflict resolution** — owned-object transactions flow through
//!    consensus and the "first writer in consensus order wins" rule is applied after ordering
//!    (`post_consensus_validation.rs`).
//! 2. **The execution scheduler** — survivors are enqueued and driven to execution / finality.
//!
//! Assertions are deliberately **black-box** (finality and execution status
//! only), so the same scenario is valid against either scheduler
//! implementation. Each scenario therefore runs twice: once against the legacy
//! `TransactionManager` and once against the new `ExecutionScheduler`, with
//! `assert_scheduler` confirming which one is actually in effect on the
//! fullnode.
//!
//! The tests run at the latest protocol version, where the P-COOL flow is on.
//!
//! Run with: `cargo simtest -p iota-e2e-tests test_pcool`.
//!
//! Under `enable_pcool_flow`, `ValidatorService::handle_transaction` is
//! disabled, so transactions must be submitted via the fullnode RPC path
//! (`TransactionDriver`), which is what
//! `WalletContext::execute_transaction_may_fail` uses.

use std::str::FromStr;

use iota_macros::sim_test;
use iota_sdk_types::{ObjectId, StructTag, TypeTag};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{base_types::dbg_addr, coin::CoinMetadata, transaction::CallArg};
use test_cluster::{TestCluster, TestClusterBuilder};

/// Builds a P-COOL test cluster. When `use_execution_scheduler` is set, every
/// node selects the new `ExecutionScheduler` (via the
/// `ENABLE_EXECUTION_SCHEDULER` env var read in
/// `ExecutionSchedulerWrapper::new`); otherwise the legacy `TransactionManager`
/// is used.
async fn build_pcool_cluster(use_execution_scheduler: bool) -> TestCluster {
    // Both env vars are set explicitly so the choice is pinned regardless of
    // DEFAULT_USE_EXECUTION_SCHEDULER (`ENABLE_TRANSACTION_MANAGER` is the
    // opt-out and takes precedence). Otherwise the TransactionManager variant
    // would silently follow the default and select the wrong scheduler if the
    // default is ever flipped to ExecutionScheduler. Read at runtime by every
    // node in this one process, so this selects the scheduler
    // cluster-wide; nextest/simtest isolate each test in its own process, so it
    // does not leak. Must be set before the nodes are constructed.
    // SAFETY (edition 2021): plain env mutation, no other threads race it here.
    if use_execution_scheduler {
        std::env::set_var("ENABLE_EXECUTION_SCHEDULER", "1");
        std::env::remove_var("ENABLE_TRANSACTION_MANAGER");
    } else {
        std::env::set_var("ENABLE_TRANSACTION_MANAGER", "1");
        std::env::remove_var("ENABLE_EXECUTION_SCHEDULER");
    }
    TestClusterBuilder::new()
        // Long epoch so reconfiguration never interferes mid-test.
        .with_epoch_duration_ms(600_000)
        .build()
        .await
}

/// Fails loudly if P-COOL is not actually in effect on the fullnode, so the
/// tests can never pass for the wrong reason.
fn assert_pcool_active(test_cluster: &TestCluster) {
    let active = test_cluster.fullnode_handle.iota_node.with(|node| {
        node.state()
            .epoch_store_for_testing()
            .protocol_config()
            .enable_pcool_flow()
    });
    assert!(
        active,
        "enable_pcool_flow must be active on the fullnode for this test to exercise the \
         post-consensus path"
    );
}

/// Confirms the fullnode is running the expected scheduler, so a parameterized
/// run can never silently fall back to the other implementation.
fn assert_scheduler(test_cluster: &TestCluster, expect_execution_scheduler: bool) {
    let uses_execution_scheduler = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().uses_execution_scheduler());
    assert_eq!(
        uses_execution_scheduler, expect_execution_scheduler,
        "fullnode scheduler mismatch: expected execution_scheduler={expect_execution_scheduler}, \
         got {uses_execution_scheduler}"
    );
}

#[sim_test]
async fn test_pcool_owned_object_transfer_reaches_finality_transaction_manager() {
    let test_cluster = build_pcool_cluster(false).await;
    assert_scheduler(&test_cluster, false);
    run_owned_object_transfer_reaches_finality(&test_cluster).await;
}

#[sim_test]
async fn test_pcool_owned_object_transfer_reaches_finality_execution_scheduler() {
    let test_cluster = build_pcool_cluster(true).await;
    assert_scheduler(&test_cluster, true);
    run_owned_object_transfer_reaches_finality(&test_cluster).await;
}

/// A single owned-object transaction submitted under P-COOL travels through
/// consensus, survives post-consensus validation (no conflict), is enqueued,
/// and finalizes successfully. This is the happy-path proof that the
/// post-consensus -> scheduler -> execution seam works end-to-end.
async fn run_owned_object_transfer_reaches_finality(test_cluster: &TestCluster) {
    assert_pcool_active(test_cluster);

    let sender = test_cluster.get_address_0();
    let recipient = dbg_addr(2);

    let txn = test_cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .transfer_iota(Some(1_000_000), recipient)
        .build();

    // Submit via RPC -> orchestrator -> TransactionDriver (the P-COOL path) and
    // wait for local execution on the fullnode. Use the non-asserting variant so
    // the assertions below are meaningful rather than duplicated by the harness.
    let response = test_cluster
        .wallet
        .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&txn))
        .await;

    assert!(
        response
            .as_ref()
            .is_ok_and(|r| r.status_ok().unwrap_or(false)),
        "owned-object transfer must finalize successfully under P-COOL: {response:?}"
    );
}

#[sim_test]
async fn test_pcool_double_spend_resolves_to_single_winner_transaction_manager() {
    let test_cluster = build_pcool_cluster(false).await;
    assert_scheduler(&test_cluster, false);
    run_double_spend_resolves_to_single_winner(&test_cluster).await;
}

#[sim_test]
async fn test_pcool_double_spend_resolves_to_single_winner_execution_scheduler() {
    let test_cluster = build_pcool_cluster(true).await;
    assert_scheduler(&test_cluster, true);
    run_double_spend_resolves_to_single_winner(&test_cluster).await;
}

/// Two transactions that spend the SAME owned coin (a double-spend /
/// equivocation) are submitted concurrently. Under P-COOL the conflict is
/// resolved deterministically by consensus order: exactly one finalizes and the
/// other is dropped post-consensus. This is the headline P-COOL guarantee, and
/// it exercises conflict resolution + scheduler execution together.
async fn run_double_spend_resolves_to_single_winner(test_cluster: &TestCluster) {
    assert_pcool_active(test_cluster);

    let sender = test_cluster.get_address_0();
    let recipient1 = dbg_addr(2);
    let recipient2 = dbg_addr(3);

    // Both transactions use the SAME gas coin as their (owned) input, so they
    // conflict on that object. Distinct recipients give them distinct digests.
    let gas = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(sender)
        .await
        .unwrap()
        .unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;

    let tx1 = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(Some(1), recipient1)
            .build(),
    );
    let tx2 = test_cluster.wallet.sign_transaction(
        &TestTransactionBuilder::new(sender, gas, rgp)
            .transfer_iota(Some(1), recipient2)
            .build(),
    );

    // Submit both concurrently and let P-COOL resolve the conflict. Bound the
    // wait so a regression where the dropped loser neither finalizes nor errors
    // fails clearly instead of hanging to the harness slow-timeout.
    let (r1, r2) = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        tokio::join!(
            test_cluster.wallet.execute_transaction_may_fail(tx1),
            test_cluster.wallet.execute_transaction_may_fail(tx2),
        )
    })
    .await
    .expect("double-spend submissions did not resolve within 60s");

    // The loser is dropped post-consensus, so its submission errors (or, at
    // worst, comes back with a non-success status); the winner finalizes
    // successfully. Treat "finalized successfully" as Ok AND status Success.
    let ok1 = r1.as_ref().is_ok_and(|r| r.status_ok().unwrap_or(false));
    let ok2 = r2.as_ref().is_ok_and(|r| r.status_ok().unwrap_or(false));

    assert!(
        ok1 ^ ok2,
        "exactly one of two conflicting owned-object transactions must finalize \
         under P-COOL (ok1={ok1}, ok2={ok2}); r1={r1:?}, r2={r2:?}"
    );
}

#[sim_test]
async fn test_pcool_immutable_object_read_twice_transaction_manager() {
    let test_cluster = build_pcool_cluster(false).await;
    assert_scheduler(&test_cluster, false);
    run_immutable_object_read_twice(&test_cluster).await;
}

#[sim_test]
async fn test_pcool_immutable_object_read_twice_execution_scheduler() {
    let test_cluster = build_pcool_cluster(true).await;
    assert_scheduler(&test_cluster, true);
    run_immutable_object_read_twice(&test_cluster).await;
}

/// Any number of transactions may read an immutable object within one epoch.
/// Reads the genesis `CoinMetadata` twice in a row. Immutable inputs are not
/// locked: they never get a new version, so a lock taken by the first read
/// would never be released.
async fn run_immutable_object_read_twice(test_cluster: &TestCluster) {
    assert_pcool_active(test_cluster);

    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;

    // Resolved from the genesis objects rather than through `coin_read_api`:
    // the RPC's package-object lookup sits behind a process-global cache
    // (keyed only by package id and struct tag), which under MSIM_TEST_NUM > 1
    // serves the object id from a previous iteration's genesis.
    let metadata_id = test_cluster
        .get_genesis()
        .objects()
        .iter()
        .find(|object| {
            object.data.as_opt_struct().is_some_and(|move_struct| {
                CoinMetadata::is_coin_metadata_with_coin_type(move_struct.struct_tag())
                    .is_some_and(StructTag::is_gas)
            })
        })
        .expect("the IOTA coin metadata object exists at genesis")
        .id();
    let metadata_ref = test_cluster.get_latest_object_ref(&metadata_id).await;

    // Sequential, not concurrent: the point is that the first read leaves no
    // lock behind for the second to trip over.
    for attempt in 1..=2 {
        let gas = test_cluster
            .wallet
            .get_one_gas_object_owned_by_address(sender)
            .await
            .unwrap()
            .unwrap();
        let txn = TestTransactionBuilder::new(sender, gas, rgp)
            .move_call(
                ObjectId::FRAMEWORK,
                "coin",
                "get_decimals",
                vec![CallArg::ImmutableOrOwned(metadata_ref)],
            )
            .with_type_args(vec![TypeTag::from_str("0x2::iota::IOTA").unwrap()])
            .build();

        let response = test_cluster
            .wallet
            .execute_transaction_may_fail(test_cluster.wallet.sign_transaction(&txn))
            .await;

        assert!(
            response
                .as_ref()
                .is_ok_and(|r| r.status_ok().unwrap_or(false)),
            "read {attempt} of an immutable object must finalize under P-COOL: {response:?}"
        );
    }
}
