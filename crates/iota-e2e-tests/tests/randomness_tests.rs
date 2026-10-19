// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

#[cfg(msim)]
use iota_macros::register_fail_point_async;
use iota_macros::sim_test;
use iota_sdk_types::ObjectId;
use iota_test_transaction_builder::{emit_new_random_u128, publish_basics_package};
#[cfg(msim)]
use rand::distr::Distribution;
use test_cluster::{TestCluster, TestClusterBuilder};

#[sim_test]
async fn test_check_randomness_state_object_exists() {
    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(1.into())
        .with_epoch_duration_ms(10000)
        .build()
        .await;

    for h in &test_cluster.all_node_handles() {
        h.with(|node| {
            node.state()
                .get_object_cache_reader()
                .get_latest_object_ref_or_tombstone(ObjectId::RANDOMNESS_STATE)
                .expect("randomness state object should exist");
        });
    }
}

/// Builds a cluster with every node pinned to the requested scheduler. Both
/// env vars are set explicitly so the choice is pinned regardless of
/// `DEFAULT_USE_EXECUTION_SCHEDULER` (`ENABLE_TRANSACTION_MANAGER` is the
/// opt-out and takes precedence). Read at node construction under the
/// simulator (single process), so this selects the scheduler cluster-wide;
/// nextest/simtest isolate each test in its own process, so it does not leak.
async fn build_cluster_with_scheduler(use_execution_scheduler: bool) -> TestCluster {
    if use_execution_scheduler {
        std::env::set_var("ENABLE_EXECUTION_SCHEDULER", "1");
        std::env::remove_var("ENABLE_TRANSACTION_MANAGER");
    } else {
        std::env::set_var("ENABLE_TRANSACTION_MANAGER", "1");
        std::env::remove_var("ENABLE_EXECUTION_SCHEDULER");
    }
    let test_cluster = TestClusterBuilder::new()
        // Long epoch so reconfiguration never races the first randomness round.
        .with_epoch_duration_ms(600_000)
        .build()
        .await;
    for handle in test_cluster.all_node_handles() {
        handle.with(|node| {
            assert_eq!(
                node.state().uses_execution_scheduler(),
                use_execution_scheduler,
                "scheduler mismatch: expected execution_scheduler={use_execution_scheduler}"
            );
        });
    }
    test_cluster
}

/// A user transaction reading the `Random` object reaches finality: it is
/// deferred until a round opens, the round's update executes, and it then runs
/// on the bumped version. A break anywhere in that chain leaves it waiting
/// forever, so the wait is bounded.
///
/// The delay injected into local randomness generation lets checkpoint
/// execution win the race on some node, so a round's transaction may execute
/// there without that node having generated it. Whether that happened is not
/// asserted — the path is pinned by unit tests; this run has to reach finality
/// either way.
async fn run_randomness_using_transaction_reaches_finality(use_execution_scheduler: bool) {
    #[cfg(msim)]
    register_fail_point_async("randomness-delay", || async {
        let delay = {
            let dist = rand::distr::Uniform::new(10, 1000).unwrap();
            let mut rng = rand::rng();
            dist.sample(&mut rng)
        };
        tokio::time::sleep(Duration::from_millis(delay)).await;
    });

    let test_cluster = build_cluster_with_scheduler(use_execution_scheduler).await;

    let package_ref = publish_basics_package(&test_cluster.wallet).await;

    // The bound covers DKG completion plus the first randomness round with a
    // wide margin (simulated time, so a healthy run never waits it out).
    let response = tokio::time::timeout(
        Duration::from_secs(300),
        emit_new_random_u128(&test_cluster.wallet, package_ref.object_id),
    )
    .await
    .expect("randomness-using transaction did not reach finality");

    // Success is already asserted inside the helper; the emitted event is what
    // proves the transaction actually consumed a random value.
    let events = response.events.unwrap();
    assert_eq!(1, events.data.len(), "expected 1 event: {:?}", events.data);
    assert_eq!(
        "RandomU128Event",
        events.data[0].struct_tag.name().to_string().as_str()
    );
}

#[sim_test]
async fn test_randomness_using_transaction_reaches_finality_transaction_manager() {
    run_randomness_using_transaction_reaches_finality(false).await;
}

