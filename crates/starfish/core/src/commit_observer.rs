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
use tracing::{debug, info, instrument, warn};

use crate::{
    CommitConsumer, CommittedSubDag,
    block_header::{BlockHeaderAPI, VerifiedBlockHeader},
    commit::{
        CommitAPI, CommitIndex, PendingSubDag, TrustedCommit, load_pending_subdag_from_store,
    },
    commit_solidifier::CommitSolidifier,
    context::Context,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    leader_schedule::LeaderSchedule,
    linearizer::Linearizer,
    storage::Store,
    transaction_ref::GenericTransactionRef,
};

#[derive(Clone, Copy)]
pub(crate) enum CommittedSubDagSource {
    FastCommitSyncer,
    Consensus,
    Recover,
}

impl CommittedSubDagSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CommittedSubDagSource::FastCommitSyncer => "fast_commit_syncer",
            CommittedSubDagSource::Consensus => "consensus",
            CommittedSubDagSource::Recover => "recover",
        }
    }
}

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
    linearizer: Linearizer,
    /// Component to deterministically collect subdags for committed leaders.
    commit_solidifier: CommitSolidifier,
    /// An unbounded channel to send committed sub-dags to the consumer of
    /// consensus output.
    sender: UnboundedSender<CommittedSubDag>,
    /// Persistent storage for blocks, commits and other consensus data.
    store: Arc<dyn Store>,
    /// Dag state for direct access to block headers
    dag_state: Arc<RwLock<DagState>>,

    leader_schedule: Arc<LeaderSchedule>,
    /// Tracks the last commit index sent through the channel.
    /// Used to prevent resending already sent commits.
    last_sent_commit_index: CommitIndex,
}

impl CommitObserver {
    pub(crate) fn new(
        context: Arc<Context>,
        commit_consumer: CommitConsumer,
        dag_state: Arc<RwLock<DagState>>,
        store: Arc<dyn Store>,
        leader_schedule: Arc<LeaderSchedule>,
    ) -> Self {
        let last_processed_commit_index = commit_consumer.last_processed_commit_index;
        let mut observer = Self {
            linearizer: Linearizer::new(
                context.clone(),
                dag_state.clone(),
                leader_schedule.clone(),
            ),
            commit_solidifier: CommitSolidifier::new(dag_state.clone()),
            context,
            sender: commit_consumer.sender,
            store,
            dag_state,
            leader_schedule,
            last_sent_commit_index: last_processed_commit_index,
        };

        observer
            .recover_and_send_commits(last_processed_commit_index, CommittedSubDagSource::Recover);
        observer
    }

    /// Reinitialize the CommitObserver at a new commit index.
    /// Uses the existing `recover_and_send_commits` method which handles:
    /// - Recovering linearizer state (transaction ack tracker, traversed
    ///   headers)
    /// - Only re-sends commits that are > last_commit_index (none in this case)
    pub(crate) fn reinitialize(&mut self, last_commit_index: CommitIndex) {
        let now = Instant::now();

        // Clear linearizer state
        self.linearizer.clear_state();
        self.last_sent_commit_index = last_commit_index;

        // Reuse existing recovery logic - it won't resend commits since
        // they're all <= last_commit_index
        self.recover_and_send_commits(last_commit_index, CommittedSubDagSource::FastCommitSyncer);

        info!(
            "CommitObserver reinitialized at commit index {}, took {:?}",
            last_commit_index,
            now.elapsed()
        );
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
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn handle_committed_leaders(
        &mut self,
        committed_leaders: Vec<VerifiedBlockHeader>,
        source: CommittedSubDagSource,
    ) -> ConsensusResult<(
        Vec<PendingSubDag>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["CommitObserver::handle_committed_leaders"])
            .start_timer();

        let pending_sub_dags = self.linearizer.get_pending_sub_dags(committed_leaders);

        // First, add the commits to the commit solidifier to make sure that the data is
        // available. This function returns not only the just-created commits but also
        // any pending ones that'd become solid since the last commit.
        let (solid_sub_dags, missing_transactions) = self
            .commit_solidifier
            .try_get_solid_sub_dags(&pending_sub_dags);

        tracing::trace!("Missing committed transactions {missing_transactions:#?}");

        // Retrieve the transaction acknowledgment authors for the missing
        // transactions. This will be used by the transaction synchronizer to
        // fetch the missing transactions from the authorities that acknowledged
        // them.
        let missing_transaction_acknowledgers = self
            .linearizer
            .get_transaction_ack_authors(missing_transactions);

        self.finalize_and_send_solid_subdags(&pending_sub_dags, &solid_sub_dags, source)?;

        Ok((pending_sub_dags, missing_transaction_acknowledgers))
    }

