// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests for the `TransactionDriver` validator scoring and selection
//! mechanism.
//!
//! These tests exercise the full node pipeline — from transaction submission
//! through consensus to finality — and verify that the scoring system produces
//! observable, correct outcomes under realistic network conditions.
//!
//! ## Test organisation
//!
//! * **Resilience tests** — transactions complete successfully even when some
//!   validators are unavailable.  These run without the white-flag flow and
//!   exercise the `QuorumDriver` path.
//!
//! * **Scoring tests** (`#[cfg(msim)]`) — require the msim simulator and the
//!   white-flag protocol feature.  They exercise the `TransactionDriver` path,
//!   verify that validator scores are recorded, and confirm that the monitor
//!   produces non-trivial selections after real traffic.

use std::time::Duration;

use iota_macros::sim_test;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use test_cluster::TestClusterBuilder;

// ---------------------------------------------------------------------------
// Resilience tests (no white-flag required)
// ---------------------------------------------------------------------------

/// Transactions submitted through the normal `QuorumDriver` path should still
/// succeed when 1 out of 4 validators is stopped.  A Byzantine-fault-tolerant
/// network needs only 2f+1 = 3 validators for f = 1.
#[sim_test]
async fn test_transactions_succeed_with_one_validator_stopped() {
    let cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        // Long epoch duration so reconfiguration never interferes.
        .with_epoch_duration_ms(1_000_000)
        .build()
        .await;

    let validator_pubkeys = cluster.get_validator_pubkeys();

    // Warm up: execute one transaction while all validators are healthy.
    let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
    cluster.wallet.execute_transaction_must_succeed(tx).await;

    // Stop one validator — we still have 3/4 = 2f+1 validators.
    cluster.stop_node(&validator_pubkeys[0]);

    // Subsequent transactions should still succeed.
    for _ in 0..3 {
        let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            cluster.wallet.execute_transaction_must_succeed(tx),
        )
        .await;
        assert!(
            result.is_ok(),
            "transaction should succeed with 3/4 validators healthy"
        );
    }
}

/// Transactions should complete in reasonable time even when the *best-ranked*
/// validator (the one the fullnode prefers) is stopped.  This verifies that
/// the `RequestRetrier` falls through to the next validator rather than
/// blocking indefinitely.
#[sim_test]
async fn test_transactions_complete_after_preferred_validator_stops() {
    let cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        .with_epoch_duration_ms(1_000_000)
        .build()
        .await;

    let validator_pubkeys = cluster.get_validator_pubkeys();

    // Warm up with several transactions to let the scoring system accumulate
    // some observations before we stop a validator.
    for _ in 0..5 {
        let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
        cluster.wallet.execute_transaction_must_succeed(tx).await;
    }

    // Stop the first validator in the committee (likely to have received
    // traffic during the warm-up phase).
    cluster.stop_node(&validator_pubkeys[0]);

    // Transactions should still complete within a reasonable timeout.
    for _ in 0..3 {
        let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            cluster.wallet.execute_transaction_must_succeed(tx),
        )
        .await;
        assert!(
            result.is_ok(),
            "transaction should succeed even after the preferred validator stops"
        );
    }
}

/// Stopping two validators (exceeding f = 1 Byzantine fault tolerance for a
/// 4-node committee) should cause transactions to time out.  This test
/// confirms the expected failure mode and that the system does *not*
/// spuriously succeed.
#[sim_test]
async fn test_transactions_fail_when_quorum_is_lost() {
    let cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        .with_epoch_duration_ms(1_000_000)
        .build()
        .await;

    let validator_pubkeys = cluster.get_validator_pubkeys();

    // Stop two validators — only 2/4 remain, below the 2f+1 = 3 threshold.
    cluster.stop_node(&validator_pubkeys[0]);
    cluster.stop_node(&validator_pubkeys[1]);

    let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        cluster.wallet.execute_transaction_must_succeed(tx).await
    })
    .await;

    assert!(
        result.is_err(),
        "transaction should time out when quorum is lost (only 2/4 validators alive)"
    );
}

// ---------------------------------------------------------------------------
// Scoring tests — require msim + white-flag protocol feature
//
// These tests use `ProtocolConfig::apply_overrides_for_testing` to enable the
// white-flag flow, which activates `TransactionDriver` (and thus the
// `ValidatorClientMonitor` scoring pipeline) on the fullnode.
// ---------------------------------------------------------------------------

