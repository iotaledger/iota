// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Instant};

use iota_protocol_config::ProtocolConfig;
use itertools::Itertools;
use parking_lot::RwLock;
use prometheus::Registry;
use starfish_config::{AuthorityIndex, Committee, NetworkKeyPair, Parameters, ProtocolKeyPair};
use tracing::{info, warn};

use crate::{
    CommitConsumer, CommitConsumerMonitor,
    authority_service::AuthorityService,
    block_manager::BlockManager,
    block_verifier::SignedBlockVerifier,
    commit_observer::CommitObserver,
    commit_syncer::{CommitSyncerHandle, fast::FastCommitSyncer, regular::RegularCommitSyncer},
    commit_vote_monitor::CommitVoteMonitor,
    context::{Clock, Context},
    cordial_knowledge::{CordialKnowledge, CordialKnowledgeHandle},
    core::{Core, CoreSignals},
    core_thread::{ChannelCoreThreadDispatcher, CoreThreadHandle},
    dag_state::DagState,
    header_synchronizer::{HeaderSynchronizer, HeaderSynchronizerHandle},
    leader_schedule::LeaderSchedule,
    leader_timeout::{LeaderTimeoutTask, LeaderTimeoutTaskHandle},
    metrics::initialise_metrics,
    network::tonic_network::{TonicClient, TonicManager},
    shard_reconstructor::{ShardReconstructor, ShardReconstructorHandle},
    storage::rocksdb_store::RocksDBStore,
    subscriber::Subscriber,
    transaction::{TransactionClient, TransactionConsumer, TransactionVerifier},
    transactions_synchronizer::{TransactionsSynchronizer, TransactionsSynchronizerHandle},
};

pub struct ConsensusAuthority {
    context: Arc<Context>,
    start_time: Instant,
    transaction_client: Arc<TransactionClient>,
    header_synchronizer: Arc<HeaderSynchronizerHandle>,
    transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
    commit_consumer_monitor: Arc<CommitConsumerMonitor>,
    shard_reconstructor: Arc<ShardReconstructorHandle>,
    cordial_knowledge: Arc<CordialKnowledgeHandle>,
    regular_commit_syncer_handle: CommitSyncerHandle,
    fast_commit_syncer_handle: Option<CommitSyncerHandle>,
    leader_timeout_handle: LeaderTimeoutTaskHandle,
    core_thread_handle: CoreThreadHandle,
    subscriber: Subscriber<TonicClient, AuthorityService<ChannelCoreThreadDispatcher>>,
    network_manager: TonicManager<AuthorityService<ChannelCoreThreadDispatcher>>,
    #[cfg(msim)]
    store: Arc<RocksDBStore>,
    dag_state: Arc<RwLock<DagState>>,
    #[cfg(test)]
    sync_last_known_own_block: bool,
}

impl ConsensusAuthority {
    /// This function initializes and starts the consensus authority node
    /// It ensures that the authority node is fully initialized and
    /// ready to participate in the consensus process.
    pub async fn start(
        epoch_start_timestamp_ms: u64,
        own_index: AuthorityIndex,
        committee: Committee,
        parameters: Parameters,
        protocol_config: ProtocolConfig,
        // To avoid accidentally leaking the private key, the protocol key pair should only be
        // kept in Core.
        protocol_keypair: ProtocolKeyPair,
        network_keypair: NetworkKeyPair,
        clock: Arc<Clock>,
        transaction_verifier: Arc<dyn TransactionVerifier>,
        commit_consumer: CommitConsumer,
        registry: Registry,
        boot_counter: u64,
    ) -> Self {
        assert!(
            committee.is_valid_index(own_index),
            "Invalid own index {own_index}"
        );
        let own_hostname = &committee.authority(own_index).hostname;
        info!(
            "Starting consensus authority {} {}, {:?}, boot counter {}",
            own_index, own_hostname, protocol_config.version, boot_counter
        );
        info!(
            "Consensus authorities: {}",
            committee
                .authorities()
                .map(|(i, a)| format!("{}: {}", i, a.hostname))
                .join(", ")
        );
        info!("Consensus parameters: {:?}", parameters);
        info!("Consensus committee: {:?}", committee);
        let context = Arc::new(Context::new(
            epoch_start_timestamp_ms,
            own_index,
            committee,
            parameters,
            protocol_config,
            initialise_metrics(registry),
            clock,
        ));
        let start_time = Instant::now();

        let (tx_client, tx_receiver) = TransactionClient::new(context.clone());
        let tx_consumer = TransactionConsumer::new(tx_receiver, context.clone());

        let (core_signals, signals_receivers) = CoreSignals::new(context.clone());

        let mut network_manager =
            TonicManager::<AuthorityService<ChannelCoreThreadDispatcher>>::new(
                context.clone(),
                network_keypair,
            );
        let network_client = network_manager.client();

        let store_path = context.parameters.db_path.as_path().to_str().unwrap();
        let store = Arc::new(RocksDBStore::new(store_path, context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let highest_known_commit_at_startup = dag_state.read().last_commit_index();

        // Sync last known own block is enabled when:
        // 1. This is the first boot of the authority node (e.g. disable if the
        //    validator was active in the previous epoch) and
        // 2. The timeout for syncing last known own block is not set to zero.
        let sync_last_known_own_block = boot_counter == 0
            && !context
                .parameters
                .sync_last_known_own_block_timeout
                .is_zero();
        info!("Sync last known own block: {sync_last_known_own_block}");

        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            transaction_verifier,
        ));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());

        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let commit_consumer_monitor = commit_consumer.monitor();
        commit_consumer_monitor
            .set_highest_observed_commit_at_startup(highest_known_commit_at_startup);
        let commit_observer = CommitObserver::new(
            context.clone(),
            commit_consumer,
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let fast_sync_ongoing = dag_state.read().fast_sync_ongoing();

        let core = Core::new(
            context.clone(),
            leader_schedule,
            tx_consumer,
            block_manager,
            // For streaming RPC, Core will be notified when consumer is available.
            // For non-streaming RPC, there is no way to know so default to true.
            // When there is only one (this) authority, assume subscriber exists.
            context.committee.size() == 1,
            commit_observer,
            core_signals,
            protocol_keypair,
            dag_state.clone(),
            sync_last_known_own_block,
        );

        let (core_dispatcher, core_thread_handle) =
            ChannelCoreThreadDispatcher::start(context.clone(), core, fast_sync_ongoing);
        let core_dispatcher = Arc::new(core_dispatcher);

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let leader_timeout_handle = LeaderTimeoutTask::start(
            core_dispatcher.clone(),
            transactions_synchronizer.clone(),
            &signals_receivers,
            context.clone(),
        );

        let shard_reconstructor =
            ShardReconstructor::start(context.clone(), dag_state.clone(), core_dispatcher.clone());

        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            sync_last_known_own_block,
        );

        // Both commit syncers run, but only one actively fetches based on the gap.
        // CommitSyncer handles small gaps, FastCommitSyncer handles large gaps.
        let regular_commit_syncer_handle = RegularCommitSyncer::new(
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            commit_consumer_monitor.clone(),
            network_client.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        )
        .start();

        // FastCommitSyncer is enabled when both the protocol-level flag and the local
        // config flag are enabled. The protocol flag also controls gRPC endpoint
        // availability, while the local flag allows operators to disable the
        // syncer without a protocol upgrade.
        let fast_commit_syncer_handle = if context.protocol_config.consensus_fast_commit_sync()
            && context.parameters.enable_fast_commit_syncer
        {
            Some(
                FastCommitSyncer::new(
                    context.clone(),
                    core_dispatcher.clone(),
                    commit_vote_monitor.clone(),
                    commit_consumer_monitor.clone(),
                    network_client.clone(),
                    block_verifier.clone(),
                    dag_state.clone(),
                )
                .start(),
            )
        } else {
            None
        };