    /// Evicts linearizer, updates dag_state with the last solid subdag and
    /// makes flush to storage.
    fn update_with_solid_subdags_and_flush(&mut self, solid_subdags: &[CommittedSubDag]) {
        if let Some(last_solid_subdag) = solid_subdags.last() {
            // Evict linearizer up to the last solid subdag leader round
            self.linearizer
                .evict_linearizer(last_solid_subdag.leader.round);
            // Update dag_state with the last solid subdag base
            self.dag_state
                .write()
                .update_last_solid_subdag_base(last_solid_subdag.base.clone());
        }
        self.dag_state.write().flush();
    }

    /// Finalizes solid subdags: updates state, flushes to storage, sends
    /// through channel, and reports metrics.
    pub(crate) fn finalize_and_send_solid_subdags(
        &mut self,
        pending_sub_dags: &[PendingSubDag],
        solid_subdags: &[CommittedSubDag],
        source: CommittedSubDagSource,
    ) -> ConsensusResult<()> {
        self.update_with_solid_subdags_and_flush(solid_subdags);
        self.send_sub_dags(solid_subdags, source)?;
        self.report_metrics(pending_sub_dags, solid_subdags, source);
        Ok(())
    }

    /// Builds a CommittedSubDag from a stored commit by loading transactions
    /// from dag_state and no headers. Returns None if any transactions are
    /// missing.
    fn build_committed_subdag_from_commit(
        &self,
        commit: &TrustedCommit,
        reputation_scores: Vec<(AuthorityIndex, u64)>,
    ) -> Option<CommittedSubDag> {
        let tx_refs = commit.committed_transactions();
        let transactions = match self
            .dag_state
            .read()
            .try_get_all_verified_transactions(&tx_refs)
        {
            Ok(transactions) => transactions,
            Err(missing_refs) => {
                warn!(
                    "Missing {} transactions for commit {}: {:?}",
                    missing_refs.len(),
                    commit.index(),
                    missing_refs,
                );
                return None;
            }
        };

        Some(CommittedSubDag::new(
            commit.leader(),
            vec![], // Empty headers for recovery
            commit.block_headers().to_vec(),
            transactions,
            commit.timestamp_ms(),
            commit.reference(),
            reputation_scores,
        ))
    }

    fn recover_and_send_commits(
        &mut self,
        last_processed_commit_index: CommitIndex,
        source: CommittedSubDagSource,
    ) {
        let last_commit = self
            .store
            .read_last_commit()
            .expect("Reading the last commit should not fail");
        let last_commit_index = last_commit
            .as_ref()
            .map(|commit| commit.index())
            .unwrap_or(0);
        assert!(
            last_commit_index >= last_processed_commit_index,
            "The consensus DB is behind the node DB!"
        );
        if last_commit_index == 0 {
            info!("No commits to recover in commit observer");
            return;
        }

        // Phase 1: Resend all solid committed sub-dags that haven't been processed
        self.resend_unprocessed_solid_commits(
            last_processed_commit_index,
            last_commit_index,
            source,
        );

        // Phase 2: Recover linearizer and solidifier state
        // Skip if fast sync is ongoing - block data may not be available and
        // this will be reinitialized by fast commit syncer anyway
        if self.store.read_fast_sync_ongoing() {
            info!("Skipping linearizer/solidifier recovery - fast sync ongoing");
            return;
        }
        self.recover_linearizer_and_solidifier_state(last_commit_index, source);
    }

