// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{runtime::Handle, task::JoinSet};
use tracing::{debug, info};

use crate::{
    CommitConsumerMonitor, CommitIndex,
    block_header::VerifiedTransactions,
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, CommittedSubDag},
    commit_syncer::{
        CommitSyncType, CommitSyncer, Inner, verify_transactions_with_transactions_refs,
    },
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactionsV2},
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

pub(crate) struct FastCommitSyncer<C: NetworkClient> {
    // States shared by scheduler and fetch tasks.

    // Shared components' wrapper.
    inner: Arc<Inner<C>>,

    // States only used by the scheduler.

    // Inflight requests to fetch commits from different authorities.
    inflight_fetches: JoinSet<(u32, Vec<CommittedSubDag>)>,
    // Additional ranges of commits to fetch.
    pending_fetches: BTreeSet<CommitRange>,
    // Fetched commits and blocks by commit range.
    fetched_ranges: BTreeMap<CommitRange, Vec<CommittedSubDag>>,
    // Highest commit index among inflight and pending fetches.
    // Used to determine the start of new ranges to be fetched.
    highest_scheduled_index: Option<CommitIndex>,
    // Highest index among fetched commits, after commits and blocks are verified.
    // Used for metrics.
    highest_fetched_commit_index: CommitIndex,
    // The commit index that is the max of highest local commit index and commit index inflight to
    // Core. Used to determine if fetched blocks can be sent to Core without gaps.
    synced_commit_index: CommitIndex,
}

impl<C: NetworkClient> FastCommitSyncer<C> {
    pub(crate) fn new(
        context: Arc<Context>,
        core_thread_dispatcher: Arc<dyn CoreThreadDispatcher>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        commit_consumer_monitor: Arc<CommitConsumerMonitor>,
        network_client: Arc<C>,
        block_verifier: Arc<dyn BlockVerifier>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Self {
        let inner = Arc::new(Inner {
            context,
            core_thread_dispatcher,
            commit_vote_monitor,
            commit_consumer_monitor,
            network_client,
            block_verifier,
            dag_state,
            sync_type: CommitSyncType::Fast,
        });
        let synced_commit_index = inner.dag_state.read().last_commit_index();
        FastCommitSyncer {
            inner,
            inflight_fetches: JoinSet::new(),
            pending_fetches: BTreeSet::new(),
            fetched_ranges: BTreeMap::new(),
            highest_scheduled_index: None,
            highest_fetched_commit_index: 0,
            synced_commit_index,
        }
    }
}

#[async_trait::async_trait]
impl<C: NetworkClient> CommitSyncer<C> for FastCommitSyncer<C> {
    type FetchedData = Vec<CommittedSubDag>;
    fn inner(&self) -> &Arc<Inner<C>> {
        &self.inner
    }

    fn inflight_fetches(&mut self) -> &mut JoinSet<(u32, Self::FetchedData)> {
        &mut self.inflight_fetches
    }

    fn inflight_fetches_len(&self) -> usize {
        self.inflight_fetches.len()
    }

    fn pending_fetches_len(&self) -> usize {
        self.pending_fetches.len()
    }

    fn highest_scheduled_index(&mut self) -> &mut Option<CommitIndex> {
        &mut self.highest_scheduled_index
    }

    fn synced_commit_index(&mut self) -> &mut CommitIndex {
        &mut self.synced_commit_index
    }

    fn pending_fetches(&mut self) -> &mut BTreeSet<CommitRange> {
        &mut self.pending_fetches
    }

    fn fetched_ranges(&mut self) -> &mut BTreeMap<CommitRange, Self::FetchedData> {
        &mut self.fetched_ranges
    }

    #[cfg(test)]
    fn highest_fetched_commit_index(&self) -> CommitIndex {
        self.highest_fetched_commit_index
    }

