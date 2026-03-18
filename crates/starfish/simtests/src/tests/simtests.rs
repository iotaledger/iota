// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(test), expect(unused))]

#[cfg(msim)]
mod test {
    use std::{collections::HashSet, sync::Arc, time::Duration};

    use iota_config::local_ip_utils;
    use iota_macros::sim_test;
    use iota_network_stack::Multiaddr;
    use iota_protocol_config::ProtocolConfig;
    use iota_simulator::{
        SimConfig,
        configs::{bimodal_latency_ms, env_config, uniform_latency_ms},
    };
    use prometheus::Registry;
    use rand::{Rng, SeedableRng as _, rngs::StdRng};
    use starfish_config::{
        Authority, AuthorityKeyPair, Committee, Epoch, NetworkKeyPair, ProtocolKeyPair, Stake,
    };
    use starfish_core::transaction::BlockStatus;
    use tempfile::TempDir;
    use tokio::{sync::RwLock, time::sleep};
    use typed_store::DBMetrics;

    use crate::node::{AuthorityNode, Config, RestartMode};

    fn test_config() -> SimConfig {
        env_config(
            uniform_latency_ms(10..20),
            [
                (
                    "regional_high_variance",
                    bimodal_latency_ms(30..40, 300..800, 0.01),
                ),
                (
                    "global_high_variance",
                    bimodal_latency_ms(60..80, 500..1500, 0.01),
                ),
            ],
        )
    }

    /// Creates a committee for local testing, and the corresponding key pairs
    /// for the authorities.
    pub fn local_committee_and_keys(
        epoch: Epoch,
        authorities_stake: Vec<Stake>,
    ) -> (Committee, Vec<(NetworkKeyPair, ProtocolKeyPair)>) {
        let mut authorities = vec![];
        let mut key_pairs = vec![];
        let mut rng = StdRng::from_seed([0; 32]);
        for (i, stake) in authorities_stake.into_iter().enumerate() {
            let authority_keypair = AuthorityKeyPair::generate(&mut rng);
            let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
            let network_keypair = NetworkKeyPair::generate(&mut rng);
            authorities.push(Authority {
                stake,
                address: get_available_local_address(),
                hostname: format!("test_host_{i}").to_string(),
                authority_key: authority_keypair.public(),
                protocol_key: protocol_keypair.public(),
                network_key: network_keypair.public(),
            });
            key_pairs.push((network_keypair, protocol_keypair));
        }

        let committee = Committee::new(epoch, authorities);
        (committee, key_pairs)
    }

    /// Returns a local address for testing purposes.
    fn get_available_local_address() -> Multiaddr {
        let ip = local_ip_utils::get_new_ip();

        local_ip_utils::new_udp_address_for_testing(&ip)
    }

    /// Verifies commit digest consistency across all running authorities.
    /// Checks that all authorities have identical commit digests for shared
    /// commit indices.
    async fn verify_commit_consistency(authorities: &[AuthorityNode], step: &str) {
        let commit_indices: Vec<u32> = authorities
            .iter()
            .filter(|a| a.is_running())
            .map(|a| a.commit_consumer_monitor().highest_handled_commit())
            .collect();

        if commit_indices.is_empty() {
            return;
        }

        let min_commit: u32 = *commit_indices.iter().min().unwrap();
        let max_commit = *commit_indices.iter().max().unwrap();

        for commit_idx in min_commit..=max_commit {
            let mut digests = Vec::new();
            for (i, authority) in authorities.iter().enumerate() {
                if authority.is_running() {
                    if let Some(digest) = authority.get_commit_digest(commit_idx).await {
                        digests.push((i, digest));
                    }
                }
            }
            if digests.len() > 1 {
                let (_, first_digest) = &digests[0];
                for (auth_idx, digest) in digests.iter().skip(1) {
                    assert_eq!(
                        first_digest, digest,
                        "{step}: Commit {commit_idx} digest mismatch at authority {auth_idx}"
                    );
                }
            }
        }
    }

    /// Configuration for restart cycle timing
    struct RestartCycleConfig {
        /// Duration to wait before stopping an authority
        pre_stop_wait: Duration,
        /// Duration to wait after stopping, before restarting
        stop_duration: Duration,
        /// Duration to wait after restart for catch-up
        post_restart_wait: Duration,
        /// Number of stop/restart cycles per authority
        cycles_per_authority: usize,
    }