    /// Recovers linearizer trackers from recent commits and seeds the
    /// commit solidifier with any unprocessed commits.
    fn recover_linearizer_and_solidifier_state(
        &mut self,
        last_commit_index: CommitIndex,
        source: CommittedSubDagSource,
    ) {
        let linearizer_recovery_start = last_commit_index
            .saturating_sub(self.context.protocol_config.gc_depth() * 2)
            .max(1);
        let solidifier_recovery_start = self.last_sent_commit_index.saturating_add(1);
        let recovery_start = linearizer_recovery_start.min(solidifier_recovery_start);

        let recovery_commits = self
            .store
            .scan_commits((recovery_start..=last_commit_index).into())
            .expect("Scanning commits should not fail");

        info!(
            "Recovering linearizer/solidifier state from {} commits (indices {}..={})",
            recovery_commits.len(),
            recovery_start,
            last_commit_index
        );

        self.commit_solidifier
            .set_last_solid_committed_index(self.last_sent_commit_index);

        let mut pending_for_solidifier = Vec::new();
        for commit in recovery_commits {
            // Recovery only needs headers/acks, so reputation scores are irrelevant here.
            let commit_index = commit.index();
            let pending_sub_dag =
                load_pending_subdag_from_store(self.store.as_ref(), commit, vec![]);

            if commit_index >= linearizer_recovery_start {
                // Rebuild traversed headers tracker
                self.linearizer
                    .record_traversed_headers(pending_sub_dag.headers.iter());

                // Recover transaction acknowledgments tracker state
                for ((round, authority_idx), transaction_acknowledgments) in
                    pending_sub_dag.transaction_acknowledgments().into_iter()
                {
                    self.linearizer.add_committed_transaction_acks(
                        round,
                        authority_idx,
                        transaction_acknowledgments,
                    );
                }
            }

            if commit_index >= solidifier_recovery_start {
                pending_for_solidifier.push(pending_sub_dag);
            }
        }

        if !pending_for_solidifier.is_empty() {
            let (solid_sub_dags, _missing) = self
                .commit_solidifier
                .try_get_solid_sub_dags(&pending_for_solidifier);
            self.finalize_and_send_solid_subdags(&[], &solid_sub_dags, source)
                .expect("We should successfully send solid commits during recovery");
        }
    }

    /// Sends committed sub-dags through the channel.
    /// Skips commits that have already been sent (index <=
    /// last_sent_commit_index). Returns the list of commit indices that
    /// were actually sent. Note: Caller is responsible for reporting
    /// metrics via `report_metrics`.
    fn send_sub_dags(
        &mut self,
        committed_subdags: &[CommittedSubDag],
        source: CommittedSubDagSource,
    ) -> ConsensusResult<Vec<CommitIndex>> {
        if committed_subdags.is_empty() {
            return Ok(Vec::new());
        }

        let mut sent_commit_indices = Vec::with_capacity(committed_subdags.len());

        for committed_subdag in committed_subdags.iter() {
            // Skip commits that have already been sent
            if committed_subdag.commit_ref.index <= self.last_sent_commit_index {
                debug!(
                    "Skipping already sent commit (index: {} <= last sent: {})",
                    committed_subdag.commit_ref.index, self.last_sent_commit_index
                );
                continue;
            }

            // Ensure commits are sent in order
            assert_eq!(
                committed_subdag.commit_ref.index,
                self.last_sent_commit_index + 1,
            );

            if let Err(err) = self.sender.send(committed_subdag.clone()) {
                warn!("Failed to send committed sub-dag, probably due to shutdown: {err:?}");
                return Err(ConsensusError::Shutdown);
            }

            info!(
                "Sending commit to execution (index: {}, leader {}, source: {})",
                committed_subdag.commit_ref,
                committed_subdag.leader,
                source.as_str()
            );

            self.last_sent_commit_index = committed_subdag.commit_ref.index;
            sent_commit_indices.push(committed_subdag.commit_ref.index);
        }

        Ok(sent_commit_indices)
    }