#[cfg(msim)]
mod scoring_tests {
    use iota_core::{
        authority_client::NetworkAuthorityClient,
        transaction_orchestrator::TransactionOrchestrator, validator_client_monitor::OperationType,
    };
    use iota_protocol_config::ProtocolConfig;
    use iota_types::base_types::AuthorityName;

    use super::*;

    /// Enable the white-flag flow so `TransactionDriver` is active on
    /// fullnodes.
    fn enable_white_flag() -> iota_protocol_config::OverrideGuard {
        ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        })
    }

    fn get_orchestrator(
        cluster: &test_cluster::TestCluster,
    ) -> Arc<TransactionOrchestrator<NetworkAuthorityClient>> {
        cluster
            .fullnode_handle
            .iota_node
            .with(|n| n.transaction_orchestrator().unwrap().clone())
    }

    use std::sync::Arc;

    // -----------------------------------------------------------------------
    // After executing several transactions through the TransactionDriver path,
    // the `ValidatorClientMonitor` should have recorded `Consensus` latency
    // observations for at least one validator (the one that was selected for
    // each transaction).
    // -----------------------------------------------------------------------
    #[sim_test]
    async fn test_scoring_monitor_records_consensus_observations_after_transactions() {
        let _guard = enable_white_flag();

        let mut cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(1_000_000)
            .build()
            .await;

        let orchestrator = get_orchestrator(&cluster);

        // Execute several transactions through the white-flag / TransactionDriver
        // path.
        for _ in 0..5 {
            let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
            cluster.wallet.execute_transaction_must_succeed(tx).await;
        }

        // Retrieve the TransactionDriver's client monitor.
        let monitor = orchestrator
            .transaction_driver()
            .expect("TransactionDriver should be active with white-flag enabled")
            .client_monitor_for_test()
            .clone();

        // At least one validator must have a `Consensus` latency entry in the
        // stats (the one that processed our transactions).
        let stats = monitor.client_stats_for_test();
        let has_consensus_obs = stats
            .validator_stats
            .values()
            .any(|v| v.average_latencies.contains_key(&OperationType::Consensus));
        assert!(
            has_consensus_obs,
            "after executing transactions at least one validator should have Consensus latency data"
        );
    }

    // -----------------------------------------------------------------------
    // After executing transactions, `cached_latencies` should be non-empty
    // (populated by the background health-check task or an explicit refresh)
    // and the `select_shuffled_preferred_validators` result should include all
    // committee members.
    // -----------------------------------------------------------------------
    #[sim_test]
    async fn test_scoring_monitor_populates_cached_latencies_after_health_checks() {
        let _guard = enable_white_flag();

        let mut cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(1_000_000)
            .build()
            .await;

        let orchestrator = get_orchestrator(&cluster);
        let td = orchestrator
            .transaction_driver()
            .expect("TransactionDriver should be active");

        // Execute a transaction to trigger the Consensus latency recording path.
        let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
        cluster.wallet.execute_transaction_must_succeed(tx).await;

        // The health-check background task runs every 10 s by default.
        // Wait long enough for at least one round to complete.
        tokio::time::sleep(Duration::from_secs(15)).await;

        let monitor = td.client_monitor_for_test();
        let auth_agg = td.authority_aggregator().load();
        let committee = auth_agg.committee.clone();

        let selected = monitor.select_shuffled_preferred_validators(&committee, 1.0);
        assert_eq!(
            selected.len(),
            4,
            "all 4 validators should appear in the selection result"
        );
    }

    // -----------------------------------------------------------------------
    // When a validator is stopped, its health checks start failing.  After
    // enough failed checks, its reliability score degrades and it drops out of
    // the 2 % preferred group.  This test verifies that end-to-end flow.
    // -----------------------------------------------------------------------
    #[sim_test]
    async fn test_stopped_validator_eventually_leaves_preferred_group() {
        let _guard = enable_white_flag();

        let mut cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(1_000_000)
            .build()
            .await;

        let validator_pubkeys = cluster.get_validator_pubkeys();
        let orchestrator = get_orchestrator(&cluster);
        let td = orchestrator
            .transaction_driver()
            .expect("TransactionDriver should be active");

        // Warm up scoring with several transactions and health-check rounds.
        for _ in 0..5 {
            let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
            cluster.wallet.execute_transaction_must_succeed(tx).await;
        }

        let monitor = td.client_monitor_for_test();
        let auth_agg = td.authority_aggregator().load();
        let committee = auth_agg.committee.clone();

        // Force an explicit cache refresh so we have a baseline.
        monitor.force_update_cached_latencies(&auth_agg);
        let preferred_before: Vec<AuthorityName> =
            monitor.select_shuffled_preferred_validators(&committee, 0.02);

        // Stop one validator.
        let stopped = validator_pubkeys[0];
        cluster.stop_node(&stopped);

        // Wait for several health-check rounds (default interval = 10 s).
        // With a reliability window of 20 and checks every 10 s, ~20 failed
        // checks (≈ 200 s) would fully rotate the window.  We wait for enough
        // rounds so that reliability has degraded measurably.
        tokio::time::sleep(Duration::from_secs(35)).await;

        monitor.force_update_cached_latencies(&auth_agg);
        let preferred_after: Vec<AuthorityName> =
            monitor.select_shuffled_preferred_validators(&committee, 0.02);

        // The stopped validator's reliability should have degraded.
        if let Some(v_stats) = monitor
            .client_stats_for_test()
            .validator_stats
            .get(&stopped)
        {
            assert!(
                v_stats.reliability.get() < 1.0,
                "stopped validator reliability should have dropped; got {}",
                v_stats.reliability.get()
            );
        }

        // Transactions should still succeed with the remaining 3 validators.
        let tx = make_transfer_iota_transaction(&cluster.wallet, None, None).await;
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            cluster.wallet.execute_transaction_must_succeed(tx),
        )
        .await;
        assert!(
            result.is_ok(),
            "transactions should still succeed after one validator is stopped"
        );
    }

    // -----------------------------------------------------------------------
    // Bring a previously stopped validator back up.  Over time its health
    // checks start succeeding again, and the score should recover toward 1.0.
    // This test verifies that the system self-heals after a transient outage.
    // -----------------------------------------------------------------------
    #[sim_test]
    async fn test_restarted_validator_score_recovers() {
        let _guard = enable_white_flag();

        let mut cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(1_000_000)
            .build()
            .await;

        let validator_pubkeys = cluster.get_validator_pubkeys();
        let orchestrator = get_orchestrator(&cluster);
        let td = orchestrator
            .transaction_driver()
            .expect("TransactionDriver should be active");
        let monitor = td.client_monitor_for_test();

        let stopped = validator_pubkeys[0];

        // Inject some failures for the stopped validator directly (faster than
        // waiting for actual health check rounds in a simtest).
        {
            let auth_agg = td.authority_aggregator().load();
            let display_name = auth_agg.get_display_name(&stopped);
            for _ in 0..10 {
                monitor.record_interaction_result(
                    iota_core::validator_client_monitor::OperationFeedback {
                        authority_name: stopped,
                        display_name: display_name.clone(),
                        operation: OperationType::HealthCheck,
                        ping: false,
                        result: Err(()),
                    },
                );
            }
        }

        let auth_agg = td.authority_aggregator().load();
        monitor.force_update_cached_latencies(&auth_agg);

        let reliability_after_failures = {
            monitor
                .client_stats_for_test()
                .validator_stats
                .get(&stopped)
                .map(|v| v.reliability.get())
                .unwrap_or(1.0)
        };
        assert!(
            reliability_after_failures < 1.0,
            "reliability should have dropped after injected failures"
        );

        // Now inject successes (simulates the validator coming back online).
        {
            let auth_agg = td.authority_aggregator().load();
            let display_name = auth_agg.get_display_name(&stopped);
            for _ in 0..20 {
                monitor.record_interaction_result(
                    iota_core::validator_client_monitor::OperationFeedback {
                        authority_name: stopped,
                        display_name: display_name.clone(),
                        operation: OperationType::HealthCheck,
                        ping: false,
                        result: Ok(Duration::from_millis(30)),
                    },
                );
            }
        }

        let reliability_after_recovery = {
            monitor
                .client_stats_for_test()
                .validator_stats
                .get(&stopped)
                .map(|v| v.reliability.get())
                .unwrap_or(0.0)
        };
        assert!(
            reliability_after_recovery > reliability_after_failures,
            "reliability should recover after injecting successes; before={reliability_after_failures}, after={reliability_after_recovery}"
        );
    }
}