    async fn handle_fetch_result(
        &mut self,
        target_end: CommitIndex,
        fetched_data_set: Self::FetchedData,
    ) {
        assert!(!fetched_data_set.is_empty());

        let total_transactions_size_bytes = fetched_data_set
            .iter()
            .flat_map(|subdag| &subdag.transactions)
            .map(|txns| txns.serialized().len() as u64)
            .sum();

        let metrics = &self.inner.context.metrics.node_metrics;
        let sync_label = self.inner.sync_type.as_str();
        metrics
            .commit_sync_fetched_commits
            .with_label_values(&[sync_label])
            .inc_by(fetched_data_set.len() as u64);
        metrics
            .commit_sync_total_fetched_transactions_size
            .with_label_values(&[sync_label])
            .inc_by(total_transactions_size_bytes);

        let (commit_start, commit_end) = (
            fetched_data_set.first().unwrap().commit_ref.index,
            fetched_data_set.last().unwrap().commit_ref.index,
        );
        self.highest_fetched_commit_index = self.highest_fetched_commit_index.max(commit_end);
        metrics
            .commit_sync_highest_fetched_index
            .with_label_values(&[sync_label])
            .set(self.highest_fetched_commit_index as i64);

        // Allow returning partial results, and try fetching the rest separately.
        if commit_end < target_end {
            self.pending_fetches
                .insert((commit_end + 1..=target_end).into());
        }
        // Make sure synced_commit_index is up to date.
        self.synced_commit_index = self
            .synced_commit_index
            .max(self.inner.dag_state.read().last_commit_index());
        // Only add new blocks if at least some of them are not already synced.
        if self.synced_commit_index < commit_end {
            self.fetched_ranges
                .insert((commit_start..=commit_end).into(), fetched_data_set);
        }
        // Try to process as many fetched blocks as possible.
        while let Some((fetched_commit_range, _subdags)) = self.fetched_ranges.first_key_value() {
            // Only pop fetched_ranges if there is no gap with blocks already synced.
            // Note: start, end and synced_commit_index are all inclusive.
            let (fetched_commit_range, subdags) =
                if fetched_commit_range.start() <= self.synced_commit_index + 1 {
                    self.fetched_ranges.pop_first().unwrap()
                } else {
                    // Found gap between earliest fetched block and latest synced block,
                    // so not sending additional blocks to Core.
                    metrics
                        .commit_sync_gap_on_processing
                        .with_label_values(&[sync_label])
                        .inc();
                    break;
                };
            // Avoid sending to Core a whole batch of already synced blocks.
            if fetched_commit_range.end() <= self.synced_commit_index {
                continue;
            }

            debug!(
                "[{}] Fetched {} subdags with transactions for commit range {:?}",
                sync_label,
                subdags.len(),
                fetched_commit_range,
            );

            // If core thread cannot handle the incoming blocks, it is ok to block here.

            if let Err(e) = self
                .inner
                .core_thread_dispatcher
                .add_subdags_from_fast_sync(subdags)
                .await
            {
                info!(
                    "[{}] Failed to dispatch subdags to core, shutting down: {}",
                    sync_label, e
                );
                return;
            }

            // Once subdags are sent to Core, ratchet up synced_commit_index
            self.synced_commit_index = self.synced_commit_index.max(fetched_commit_range.end());
        }

        metrics
            .commit_sync_inflight_fetches
            .with_label_values(&[sync_label])
            .set(self.inflight_fetches.len() as i64);
        metrics
            .commit_sync_pending_fetches
            .with_label_values(&[sync_label])
            .set(self.pending_fetches.len() as i64);
        metrics
            .commit_sync_highest_synced_index
            .with_label_values(&[sync_label])
            .set(self.synced_commit_index as i64);
    }