        let network_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer.clone(),
            transactions_synchronizer.clone(),
            core_dispatcher,
            signals_receivers.block_broadcast_receiver(),
            dag_state.clone(),
            store.clone(),
            shard_reconstructor.transaction_message_sender(),
            cordial_knowledge.clone(),
        ));

        let subscriber = Subscriber::new(
            context.clone(),
            network_client,
            network_service.clone(),
            dag_state.clone(),
        );
        for (peer, _) in context.committee.authorities() {
            if peer != context.own_index {
                subscriber.subscribe(peer);
            }
        }

        network_manager.install_service(network_service).await;

        info!(
            "Consensus authority started, took {:?}",
            start_time.elapsed()
        );

        Self {
            context,
            start_time,
            transaction_client: Arc::new(tx_client),
            header_synchronizer,
            shard_reconstructor,
            cordial_knowledge,
            transactions_synchronizer,
            commit_consumer_monitor,
            regular_commit_syncer_handle,
            fast_commit_syncer_handle,
            leader_timeout_handle,
            core_thread_handle,
            subscriber,
            network_manager,
            #[cfg(msim)]
            store,
            dag_state: dag_state.clone(),
            #[cfg(test)]
            sync_last_known_own_block,
        }
    }

    pub async fn stop(mut self) {
        info!(
            "Stopping authority. Total run time: {:?}",
            self.start_time.elapsed()
        );

        // First shutdown components calling into Core.
        if let Err(e) = self.header_synchronizer.stop().await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            warn!(
                "Failed to stop synchronizer when shutting down consensus: {:?}",
                e
            );
        };

        if let Err(e) = self.transactions_synchronizer.stop().await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            warn!(
                "Failed to stop transactions synchronizer when shutting down consensus: {:?}",
                e
            );
        };

        if let Err(e) = self.shard_reconstructor.stop().await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            warn!(
                "Failed to stop shard reconstructor when shutting down consensus: {:?}",
                e
            );
        };

        if let Err(e) = self.cordial_knowledge.stop().await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
            warn!(
                "Failed to stop cordial knowledge manager when shutting down consensus: {:?}",
                e
            );
        }

        self.regular_commit_syncer_handle.stop().await;
        if let Some(handle) = self.fast_commit_syncer_handle.take() {
            handle.stop().await;
        }
        self.leader_timeout_handle.stop().await;
        // Shutdown Core to stop block productions and broadcast.
        // When using streaming, all subscribers to broadcast blocks stop after this.
        self.core_thread_handle.stop().await;
        // Final flush to ensure all buffered data (including pending transactions
        // referenced by not-yet-solid commits) is persisted before shutdown.
        self.dag_state.write().flush();
        // Stop outgoing long-lived streams before stopping network server.
        self.subscriber.stop();
        self.network_manager.stop().await;

        self.context
            .metrics
            .node_metrics
            .uptime
            .observe(self.start_time.elapsed().as_secs_f64());
    }

    #[cfg(msim)]
    pub async fn stop_and_clear_transactions(self) -> Result<(), String> {
        let store = self.store.clone();
        self.stop().await;
        store
            .delete_all_transactions()
            .map_err(|err| err.to_string())
    }

    pub fn transaction_client(&self) -> Arc<TransactionClient> {
        self.transaction_client.clone()
    }

    pub async fn replay_complete(&self) {
        self.commit_consumer_monitor.replay_complete().await;
    }

    #[cfg(test)]
    pub(crate) fn context(&self) -> &Arc<Context> {
        &self.context
    }

    #[cfg(test)]
    fn sync_last_known_own_block_enabled(&self) -> bool {
        self.sync_last_known_own_block
    }

    /// Stop transaction synchronizer for testing pending subdags.
    #[cfg(test)]
    pub(crate) async fn stop_transactions_synchronizer_for_test(
        &self,
    ) -> Result<(), tokio::task::JoinError> {
        self.transactions_synchronizer.stop().await
    }

    /// Stop shard reconstructor for testing pending subdags.
    #[cfg(test)]
    pub(crate) async fn stop_shard_reconstructor_for_test(
        &self,
    ) -> Result<(), tokio::task::JoinError> {
        self.shard_reconstructor.stop().await
    }

    /// Unsubscribe from a specific peer for testing network partition
    /// scenarios.
    #[cfg(test)]
    pub(crate) fn unsubscribe_from_peer_for_test(&self, peer: AuthorityIndex) {
        self.subscriber.unsubscribe(peer);
    }

    /// Access dag_state for testing pending subdags scenarios.
    #[cfg(test)]
    pub(crate) fn dag_state_for_test(&self) -> &Arc<RwLock<DagState>> {
        &self.dag_state
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(non_snake_case)]

    use std::{
        cmp::max,
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::Duration,
    };

    use iota_metrics::monitored_mpsc::{UnboundedReceiver, unbounded_channel};
    use iota_protocol_config::ProtocolConfig;
    use prometheus::Registry;
    use rstest::rstest;
    use starfish_config::{Parameters, local_committee_and_keys};
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout};
    use typed_store::DBMetrics;

    use super::*;
    use crate::{
        CommittedSubDag, block_header::GENESIS_ROUND, commit::CommitIndex,
        transaction::NoopTransactionVerifier,
    };

    #[rstest]
    #[tokio::test]
    async fn test_authority_start_and_stop(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let (committee, keypairs) = local_committee_and_keys(0, vec![1]);
        let registry = Registry::new();

        let temp_dir = TempDir::new().unwrap();
        let parameters = Parameters {
            db_path: temp_dir.keep(),
            enable_fast_commit_syncer: consensus_fast_commit_sync,
            ..Default::default()
        };
        let txn_verifier = NoopTransactionVerifier {};

        let own_index = committee.to_authority_index(0).unwrap();
        let protocol_keypair = keypairs[own_index].1.clone();
        let network_keypair = keypairs[own_index].0.clone();

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_consumer = CommitConsumer::new(sender, 0);

        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);

        let authority = ConsensusAuthority::start(
            0,
            own_index,
            committee,
            parameters,
            protocol_config,
            protocol_keypair,
            network_keypair,
            Arc::new(Clock::default()),
            Arc::new(txn_verifier),
            commit_consumer,
            registry,
            0,
        )
        .await;

        assert_eq!(authority.context().own_index, own_index);
        assert_eq!(authority.context().committee.epoch(), 0);
        assert_eq!(authority.context().committee.size(), 1);

        authority.stop().await;
    }

    /// This test checks that an authority can be restarted and still get synced
    /// with the rest of the committee.
    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_restart_authority_committee(
        #[values(4, 6)] num_of_authorities: usize,
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        let (committee, keypairs) =
            local_committee_and_keys(0, vec![1; num_of_authorities].to_vec());
        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);

        let temp_dirs = (0..num_of_authorities)
            .map(|_| TempDir::new().unwrap())
            .collect::<Vec<_>>();

        let mut output_receivers = Vec::with_capacity(committee.size());
        let mut authorities = Vec::with_capacity(committee.size());
        let mut boot_counters = vec![0; num_of_authorities];
        let mut consumer_monitors = Vec::with_capacity(committee.size());

        for (index, _authority_info) in committee.authorities() {
            let (authority, receiver, monitor) = make_authority(
                index,
                &temp_dirs[index.value()],
                committee.clone(),
                keypairs.clone(),
                boot_counters[index],
                protocol_config.clone(),
                consensus_fast_commit_sync,
            )
            .await;
            boot_counters[index] += 1;
            output_receivers.push(receiver);
            consumer_monitors.push(monitor);
            authorities.push(authority);
        }

        const NUM_TRANSACTIONS: u8 = 24;
        let mut submitted_transactions = BTreeSet::<Vec<u8>>::new();
        for i in 0..NUM_TRANSACTIONS {
            let txn = vec![i; 16];
            submitted_transactions.insert(txn.clone());
            authorities[i as usize % authorities.len()]
                .transaction_client()
                .submit(vec![txn])
                .await
                .unwrap();
        }

        let total_timeout = Duration::from_secs(30);
        let start = Instant::now();
        for (index, receiver) in output_receivers.iter_mut().enumerate() {
            let mut expected_transactions = submitted_transactions.clone();
            loop {
                if start.elapsed() > total_timeout {
                    panic!(
                        "Test failed: Not all transactions were committed after {total_timeout:?}. Missing: {expected_transactions:?}"
                    );
                }
                let committed_subdag =
                    tokio::time::timeout(Duration::from_secs(1), receiver.recv())
                        .await
                        .unwrap()
                        .unwrap();
                let commit_index = committed_subdag.commit_ref.index;
                consumer_monitors[index].set_highest_handled_commit(commit_index);
                for txns in committed_subdag.transactions {
                    for txn in txns.transactions().iter().map(|t| t.data().to_vec()) {
                        assert!(
                            expected_transactions.remove(&txn),
                            "Transaction not submitted or already seen: {txn:?}"
                        );
                    }
                }

                if expected_transactions.is_empty() {
                    break;
                }
            }
        }

        // Stop authority 0.
        let stopped_authority_index = committee.to_authority_index(0).unwrap();
        authorities
            .remove(stopped_authority_index.value())
            .stop()
            .await;

        // Add some new transactions while authority 0 is down.
        const BIG_NUM_TRANSACTIONS: u8 = 120;
        for i in NUM_TRANSACTIONS..BIG_NUM_TRANSACTIONS {
            let txn = vec![i; 16];
            submitted_transactions.insert(txn.clone());
            authorities[i as usize % authorities.len()]
                .transaction_client()
                .submit(vec![txn])
                .await
                .unwrap();
        }

        sleep(Duration::from_secs(5)).await;

        // After a long sleep, add some new transactions while authority 0 is down.
        // We expect that the transaction synchronizer of authority 0 will kick in to
        // download the transactions that were submitted while it was down.
        for i in BIG_NUM_TRANSACTIONS..2 * BIG_NUM_TRANSACTIONS {
            let txn = vec![i; 16];
            submitted_transactions.insert(txn.clone());
            authorities[i as usize % authorities.len()]
                .transaction_client()
                .submit(vec![txn])
                .await
                .unwrap();
        }

        // Restart authority 0 and let it run.
        let (authority, receiver, monitor) = make_authority(
            stopped_authority_index,
            &temp_dirs[stopped_authority_index.value()],
            committee.clone(),
            keypairs.clone(),
            boot_counters[stopped_authority_index],
            protocol_config.clone(),
            consensus_fast_commit_sync,
        )
        .await;
        boot_counters[stopped_authority_index] += 1;
        output_receivers[stopped_authority_index] = receiver;
        consumer_monitors[stopped_authority_index] = monitor;
        authorities.insert(stopped_authority_index.value(), authority);

        let mut expected_transactions = submitted_transactions.clone();

        let start_time = Instant::now();
        let mut last_committed_index = vec![0; num_of_authorities];
        let mut last_round_committed_blocks = vec![0; num_of_authorities];
        loop {
            if start_time.elapsed() > Duration::from_secs(60) {
                break;
            }
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                // Manually update the commit consumer monitor with the highest handled commit
                let deadline = Instant::now() + Duration::from_millis(25);
                while Instant::now() < deadline {
                    let remaining = deadline - Instant::now();
                    if let Ok(Some(committed_subdag)) =
                        tokio::time::timeout(remaining, receiver.recv()).await
                    {
                        for block_ref in &committed_subdag.base.committed_header_refs {
                            if block_ref.round > GENESIS_ROUND {
                                let author_index = block_ref.author;
                                last_round_committed_blocks[author_index] =
                                    max(last_round_committed_blocks[author_index], block_ref.round);
                            }
                        }

                        if index == stopped_authority_index.value() {
                            for txns in &committed_subdag.transactions {
                                for txn in txns.transactions().iter().map(|t| t.data().to_vec()) {
                                    assert!(
                                        expected_transactions.remove(&txn),
                                        "Transaction not submitted or already seen: {txn:?}"
                                    );
                                }
                            }
                        }
                        let commit_index = committed_subdag.commit_ref.index;
                        assert!(last_committed_index[index] < commit_index);
                        last_committed_index[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    } else {
                        // If we time out, we assume that no new dags were committed.
                        break;
                    }
                }
            }
        }

        // Stop all authorities and exit.
        for authority in authorities {
            authority.stop().await;
        }

        // Expect that authorities get synced and commit indices are close enough.
        let min_commit_index = last_committed_index.iter().min().unwrap();
        let max_commit_index = last_committed_index.iter().max().unwrap();
        assert!(
            max_commit_index - min_commit_index < 5,
            "Commit indices are not close enough: min = {min_commit_index}, max = {max_commit_index}, all = {last_committed_index:?}"
        );

        // Expect that all transactions were submitted and processed.
        assert!(
            expected_transactions.is_empty(),
            "Not all transactions were submitted and processed: {expected_transactions:?}",
        );

        // Expect that all authorities have committed blocks in rounds that are close
        // enough.
        let min_round = last_round_committed_blocks.iter().min().unwrap();
        let max_round = last_round_committed_blocks.iter().max().unwrap();
        assert!(
            max_round - min_round < 5,
            "Committed block rounds are not close enough: min = {min_round}, max = {max_round}, all = {last_round_committed_blocks:?}"
        );
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_small_committee(
        #[values(1, 2, 3)] num_authorities: usize,
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        let (committee, keypairs) = local_committee_and_keys(0, vec![1; num_authorities]);
        let mut protocol_config: ProtocolConfig = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);

        let temp_dirs = (0..num_authorities)
            .map(|_| TempDir::new().unwrap())
            .collect::<Vec<_>>();

        let mut output_receivers = Vec::with_capacity(committee.size());
        let mut authorities: Vec<ConsensusAuthority> = Vec::with_capacity(committee.size());
        let mut boot_counters = vec![0; num_authorities];

        for (index, _authority_info) in committee.authorities() {
            let (authority, receiver, _) = make_authority(
                index,
                &temp_dirs[index.value()],
                committee.clone(),
                keypairs.clone(),
                boot_counters[index],
                protocol_config.clone(),
                consensus_fast_commit_sync,
            )
            .await;
            boot_counters[index] += 1;
            output_receivers.push(receiver);
            authorities.push(authority);
        }

        const NUM_TRANSACTIONS: u8 = 15;
        let mut submitted_transactions = BTreeSet::<Vec<u8>>::new();
        for i in 0..NUM_TRANSACTIONS {
            let txn = vec![i; 16];
            submitted_transactions.insert(txn.clone());
            authorities[i as usize % authorities.len()]
                .transaction_client()
                .submit(vec![txn])
                .await
                .unwrap();
        }

        let total_timeout = Duration::from_secs(30);
        let start = Instant::now();
        for receiver in output_receivers.iter_mut() {
            let mut expected_transactions = submitted_transactions.clone();
            loop {
                if start.elapsed() > total_timeout {
                    panic!(
                        "Test failed: Not all transactions were committed after {total_timeout:?}. Missing: {expected_transactions:?}",
                    );
                }
                let committed_subdag =
                    tokio::time::timeout(Duration::from_secs(1), receiver.recv())
                        .await
                        .unwrap()
                        .unwrap();
                for txns in committed_subdag.transactions {
                    for txn in txns.transactions().iter().map(|t| t.data().to_vec()) {
                        assert!(
                            expected_transactions.remove(&txn),
                            "Transaction not submitted or already seen: {txn:?}"
                        );
                    }
                }

                if expected_transactions.is_empty() {
                    break;
                }
            }
        }

        // Stop authority 0.
        let index = committee.to_authority_index(0).unwrap();
        authorities.remove(index.value()).stop().await;
        sleep(Duration::from_secs(10)).await;

        // Restart authority 0 and let it run.
        let (authority, receiver, _) = make_authority(
            index,
            &temp_dirs[index.value()],
            committee.clone(),
            keypairs.clone(),
            boot_counters[index],
            protocol_config.clone(),
            consensus_fast_commit_sync,
        )
        .await;
        boot_counters[index] += 1;
        output_receivers[index] = receiver;
        authorities.insert(index.value(), authority);
        sleep(Duration::from_secs(10)).await;

        // Stop all authorities and exit.
        for authority in authorities {
            authority.stop().await;
        }
    }

    /// This test checks that an authority can recover from amnesia
    /// successfully.
    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_amnesia_recovery_success(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        const NUM_OF_AUTHORITIES: usize = 4;
        let (committee, keypairs) = local_committee_and_keys(0, [1; NUM_OF_AUTHORITIES].to_vec());
        let mut output_receivers = vec![];
        let mut authorities = BTreeMap::new();
        let mut temp_dirs = BTreeMap::new();
        let mut boot_counters = [0; NUM_OF_AUTHORITIES];

        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);

        for (index, _authority_info) in committee.authorities() {
            let dir = TempDir::new().unwrap();
            let (authority, receiver, _) = make_authority(
                index,
                &dir,
                committee.clone(),
                keypairs.clone(),
                boot_counters[index],
                protocol_config.clone(),
                consensus_fast_commit_sync,
            )
            .await;
            assert!(
                authority.sync_last_known_own_block_enabled(),
                "Expected syncing of last known own block to be enabled as all authorities are of empty db and boot for first time."
            );
            boot_counters[index] += 1;
            output_receivers.push(receiver);
            authorities.insert(index, authority);
            temp_dirs.insert(index, dir);
        }

        // Now we take the receiver of authority 1 and we wait until we see at least one
        // block committed from this authority. That way we'll be 100% sure
        // that at least one block has been proposed and successfully received
        // by a quorum of nodes.
        let index_1 = committee.to_authority_index(1).unwrap();
        'outer: while let Some(result) =
            timeout(Duration::from_secs(10), output_receivers[index_1].recv())
                .await
                .expect("Timed out while waiting for at least one committed block from authority 1")
        {
            for block_ref in &result.base.committed_header_refs {
                if block_ref.round > GENESIS_ROUND && block_ref.author == index_1 {
                    break 'outer;
                }
            }
        }

        // Stop authorities 1, 2 & 3.
        // * Authority 1 will be used to wipe out their DB and practically "force" the
        //   amnesia recovery.
        // * Authorities 2 and 3 are stopped to simulate less than 2f+1 availability,
        //   which will make authority 1 retry during amnesia recovery until it has
        //   finally managed to successfully get back 2f+1 responses, once authority 2
        //   is up and running again.
        sleep(Duration::from_secs(1)).await;
        authorities.remove(&index_1).unwrap().stop().await;
        // We wait for the rest of the authorities to create some blocks without
        // authority 1.
        sleep(Duration::from_secs(5)).await;
        let index_2 = committee.to_authority_index(2).unwrap();
        authorities.remove(&index_2).unwrap().stop().await;
        let index_3 = committee.to_authority_index(3).unwrap();
        authorities.remove(&index_3).unwrap().stop().await;
        sleep(Duration::from_secs(5)).await;

        // Drain any remaining messages from the receiver of the last working authority
        // before restarting authority 1 and remember the last commit index
        let index_0 = committee.to_authority_index(0).unwrap();
        let mut last_commit_before_restart = 0u32;
        while let Ok(Some(committed_subdag)) =
            timeout(Duration::from_millis(100), output_receivers[index_0].recv()).await
        {
            last_commit_before_restart = committed_subdag.commit_ref.index;
        }

        // Authority 1: create a new directory to simulate amnesia. The node will
        // attempt to synchronize the last own block and recover from there. It
        // won't be able to do that successfully as authority 2 is still down.
        // We don't expect any output while there is no quorum.
        let dir = TempDir::new().unwrap();
        // We do reset the boot counter for this one to simulate a "binary" restart
        boot_counters[index_1] = 0;
        let (authority, receiver_1, _) = make_authority(
            index_1,
            &dir,
            committee.clone(),
            keypairs.clone(),
            boot_counters[index_1],
            protocol_config.clone(),
            consensus_fast_commit_sync,
        )
        .await;
        assert!(
            authority.sync_last_known_own_block_enabled(),
            "Authority should have the sync of last own block enabled"
        );
        output_receivers[index_1] = receiver_1;
        boot_counters[index_1] += 1;
        authorities.insert(index_1, authority);
        temp_dirs.insert(index_1, dir);
        // let it run for some time
        sleep(Duration::from_secs(5)).await;

        // Drain any messages from the new receiver and verify there are no NEW commits.
        // The new authority may receive CommittedSubDags via block subscription for
        // blocks that were committed BEFORE the restart (commit index <=
        // last_commit_before_restart). However, it should NOT create any NEW
        // commits since there's no quorum (only 2/4).
        while let Ok(Some(committed_subdag)) =
            timeout(Duration::from_millis(100), output_receivers[index_1].recv()).await
        {
            if committed_subdag.commit_ref.index > last_commit_before_restart {
                panic!(
                    "Expected no new commits after restart, but received commit index {} (last before restart was {})",
                    committed_subdag.commit_ref.index, last_commit_before_restart
                );
            }
        }

        // Now spin up authority 2 using its earlier directory - so no amnesia recovery
        // should be forced here. Authority 1 should be able to recover from
        // amnesia successfully.
        let (authority, _receiver, _) = make_authority(
            index_2,
            &temp_dirs[&index_2],
            committee.clone(),
            keypairs,
            boot_counters[index_2],
            protocol_config.clone(),
            consensus_fast_commit_sync,
        )
        .await;
        assert!(
            !authority.sync_last_known_own_block_enabled(),
            "Authority should not have attempted to sync the last own block"
        );
        boot_counters[index_2] += 1;
        authorities.insert(index_2, authority);
        sleep(Duration::from_secs(5)).await;

        // We wait until we see at least one committed block authored from this
        // authority
        let received_from_authority_1 = timeout(Duration::from_secs(10), async {
            'outer: while let Some(result) = output_receivers[index_1].recv().await {
                for block_ref in &result.base.committed_header_refs {
                    if block_ref.round > GENESIS_ROUND && block_ref.author == index_1 {
                        break 'outer;
                    }
                }
            }
        })
        .await;

        if received_from_authority_1.is_err() {
            panic!(
                "Timed out while waiting for at least one committed block from authority {index_1}"
            );
        }

        // Stop all authorities and exit.
        for (_, authority) in authorities {
            authority.stop().await;
        }
    }

    // TODO: create a fixture
    async fn make_authority(
        index: AuthorityIndex,
        db_dir: &TempDir,
        committee: Committee,
        keypairs: Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        boot_counter: u64,
        protocol_config: ProtocolConfig,
        consensus_fast_commit_sync: bool,
    ) -> (
        ConsensusAuthority,
        UnboundedReceiver<CommittedSubDag>,
        Arc<CommitConsumerMonitor>,
    ) {
        let registry = Registry::new();

        // Cache less blocks to exercise commit sync.
        let parameters = Parameters {
            db_path: db_dir.path().to_path_buf(),
            dag_state_cached_rounds: 5,
            commit_sync_parallel_fetches: 2,
            commit_sync_batch_size: 10,
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            enable_fast_commit_syncer: consensus_fast_commit_sync,
            ..Default::default()
        };
        let txn_verifier = NoopTransactionVerifier {};

        let protocol_keypair = keypairs[index].1.clone();
        let network_keypair = keypairs[index].0.clone();

        let (sender, receiver) = unbounded_channel("consensus_output");
        let commit_consumer = CommitConsumer::new(sender, 0);

        let consensus_consumer_monitor = commit_consumer.monitor();

        let authority = ConsensusAuthority::start(
            0,
            index,
            committee,
            parameters,
            protocol_config,
            protocol_keypair,
            network_keypair,
            Arc::new(Clock::default()),
            Arc::new(txn_verifier),
            commit_consumer,
            registry,
            boot_counter,
        )
        .await;

        (authority, receiver, consensus_consumer_monitor)
    }

    // Helper with custom parameters for fast commit syncer testing
    pub(crate) async fn make_authority_with_params(
        index: AuthorityIndex,
        _db_dir: &TempDir,
        committee: Committee,
        keypairs: Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        boot_counter: u64,
        protocol_config: ProtocolConfig,
        parameters: Parameters,
        last_processed_commit_index: CommitIndex,
    ) -> (
        ConsensusAuthority,
        UnboundedReceiver<CommittedSubDag>,
        Arc<CommitConsumerMonitor>,
    ) {
        let registry = Registry::new();
        let txn_verifier = NoopTransactionVerifier {};

        let protocol_keypair = keypairs[index].1.clone();
        let network_keypair = keypairs[index].0.clone();

        let (sender, receiver) = unbounded_channel("consensus_output");
        let commit_consumer = CommitConsumer::new(sender, last_processed_commit_index);

        let consensus_consumer_monitor = commit_consumer.monitor();

        let authority = ConsensusAuthority::start(
            0,
            index,
            committee,
            parameters,
            protocol_config,
            protocol_keypair,
            network_keypair,
            Arc::new(Clock::default()),
            Arc::new(txn_verifier),
            commit_consumer,
            registry,
            boot_counter,
        )
        .await;

        (authority, receiver, consensus_consumer_monitor)
    }

    /// Test that FastCommitSyncer does not cause consensus divergence after
    /// restart. Verifies that all authorities agree on the same commit
    /// sequence (digest and leader) even when one authority uses fast sync
    /// to catch up after crashes.
    #[tokio::test(flavor = "current_thread")]
    async fn test_fast_commit_syncer_on_restart() {
        telemetry_subscribers::init_for_testing();
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        const NUM_AUTHORITIES: usize = 7;
        const COMMIT_GAP_THRESHOLD: u32 = 50;
        const CATCH_UP_THRESHOLD: u32 = 50;
        const SECOND_RECOVERY_THRESHOLD: u32 = 200;

        let stable_work_duration_time = Duration::from_secs(30);

        let (committee, keypairs) = local_committee_and_keys(0, vec![1; NUM_AUTHORITIES]);
        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(true);

        let temp_dirs: Vec<TempDir> = (0..NUM_AUTHORITIES)
            .map(|_| TempDir::new().unwrap())
            .collect();

        let mut authorities = Vec::with_capacity(NUM_AUTHORITIES);
        let mut boot_counters = [0u64; NUM_AUTHORITIES];
        let mut consumer_monitors = Vec::with_capacity(NUM_AUTHORITIES);
        let mut output_receivers = Vec::with_capacity(NUM_AUTHORITIES);

        use std::collections::HashMap;

        use crate::{
            block_header::BlockRef,
            commit::{CommitDigest, CommitIndex},
        };
        let mut authority_commits: Vec<HashMap<CommitIndex, (CommitDigest, BlockRef)>> =
            vec![HashMap::new(); NUM_AUTHORITIES];

        // Start all authorities
        for (index, _) in committee.authorities() {
            let parameters = Parameters {
                db_path: temp_dirs[index.value()].path().to_path_buf(),
                dag_state_cached_rounds: 5,
                commit_sync_parallel_fetches: 2,
                commit_sync_batch_size: 10,
                commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
                fast_commit_sync_batch_size: 20,
                enable_fast_commit_syncer: true,
                sync_last_known_own_block_timeout: Duration::from_millis(2_000),
                ..Default::default()
            };
            let (authority, receiver, monitor) = make_authority_with_params(
                index,
                &temp_dirs[index.value()],
                committee.clone(),
                keypairs.clone(),
                boot_counters[index],
                protocol_config.clone(),
                parameters,
                0,
            )
            .await;
            boot_counters[index] += 1;
            authorities.push(authority);
            output_receivers.push(receiver);
            consumer_monitors.push(monitor);
        }

        // Drain receivers during initial operation to track commits
        let start_time = Instant::now();
        let mut initial_committed_index = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > initial_committed_index[index] {
                        initial_committed_index[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // First crash: stop authority 0
        let stopped_index: usize = 0;
        authorities.remove(stopped_index).stop().await;

        // Drain other authorities while authority 0 is stopped
        let start_time = Instant::now();
        let mut commits_while_stopped = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                if index == stopped_index {
                    continue; // Skip stopped authority
                }
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > commits_while_stopped[index] {
                        commits_while_stopped[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // First recovery: restart authority 0
        let last_processed_before_restart =
            consumer_monitors[stopped_index].highest_handled_commit();
        let parameters = Parameters {
            db_path: temp_dirs[stopped_index].path().to_path_buf(),
            dag_state_cached_rounds: 5,
            commit_sync_parallel_fetches: 2,
            commit_sync_batch_size: 10,
            commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
            fast_commit_sync_batch_size: 20,
            enable_fast_commit_syncer: true,
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        };
        let (authority, receiver, monitor) = make_authority_with_params(
            committee.to_authority_index(stopped_index).unwrap(),
            &temp_dirs[stopped_index],
            committee.clone(),
            keypairs.clone(),
            boot_counters[stopped_index],
            protocol_config.clone(),
            parameters,
            last_processed_before_restart,
        )
        .await;

        boot_counters[stopped_index] += 1;
        let _ = boot_counters[stopped_index];
        output_receivers[stopped_index] = receiver;
        consumer_monitors[stopped_index] = monitor;
        authorities.insert(stopped_index, authority);

        // Wait for authority 0 to catch up after first recovery
        let mut last_committed_index = [0; NUM_AUTHORITIES];
        let mut last_round_committed_blocks = [0; NUM_AUTHORITIES];
        let start_time = Instant::now();
        let max_wait = Duration::from_secs(120);
        loop {
            if start_time.elapsed() > max_wait {
                break;
            }
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                let deadline = Instant::now() + Duration::from_millis(25);
                while Instant::now() < deadline {
                    let remaining = deadline - Instant::now();
                    if let Ok(Some(committed_subdag)) =
                        tokio::time::timeout(remaining, receiver.recv()).await
                    {
                        for block_ref in &committed_subdag.base.committed_header_refs {
                            if block_ref.round > GENESIS_ROUND {
                                let author_index = block_ref.author;
                                last_round_committed_blocks[author_index] =
                                    max(last_round_committed_blocks[author_index], block_ref.round);
                            }
                        }

                        let commit_index = committed_subdag.commit_ref.index;
                        let commit_digest = committed_subdag.commit_ref.digest;
                        let leader = committed_subdag.leader;

                        authority_commits[index].insert(commit_index, (commit_digest, leader));

                        if commit_index > last_committed_index[index] {
                            last_committed_index[index] = commit_index;
                            consumer_monitors[index].set_highest_handled_commit(commit_index);
                        }
                    } else {
                        break;
                    }
                }
            }

            let max_round = *last_round_committed_blocks.iter().max().unwrap_or(&0);
            if last_round_committed_blocks[stopped_index] > 0
                && last_round_committed_blocks[stopped_index] + CATCH_UP_THRESHOLD >= max_round
            {
                break;
            }
        }

        let max_round_first = *last_round_committed_blocks.iter().max().unwrap();
        assert!(
            last_round_committed_blocks[stopped_index] > 0,
            "Authority should have created blocks after first fast sync"
        );
        assert!(
            last_round_committed_blocks[stopped_index] + CATCH_UP_THRESHOLD >= max_round_first,
            "Authority should be caught up after first fast sync"
        );

        // Drain receivers during normal operation after first recovery
        let start_time = Instant::now();
        let mut commits_after_first_recovery = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > commits_after_first_recovery[index] {
                        commits_after_first_recovery[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // Second crash: stop authority 0 again
        authorities.remove(stopped_index).stop().await;

        // Drain other authorities while authority 0 is stopped (second time)
        let start_time = Instant::now();
        let mut commits_while_stopped_2 = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                if index == stopped_index {
                    continue;
                }
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > commits_while_stopped_2[index] {
                        commits_while_stopped_2[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // Second recovery: restart authority 0 again
        let last_processed_before_second_restart =
            consumer_monitors[stopped_index].highest_handled_commit();

        let parameters = Parameters {
            db_path: temp_dirs[stopped_index].path().to_path_buf(),
            dag_state_cached_rounds: 5,
            commit_sync_parallel_fetches: 2,
            commit_sync_batch_size: 10,
            commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
            fast_commit_sync_batch_size: 20,
            enable_fast_commit_syncer: true,
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        };
        let (authority, receiver, monitor) = make_authority_with_params(
            committee.to_authority_index(stopped_index).unwrap(),
            &temp_dirs[stopped_index],
            committee.clone(),
            keypairs.clone(),
            boot_counters[stopped_index],
            protocol_config.clone(),
            parameters,
            last_processed_before_second_restart,
        )
        .await;
        output_receivers[stopped_index] = receiver;
        consumer_monitors[stopped_index] = monitor;
        authorities.insert(stopped_index, authority);

        // Wait for authority 0 to catch up after second recovery
        let round_before_second_sync = last_round_committed_blocks[stopped_index];
        last_committed_index[stopped_index] = 0;
        let start_time = Instant::now();
        loop {
            if start_time.elapsed() > max_wait {
                break;
            }
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                let deadline = Instant::now() + Duration::from_millis(25);
                while Instant::now() < deadline {
                    let remaining = deadline - Instant::now();
                    if let Ok(Some(committed_subdag)) =
                        tokio::time::timeout(remaining, receiver.recv()).await
                    {
                        for block_ref in &committed_subdag.base.committed_header_refs {
                            if block_ref.round > GENESIS_ROUND {
                                let author_index = block_ref.author;
                                last_round_committed_blocks[author_index] =
                                    max(last_round_committed_blocks[author_index], block_ref.round);
                            }
                        }

                        let commit_index = committed_subdag.commit_ref.index;
                        let commit_digest = committed_subdag.commit_ref.digest;
                        let leader = committed_subdag.leader;

                        // Track this commit
                        authority_commits[index].insert(commit_index, (commit_digest, leader));

                        // Only update if this is a new commit (handles replay during recovery)
                        if commit_index > last_committed_index[index] {
                            last_committed_index[index] = commit_index;
                            consumer_monitors[index].set_highest_handled_commit(commit_index);
                        }
                    } else {
                        break;
                    }
                }
            }

            let max_round = *last_round_committed_blocks.iter().max().unwrap_or(&0);
            let made_progress =
                last_round_committed_blocks[stopped_index] > round_before_second_sync;
            let is_close =
                last_round_committed_blocks[stopped_index] + CATCH_UP_THRESHOLD >= max_round;

            if made_progress && is_close {
                break;
            }
        }

        for authority in authorities {
            authority.stop().await;
        }

        // Verify authority 0 made progress and caught up after second recovery
        assert!(
            last_round_committed_blocks[stopped_index] > round_before_second_sync,
            "Authority should have created new blocks after second fast sync"
        );
        let max_round = *last_round_committed_blocks.iter().max().unwrap();
        assert!(
            last_round_committed_blocks[stopped_index] + SECOND_RECOVERY_THRESHOLD >= max_round,
            "Authority should be caught up after second fast sync"
        );

        // Verify commit sequence consistency across all authorities
        let overall_min = authority_commits
            .iter()
            .filter_map(|commits| commits.keys().min())
            .min()
            .copied()
            .unwrap_or(0);
        let overall_max = authority_commits
            .iter()
            .filter_map(|commits| commits.keys().max())
            .max()
            .copied()
            .unwrap_or(0);

        let mut mismatches = Vec::new();
        let mut mismatch_details = Vec::new();

        for commit_idx in overall_min..=overall_max {
            // Collect all (authority -> values) observations for this commit.
            let mut commit_data: Vec<(usize, CommitDigest, BlockRef)> = Vec::new();
            let mut missing = Vec::new();
            for (auth_idx, commits) in authority_commits.iter().enumerate() {
                if let Some((digest, leader)) = commits.get(&commit_idx) {
                    commit_data.push((auth_idx, *digest, *leader));
                } else {
                    missing.push(auth_idx);
                }
            }

            // If fewer than 2 authorities have data for this commit index, we can't
            // establish divergence (current test semantics). Still record missing
            // for debugging if we later find mismatches elsewhere.
            if commit_data.len() < 2 {
                continue;
            }

            // Group authorities by digest and leader for better diagnostics.
            use std::collections::BTreeMap;
            let mut by_digest: BTreeMap<CommitDigest, Vec<usize>> = BTreeMap::new();
            let mut by_leader: BTreeMap<BlockRef, Vec<usize>> = BTreeMap::new();
            for (auth_idx, digest, leader) in &commit_data {
                by_digest.entry(*digest).or_default().push(*auth_idx);
                by_leader.entry(*leader).or_default().push(*auth_idx);
            }

            // Preserve the old pairwise mismatch summarization, but also attach
            // full observed values.
            if commit_data.len() >= 2 {
                let (first_auth, first_digest, first_leader) = commit_data[0];
                for &(auth_idx, digest, leader) in &commit_data[1..] {
                    if digest != first_digest {
                        mismatches.push(format!(
                            "Commit {commit_idx} digest mismatch: authority {first_auth} vs {auth_idx}"
                        ));
                    }
                    if leader != first_leader {
                        mismatches.push(format!(
                            "Commit {commit_idx} leader mismatch: authority {first_auth} vs {auth_idx}"
                        ));
                    }
                }
            }

            if by_digest.len() > 1 || by_leader.len() > 1 {
                let mut detail = String::new();
                detail.push_str(&format!("Commit {commit_idx} mismatch details:\n"));

                if by_digest.len() > 1 {
                    detail.push_str("  Digests observed:\n");
                    for (digest, auths) in &by_digest {
                        detail.push_str(&format!("    {digest:?}: authorities {auths:?}\n"));
                    }
                } else {
                    // Keep a single-line summary for consistency.
                    let (digest, auths) = by_digest.iter().next().unwrap();
                    detail.push_str(&format!(
                        "  Digest consistent: {digest:?} (authorities {auths:?})\n"
                    ));
                }

                if by_leader.len() > 1 {
                    detail.push_str("  Leaders observed:\n");
                    for (leader, auths) in &by_leader {
                        detail.push_str(&format!("    {leader:?}: authorities {auths:?}\n"));
                    }
                } else {
                    let (leader, auths) = by_leader.iter().next().unwrap();
                    detail.push_str(&format!(
                        "  Leader consistent: {leader:?} (authorities {auths:?})\n"
                    ));
                }

                // Helpful to see who didn't report this commit index at all.
                if !missing.is_empty() {
                    detail.push_str(&format!("  Missing authorities: {missing:?}\n"));
                }

                mismatch_details.push(detail);
            }
        }

        // Print all mismatch details (if any) before the assert so failures are
        // actionable in CI logs.
        if !mismatch_details.is_empty() {
            eprintln!("\n=== COMMIT SEQUENCE DIVERGENCE DETAILS ===");
            for d in &mismatch_details {
                eprintln!("{d}");
            }
        }

        assert!(
            mismatches.is_empty(),
            "Commit sequence divergence detected - {} mismatches found. \
            This indicates the fast sync authority has a different leader schedule than other authorities.\n\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }

    /// Test that FastCommitSyncer can recover if it crashes before finishing.
    #[tokio::test(flavor = "current_thread")]
    async fn test_fast_commit_syncer_fail_on_unfinished_fast_sync_restart() {
        telemetry_subscribers::init_for_testing();
        let db_registry = Registry::new();
        DBMetrics::init(&db_registry);

        const NUM_AUTHORITIES: usize = 7;
        const COMMIT_GAP_THRESHOLD: u32 = 50;
        const CATCH_UP_THRESHOLD: u32 = 50;
        const SECOND_RECOVERY_THRESHOLD: u32 = 200;

        let stable_work_duration_time = Duration::from_secs(30);

        let (committee, keypairs) = local_committee_and_keys(0, vec![1; NUM_AUTHORITIES]);
        let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config.set_consensus_fast_commit_sync_for_testing(true);

        let temp_dirs: Vec<TempDir> = (0..NUM_AUTHORITIES)
            .map(|_| TempDir::new().unwrap())
            .collect();

        let mut authorities = Vec::with_capacity(NUM_AUTHORITIES);
        let mut boot_counters = [0u64; NUM_AUTHORITIES];
        let mut consumer_monitors = Vec::with_capacity(NUM_AUTHORITIES);
        let mut output_receivers = Vec::with_capacity(NUM_AUTHORITIES);

        use std::collections::HashMap;

        use crate::{
            block_header::BlockRef,
            commit::{CommitDigest, CommitIndex},
        };
        let mut authority_commits: Vec<HashMap<CommitIndex, (CommitDigest, BlockRef)>> =
            vec![HashMap::new(); NUM_AUTHORITIES];

        // Start all authorities
        for (index, _) in committee.authorities() {
            let parameters = Parameters {
                db_path: temp_dirs[index.value()].path().to_path_buf(),
                dag_state_cached_rounds: 5,
                commit_sync_parallel_fetches: 2,
                commit_sync_batch_size: 10,
                commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
                fast_commit_sync_batch_size: 20,
                enable_fast_commit_syncer: true,
                sync_last_known_own_block_timeout: Duration::from_millis(2_000),
                ..Default::default()
            };
            let (authority, receiver, monitor) = make_authority_with_params(
                index,
                &temp_dirs[index.value()],
                committee.clone(),
                keypairs.clone(),
                boot_counters[index],
                protocol_config.clone(),
                parameters,
                0,
            )
            .await;
            boot_counters[index] += 1;
            authorities.push(authority);
            output_receivers.push(receiver);
            consumer_monitors.push(monitor);
        }

        // Drain receivers during initial operation to track commits
        let start_time = Instant::now();
        let mut initial_committed_index = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > initial_committed_index[index] {
                        initial_committed_index[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // First crash: stop authority 0
        let stopped_index: usize = 0;
        authorities.remove(stopped_index).stop().await;

        // Drain other authorities while authority 0 is stopped
        let start_time = Instant::now();
        let mut commits_while_stopped = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                if index == stopped_index {
                    continue; // Skip stopped authority
                }
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > commits_while_stopped[index] {
                        commits_while_stopped[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // First recovery: restart authority 0
        let last_processed_before_restart =
            consumer_monitors[stopped_index].highest_handled_commit();
        let parameters = Parameters {
            db_path: temp_dirs[stopped_index].path().to_path_buf(),
            dag_state_cached_rounds: 5,
            commit_sync_parallel_fetches: 2,
            commit_sync_batch_size: 10,
            commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
            fast_commit_sync_batch_size: 20,
            enable_fast_commit_syncer: true,
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        };
        let (authority, receiver, monitor) = make_authority_with_params(
            committee.to_authority_index(stopped_index).unwrap(),
            &temp_dirs[stopped_index],
            committee.clone(),
            keypairs.clone(),
            boot_counters[stopped_index],
            protocol_config.clone(),
            parameters,
            last_processed_before_restart,
        )
        .await;
        boot_counters[stopped_index] += 1;
        output_receivers[stopped_index] = receiver;
        consumer_monitors[stopped_index] = monitor;
        authorities.insert(stopped_index, authority);

        // Wait for authority 0 to catch up after first recovery
        let mut last_committed_index = [0; NUM_AUTHORITIES];
        let mut last_round_committed_blocks = [0; NUM_AUTHORITIES];
        let start_time = Instant::now();
        // Wait only 500ms for the fast sync to start up and not let it finish
        let max_wait_first = Duration::from_millis(500);
        loop {
            if start_time.elapsed() > max_wait_first {
                break;
            }
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                let deadline = Instant::now() + Duration::from_millis(25);
                while Instant::now() < deadline {
                    let remaining = deadline - Instant::now();
                    if let Ok(Some(committed_subdag)) =
                        tokio::time::timeout(remaining, receiver.recv()).await
                    {
                        for block_ref in &committed_subdag.base.committed_header_refs {
                            if block_ref.round > GENESIS_ROUND {
                                let author_index = block_ref.author;
                                last_round_committed_blocks[author_index] =
                                    max(last_round_committed_blocks[author_index], block_ref.round);
                            }
                        }

                        let commit_index = committed_subdag.commit_ref.index;
                        let commit_digest = committed_subdag.commit_ref.digest;
                        let leader = committed_subdag.leader;

                        authority_commits[index].insert(commit_index, (commit_digest, leader));

                        if commit_index > last_committed_index[index] {
                            last_committed_index[index] = commit_index;
                            consumer_monitors[index].set_highest_handled_commit(commit_index);
                        }
                    } else {
                        break;
                    }
                }
            }

            let max_round = *last_round_committed_blocks.iter().max().unwrap_or(&0);
            if last_round_committed_blocks[stopped_index] > 0
                && last_round_committed_blocks[stopped_index] + CATCH_UP_THRESHOLD >= max_round
            {
                break;
            }
        }

        // Second crash: stop authority 0 again
        authorities.remove(stopped_index).stop().await;

        // Drain other authorities while authority 0 is stopped (second time)
        let start_time = Instant::now();
        let mut commits_while_stopped_2 = [0u32; NUM_AUTHORITIES];
        while start_time.elapsed() < stable_work_duration_time {
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                if index == stopped_index {
                    continue;
                }
                while let Ok(committed_subdag) = receiver.try_recv() {
                    let commit_index = committed_subdag.commit_ref.index;
                    if commit_index > commits_while_stopped_2[index] {
                        commits_while_stopped_2[index] = commit_index;
                        consumer_monitors[index].set_highest_handled_commit(commit_index);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }

        // Second recovery: restart authority 0 again
        let last_processed_before_second_restart =
            consumer_monitors[stopped_index].highest_handled_commit();

        let parameters = Parameters {
            db_path: temp_dirs[stopped_index].path().to_path_buf(),
            dag_state_cached_rounds: 5,
            commit_sync_parallel_fetches: 2,
            commit_sync_batch_size: 10,
            commit_sync_gap_threshold: COMMIT_GAP_THRESHOLD,
            fast_commit_sync_batch_size: 20,
            enable_fast_commit_syncer: true,
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        };
        let (authority, receiver, monitor) = make_authority_with_params(
            committee.to_authority_index(stopped_index).unwrap(),
            &temp_dirs[stopped_index],
            committee.clone(),
            keypairs.clone(),
            boot_counters[stopped_index],
            protocol_config.clone(),
            parameters,
            last_processed_before_second_restart,
        )
        .await;
        output_receivers[stopped_index] = receiver;
        consumer_monitors[stopped_index] = monitor;
        authorities.insert(stopped_index, authority);

        // Wait for authority 0 to catch up after second recovery
        let max_wait_second = Duration::from_secs(120);
        let round_before_second_sync = last_round_committed_blocks[stopped_index];
        last_committed_index[stopped_index] = 0;
        let start_time = Instant::now();
        loop {
            if start_time.elapsed() > max_wait_second {
                break;
            }
            for (index, receiver) in output_receivers.iter_mut().enumerate() {
                let deadline = Instant::now() + Duration::from_millis(25);
                while Instant::now() < deadline {
                    let remaining = deadline - Instant::now();
                    if let Ok(Some(committed_subdag)) =
                        tokio::time::timeout(remaining, receiver.recv()).await
                    {
                        for block_ref in &committed_subdag.base.committed_header_refs {
                            if block_ref.round > GENESIS_ROUND {
                                let author_index = block_ref.author;
                                last_round_committed_blocks[author_index] =
                                    max(last_round_committed_blocks[author_index], block_ref.round);
                            }
                        }

                        let commit_index = committed_subdag.commit_ref.index;
                        let commit_digest = committed_subdag.commit_ref.digest;
                        let leader = committed_subdag.leader;

                        // Track this commit
                        authority_commits[index].insert(commit_index, (commit_digest, leader));

                        // Only update if this is a new commit (handles replay during recovery)
                        if commit_index > last_committed_index[index] {
                            last_committed_index[index] = commit_index;
                            consumer_monitors[index].set_highest_handled_commit(commit_index);
                        }
                    } else {
                        break;
                    }
                }
            }

            let max_round = *last_round_committed_blocks.iter().max().unwrap_or(&0);
            let made_progress =
                last_round_committed_blocks[stopped_index] > round_before_second_sync;
            let is_close =
                last_round_committed_blocks[stopped_index] + CATCH_UP_THRESHOLD >= max_round;

            if made_progress && is_close {
                break;
            }
        }

        for authority in authorities {
            authority.stop().await;
        }

        // Verify authority 0 made progress and caught up after second recovery
        assert!(
            last_round_committed_blocks[stopped_index] > round_before_second_sync,
            "Authority should have created new blocks after second fast sync"
        );
        let max_round = *last_round_committed_blocks.iter().max().unwrap();
        assert!(
            last_round_committed_blocks[stopped_index] + SECOND_RECOVERY_THRESHOLD >= max_round,
            "Authority should be caught up after second fast sync"
        );

        // Verify commit sequence consistency across all authorities
        let overall_min = authority_commits
            .iter()
            .filter_map(|commits| commits.keys().min())
            .min()
            .copied()
            .unwrap_or(0);
        let overall_max = authority_commits
            .iter()
            .filter_map(|commits| commits.keys().max())
            .max()
            .copied()
            .unwrap_or(0);

        let mut mismatches = Vec::new();
        let mut mismatch_details = Vec::new();

        for commit_idx in overall_min..=overall_max {
            // Collect all (authority -> values) observations for this commit.
            let mut commit_data: Vec<(usize, CommitDigest, BlockRef)> = Vec::new();
            let mut missing = Vec::new();
            for (auth_idx, commits) in authority_commits.iter().enumerate() {
                if let Some((digest, leader)) = commits.get(&commit_idx) {
                    commit_data.push((auth_idx, *digest, *leader));
                } else {
                    missing.push(auth_idx);
                }
            }

            // If fewer than 2 authorities have data for this commit index, we can't
            // establish divergence (current test semantics). Still record missing
            // for debugging if we later find mismatches elsewhere.
            if commit_data.len() < 2 {
                continue;
            }

            // Group authorities by digest and leader for better diagnostics.
            use std::collections::BTreeMap;
            let mut by_digest: BTreeMap<CommitDigest, Vec<usize>> = BTreeMap::new();
            let mut by_leader: BTreeMap<BlockRef, Vec<usize>> = BTreeMap::new();
            for (auth_idx, digest, leader) in &commit_data {
                by_digest.entry(*digest).or_default().push(*auth_idx);
                by_leader.entry(*leader).or_default().push(*auth_idx);
            }

            // Preserve the old pairwise mismatch summarization, but also attach
            // full observed values.
            if commit_data.len() >= 2 {
                let (first_auth, first_digest, first_leader) = commit_data[0];
                for &(auth_idx, digest, leader) in &commit_data[1..] {
                    if digest != first_digest {
                        mismatches.push(format!(
                            "Commit {commit_idx} digest mismatch: authority {first_auth} vs {auth_idx}"
                        ));
                    }
                    if leader != first_leader {
                        mismatches.push(format!(
                            "Commit {commit_idx} leader mismatch: authority {first_auth} vs {auth_idx}"
                        ));
                    }
                }
            }

            if by_digest.len() > 1 || by_leader.len() > 1 {
                let mut detail = String::new();
                detail.push_str(&format!("Commit {commit_idx} mismatch details:\n"));

                if by_digest.len() > 1 {
                    detail.push_str("  Digests observed:\n");
                    for (digest, auths) in &by_digest {
                        detail.push_str(&format!("    {digest:?}: authorities {auths:?}\n"));
                    }
                } else {
                    // Keep a single-line summary for consistency.
                    let (digest, auths) = by_digest.iter().next().unwrap();
                    detail.push_str(&format!(
                        "  Digest consistent: {digest:?} (authorities {auths:?})\n"
                    ));
                }

                if by_leader.len() > 1 {
                    detail.push_str("  Leaders observed:\n");
                    for (leader, auths) in &by_leader {
                        detail.push_str(&format!("    {leader:?}: authorities {auths:?}\n"));
                    }
                } else {
                    let (leader, auths) = by_leader.iter().next().unwrap();
                    detail.push_str(&format!(
                        "  Leader consistent: {leader:?} (authorities {auths:?})\n"
                    ));
                }

                // Helpful to see who didn't report this commit index at all.
                if !missing.is_empty() {
                    detail.push_str(&format!("  Missing authorities: {missing:?}\n"));
                }

                mismatch_details.push(detail);
            }
        }

        // Print all mismatch details (if any) before the assert so failures are
        // actionable in CI logs.
        if !mismatch_details.is_empty() {
            eprintln!("\n=== COMMIT SEQUENCE DIVERGENCE DETAILS ===");
            for d in &mismatch_details {
                eprintln!("{d}");
            }
        }

        assert!(
            mismatches.is_empty(),
            "Commit sequence divergence detected - {} mismatches found. \
            This indicates the fast sync authority has a different leader schedule than other authorities.\n\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }
}
