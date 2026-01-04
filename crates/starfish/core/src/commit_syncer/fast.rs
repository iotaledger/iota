// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use iota_metrics::spawn_logged_monitored_task;
use itertools::Itertools as _;
use parking_lot::RwLock;
use rand::{prelude::SliceRandom as _, rngs::ThreadRng};
use starfish_config::AuthorityIndex;
use tokio::{
    runtime::Handle,
    sync::oneshot,
    task::JoinSet,
    time::{MissedTickBehavior, sleep},
};
use tracing::{debug, info, warn};

use crate::{
    CommitConsumerMonitor, CommitIndex,
    block_header::VerifiedTransactions,
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, CommittedSubDag},
    commit_syncer::{
        CommitSyncType, CommitSyncerHandle, Inner, verify_transactions_with_transactions_refs,
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

    // Shared components wrapper.
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

    pub(crate) fn start(self) -> CommitSyncerHandle {
        let (tx_shutdown, rx_shutdown) = oneshot::channel();
        let schedule_task = spawn_logged_monitored_task!(self.schedule_loop(rx_shutdown,));
        CommitSyncerHandle {
            schedule_task,
            tx_shutdown,
        }
    }
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %self.inner.context.own_index)))]
    async fn schedule_loop(mut self, mut rx_shutdown: oneshot::Receiver<()>) {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Periodically, schedule new fetches if the node is falling behind.
                _ = interval.tick() => {
                    self.try_schedule_once();
                }
                // Handles results from fetch tasks.
                Some(result) = self.inflight_fetches.join_next(), if !self.inflight_fetches.is_empty() => {
                    if let Err(e) = result {
                        if e.is_panic() {
                            std::panic::resume_unwind(e.into_panic());
                        }
                        warn!("[{}] Fetch cancelled. FastCommitSyncer shutting down: {}", self.inner.sync_type.as_str(), e);
                        // If any fetch is cancelled or panicked, try to shutdown and exit the loop.
                        self.inflight_fetches.shutdown().await;
                        return;
                    }
                    let (target_end, committed_subdags) = result.unwrap();
                    self.handle_fetch_result(target_end, committed_subdags).await;
                }
                _ = &mut rx_shutdown => {
                    // Shutdown requested.
                    info!("[{}] FastCommitSyncer shutting down ...", self.inner.sync_type.as_str());
                    self.inflight_fetches.shutdown().await;
                    return;
                }
            }

            self.try_start_fetches();
        }
    }

    fn try_schedule_once(&mut self) {
        let quorum_commit_index = self.inner.commit_vote_monitor.quorum_commit_index();
        let local_commit_index = self.inner.dag_state.read().last_commit_index();

        // Skip scheduling if gap is small - CommitSyncer handles small gaps.
        let gap = quorum_commit_index.saturating_sub(local_commit_index);
        if gap <= self.inner.context.parameters.commit_sync_gap_threshold {
            return;
        }

        let metrics = &self.inner.context.metrics.node_metrics;
        metrics
            .commit_sync_quorum_index
            .set(quorum_commit_index as i64);
        metrics
            .commit_sync_local_index
            .set(local_commit_index as i64);
        let highest_handled_index = self.inner.commit_consumer_monitor.highest_handled_commit();
        let highest_scheduled_index = self.highest_scheduled_index.unwrap_or(0);
        // Update synced_commit_index periodically to make sure it is not smaller than
        // local commit index.
        self.synced_commit_index = self.synced_commit_index.max(local_commit_index);
        let unhandled_commits_threshold = self.unhandled_commits_threshold();

        // TODO: cleanup inflight fetches that are no longer needed.
        let fetch_after_index = self
            .synced_commit_index
            .max(self.highest_scheduled_index.unwrap_or(0));
        // When the node is falling behind, schedule pending fetches which will be
        // executed on later.
        let step = self
            .inner
            .sync_type
            .commit_sync_batch_size(&self.inner.context);

        info!(
            "[{}] Checking to schedule fetches: synced_commit_index={}, highest_handled_index={}, highest_scheduled_index={}, quorum_commit_index={}, unhandled_commits_threshold={}, fetch_after_index={}, step={}",
            self.inner.sync_type.as_str(),
            self.synced_commit_index,
            highest_handled_index,
            highest_scheduled_index,
            quorum_commit_index,
            unhandled_commits_threshold,
            fetch_after_index,
            step
        );

        for prev_end in (fetch_after_index..=quorum_commit_index).step_by(step as usize) {
            // Create range with inclusive start and end.
            let range_start = prev_end + 1;
            let range_end = prev_end + step;
            // Commit range is not fetched when [range_start, range_end] contains less
            // number of commits than the target batch size. This is to avoid
            // the cost of processing more and smaller batches. Block broadcast,
            // subscription and synchronization will help the node catchup.
            if quorum_commit_index < range_end {
                break;
            }
            // Pause scheduling new fetches when handling of commits is lagging.
            if highest_handled_index + unhandled_commits_threshold < range_end {
                warn!(
                    "[{}] Skip scheduling new commit fetches: consensus handler is lagging. highest_handled_index={}, highest_scheduled_index={}",
                    self.inner.sync_type.as_str(),
                    highest_handled_index,
                    highest_scheduled_index
                );
                break;
            }
            info!(
                "[{}] Scheduling fetch for commit range {}..={}",
                self.inner.sync_type.as_str(),
                range_start,
                range_end
            );
            self.pending_fetches
                .insert((range_start..=range_end).into());
            // quorum_commit_index should be non-decreasing, so highest_scheduled_index
            // should not decrease either.
            self.highest_scheduled_index = Some(range_end);
        }
    }

    async fn handle_fetch_result(
        &mut self,
        target_end: CommitIndex,
        committed_subdags: Vec<CommittedSubDag>,
    ) {
        assert!(!committed_subdags.is_empty());

        let total_transactions_size_bytes = committed_subdags
            .iter()
            .flat_map(|subdag| &subdag.transactions)
            .map(|txns| txns.serialized().len() as u64)
            .sum();

        let metrics = &self.inner.context.metrics.node_metrics;
        let sync_label = self.inner.sync_type.as_str();
        metrics
            .commit_sync_fetched_commits
            .with_label_values(&[sync_label])
            .inc_by(committed_subdags.len() as u64);
        metrics
            .commit_sync_total_fetched_transactions_size
            .with_label_values(&[sync_label])
            .inc_by(total_transactions_size_bytes);

        let (commit_start, commit_end) = (
            committed_subdags.first().unwrap().commit_ref.index,
            committed_subdags.last().unwrap().commit_ref.index,
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
                .insert((commit_start..=commit_end).into(), committed_subdags);
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

    fn try_start_fetches(&mut self) {
        // Cap parallel fetches based on configured limit and committee size, to avoid
        // overloading the network. Also when there are too many fetched block headers
        // that cannot be sent to Core before an earlier fetch has not finished,
        // reduce parallelism so the earlier fetch can retry on a better host and
        // succeed.
        let target_parallel_fetches = self
            .inner
            .context
            .parameters
            .commit_sync_parallel_fetches
            .min(self.inner.context.committee.size() * 2 / 3)
            .min(
                self.inner
                    .context
                    .parameters
                    .commit_sync_batches_ahead
                    .saturating_sub(self.fetched_ranges.len()),
            )
            .max(1);
        // Start new fetches if there are pending batches and available slots.
        loop {
            if self.inflight_fetches.len() >= target_parallel_fetches {
                break;
            }
            if !self.pending_fetches.is_empty() {
                info!(
                    "[{}] Pending fetches: {:?}, target parallel fetches: {}, inflight fetch number: {}",
                    self.inner.sync_type.as_str(),
                    self.pending_fetches,
                    target_parallel_fetches,
                    self.inflight_fetches.len()
                );
            }
            let Some(commit_range) = self.pending_fetches.pop_first() else {
                break;
            };
            self.inflight_fetches
                .spawn(Self::fetch_loop(self.inner.clone(), commit_range));
        }

        let metrics = &self.inner.context.metrics.node_metrics;
        let sync_label = self.inner.sync_type.as_str();
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

    // Retries fetching commits and block headers from available authorities, until
    // a request succeeds where at least a prefix of the commit range is
    // fetched. Returns the fetched commits and block headers referenced by the
    // commits.
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %inner.context.own_index)))]
    async fn fetch_loop(
        inner: Arc<Inner<C>>,
        commit_range: CommitRange,
    ) -> (CommitIndex, Vec<CommittedSubDag>) {
        // Individual request base timeout.
        const TIMEOUT: Duration = Duration::from_secs(10);
        // Max per-request timeout will be base timeout times a multiplier.
        // At the extreme, this means there will be 120s timeout to fetch
        // max_blocks_per_fetch blocks.
        const MAX_TIMEOUT_MULTIPLIER: u32 = 12;
        // timeout * max number of targets should be reasonably small, so the
        // system can adjust to slow network or large data sizes quickly.
        const MAX_NUM_TARGETS: usize = 24;
        let mut timeout_multiplier = 0;

        let _timer = inner
            .context
            .metrics
            .node_metrics
            .commit_sync_fetch_loop_latency
            .start_timer();
        info!(
            "[{}] Starting to fetch commits in {commit_range:?} ...",
            inner.sync_type.as_str()
        );
        loop {
            // Attempt to fetch commits and blocks through min(committee size,
            // MAX_NUM_TARGETS) peers.
            let mut target_authorities = inner
                .context
                .committee
                .authorities()
                .filter_map(|(i, _)| {
                    if i != inner.context.own_index {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect_vec();
            target_authorities.shuffle(&mut ThreadRng::default());
            target_authorities.truncate(MAX_NUM_TARGETS);
            // Increase timeout multiplier for each loop until MAX_TIMEOUT_MULTIPLIER.
            timeout_multiplier = (timeout_multiplier + 1).min(MAX_TIMEOUT_MULTIPLIER);
            let request_timeout = TIMEOUT * timeout_multiplier;

            let fetch_timeout = request_timeout * 2;
            // Try fetching from the selected target authority.
            for authority in target_authorities {
                match tokio::time::timeout(
                    fetch_timeout,
                    Self::fetch_once(
                        inner.clone(),
                        authority,
                        commit_range.clone(),
                        request_timeout,
                    ),
                )
                .await
                {
                    Ok(Ok(committed_subdags)) => {
                        info!(
                            "[{}] Finished fetching commits in {commit_range:?}",
                            inner.sync_type.as_str()
                        );
                        return (commit_range.end(), committed_subdags);
                    }
                    Ok(Err(e)) => {
                        let hostname = inner
                            .context
                            .committee
                            .authority(authority)
                            .hostname
                            .clone();
                        warn!(
                            "[{}] Failed to fetch {commit_range:?} from {hostname}: {}",
                            inner.sync_type.as_str(),
                            e
                        );
                        let error: &'static str = e.into();
                        inner
                            .context
                            .metrics
                            .node_metrics
                            .commit_sync_fetch_once_errors
                            .with_label_values(&[
                                hostname.as_str(),
                                error,
                                inner.sync_type.as_str(),
                            ])
                            .inc();
                    }
                    Err(_) => {
                        let hostname = inner
                            .context
                            .committee
                            .authority(authority)
                            .hostname
                            .clone();
                        warn!(
                            "[{}] Timed out fetching {commit_range:?} from {authority}",
                            inner.sync_type.as_str()
                        );
                        inner
                            .context
                            .metrics
                            .node_metrics
                            .commit_sync_fetch_once_errors
                            .with_label_values(&[
                                hostname.as_str(),
                                "FetchTimeout",
                                inner.sync_type.as_str(),
                            ])
                            .inc();
                    }
                }
            }
            // Avoid busy looping, by waiting for a while before retrying.
            sleep(TIMEOUT).await;
        }
    }

    // Fetches commits and transactions from a single authority.
    async fn fetch_once(
        inner: Arc<Inner<C>>,
        target_authority: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<Vec<CommittedSubDag>> {
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

    fn unhandled_commits_threshold(&self) -> CommitIndex {
        self.inner.context.parameters.commit_sync_batch_size
            * (self.inner.context.parameters.commit_sync_batches_ahead as u32)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn pending_fetches(&self) -> BTreeSet<CommitRange> {
        self.pending_fetches.clone()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn fetched_ranges(&self) -> BTreeMap<CommitRange, Vec<CommittedSubDag>> {
        self.fetched_ranges.clone()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn highest_scheduled_index(&self) -> Option<CommitIndex> {
        self.highest_scheduled_index
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn highest_fetched_commit_index(&self) -> CommitIndex {
        self.highest_fetched_commit_index
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn synced_commit_index(&self) -> CommitIndex {
        self.synced_commit_index
    }
}
