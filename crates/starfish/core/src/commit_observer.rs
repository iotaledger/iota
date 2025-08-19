// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use iota_metrics::monitored_mpsc::UnboundedSender;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::time::Instant;
use tracing::{debug, info};

use crate::{
    BlockRef, CommitConsumer, CommittedSubDag,
    block_header::{BlockHeaderAPI, VerifiedBlockHeader},
    commit::{CommitAPI, CommitIndex, PendingSubDag, load_pending_subdag_from_store},
    context::Context,
    dag_state::{DagState, MAX_TRANSACTIONS_ACK_DEPTH},
    data_manager::DataManager,
    error::{ConsensusError, ConsensusResult},
    leader_schedule::LeaderSchedule,
    linearizer::{Linearizer, MAX_LINEARIZER_DEPTH},
    storage::Store,
};

/// Role of CommitObserver
/// - Called by core when try_commit() returns newly committed leaders.
/// - The newly committed leaders are sent to commit observer and then commit
///   observer gets subdags for each leader via the commit interpreter
///   (linearizer)
/// - The committed subdags are sent as consensus output via an unbounded tokio
///   channel.
///
/// No back pressure mechanism is needed as backpressure is handled as input
/// into consensus.
///
/// - Commit metadata including index is persisted in store, before the
///   CommittedSubDag is sent to the consumer.
/// - When CommitObserver is initialized a last processed commit index can be
///   used to ensure any missing commits are re-sent.
pub(crate) struct CommitObserver {
    context: Arc<Context>,
    /// Component to deterministically collect subdags for committed leaders.
    commit_interpreter: Linearizer,
    /// Component to deterministically collect subdags for committed leaders.
    commit_solidifier: DataManager,
    /// An unbounded channel to send committed sub-dags to the consumer of
    /// consensus output.
    sender: UnboundedSender<CommittedSubDag>,
    /// Persistent storage for blocks, commits and other consensus data.
    store: Arc<dyn Store>,
    /// Dag state for direct access to block headers
    dag_state: Arc<RwLock<DagState>>,

    leader_schedule: Arc<LeaderSchedule>,
}

impl CommitObserver {
    pub(crate) fn new(
        context: Arc<Context>,
        commit_consumer: CommitConsumer,
        dag_state: Arc<RwLock<DagState>>,
        store: Arc<dyn Store>,
        leader_schedule: Arc<LeaderSchedule>,
    ) -> Self {
        let mut observer = Self {
            commit_interpreter: Linearizer::new(
                context.clone(),
                dag_state.clone(),
                leader_schedule.clone(),
            ),
            commit_solidifier: DataManager::new(dag_state.clone()),
            context,
            sender: commit_consumer.sender,
            store,
            dag_state,
            leader_schedule,
        };

        observer.recover_and_send_commits(commit_consumer.last_processed_commit_index);
        observer
    }

    /// Handles the creation of commits from a set of passed leaders.
    ///
    /// # Returns
    /// A tuple containing:
    /// - A vector of sub-dags, which include block references to committed
    ///   transactions (but not the transactions themselves) created by the
    ///   committed leaders.
    /// - A vector of block references to transactions that were missing during
    ///   the commit.
    pub(crate) fn handle_commit(
        &mut self,
        committed_leaders: Vec<VerifiedBlockHeader>,
    ) -> ConsensusResult<(
        Vec<PendingSubDag>,
        BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["CommitObserver::handle_commit"])
            .start_timer();

        let pending_sub_dags = self.commit_interpreter.handle_commit(committed_leaders);

        // First, add the commits to the commit solidifier to make sure that the data is
        // available. This function returns not only the just-created commits but also
        // any pending ones that'd become solid since the last commit.
        let (solid_sub_dags, missing_transactions) =
            self.commit_solidifier.try_commit(&pending_sub_dags);

        tracing::trace!("Missing committed data {missing_transactions:#?}");

        // Retrieve the transaction acknowledgment authors for the missing transactions.
        // This will be used by the transactions synchronizer to fetch the missing
        // transactions from the authorities that acknowledged them.
        let missing_transaction_acknowledgers = self
            .commit_interpreter
            .get_transaction_ack_authors(missing_transactions);

