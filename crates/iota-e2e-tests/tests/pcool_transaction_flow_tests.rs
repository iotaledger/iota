// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end characterization tests for the P-COOL (certificate-less /
//! post-consensus owned-object locking) transaction flow.
//!
//! They cover the two subsystems that cooperate after consensus, end-to-end and
//! through the production RPC path:
//!
//! 1. **P-COOL post-consensus conflict resolution** — owned-object transactions
//!    flow through consensus and the "first writer in consensus order wins"
//!    rule is applied after ordering (`post_consensus_validation.rs`).
//! 2. **The execution scheduler** — survivors are enqueued and driven to
//!    execution / finality.
//!
//! Assertions are deliberately **black-box** (finality and execution status
//! only), so the same scenario is valid against either scheduler
//! implementation. Each scenario therefore runs twice: once against the legacy
//! `TransactionManager` and once against the new `ExecutionScheduler`, with
//! `assert_scheduler` confirming which one is actually in effect on the
//! fullnode.
//!
//! ## Why `#[cfg(msim)]`
//!
//! P-COOL is enabled with a protocol-config override, which is
//! **thread-local**. The validator/fullnode nodes only observe it when they run
//! on the test's OS thread — true under the `msim` simulator (it schedules
//! every node on the calling thread), but not under plain multi-threaded tokio,
//! where `iota-swarm` spawns each node on its own thread and the flag would
//! silently be `false`. So they are gated to the simulator, and
//! `assert_pcool_active` additionally fails loudly if the flag is not in effect
//! on the fullnode.
//!
//! Run with: `cargo simtest -p iota-e2e-tests test_pcool`.
//!
//! Under `enable_pcool_flow`, `ValidatorService::handle_transaction` is
//! disabled, so transactions must be submitted via the fullnode RPC path
//! (`TransactionDriver`), which is what
//! `WalletContext::execute_transaction_may_fail` uses.

#![cfg(msim)]

use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::base_types::dbg_addr;
use test_cluster::{TestCluster, TestClusterBuilder};

/// Applies a thread-local protocol-config override that enables the P-COOL
/// flow. The returned guard is RAII and must be held for the whole test.
fn enable_pcool_for_testing() -> iota_protocol_config::OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    })
}

/// Builds a P-COOL test cluster pinned to the requested scheduler. Both env
/// vars are set explicitly (read by `ExecutionSchedulerWrapper::new` on every
/// node), so the choice holds regardless of `DEFAULT_USE_EXECUTION_SCHEDULER`;
/// `assert_scheduler` double-checks it on the fullnode.
///
/// Returns the protocol-config override guard, which must be held for the test.
async fn build_pcool_cluster(
    use_execution_scheduler: bool,
) -> (TestCluster, iota_protocol_config::OverrideGuard) {
    // Read at runtime by every node under the simulator (single process), so this
    // selects the scheduler cluster-wide. nextest/simtest isolate each test in its
    // own process, so it does not leak. Must be set before the nodes are built.
    // SAFETY (edition 2021): plain env mutation, no other threads race it here.
    if use_execution_scheduler {
        std::env::set_var("ENABLE_EXECUTION_SCHEDULER", "1");
        std::env::remove_var("ENABLE_TRANSACTION_MANAGER");
    } else {
        std::env::set_var("ENABLE_TRANSACTION_MANAGER", "1");
        std::env::remove_var("ENABLE_EXECUTION_SCHEDULER");
    }
    let guard = enable_pcool_for_testing();
    let test_cluster = TestClusterBuilder::new()
        // Long epoch so reconfiguration never interferes mid-test.
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    (test_cluster, guard)
}

/// Fails loudly if P-COOL is not actually in effect on the fullnode, so the
/// tests can never pass for the wrong reason (the override silently not
/// reaching the nodes — see the module docs).
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
         post-consensus path; if false, the protocol-config override did not reach the nodes"
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
    let (test_cluster, _guard) = build_pcool_cluster(false).await;
    assert_scheduler(&test_cluster, false);
    run_owned_object_transfer_reaches_finality(&test_cluster).await;
}

#[sim_test]
async fn test_pcool_owned_object_transfer_reaches_finality_execution_scheduler() {
    let (test_cluster, _guard) = build_pcool_cluster(true).await;
    assert_scheduler(&test_cluster, true);
    run_owned_object_transfer_reaches_finality(&test_cluster).await;
}

/// I1: A single owned-object transaction submitted under P-COOL travels through
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
    let (test_cluster, _guard) = build_pcool_cluster(false).await;
    assert_scheduler(&test_cluster, false);
    run_double_spend_resolves_to_single_winner(&test_cluster).await;
}

#[sim_test]
async fn test_pcool_double_spend_resolves_to_single_winner_execution_scheduler() {
    let (test_cluster, _guard) = build_pcool_cluster(true).await;
    assert_scheduler(&test_cluster, true);
    run_double_spend_resolves_to_single_winner(&test_cluster).await;
}

/// I2: Two transactions that spend the SAME owned coin (a double-spend /
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
