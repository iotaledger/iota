// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests that overload monitor only starts on validators.
#[cfg(msim)]
mod simtests {
    use std::{
        num::NonZeroUsize,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use iota_config::node::AuthorityOverloadConfig;
    use iota_core::{
        authority::AuthorityState, authority_client::NetworkAuthorityClient,
        transaction_orchestrator::TransactionOrchestrator,
    };
    use iota_macros::{register_fail_point, sim_test};
    use iota_swarm_config::network_config_builder::ConfigBuilder;
    use iota_test_transaction_builder::make_transfer_iota_transaction;
    use iota_types::{
        base_types::AuthorityName,
        quorum_driver_types::{
            ExecuteTransactionRequestType, ExecuteTransactionRequestV1, QuorumDriverError,
        },
        transaction::TransactionEnvelope,
    };
    use test_cluster::{TestCluster, TestClusterBuilder};

    fn make_socket_addr() -> std::net::SocketAddr {
        std::net::SocketAddr::new([127, 0, 0, 1].into(), 0)
    }

    fn fullnode_orchestrator(
        test_cluster: &TestCluster,
    ) -> Arc<TransactionOrchestrator<NetworkAuthorityClient>> {
        test_cluster
            .fullnode_handle
            .iota_node
            .with(|node| node.transaction_orchestrator())
            .expect("the fullnode runs a transaction orchestrator")
    }

    async fn execute(
        orchestrator: &TransactionOrchestrator<NetworkAuthorityClient>,
        transaction: TransactionEnvelope,
        request_type: ExecuteTransactionRequestType,
    ) -> Result<(), QuorumDriverError> {
        orchestrator
            .execute_transaction_block(
                ExecuteTransactionRequestV1::new(transaction),
                request_type,
                Some(make_socket_addr()),
            )
            .await
            .map(|_| ())
    }

    /// Waits until every validator has recorded `percentage` as the last
    /// overload notification of `authority`.
    async fn wait_for_overload_notification(
        states: &[Arc<AuthorityState>],
        authority: AuthorityName,
        percentage: u8,
    ) {
        tokio::time::timeout(Duration::from_secs(60), async {
            while !states.iter().all(|state| {
                state
                    .epoch_store_for_testing()
                    .load_overload_notification(&authority)
                    .unwrap()
                    == percentage
            }) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!("validators did not record the overload notification {percentage} in time")
        });
    }

    fn quorum_load_shedding_percentages(states: &[Arc<AuthorityState>]) -> Vec<u8> {
        states
            .iter()
            .map(|state| {
                state
                    .epoch_store_for_testing()
                    .get_quorum_load_shedding_percentage()
                    .unwrap()
            })
            .collect()
    }

    /// A validator's local overload is broadcast through consensus. Once a
    /// quorum reports full overload, every user transaction is dropped after
    /// consensus, and the fullnode reports the overload to the client with a
    /// retry hint. A retry of the same transaction succeeds once the
    /// overload clears.
    #[sim_test]
    async fn local_overload_is_broadcast_and_shed_after_consensus() {
        telemetry_subscribers::init_for_testing();

        let test_cluster = TestClusterBuilder::new()
            .with_authority_overload_config(AuthorityOverloadConfig {
                // Keeps the overload monitor from replacing the overload this
                // test sets.
                max_load_shedding_percentage: 0,
                overload_monitor_interval: Duration::from_secs(1),
                ..Default::default()
            })
            .build()
            .await;
        let orchestrator = fullnode_orchestrator(&test_cluster);
        let states: Vec<_> = test_cluster
            .swarm
            .validator_node_handles()
            .iter()
            .map(|handle| handle.with(|node| node.state()))
            .collect();

        states[0].overload_info.set_overload(40);
        wait_for_overload_notification(&states, states[0].name, 40).await;
        assert_eq!(
            quorum_load_shedding_percentages(&states),
            vec![0; states.len()]
        );
        let transaction = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
        execute(
            &orchestrator,
            transaction,
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await
        .expect("one overloaded validator is below the quorum, so nothing is shed");

        for state in &states {
            state.overload_info.set_overload(100);
        }
        for state in &states {
            wait_for_overload_notification(&states, state.name, 100).await;
        }
        assert_eq!(
            quorum_load_shedding_percentages(&states),
            vec![100; states.len()]
        );
        let transaction = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
        let error = execute(
            &orchestrator,
            transaction.clone(),
            ExecuteTransactionRequestType::WaitForEffectsCert,
        )
        .await
        .expect_err("every user transaction is shed at a quorum percentage of 100");
        assert!(
            matches!(error, QuorumDriverError::SystemOverloadRetryAfter { .. }),
            "expected SystemOverloadRetryAfter, got {error:?}"
        );
        for state in &states {
            assert!(
                state
                    .metrics
                    .consensus_handler_load_shedding_dropped_transactions
                    .get()
                    > 0
            );
        }

        for state in &states {
            state.overload_info.clear_overload();
        }
        for state in &states {
            wait_for_overload_notification(&states, state.name, 0).await;
        }
        execute(
            &orchestrator,
            transaction,
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await
        .expect("the shed transaction succeeds on retry once the overload clears");
    }

    /// With the consensus queue of every validator at its limit, admission
    /// rejects every submission, and the fullnode keeps retrying until it
    /// reports the overload to the client.
    #[sim_test]
    async fn full_consensus_queue_is_reported_to_the_client_as_overload() {
        telemetry_subscribers::init_for_testing();

        let mut network_config = ConfigBuilder::new_with_temp_dir()
            .committee_size(NonZeroUsize::new(4).unwrap())
            .build();
        for config in &mut network_config.validator_configs {
            config
                .consensus_config
                .as_mut()
                .expect("validators have a consensus config")
                .max_pending_transactions = Some(0);
        }
        let test_cluster = TestClusterBuilder::new()
            .set_network_config(network_config)
            .build()
            .await;
        let orchestrator = fullnode_orchestrator(&test_cluster);
        let total_stake = test_cluster
            .swarm
            .validator_node_handles()
            .first()
            .expect("the cluster has validators")
            .with(|node| node.state().clone_committee_for_testing().total_votes());

        let transaction = make_transfer_iota_transaction(&test_cluster.wallet, None, None).await;
        let started = tokio::time::Instant::now();
        let error = execute(
            &orchestrator,
            transaction,
            ExecuteTransactionRequestType::WaitForEffectsCert,
        )
        .await
        .expect_err("every validator rejects the transaction at admission");
        assert!(
            matches!(
                &error,
                QuorumDriverError::SystemOverload { overloaded_stake, errors }
                    if *overloaded_stake == total_stake
                        && errors.iter().all(|(error, _, _)| error
                            .to_string()
                            .contains("too many transactions pending in consensus"))
            ),
            "expected SystemOverload from the full stake, got {error:?}"
        );
        // The driver waits at least 10 s after an overloaded attempt, so a
        // later failure means the fullnode retried.
        assert!(
            started.elapsed() > Duration::from_secs(10),
            "the fullnode returned after {:?} without retrying",
            started.elapsed()
        );
    }

    #[sim_test]
    async fn overload_monitor_in_different_nodes() {
        telemetry_subscribers::init_for_testing();

        // Uses a fail point to count the number of nodes that start overload monitor.
        let counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        register_fail_point("starting_overload_monitor", move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Creates a cluster, and tests that number of nodes with overload monitor is
        // equal to the number of validators.
        let test_cluster = TestClusterBuilder::new().build().await;
        let nodes_with_overload_monitor = counter.load(Ordering::SeqCst);
        assert_eq!(
            nodes_with_overload_monitor,
            test_cluster.swarm.validator_node_handles().len()
        );

        // Tests (indirectly) that fullnodes don't run overload monitor.
        assert!(
            test_cluster.swarm.all_nodes().collect::<Vec<_>>().len() > nodes_with_overload_monitor
        );
    }
}

// TODO: move other overload relate tests from execution_driver_tests.rs to
// here.