        let mut sent_sub_dags = Vec::with_capacity(solid_sub_dags.len());
        for solid_sub_dag in solid_sub_dags.iter() {
            // Failures in sender.send() are assumed to be permanent
            if let Err(err) = self.sender.send(solid_sub_dag.clone()) {
                tracing::error!(
                    "Failed to send committed sub-dag, probably due to shutdown: {err:?}"
                );
                return Err(ConsensusError::Shutdown);
            }
            tracing::debug!(
                "Sending to execution commit {} leader {}",
                solid_sub_dag.commit_ref,
                solid_sub_dag.leader
            );
            sent_sub_dags.push(solid_sub_dag);
        }
        self.report_metrics(&pending_sub_dags, &solid_sub_dags);

        // Evict the ack tracker using the information from the latest solid subdag
        if !solid_sub_dags.is_empty() {
            let max_solid_commit_leader_round = solid_sub_dags
                .last()
                .expect("There should be at least one solid subdag")
                .leader
                .round;
            self.commit_interpreter
                .evict_old_acknowledgments(max_solid_commit_leader_round);
        }
        tracing::trace!("Committed & sent {sent_sub_dags:#?}");

        Ok((pending_sub_dags, missing_transaction_acknowledgers))
    }

    fn recover_and_send_commits(&mut self, last_processed_commit_index: CommitIndex) {
        let now = Instant::now();
        // TODO: remove this check, to allow consensus to regenerate commits?
        let last_commit = self
            .store
            .read_last_commit()
            .expect("Reading the last commit should not fail");

        // Value used to recover transactions_ack_tracker in the linearizer.
        let mut recovery_lower_bound = last_processed_commit_index + 1;
        if let Some(last_commit) = &last_commit {
            let last_commit_index = last_commit.index();

            // The earliest commit that still might acknowledge not-yet-committed
            // transactions that still have a chance of being committed is no higher than
            // `last_pending_commit_index - MAX_TRANSACTIONS_ACK_CHECK -
            // MAX_LINEARIZER_DEPTH`.

            let commit_index_to_recover_acks =
                last_commit_index.saturating_sub(MAX_TRANSACTIONS_ACK_DEPTH + MAX_LINEARIZER_DEPTH);

            recovery_lower_bound = recovery_lower_bound
                .min(commit_index_to_recover_acks)
                .max(1);
            assert!(last_commit_index >= last_processed_commit_index);
            if last_commit_index == last_processed_commit_index {
                debug!(
                    "Nothing to recover for commit observer as commit index {last_commit_index} = \
                    {last_processed_commit_index} last processed index"
                );
            }
        };

        // Retrieve all the commits from the recover lower bound until the end.
        let recovery_commits = self
            .store
            .scan_commits((recovery_lower_bound..=CommitIndex::MAX).into())
            .expect("Scanning commits should not fail");

        info!(
            "Recovering commit observer after last processed index {last_processed_commit_index} and \
            recovery lower bound {recovery_lower_bound} with last commit {} and {} recovery commits",
            last_commit.map(|c| c.index()).unwrap_or_default(),
            recovery_commits.len()
        );

        // Recover transaction acknowledgment tracker in the linearizer using all the
        // commits and resend all the committed sub-dags to the consensus output channel
        // for all the commits above the last processed index.
        let mut next_commit_index_to_recover = recovery_lower_bound;
        let mut last_sent_commit_index = last_processed_commit_index;
        let num_recovery_commits = recovery_commits.len();

        for (index, commit) in recovery_commits.into_iter().enumerate() {
            let commit_index = commit.index();
            // Commit index must be continuous during recovert.
            assert_eq!(commit_index, next_commit_index_to_recover);
            if index == 0 {
                self.commit_solidifier
                    .set_last_committed_index(commit_index.saturating_sub(1));
            }
            // On recovery leader schedule will be updated with the current scores
            // and the scores will be passed along with the last commit sent to
            // iota so that the current scores are available for submission.
            let reputation_scores = if index == num_recovery_commits - 1 {
                self.leader_schedule
                    .leader_swap_table
                    .read()
                    .reputation_scores_desc
                    .clone()
            } else {
                vec![]
            };

            info!("Processing commit {} during recovery", commit_index);

            let pending_sub_dag =
                load_pending_subdag_from_store(self.store.as_ref(), commit, reputation_scores);

            // Recover transaction acknowledgments tracker state by adding transaction
            // acknowledgments from all pending sub-dags that still might
            // correctly acknowledge transactions.
            for ((round, authority_idx), transaction_acknowledgments) in
                pending_sub_dag.transaction_acknowledgments().into_iter()
            {
                self.commit_interpreter.add_committed_transaction_acks(
                    round,
                    authority_idx,
                    transaction_acknowledgments,
                );
            }
            // Put all the pending sub-dags into the commit solidifier to make sure that
            // they are tracked there. The commit will be sent to IOTA here if all the
            // transactions are available, or will be kept in the buffer and sent later when
            // the transactions become available.
            let (solid_sub_dags, _missing) = self.commit_solidifier.try_commit(&[pending_sub_dag]);
            // Only submit unprocessed commits to IOTA
            for solid_sub_dag in solid_sub_dags {
                if solid_sub_dag.commit_ref.index > last_processed_commit_index {
                    // Commit index must be continuous during recovery.
                    assert_eq!(solid_sub_dag.commit_ref.index, last_sent_commit_index + 1);
                    info!(
                        "Sending solid commit {} during recovery",
                        solid_sub_dag.commit_ref.index
                    );
                    self.sender.send(solid_sub_dag).unwrap_or_else(|e| {
                        panic!(
                            "Failed to send commit during recovery, probably due to shutdown: {:?}",
                            e
                        )
                    });

                    last_sent_commit_index += 1;
                }
            }

            next_commit_index_to_recover += 1;
        }

        info!(
            "Commit observer recovery completed, took {:?}",
            now.elapsed()
        );
    }

    /// Get all missing transactions from pending subdags along with authorities
    /// who acknowledged them
    pub(crate) fn get_missing_transaction_data(
        &self,
    ) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        let missing_refs = self.commit_solidifier.get_missing_transaction_data();
        self.commit_interpreter
            .get_transaction_ack_authors(missing_refs.into_iter().collect())
    }

    fn report_metrics(
        &self,
        pending_sub_dags: &[PendingSubDag],
        committed_sub_dags: &[CommittedSubDag],
    ) {
        let metrics = &self.context.metrics.node_metrics;
        let utc_now = self.context.clock.timestamp_utc_ms();

        // First report block_header-related metrics for pending subdags
        for commit in pending_sub_dags {
            debug!(
                "Consensus pending subdag {} with leader {} has {} blocks",
                commit.commit_ref,
                commit.leader,
                commit.blocks.len()
            );

            metrics
                .last_committed_leader_round
                .set(commit.leader.round as i64);
            metrics
                .last_commit_index
                .set(commit.commit_ref.index as i64);
            metrics
                .blocks_per_commit_count
                .observe(commit.blocks.len() as f64);

            for block in &commit.blocks {
                let latency_ms = utc_now
                    .checked_sub(block.timestamp_ms())
                    .unwrap_or_default();
                metrics
                    .block_commit_latency
                    .observe(Duration::from_millis(latency_ms).as_secs_f64());
            }
        }

        self.context
            .metrics
            .node_metrics
            .sub_dags_per_commit_count
            .observe(pending_sub_dags.len() as f64);

        // Now report transaction-related metrics for committed subdags
        for commit in committed_sub_dags {
            info!(
                "Consensus available subdag {} with leader {} has transactions from {} blocks",
                commit.commit_ref,
                commit.leader,
                commit.transactions.len()
            );

            // Report the actual number of committed transactions
            metrics.transactions_per_commit_count.observe(
                commit
                    .transactions
                    .iter()
                    .map(|x| x.transactions().len())
                    .sum::<usize>() as f64,
            );

            let block_refs_for_committed_txs = commit
                .transactions
                .iter()
                .map(|tx| tx.block_ref())
                .collect::<Vec<_>>();

            // Read only cached block headers from storage for the transactions in the
            // commit. Headers are needed to calculate the latency of the
            // transactions. The metrics reflects only the latency for cached
            // block headers
            let headers_for_committed_txs = self
                .dag_state
                .read()
                .get_cached_block_headers(&block_refs_for_committed_txs)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            for block_header in headers_for_committed_txs {
                let latency_ms = utc_now
                    .checked_sub(block_header.timestamp_ms())
                    .unwrap_or_default();
                metrics
                    .transaction_commit_latency
                    .observe(Duration::from_millis(latency_ms).as_secs_f64());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_metrics::monitored_mpsc::{UnboundedReceiver, unbounded_channel};
    use parking_lot::RwLock;

    use super::*;
    use crate::{
        block_header::BlockRef, context::Context, dag_state::DagState,
        storage::mem_store::MemStore, test_dag_builder::DagBuilder,
    };

    #[tokio::test]
    async fn test_handle_commit() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let mem_store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            mem_store.clone(),
        )));
        let last_processed_commit_index = 0;
        let (sender, mut receiver) = unbounded_channel("consensus_output");

        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let mut observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender, last_processed_commit_index),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule,
        );

        // Populate fully connected test blocks for round 0 ~ 10, authorities 0 ~ 3.
        let num_rounds = 10;
        let mut builder = DagBuilder::new(context.clone());
        builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        let leaders = builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        let (commits, _missing_transactions_refs) =
            observer.handle_commit(leaders.clone()).unwrap();

        // Check commits are returned by CommitObserver::handle_commit is accurate
        let mut expected_stored_refs: Vec<BlockRef> = vec![];
        for (idx, subdag) in commits.iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());
            let expected_ts = if idx == 0 {
                leaders[idx].timestamp_ms()
            } else {
                leaders[idx]
                    .timestamp_ms()
                    .max(commits[idx - 1].timestamp_ms)
            };
            assert_eq!(expected_ts, subdag.timestamp_ms);
            if idx == 0 {
                // First subdag includes the leader block plus all ancestor blocks
                // of the leader minus the genesis round blocks
                assert_eq!(subdag.blocks.len(), 1);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.blocks.len(), num_authorities);
            }
            for block_header in subdag.blocks.iter() {
                expected_stored_refs.push(block_header.reference());
                assert!(block_header.round() <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }

        // Check commits sent over consensus output channel is accurate
        let mut processed_subdag_index = 0;
        while let Ok(subdag) = receiver.try_recv() {
            assert_eq!(subdag.base, commits[processed_subdag_index].base);
            assert_eq!(subdag.reputation_scores_desc, vec![]);
            processed_subdag_index = subdag.commit_ref.index as usize;
            if processed_subdag_index == leaders.len() {
                break;
            }
        }
        assert_eq!(processed_subdag_index, leaders.len());

        verify_channel_empty(&mut receiver);

        // Check commits have been persisted to storage
        let last_commit = mem_store.read_last_commit().unwrap().unwrap();
        assert_eq!(
            last_commit.index(),
            commits.last().unwrap().commit_ref.index
        );
        let all_stored_commits = mem_store
            .scan_commits((0..=CommitIndex::MAX).into())
            .unwrap();
        assert_eq!(all_stored_commits.len(), leaders.len());
        let blocks_existence = mem_store
            .contains_block_headers(&expected_stored_refs)
            .unwrap();
        assert!(blocks_existence.iter().all(|exists| *exists));
    }

    #[tokio::test]
    async fn test_recover_and_send_commits() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let mem_store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            mem_store.clone(),
        )));
        let last_processed_commit_index = 0;
        let (sender, mut receiver) = unbounded_channel("consensus_output");

        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let mut observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), last_processed_commit_index),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule.clone(),
        );

        // Populate fully connected test blocks for round 0 ~ 10, authorities 0 ~ 3.
        let num_rounds = 10;
        let mut builder = DagBuilder::new(context.clone());
        builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        let leaders = builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Commit first batch of leaders (2) and "receive" the subdags as the
        // consumer of the consensus output channel.
        let expected_last_processed_index: usize = 2;
        let (mut created_commits, _missing_transactions_refs) = observer
            .handle_commit(
                leaders
                    .clone()
                    .into_iter()
                    .take(expected_last_processed_index)
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        // Check commits sent over consensus output channel is accurate
        let mut processed_subdag_index = 0;
        while let Ok(subdag) = receiver.try_recv() {
            tracing::info!("Processed subdag with index {}", subdag.commit_ref.index);
            assert_eq!(subdag.base, created_commits[processed_subdag_index].base);
            assert_eq!(subdag.reputation_scores_desc, vec![]);
            processed_subdag_index = subdag.commit_ref.index as usize;
            if processed_subdag_index == expected_last_processed_index {
                break;
            }
        }
        assert_eq!(processed_subdag_index, expected_last_processed_index);

        verify_channel_empty(&mut receiver);

        // Check last stored commit is correct
        let last_commit = mem_store.read_last_commit().unwrap().unwrap();
        assert_eq!(
            last_commit.index(),
            expected_last_processed_index as CommitIndex
        );

        // Handle next batch of leaders (1), these will be sent by consensus but not
        // "processed" by consensus output channel. Simulating something happened on
        // the consumer side where the commits were not persisted.
        created_commits.append(
            &mut observer
                .handle_commit(
                    leaders
                        .clone()
                        .into_iter()
                        .skip(expected_last_processed_index)
                        .collect::<Vec<_>>(),
                )
                .unwrap()
                .0,
        );

        let expected_last_sent_index = num_rounds as usize;
        while let Ok(subdag) = receiver.try_recv() {
            tracing::info!("{subdag} was sent but not processed by consumer");
            assert_eq!(subdag.base, created_commits[processed_subdag_index].base);
            assert_eq!(subdag.reputation_scores_desc, vec![]);
            processed_subdag_index = subdag.commit_ref.index as usize;
            if processed_subdag_index == expected_last_sent_index {
                break;
            }
        }
        assert_eq!(processed_subdag_index, expected_last_sent_index);

        verify_channel_empty(&mut receiver);

        // Check last stored commit is correct. We should persist the last commit
        // that was sent over the channel regardless of how the consumer handled
        // the commit on their end.
        let last_commit = mem_store.read_last_commit().unwrap().unwrap();
        assert_eq!(last_commit.index(), expected_last_sent_index as CommitIndex);

        // Re-create commit observer starting from index 2 which represents the
        // last processed index from the consumer over consensus output channel
        let _observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender, expected_last_processed_index as CommitIndex),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule,
        );

        // Check commits sent over consensus output channel is accurate starting
        // from last processed index of 2 and finishing at last sent index of 3.
        processed_subdag_index = expected_last_processed_index;
        while let Ok(subdag) = receiver.try_recv() {
            tracing::info!("Processed {subdag} on resubmission");
            assert_eq!(subdag.base, created_commits[processed_subdag_index].base);
            assert_eq!(subdag.reputation_scores_desc, vec![]);
            processed_subdag_index = subdag.commit_ref.index as usize;
            if processed_subdag_index == expected_last_sent_index {
                break;
            }
        }
        assert_eq!(processed_subdag_index, expected_last_sent_index);

        verify_channel_empty(&mut receiver);
    }

    #[tokio::test]
    async fn test_send_no_missing_commits() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let mem_store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            mem_store.clone(),
        )));
        let last_processed_commit_index = 0;
        let (sender, mut receiver) = unbounded_channel("consensus_output");

        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let mut observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), last_processed_commit_index),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule.clone(),
        );

        // Populate fully connected test blocks for round 0 ~ 10, authorities 0 ~ 3.
        let num_rounds = 10;
        let mut builder = DagBuilder::new(context.clone());
        builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        let leaders = builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Commit all of the leaders and "receive" the subdags as the consumer of
        // the consensus output channel.
        let expected_last_processed_index: usize = 10;
        let (created_commits, _missing_transactions_refs) =
            observer.handle_commit(leaders.clone()).unwrap();

        // Check commits sent over consensus output channel is accurate
        let mut processed_subdag_index = 0;
        while let Ok(subdag) = receiver.try_recv() {
            tracing::info!("Processed subdag with index {}", subdag.commit_ref.index);
            assert_eq!(subdag.base, created_commits[processed_subdag_index].base);
            assert_eq!(subdag.reputation_scores_desc, vec![]);
            processed_subdag_index = subdag.commit_ref.index as usize;
            if processed_subdag_index == expected_last_processed_index {
                break;
            }
        }
        assert_eq!(processed_subdag_index, expected_last_processed_index);

        verify_channel_empty(&mut receiver);

        // Check last stored commit is correct
        let last_commit = mem_store.read_last_commit().unwrap().unwrap();
        assert_eq!(
            last_commit.index(),
            expected_last_processed_index as CommitIndex
        );

        // Re-create commit observer starting from index 3 which represents the
        // last processed index from the consumer over consensus output channel
        let _observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender, expected_last_processed_index as CommitIndex),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule,
        );

        // No commits should be resubmitted as consensus store's last commit index
        // is equal to last processed index by consumer
        verify_channel_empty(&mut receiver);
    }

    /// After receiving all expected subdags, ensure channel is empty
    fn verify_channel_empty(receiver: &mut UnboundedReceiver<CommittedSubDag>) {
        match receiver.try_recv() {
            Ok(_) => {
                panic!("Expected the consensus output channel to be empty, but found more subdags.")
            }
            Err(e) => match e {
                tokio::sync::mpsc::error::TryRecvError::Empty => {}
                tokio::sync::mpsc::error::TryRecvError::Disconnected => {
                    panic!("The consensus output channel was unexpectedly closed.")
                }
            },
        }
    }
}