    /// Resends solid commits that haven't been processed by the consumer.
    /// Creates CommittedSubDag with empty headers (like fast sync).
    /// Note: it is possible that some commits in interval
    /// last_processed_commit_index+1.. last_commit_index might be not yet
    /// solid.
    fn resend_unprocessed_solid_commits(
        &mut self,
        last_processed_commit_index: CommitIndex,
        last_commit_index: CommitIndex,
        source: CommittedSubDagSource,
    ) {
        if last_processed_commit_index >= last_commit_index {
            info!("No unprocessed commits to resend");

            // Even though there are no commits to resend, we still need to initialize
            // last solid subdag in dag state so that fast sync knows where to start
            // fetching.
            if last_processed_commit_index > 0 {
                if let Some(commit) = self
                    .store
                    .scan_commits(
                        (last_processed_commit_index..=last_processed_commit_index).into(),
                    )
                    .ok()
                    .and_then(|commits| commits.into_iter().next())
                {
                    if let Some(committed_subdag) =
                        self.build_committed_subdag_from_commit(&commit, vec![])
                    {
                        self.update_with_solid_subdags_and_flush(&[committed_subdag]);
                    }
                }
            }

            return;
        }

        let unprocessed_commits = self
            .store
            .scan_commits((last_processed_commit_index + 1..=last_commit_index).into())
            .expect("Scanning commits should not fail");

        info!(
            "Resending {} unprocessed commits (indices {}..={})",
            unprocessed_commits.len(),
            last_processed_commit_index + 1,
            last_commit_index
        );

        let num_commits = unprocessed_commits.len();
        let mut committed_subdags = Vec::new();
        let mut expected_commit_index = self.last_sent_commit_index + 1;
        for (index, commit) in unprocessed_commits.into_iter().enumerate() {
            let commit_index = commit.index();
            assert_eq!(commit_index, expected_commit_index);
            expected_commit_index += 1;

            // Only the last commit carries scores for leader schedule consumers.
            let reputation_scores = if index == num_commits - 1 {
                self.leader_schedule
                    .leader_swap_table
                    .read()
                    .reputation_scores_desc
                    .clone()
            } else {
                vec![]
            };

            let Some(committed_subdag) =
                self.build_committed_subdag_from_commit(&commit, reputation_scores)
            else {
                info!(
                    "Stopping resend at commit {} due to missing transactions",
                    commit_index
                );
                break;
            };

            committed_subdags.push(committed_subdag);
        }

        // If we couldn't resend any commits, still initialize
        // last_solid_subdag_base from last_processed so fast sync
        // starts from the right position instead of index 0.
        if committed_subdags.is_empty() && last_processed_commit_index > 0 {
            if let Some(commit) = self
                .store
                .scan_commits((last_processed_commit_index..=last_processed_commit_index).into())
                .ok()
                .and_then(|commits| commits.into_iter().next())
            {
                if let Some(subdag) = self.build_committed_subdag_from_commit(&commit, vec![]) {
                    self.update_with_solid_subdags_and_flush(&[subdag]);
                }
            }
        }

        self.finalize_and_send_solid_subdags(&[], &committed_subdags, source)
            .expect("We should successfully send committed subdags during resend");
    }

    /// Get all missing transactions from pending subdags along with authorities
    /// who acknowledged them
    pub(crate) fn get_missing_transaction_data(
        &self,
    ) -> BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>> {
        let missing_refs = self.commit_solidifier.get_missing_transaction_data();
        self.linearizer
            .get_transaction_ack_authors(missing_refs.into_iter().collect())
    }