    async fn fetch_once(
        inner: Arc<Inner<C>>,
        target_authority: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<Self::FetchedData> {
        let _timer = inner
            .context
            .metrics
            .node_metrics
            .commit_sync_fetch_once_latency
            .with_label_values(&[inner.sync_type.as_str()])
            .start_timer();
        assert!(inner.context.protocol_config.consensus_transaction_ref());

        // 1. Fetch commits, voting headers, and transactions in the commit range from
        //    the target authority. Each transaction is serialized as
        //    SerializedTransactionsV2 which includes the TransactionRef.
        let (serialized_commits, serialized_proof_for_last_commit, serialized_transactions) = inner
            .network_client
            .fetch_commits_and_transactions(target_authority, commit_range.clone(), timeout)
            .await?;

        // 2. Verify the response contains block headers that can certify the last
        //    returned commit, and the returned commits are chained by digest,
        // so earlier commits are certified as well.
        let commits = Handle::current()
            .spawn_blocking({
                let inner = inner.clone();
                move || {
                    inner.verify_commits(
                        target_authority,
                        commit_range,
                        serialized_commits,
                        serialized_proof_for_last_commit,
                    )
                }
            })
            .await
            .expect("Spawn blocking should not fail")?;

        // 3. Collect all committed transaction block refs from commits
        let mut committed_tx_refs: BTreeSet<TransactionRef> = commits
            .iter()
            .flat_map(|c| c.committed_transactions())
            .filter_map(|gen_tr_ref| gen_tr_ref.expect_transaction_ref().ok())
            .collect();

        // 4. Process fetched transactions. Each serialized_transaction is a
        //    SerializedTransactionsV2 containing both the TransactionRef and the actual
        //    transaction data.
        let mut fetched_transactions = BTreeMap::new();
        for serialized_transaction in serialized_transactions {
            if let Ok(tx_v2) = bcs::from_bytes::<SerializedTransactionsV2>(&serialized_transaction)
            {
                let transaction_ref = tx_v2.transaction_ref;
                if !committed_tx_refs.contains(&transaction_ref) {
                    return Err(ConsensusError::UnexpectedTransactionForCommit {
                        peer: target_authority,
                        received: GenericTransactionRef::TransactionRef(transaction_ref),
                    });
                }
                fetched_transactions.insert(
                    GenericTransactionRef::TransactionRef(transaction_ref),
                    tx_v2.serialized_transactions,
                );
                committed_tx_refs.remove(&transaction_ref);
            } else {
                debug!(
                    "[{}] Failed to deserialize SerializedTransactionsV2: {:?}",
                    inner.sync_type.as_str(),
                    serialized_transaction
                );
                continue;
            }
        }

        // Check if any committed transactions were not fetched (committed_tx_refs
        // should be empty now)
        if !committed_tx_refs.is_empty() {
            // TODO: create subdags for prefix of commits
            return Err(ConsensusError::FetchedTransactionsMismatch {
                peer: target_authority,
                expected: committed_tx_refs.len() + fetched_transactions.len(),
                received: fetched_transactions.len(),
            });
        }

        // 5. Verify transactions
        let mut transactions_map = if !fetched_transactions.is_empty() {
            Handle::current()
                .spawn_blocking({
                    let context = inner.context.clone();

                    move || {
                        verify_transactions_with_transactions_refs(
                            &context,
                            target_authority,
                            fetched_transactions,
                        )
                    }
                })
                .await
                .expect("Spawn blocking should not fail")?
        } else {
            BTreeMap::new()
        };

        // 6. Now create the CommittedSubDags with the fetched transactions.
        // For fast commit sync, we use block headers refs and reputation scores from
        // the commit.
        let mut committed_subdags = Vec::new();
        for commit in &commits {
            // Get block headers from the commit
            let committed_header_refs = commit.block_headers().to_vec();

            // Get reputation scores from the commit
            let reputation_scores = commit.reputation_scores().to_vec();

            // Collect transactions for this commit
            let commit_transactions: Vec<VerifiedTransactions> = commit
                .committed_transactions()
                .iter()
                .filter_map(|tx_ref| transactions_map.remove(tx_ref))
                .collect();

            committed_subdags.push(CommittedSubDag::new(
                commit.leader(),
                vec![], // headers - VerifiedBlockHeader, we don't have these in fast sync
                committed_header_refs,
                commit_transactions,
                commit.timestamp_ms(),
                commit.reference(),
                reputation_scores,
            ));
        }

        Ok(committed_subdags)
    }
}