    /// Helper function for sequential authority restarts with commit sync
    /// catch-up and transaction sequencing verification.
    ///
    /// This test exercises the fast sync mechanism and verifies consensus
    /// correctness by:
    /// 1. Starting all authorities
    /// 2. Sequentially stopping and restarting each authority (except one for
    ///    quorum)
    /// 3. Verifying that restarted authorities catch up via fast sync
    /// 4. Verifying commit digest consistency across all authorities
    /// 5. Verifying that all sequenced transactions are committed by all
    ///    authorities
    /// 6. Verifying that most submitted transactions are eventually sequenced
    ///    (some may be lost during restarts when in RAM)
    ///
    /// Parameters:
    /// - `mode`: Restart mode controlling DB and state persistence
    ///   - `CleanAll`: Fresh empty DB (simulates node replacement)
    ///   - `PersistAll`: Keep DB and state (crash recovery)
    ///   - `ResetLastProcessed`: Keep DB but reset tracking (tests two-phase
    ///     recovery)
    /// - `long_run`: If true, use longer pre-stop and catch-up times
    /// - `long_restart`: If true, use longer stopped duration
    async fn run_sequential_restarts_test(mode: RestartMode, long_run: bool, long_restart: bool) {
        // ═══════════════════════════════════════════════════════════════
        // Constants
        // ═══════════════════════════════════════════════════════════════
        const NUM_OF_AUTHORITIES: usize = 7;
        const CYCLES_PER_AUTHORITY: usize = 2;

        // Timing constants
        const LONG_DURATION_SECS: u64 = 120;
        const SHORT_DURATION_SECS: u64 = 2;
        const TXN_SUBMIT_INTERVAL_MS: u64 = 10;
        const PRE_FINAL_RUN_SECS: u64 = 2 * LONG_DURATION_SECS;
        const FINAL_SETTLEMENT_WAIT_SECS: u64 = LONG_DURATION_SECS;

        // Verification thresholds
        const MIN_BASELINE_COMMIT: u32 = 100;
        const CATCH_UP_SLACK: u32 = 10;
        const MAX_COMMIT_GAP: u32 = 20;
        const MIN_INCREMENTAL_COMMIT_PROGRESS: u32 = 3;
        const MIN_SUBMITTED_TRANSACTIONS: usize = 400;

        // Clock drift range (ms)
        const MAX_CLOCK_DRIFT_MS: u64 = 150;

        // ═══════════════════════════════════════════════════════════════
        // Setup
        // ═══════════════════════════════════════════════════════════════
        telemetry_subscribers::init_for_testing();
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        // Enable fast commit sync (always enabled in this test)
        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(true);

        // Calculate timing based on flags
        let run_time = if long_run {
            Duration::from_secs(LONG_DURATION_SECS)
        } else {
            Duration::from_secs(SHORT_DURATION_SECS)
        };
        let stop_time = if long_restart {
            Duration::from_secs(LONG_DURATION_SECS)
        } else {
            Duration::from_secs(SHORT_DURATION_SECS)
        };

        let restart_config = RestartCycleConfig {
            pre_stop_wait: Duration::from_secs(LONG_DURATION_SECS),
            stop_duration: stop_time,
            post_restart_wait: run_time,
            cycles_per_authority: CYCLES_PER_AUTHORITY,
        };
        let (committee, keypairs) = local_committee_and_keys(0, [1; NUM_OF_AUTHORITIES].to_vec());

        // Generate pseudorandom clock drifts using MSIM_TEST_SEED for determinism
        let seed_value: u64 = std::env::var("MSIM_TEST_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&seed_value.to_le_bytes());
        let mut rng = StdRng::from_seed(seed);
        let clock_drifts: Vec<u64> = (0..NUM_OF_AUTHORITIES)
            .map(|_| rng.gen_range(0..MAX_CLOCK_DRIFT_MS))
            .collect();

        // Create and start all authorities
        let mut authorities = Vec::with_capacity(committee.size());
        let mut transaction_clients = Vec::with_capacity(committee.size());

        for (authority_index, _authority_info) in committee.authorities() {
            let config = Config {
                authority_index,
                db_dir: Arc::new(TempDir::new().unwrap()),
                committee: committee.clone(),
                keypairs: keypairs.clone(),
                network_type: iota_protocol_config::ConsensusNetwork::Tonic,
                boot_counter: 0,
                protocol_config: protocol_config.clone(),
                clock_drift: clock_drifts[authority_index.value() as usize],
                last_processed_commit: 0,
            };
            let node = AuthorityNode::new(config);
            node.start().await.unwrap();
            node.spawn_committed_subdag_consumer().unwrap();

            transaction_clients.push(node.transaction_client());
            authorities.push(node);
        }

        // Spawn continuous transaction submission and track sequenced transactions
        let clients_for_txns = transaction_clients.clone();
        let submitted_transactions = Arc::new(RwLock::new(HashSet::new()));
        let submitted_for_txns = submitted_transactions.clone();

        let _txn_handle = tokio::spawn(async move {
            let mut counter: u32 = 0;
            loop {
                let txn = counter.to_be_bytes().to_vec();
                let client_idx = counter as usize % clients_for_txns.len();

                // Submit transaction and wait for sequencing confirmation
                match clients_for_txns[client_idx].submit(vec![txn.clone()]).await {
                    Ok((_block_ref, status_receiver)) => {
                        // Spawn task to wait for sequencing
                        let submitted_for_txns = submitted_for_txns.clone();
                        tokio::spawn(async move {
                            match status_receiver.await {
                                Ok(BlockStatus::Sequenced(_)) => {
                                    // Transaction successfully sequenced - add to tracking set
                                    submitted_for_txns.write().await.insert(txn);
                                }
                                Ok(BlockStatus::GarbageCollected(_)) => {
                                    // Transaction was garbage collected, don't
                                    // track
                                }
                                Err(_) => {
                                    // Consensus shutting down, don't track
                                }
                            }
                        });
                    }
                    Err(_) => {
                        // Submission failed, don't track
                    }
                }

                counter = counter.wrapping_add(1);
                sleep(Duration::from_millis(TXN_SUBMIT_INTERVAL_MS)).await;
            }
        });

        // Wait for initial consensus progress
        sleep(restart_config.pre_stop_wait).await;

        // Get baseline commit index
        let baseline_commit = authorities[0]
            .commit_consumer_monitor()
            .highest_handled_commit();

        assert!(
            baseline_commit > MIN_BASELINE_COMMIT,
            "Should have made initial progress: baseline_commit={baseline_commit}, min={MIN_BASELINE_COMMIT}"
        );

        // Verify consistency after baseline progress
        verify_commit_consistency(&authorities, "after initial progress").await;

        // Sequential restart cycles for each authority
        // Only restart authorities 0..NUM_OF_AUTHORITIES-1 to have one always alive
        // authority with clean state to serve as sync source
        // TODO: once fast sync logic can handle early abortion, we can include all
        // authorities in the restart cycles
        for authority_idx in 0..(NUM_OF_AUTHORITIES - 1) {
            for cycle in 0..restart_config.cycles_per_authority {
                // Stop the authority
                let commit_at_stop = authorities[authority_idx]
                    .commit_consumer_monitor()
                    .highest_handled_commit();
                let max_commit_at_stop = authorities
                    .iter()
                    .map(|a| a.commit_consumer_monitor().highest_handled_commit())
                    .max()
                    .unwrap_or(commit_at_stop);
                authorities[authority_idx].stop().await;
                assert!(!authorities[authority_idx].is_running());

                // Wait while stopped (other authorities make progress)
                sleep(restart_config.stop_duration).await;

                // Verify consistency while authority is stopped
                verify_commit_consistency(
                    &authorities,
                    &format!("authority {authority_idx} cycle {cycle}: during stop"),
                )
                .await;

                // Restart the authority with the specified mode
                authorities[authority_idx].restart(mode).await.unwrap();
                authorities[authority_idx]
                    .spawn_committed_subdag_consumer()
                    .unwrap();

                // Wait for catch-up
                sleep(restart_config.post_restart_wait).await;

                let commits_after: Vec<u32> = authorities
                    .iter()
                    .map(|a| a.commit_consumer_monitor().highest_handled_commit())
                    .collect();
                let network_max = commits_after
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != authority_idx)
                    .map(|(_, c)| *c)
                    .max()
                    .unwrap();
                let incremental_progress = network_max.saturating_sub(max_commit_at_stop);

                if long_run {
                    assert!(
                        incremental_progress >= MIN_INCREMENTAL_COMMIT_PROGRESS,
                        "Authority {authority_idx} cycle {cycle}: incremental commit progress too low: {incremental_progress} < {MIN_INCREMENTAL_COMMIT_PROGRESS}"
                    );
                }

                // Verify consistency after restart and catch-up
                verify_commit_consistency(
                    &authorities,
                    &format!("authority {authority_idx} cycle {cycle}: after restart"),
                )
                .await;
            }
        }
        sleep(Duration::from_secs(PRE_FINAL_RUN_SECS)).await;
        // Stop transaction submission and verify all sequenced transactions are
        // committed
        _txn_handle.abort();
        sleep(Duration::from_secs(FINAL_SETTLEMENT_WAIT_SECS)).await;

        // Final verification: all authorities should be relatively close in commit
        // index
        let commit_indices: Vec<u32> = authorities
            .iter()
            .map(|a| a.commit_consumer_monitor().highest_handled_commit())
            .collect();

        let min_commit = *commit_indices.iter().min().unwrap();
        let max_commit = *commit_indices.iter().max().unwrap();

        assert!(
            max_commit - min_commit < MAX_COMMIT_GAP,
            "Gap too large: {max_commit} - {min_commit} >= {MAX_COMMIT_GAP}"
        );

        // Final commit consistency verification
        verify_commit_consistency(&authorities, "final").await;

        // Verify catch-up via fast sync: all authorities should have caught up after
        // settlement
        let final_commits: Vec<u32> = authorities
            .iter()
            .map(|a| a.commit_consumer_monitor().highest_handled_commit())
            .collect();
        let max_final = *final_commits.iter().max().unwrap();
        let min_final = *final_commits.iter().min().unwrap();

        assert!(
            max_final - min_final <= CATCH_UP_SLACK,
            "After settlement, authorities not caught up: min={min_final}, max={max_final}, gap={} (slack: {CATCH_UP_SLACK})",
            max_final - min_final
        );

        // Get submitted transactions
        let submitted = submitted_transactions.read().await.clone();

        // Collect committed transactions from all running authorities
        let mut committed_sets = Vec::new();
        for authority in authorities.iter() {
            if authority.is_running() {
                let txns = authority.get_committed_transactions().await;
                committed_sets.push(txns);
            }
        }

        assert!(
            !committed_sets.is_empty(),
            "No running authorities to verify"
        );
        // Verify all submitted transactions are in the committed set (submitted ⊆
        // committed)
        for txn in &submitted {
            assert!(
                committed_sets.iter().any(|set| set.contains(txn)),
                "Submitted transaction not in committed set: {txn:?}"
            );
        }

        // Assert minimum submitted transactions
        assert!(
            submitted.len() >= MIN_SUBMITTED_TRANSACTIONS,
            "Too few submitted transactions: {} < {MIN_SUBMITTED_TRANSACTIONS}",
            submitted.len()
        );
    }