    fn report_metrics(
        &self,
        pending_sub_dags: &[PendingSubDag],
        committed_sub_dags: &[CommittedSubDag],
        source: CommittedSubDagSource,
    ) {
        let metrics = &self.context.metrics.node_metrics;
        let utc_now = self.context.clock.timestamp_utc_ms();
        let source_label = source.as_str();

        // First report block_header-related metrics for pending subdags
        for commit in pending_sub_dags {
            debug!(
                "Pending subdag {} with leader {} has {} blocks",
                commit.commit_ref,
                commit.leader,
                commit.headers.len()
            );

            metrics
                .last_committed_leader_round
                .set(commit.leader.round as i64);
            metrics
                .last_commit_index
                .set(commit.commit_ref.index as i64);
            metrics
                .blocks_per_commit_count
                .with_label_values(&[source_label])
                .observe(commit.headers.len() as f64);

            for header in &commit.headers {
                let latency_ms = utc_now
                    .checked_sub(header.timestamp_ms())
                    .unwrap_or_default();
                metrics
                    .block_header_commit_latency
                    .observe(Duration::from_millis(latency_ms).as_secs_f64());
            }
        }

        if !pending_sub_dags.is_empty() {
            self.context
                .metrics
                .node_metrics
                .sub_dags_per_commit_count
                .with_label_values(&[source_label])
                .observe(pending_sub_dags.len() as f64);
        }

        // Now report transaction-related metrics for committed subdags
        for commit in committed_sub_dags {
            debug!(
                "Committed subdag {} with leader {} has transactions from {} blocks",
                commit.commit_ref,
                commit.leader,
                commit.transactions.len()
            );

            // Report the actual number of committed transactions
            metrics
                .transactions_per_commit_count
                .with_label_values(&[source_label])
                .observe(
                    commit
                        .transactions
                        .iter()
                        .map(|x| x.transactions().len())
                        .sum::<usize>() as f64,
                );
            // Report the number of blocks committed with transactions per commit
            metrics
                .non_empty_blocks_per_commit_count
                .with_label_values(&[source_label])
                .observe(commit.transactions.len() as f64);
            // Report the number of blocks committed with transactions per authority
            for verified_transaction in &commit.transactions {
                let authority_index = verified_transaction.author();
                let hostname = &self.context.committee.authority(authority_index).hostname;
                metrics
                    .committed_non_empty_blocks_per_authority
                    .with_label_values(&[hostname])
                    .inc();
            }

            let tx_refs_for_committed_txs = commit
                .transactions
                .iter()
                .map(|tx| tx.transaction_ref())
                .collect::<Vec<_>>();

            // Read only cached block headers from storage for the transactions in the
            // commit. Headers are needed to calculate the latency of the
            // transactions. The metrics reflects only the latency for cached
            // block headers
            let headers_for_committed_txs = self
                .dag_state
                .read()
                .get_cached_block_headers_for_transaction_refs(&tx_refs_for_committed_txs)
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
        block_header::BlockRef,
        context::Context,
        dag_state::{DagState, DataSource},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    #[tokio::test]
    async fn test_handle_commit() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let mem_store = Arc::new(MemStore::new(context.clone()));
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

        let (commits, _missing_transactions_refs) = observer
            .handle_committed_leaders(leaders.clone(), CommittedSubDagSource::Consensus)
            .unwrap();

        // Check commits are returned by CommitObserver::handle_commit is accurate
        let mut expected_stored_refs: Vec<BlockRef> = vec![];
        for (idx, subdag) in commits.iter().enumerate() {
            info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            // Calculate expected timestamp using median of parents (NEW mode)
            let block_refs = leaders[idx]
                .ancestors()
                .iter()
                .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                .cloned()
                .collect::<Vec<_>>();
            let blocks = dag_state
                .read()
                .get_verified_block_headers(&block_refs)
                .into_iter()
                .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));
            let calculated_ts =
                crate::linearizer::median_timestamp_by_stake(&context, blocks).unwrap();

            let expected_ts = if idx == 0 {
                calculated_ts
            } else {
                calculated_ts.max(commits[idx - 1].timestamp_ms)
            };
            assert_eq!(expected_ts, subdag.timestamp_ms);
            if idx == 0 {
                // First subdag includes the leader block plus all ancestor blocks
                // of the leader minus the genesis round blocks
                assert_eq!(subdag.headers.len(), 1);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.headers.len(), num_authorities);
            }
            for block_ref in subdag.base.committed_header_refs.iter() {
                expected_stored_refs.push(*block_ref);
                assert!(block_ref.round <= leaders[idx].round());
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
        let mem_store = Arc::new(MemStore::new(context.clone()));
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

        // Commit the first batch of leaders (2) and "receive" the subdags as the
        // consumer of the consensus output channel.
        let expected_last_processed_index: usize = 2;
        let (mut created_commits, _missing_transactions_refs) = observer
            .handle_committed_leaders(
                leaders
                    .clone()
                    .into_iter()
                    .take(expected_last_processed_index)
                    .collect::<Vec<_>>(),
                CommittedSubDagSource::Consensus,
            )
            .unwrap();

        // Check commits sent over consensus output channel is accurate
        let mut processed_subdag_index = 0;
        while let Ok(subdag) = receiver.try_recv() {
            info!("Processed subdag with index {}", subdag.commit_ref.index);
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
                .handle_committed_leaders(
                    leaders
                        .into_iter()
                        .skip(expected_last_processed_index)
                        .collect::<Vec<_>>(),
                    CommittedSubDagSource::Consensus,
                )
                .unwrap()
                .0,
        );

        let expected_last_sent_index = num_rounds as usize;
        while let Ok(subdag) = receiver.try_recv() {
            info!("{subdag} was sent but not processed by consumer");
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
            context,
            CommitConsumer::new(sender, expected_last_processed_index as CommitIndex),
            dag_state,
            mem_store,
            leader_schedule,
        );

        // Check commits sent over consensus output channel is accurate starting
        // from last processed index of 2 and finishing at last sent index of 3.
        processed_subdag_index = expected_last_processed_index;
        while let Ok(subdag) = receiver.try_recv() {
            info!("Processed {subdag} on resubmission");
            let expected_base = &created_commits[processed_subdag_index].base;
            assert!(subdag.headers.is_empty());
            assert_eq!(subdag.leader, expected_base.leader);
            assert_eq!(subdag.commit_ref, expected_base.commit_ref);
            assert_eq!(
                subdag.committed_header_refs,
                expected_base.committed_header_refs
            );
            assert_eq!(subdag.timestamp_ms, expected_base.timestamp_ms);
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
        let mem_store = Arc::new(MemStore::new(context.clone()));
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
        let (created_commits, _missing_transactions_refs) = observer
            .handle_committed_leaders(leaders, CommittedSubDagSource::Consensus)
            .unwrap();

        // Check commits sent over consensus output channel is accurate
        let mut processed_subdag_index = 0;
        while let Ok(subdag) = receiver.try_recv() {
            info!("Processed subdag with index {}", subdag.commit_ref.index);
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
            context,
            CommitConsumer::new(sender, expected_last_processed_index as CommitIndex),
            dag_state,
            mem_store,
            leader_schedule,
        );

        // No commits should be resubmitted as consensus store's last commit index
        // is equal to last processed index by consumer
        verify_channel_empty(&mut receiver);
    }

    #[tokio::test]
    async fn test_recovery_resends_available_commits_and_tracks_missing_transactions() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let mem_store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            mem_store.clone(),
        )));
        let (sender, mut receiver) = unbounded_channel("consensus_output");

        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        // Populate fully connected test blocks for round 1 ~ 6, authorities 0 ~ 3.
        // Only add transactions for rounds 1-3 to simulate partial transaction
        // availability. Transactions for rounds 4-6 will be "missing" during recovery.
        let num_rounds = 6;
        let mut builder = DagBuilder::new(context.clone());
        builder.layers(1..=num_rounds).build();

        {
            let mut dag_state_guard = dag_state.write();
            dag_state_guard.accept_block_headers(
                builder.block_headers.values().cloned().collect(),
                DataSource::Test,
            );
            for (block_ref, transactions) in builder.transactions.iter() {
                if block_ref.round <= 3 {
                    dag_state_guard.add_transactions(transactions.clone(), DataSource::Test);
                }
            }
        }

        let mut observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule.clone(),
        );

        let leaders = builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // All 6 rounds should produce commits (one per leader round)
        assert_eq!(leaders.len(), num_rounds as usize);

        let _ = observer
            .handle_committed_leaders(leaders, CommittedSubDagSource::Consensus)
            .unwrap();

        // Drain the receiver to simulate consumer processing commits before crash.
        // We need to determine which commits have available transactions for resending.
        while let Ok(_subdag) = receiver.try_recv() {}

        let last_commit = mem_store.read_last_commit().unwrap().unwrap();
        let last_commit_index = last_commit.index();
        let commits = mem_store
            .scan_commits((1..=last_commit_index).into())
            .unwrap();

        // Verify we stored all commits
        assert_eq!(commits.len(), num_rounds as usize);

        // Determine which commit first has missing transactions.
        // Each commit references transactions from blocks up to the leader's round.
        // Since we only added transactions for rounds <= 3, commits including
        // blocks from round > 3 will have missing transactions.
        let mut first_missing_index = None;
        let mut expected_missing_refs = Vec::new();
        {
            let dag_state_guard = dag_state.read();
            for commit in &commits {
                let committed_refs = commit.committed_transactions();
                let tx_results = dag_state_guard.get_verified_transactions(&committed_refs);
                let missing_refs = committed_refs
                    .into_iter()
                    .zip(tx_results.iter())
                    .filter_map(|(tx_ref, tx)| tx.is_none().then_some(tx_ref))
                    .collect::<Vec<_>>();
                if !missing_refs.is_empty() {
                    first_missing_index = Some(commit.index());
                    expected_missing_refs = missing_refs;
                    break;
                }
            }
        }

        let first_missing_index =
            first_missing_index.expect("Expected at least one commit with missing transactions");
        // First commit with missing transactions should occur when commits start
        // including blocks from rounds > 3. With the fully connected DAG structure,
        // this happens at commit 4 or later depending on how blocks are ordered.
        assert!(
            first_missing_index > 1,
            "Expected first missing at index > 1, got {}",
            first_missing_index
        );
        assert!(
            first_missing_index <= num_rounds as CommitIndex,
            "Expected first missing within num_rounds, got {}",
            first_missing_index
        );

        // Re-create commit observer starting from index 0 to simulate full recovery.
        // Recovery should resend commits up to (but not including) the first commit
        // with missing transactions.
        let observer = CommitObserver::new(
            context,
            CommitConsumer::new(sender, 0),
            dag_state,
            mem_store,
            leader_schedule,
        );

        // Check commits sent over consensus output channel during recovery.
        // Recovery resends subdags with empty headers (like fast sync).
        let mut expected_index = 1u32;
        while let Ok(subdag) = receiver.try_recv() {
            // Recovery resends subdags with empty headers (like fast sync)
            assert!(subdag.headers.is_empty());
            assert_eq!(subdag.commit_ref.index, expected_index);

            // Verify subdag matches the original commit structure
            let original_commit = &commits[(expected_index - 1) as usize];
            assert_eq!(subdag.leader, original_commit.leader());
            assert_eq!(
                subdag.committed_header_refs,
                original_commit.block_headers()
            );

            expected_index += 1;
        }

        // Verify exactly (first_missing_index - 1) commits were resent
        let resent_count = expected_index - 1;
        assert_eq!(resent_count, first_missing_index - 1);
        assert!(
            resent_count > 0,
            "Expected at least one commit to be resent"
        );

        // Verify missing transactions are properly tracked with acknowledgers.
        // The linearizer recovers ack state from commits within gc_depth*2 window,
        // so all commits in this small test should have acknowledgers available.
        let missing = observer.get_missing_transaction_data();
        assert!(
            !missing.is_empty(),
            "Expected missing transactions to be tracked"
        );
        assert_eq!(
            missing.len(),
            expected_missing_refs.len(),
            "Mismatch in number of missing transactions"
        );

        for missing_ref in &expected_missing_refs {
            assert!(
                missing.contains_key(missing_ref),
                "Missing ref {:?} not tracked",
                missing_ref
            );
            // Each missing transaction should have acknowledgers recorded since
            // all commits are within the recovery window (gc_depth * 2).
            let acknowledgers = missing.get(missing_ref).unwrap();
            assert!(
                !acknowledgers.is_empty(),
                "No acknowledgers tracked for {:?}",
                missing_ref
            );
        }

        // Verify no additional subdags were sent
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

    /// Test consensus node recovery and linearizer state recovery across
    /// restarts.
    /// 1. Create blocks and commit some leaders
    /// 2. Restart node (clears traversed_headers_tracker)
    /// 3. During recovery, verify that traversed headers are recorded
    /// 4. Verify that new blocks can still successfully acknowledge and commit
    ///    transactions from blocks that existed before restart
    #[tokio::test]
    async fn test_recovery_restores_persistent_state_across_restart() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;

        // Create context with traversed headers tracking enabled
        let mut protocol_config =
            iota_protocol_config::ProtocolConfig::get_for_max_version_UNSAFE();
        protocol_config
            .set_consensus_commit_transactions_only_for_traversed_headers_for_testing(true);

        let (committee, _keypairs) =
            starfish_config::local_committee_and_keys(0, vec![1; num_authorities]);
        let metrics = crate::metrics::test_metrics();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let clock = Arc::new(crate::context::Clock::default());
        let context = Arc::new(Context::new(
            0,
            starfish_config::AuthorityIndex::new_for_test(0),
            committee,
            starfish_config::Parameters {
                db_path: temp_dir.keep(),
                ..Default::default()
            },
            protocol_config,
            metrics,
            clock,
        ));

        let mem_store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            mem_store.clone(),
        )));

        let (sender, mut receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        // Phase 1: Normal operation before restart
        let mut observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            mem_store.clone(),
            leader_schedule.clone(),
        );

        let mut builder = DagBuilder::new(context.clone());
        builder
            .layers(1..=6)
            .build()
            .persist_layers(dag_state.clone());

        let all_leaders = builder
            .leader_blocks(1..=6)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Commit first 3 leaders (rounds 1-3)
        // Each leader in the first 3 rounds has transactions from previous rounds
        let (_, _) = observer
            .handle_committed_leaders(all_leaders[0..3].to_vec(), CommittedSubDagSource::Consensus)
            .unwrap();

        // Count transactions: with 4 authorities and standard DAG, each commit includes
        // transactions from blocks 2 rounds back. For commits 1-3, expect transactions.
        let mut txs_before = 0;
        while let Ok(subdag) = receiver.try_recv() {
            txs_before += subdag.transactions.len();
        }
        assert!(
            txs_before > 0,
            "Should have committed transactions before restart"
        );

        // Simulate restart:
        // Create new observer starting from 0 to trigger recovery
        // This mimics what happens when the node restarts
        let mut observer_after_restart = CommitObserver::new(
            context,
            CommitConsumer::new(sender, 0),
            dag_state.clone(),
            mem_store,
            leader_schedule,
        );

        // Drain recovery commits
        while let Ok(_subdag) = receiver.try_recv() {}

        // Create new blocks (rounds 7-8) that will acknowledge blocks from before
        // restart
        builder.layers(7..=8).build().persist_layers(dag_state);

        let new_leaders = builder
            .leader_blocks(7..=8)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Process new blocks - they acknowledge transactions from rounds 5-6
        // plus transactions from recovered blocks (rounds 1-3)
        let (_commits_after, _) = observer_after_restart
            .handle_committed_leaders(new_leaders, CommittedSubDagSource::Consensus)
            .unwrap();

        // Count transactions from new commits: new leaders in rounds 7-8 will process
        // acknowledgments from all previous rounds including recovered state
        let mut txs_after = 0;
        while let Ok(subdag) = receiver.try_recv() {
            txs_after += subdag.transactions.len();
        }

        // Verify that txs_after significantly exceeds txs_before
        assert!(txs_after >= txs_before * 4,);
    }
}