#[sim_test]
async fn test_randomness_using_transaction_reaches_finality_execution_scheduler() {
    run_randomness_using_transaction_reaches_finality(true).await;
}

#[cfg(msim)]
mod dkg_timeout {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use iota_json_rpc_types::{
        IotaExecutionStatus, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
    };
    use iota_macros::{clear_fail_point, register_fail_point_if, sim_test};
    use iota_protocol_config::ProtocolConfig;
    use iota_sdk::wallet_context::WalletContext;
    use iota_sdk_types::{ExecutionError, ObjectId};
    use iota_test_transaction_builder::{TestTransactionBuilder, randomness_state_call_arg};
    use test_cluster::TestClusterBuilder;

    async fn execute_randomness_transaction(
        context: &WalletContext,
    ) -> IotaTransactionBlockResponse {
        let (sender, gas_object) = context.get_one_gas_object().await.unwrap().unwrap();
        let gas_price = context.get_reference_gas_price().await.unwrap();
        let random_call_arg = randomness_state_call_arg(context).await;

        let transaction = context.sign_transaction(
            &TestTransactionBuilder::new(sender, gas_object, gas_price)
                .move_call(
                    ObjectId::FRAMEWORK,
                    "random",
                    "new_generator",
                    vec![random_call_arg],
                )
                .build(),
        );
        context
            .execute_transaction_may_fail(transaction)
            .await
            .unwrap()
    }

    fn is_canceled_for_randomness(status: &IotaExecutionStatus) -> bool {
        matches!(
            status,
            IotaExecutionStatus::Failure { error }
                if *error == ExecutionError::ExecutionCanceledDueToRandomnessUnavailable.to_string()
        )
    }

    #[sim_test]
    async fn test_randomness_recovers_after_dkg_timeout() {
        let suppress_dkg = Arc::new(AtomicBool::new(true));
        let suppress_dkg_for_failpoint = suppress_dkg.clone();
        register_fail_point_if("rb-dkg", move || {
            suppress_dkg_for_failpoint.load(Ordering::Relaxed)
        });
        let _protocol_config_guard =
            ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.set_random_beacon_dkg_timeout_round_for_testing(0);
                // Late completion is not enabled on every chain yet.
                config.set_allow_dkg_completion_after_timeout_for_testing(true);
                config
            });

        let test_cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(600_000)
            .build()
            .await;
        let response = execute_randomness_transaction(&test_cluster.wallet).await;
        let effects = response.effects.unwrap();
        let initial_epoch = effects.executed_epoch();
        assert!(is_canceled_for_randomness(effects.status()));

        suppress_dkg.store(false, Ordering::Relaxed);
        test_cluster.stop_all_validators().await;
        test_cluster.start_all_validators().await;

        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let response = execute_randomness_transaction(&test_cluster.wallet).await;
                let effects = response.effects.unwrap();
                assert_eq!(
                    initial_epoch,
                    effects.executed_epoch(),
                    "epoch changed during DKG recovery; recovery must complete within the \
                     initial epoch for this test to be meaningful"
                );
                match effects.status() {
                    IotaExecutionStatus::Success => break,
                    status if is_canceled_for_randomness(status) => {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    status => panic!("unexpected randomness transaction status: {status:?}"),
                }
            }
        })
        .await
        .expect("DKG should recover before the readiness deadline");

        // The epoch still advances after DKG recovers mid-epoch.
        test_cluster.force_new_epoch().await;

        clear_fail_point("rb-dkg");
    }

    /// When DKG times out and never recovers, randomness-using transactions are
    /// canceled and the epoch still changes.
    #[sim_test]
    async fn test_epoch_change_succeeds_without_dkg_recovery() {
        register_fail_point_if("rb-dkg", || true);
        let _protocol_config_guard =
            ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
                config.set_random_beacon_dkg_timeout_round_for_testing(0);
                config
            });

        let test_cluster = TestClusterBuilder::new()
            .with_num_validators(4)
            .with_epoch_duration_ms(600_000)
            .build()
            .await;

        let response = execute_randomness_transaction(&test_cluster.wallet).await;
        assert!(is_canceled_for_randomness(
            response.effects.unwrap().status()
        ));

        // DKG remains incomplete; the epoch change must not be blocked by it.
        test_cluster.force_new_epoch().await;

        clear_fail_point("rb-dkg");
    }
}