    /// Fresh DB after each restart, long run before stop, long stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_clean_db_long_run_long_stop() {
        run_sequential_restarts_test(RestartMode::CleanAll, true, true).await;
    }

    /// Fresh DB after each restart, short run before stop, short stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_clean_db_short_run_short_stop() {
        run_sequential_restarts_test(RestartMode::CleanAll, false, false).await;
    }

    /// Fresh DB after each restart, long run before stop, short stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_clean_db_long_run_short_stop() {
        run_sequential_restarts_test(RestartMode::CleanAll, true, false).await;
    }

    /// Fresh DB after each restart, short run before stop, long stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_clean_db_short_run_long_stop() {
        run_sequential_restarts_test(RestartMode::CleanAll, false, true).await;
    }

    /// Persistent DB after each restart, long run before stop, long stop
    /// duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_persistent_db_long_run_long_stop() {
        run_sequential_restarts_test(RestartMode::PersistAll, true, true).await;
    }

    /// Persistent DB after each restart, short run before stop, short stop
    /// duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_persistent_db_short_run_short_stop() {
        run_sequential_restarts_test(RestartMode::PersistAll, false, false).await;
    }

    /// Persistent DB after each restart, long run before stop, short stop
    /// duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_persistent_db_long_run_short_stop() {
        run_sequential_restarts_test(RestartMode::PersistAll, true, false).await;
    }

    /// Persistent DB after each restart, short run before stop, long stop
    /// duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_persistent_db_short_run_long_stop() {
        run_sequential_restarts_test(RestartMode::PersistAll, false, true).await;
    }

    /// DB intact but last_processed_commit reset; long run before stop, long
    /// stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_reset_last_processed_long_run_long_stop() {
        run_sequential_restarts_test(RestartMode::ResetLastProcessed, true, true).await;
    }

    /// Erase all transactions from DB, preserving commits and block headers.
    /// Tests transaction recovery via sync from peers.
    /// Long run before stop, long stop duration.
    #[sim_test(config = "test_config()")]
    async fn test_sequential_restarts_erase_transactions_long_run_long_stop() {
        run_sequential_restarts_test(RestartMode::EraseAllTransactions, true, true).await;
    }
}
