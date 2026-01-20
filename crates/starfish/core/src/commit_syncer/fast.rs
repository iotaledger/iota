// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::max,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use iota_metrics::spawn_logged_monitored_task;
use parking_lot::RwLock;
use rand::{prelude::SliceRandom as _, rngs::ThreadRng};
use starfish_config::AuthorityIndex;
use tokio::{runtime::Handle, sync::oneshot, task::JoinSet, time::MissedTickBehavior};
use tracing::{debug, info, warn};

use crate::{
    CommitConsumerMonitor, CommitIndex, VerifiedBlockHeader,
    block_header::VerifiedTransactions,
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, CommittedSubDag, TrustedCommit},
    commit_syncer::{
        CommitSyncType, CommitSyncerHandle, Inner, fetch_loop as shared_fetch_loop,
        handle_fetch_join_error, requeue_partial_range, schedule_commit_ranges,
        try_start_fetches as shared_try_start_fetches, verify_fetched_headers,
        verify_transactions_with_transactions_refs,
    },
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactionsV2},
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// Timeout for fetching block headers during close-to-quorum finalization.
const FETCH_HEADERS_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct FastCommitSyncer<C: NetworkClient> {
    // States shared by scheduler and fetch tasks.

    // Shared components wrapper.
    inner: Arc<Inner<C>>,

    // States only used by the scheduler.

    // Inflight requests to fetch commits from different authorities.
    inflight_fetches: JoinSet<(u32, Vec<TrustedCommit>, Vec<CommittedSubDag>)>,
    // Additional ranges of commits to fetch.
    pending_fetches: BTreeSet<CommitRange>,
    // Fetched commits and blocks by commit range.
    fetched_ranges: BTreeMap<CommitRange, (Vec<TrustedCommit>, Vec<CommittedSubDag>)>,
    // Highest commit index among inflight and pending fetches.
    // Used to determine the start of new ranges to be fetched.
    highest_scheduled_index: Option<CommitIndex>,
    // Highest index among fetched commits, after commits and blocks are verified.
    // Used for metrics.
    highest_fetched_commit_index: CommitIndex,
    // The commit index that is the max of highest local commit index and commit index inflight to
    // Core. Used to determine if fetched blocks can be sent to Core without gaps.
    synced_commit_index: CommitIndex,
    // Whether the syncer is in "close to quorum" mode, meaning remaining gap < batch size.
    // When this is true, the syncer will fetch block headers and transactions for cached rounds
    // before completing fast sync.
    close_to_quorum_mode: bool,
    // Whether the fast syncer has actually fetched any data. Close-to-quorum mode only
    // activates after this is true. Reset to false after reinitialization completes.
    has_fetched_data: bool,
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
            close_to_quorum_mode: false,
            has_fetched_data: false,
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
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Periodically, schedule new fetches if the node is falling behind.
                _ = interval.tick() => {
                    self.try_schedule_once();
                }
                // Handles results from fetch tasks.
                Some(result) = self.inflight_fetches.join_next(), if !self.inflight_fetches.is_empty() => {
                    if let Err(ref e) = result {
                        if e.is_panic() {
                            std::panic::resume_unwind(result.unwrap_err().into_panic());
                        }
                        if handle_fetch_join_error(e, &self.inner.sync_type) {
                            // If any fetch is cancelled or panicked, try to shutdown and exit the loop.
                            self.inflight_fetches.shutdown().await;
                            return;
                        }
                    }
                    let (target_end, commits, committed_subdags) = result.unwrap();
                    self.handle_fetch_result(target_end, commits, committed_subdags).await;
                }
                _ = &mut rx_shutdown => {
                    // Shutdown requested.
                    info!("[{}] FastCommitSyncer shutting down ...", self.inner.sync_type.as_str());
                    self.inflight_fetches.shutdown().await;
                    return;
                }
            }

            self.try_start_fetches();

            // Handle close-to-quorum mode: when all fetches complete and we're close
            // to the quorum, fetch block headers for a large enough number of rounds and
            // reinitialize.
            if self.close_to_quorum_mode
                && self.inflight_fetches.is_empty()
                && self.pending_fetches.is_empty()
                && self.fetched_ranges.is_empty()
            {
                info!(
                    "[{}] Close-to-quorum: all fetches complete, fetching headers for cached_rounds",
                    self.inner.sync_type.as_str()
                );

                match Self::fetch_headers_for_reinitialization(self.inner.clone()).await {
                    Ok(headers) => {
                        if let Err(e) = self
                            .inner
                            .core_thread_dispatcher
                            .reinitialize_components(headers)
                            .await
                        {
                            warn!(
                                "[{}] Failed to reinitialize components: {}",
                                self.inner.sync_type.as_str(),
                                e
                            );
                        } else {
                            info!(
                                "[{}] Components reinitialized, fast sync complete",
                                self.inner.sync_type.as_str()
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "[{}] Failed to fetch headers for cached rounds: {}",
                            self.inner.sync_type.as_str(),
                            e
                        );
                    }
                }

                // Reset state regardless of success - we've done what we can,
                // and regular syncer should take over now
                self.close_to_quorum_mode = false;
                self.has_fetched_data = false;

                // Exit the loop - fast sync is complete
                info!(
                    "[{}] Fast sync complete, exiting schedule loop",
                    self.inner.sync_type.as_str()
                );
                return;
            }
        }
    }

    fn try_schedule_once(&mut self) {
        let quorum_commit_index = self.inner.commit_vote_monitor.quorum_commit_index();
        let dag_state_commit_index = self.inner.dag_state.read().last_commit_index();
        let highest_handled_index = self.inner.commit_consumer_monitor.highest_handled_commit();
        let highest_scheduled_index = self.highest_scheduled_index.unwrap_or(0);
        let unhandled_commits_threshold = self.inner.unhandled_commits_threshold();
        let step = self
            .inner
            .sync_type
            .commit_sync_batch_size(&self.inner.context);

        // Skip scheduling depending on sync type and gap threshold.
        let gap = quorum_commit_index.saturating_sub(dag_state_commit_index);
        let should_schedule = self.has_fetched_data
            || self.inner.sync_type.should_schedule(
                gap,
                self.inner.context.parameters.commit_sync_gap_threshold,
                self.inner
                    .context
                    .protocol_config
                    .consensus_transaction_ref(),
            );

        if should_schedule {
            let metrics = &self.inner.context.metrics.node_metrics;
            metrics
                .commit_sync_quorum_index
                .set(quorum_commit_index as i64);
            metrics
                .commit_sync_local_index
                .set(dag_state_commit_index as i64);
            // Update synced_commit_index periodically to make sure it is not smaller than
            // local commit index.
            self.synced_commit_index = self.synced_commit_index.max(dag_state_commit_index);

            // TODO: cleanup inflight fetches that are no longer needed.
            let fetch_after_index = self
                .synced_commit_index
                .max(self.highest_scheduled_index.unwrap_or(0));

            debug!(
                "[{}] Checking to schedule fetches: synced_commit_index={}, highest_handled_index={}, highest_scheduled_index={}, quorum_commit_index={}, unhandled_commits_threshold={}, fetch_after_index={}",
                self.inner.sync_type.as_str(),
                self.synced_commit_index,
                highest_handled_index,
                highest_scheduled_index,
                quorum_commit_index,
                unhandled_commits_threshold,
                fetch_after_index,
            );

            // Schedule commit ranges for fetching using shared helper
            let schedule_result = schedule_commit_ranges(
                &self.inner,
                fetch_after_index,
                quorum_commit_index,
                highest_handled_index,
                unhandled_commits_threshold,
            );

            // Add scheduled ranges to pending fetches
            for range in schedule_result.ranges_scheduled {
                debug!(
                    "[{}] Scheduling fetch for commit range {}..={}",
                    self.inner.sync_type.as_str(),
                    range.start(),
                    range.end()
                );
                self.pending_fetches.insert(range);
            }

            // Update highest scheduled index
            if let Some(new_highest) = schedule_result.new_highest_scheduled {
                self.highest_scheduled_index = Some(new_highest);
            }
        }

        // Detect close-to-quorum mode: when remaining gap is less than a full batch.
        // Only activate if we've actually fetched data during this fast sync session.
        //
        // When close_to_quorum_mode is activated, the schedule_loop() will:
        // 1. Wait for all inflight/pending fetches to complete
        // 2. Fetch block headers for ~cached_rounds worth of commits
        // 3. Send ReinitializeComponents to core thread to properly initialize DAG
        //    state
        // 4. Reset fast sync state so regular syncer can take over
        if self.has_fetched_data && !self.close_to_quorum_mode {
            let current_fetch_after = self
                .synced_commit_index
                .max(self.highest_scheduled_index.unwrap_or(0));
            let remaining_gap = quorum_commit_index.saturating_sub(current_fetch_after);
            if remaining_gap > 0 && remaining_gap < step {
                let range_start = current_fetch_after + 1;
                let range_end = quorum_commit_index;
                debug!(
                    "[{}] Scheduling final partial fetch for commit range {}..={} (remaining_gap={})",
                    self.inner.sync_type.as_str(),
                    range_start,
                    range_end,
                    remaining_gap
                );
                self.pending_fetches
                    .insert((range_start..=range_end).into());
                self.highest_scheduled_index = Some(range_end);
            }
            if remaining_gap < step {
                self.close_to_quorum_mode = true;
                info!(
                    "[{}] Entering close-to-quorum mode: remaining_gap={}, step={}",
                    self.inner.sync_type.as_str(),
                    remaining_gap,
                    step
                );
            }
        }
    }

    async fn handle_fetch_result(
        &mut self,
        target_end: CommitIndex,
        commits: Vec<TrustedCommit>,
        committed_subdags: Vec<CommittedSubDag>,
    ) {
        assert!(!committed_subdags.is_empty());

        // Track that we have actually fetched data during this fast sync session.
        self.has_fetched_data = true;

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
        requeue_partial_range(&mut self.pending_fetches, commit_end, target_end);
        // Make sure synced_commit_index is up to date.
        self.synced_commit_index = self
            .synced_commit_index
            .max(self.inner.dag_state.read().last_commit_index());
        // Only add new blocks if at least some of them are not already synced.
        if self.synced_commit_index < commit_end {
            self.fetched_ranges.insert(
                (commit_start..=commit_end).into(),
                (commits, committed_subdags),
            );
        }
        // Try to process as many fetched blocks as possible.
        while let Some((fetched_commit_range, _)) = self.fetched_ranges.first_key_value() {
            // Only pop fetched_ranges if there is no gap with blocks already synced.
            // Note: start, end and synced_commit_index are all inclusive.
            let (fetched_commit_range, (commits, subdags)) =
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
                .add_subdags_from_fast_sync(commits, subdags)
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
        let inner = self.inner.clone();
        shared_try_start_fetches(
            &self.inner,
            &mut self.pending_fetches,
            self.fetched_ranges.len(),
            self.inflight_fetches.len(),
            self.synced_commit_index,
            |commit_range| {
                self.inflight_fetches
                    .spawn(Self::fetch_loop(inner.clone(), commit_range));
            },
        );
    }

    // Retries fetching commits and block headers from available authorities, until
    // a request succeeds where at least a prefix of the commit range is
    // fetched. Returns the fetched commits and block headers referenced by the
    // commits.
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %inner.context.own_index)))]
    async fn fetch_loop(
        inner: Arc<Inner<C>>,
        commit_range: CommitRange,
    ) -> (CommitIndex, Vec<TrustedCommit>, Vec<CommittedSubDag>) {
        let (end_index, (commits, committed_subdags)) =
            shared_fetch_loop(inner, commit_range, 2, Self::fetch_once).await;
        (end_index, commits, committed_subdags)
    }

    // Fetches commits and transactions from a single authority.
    async fn fetch_once(
        inner: Arc<Inner<C>>,
        target_authority: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<CommittedSubDag>)> {
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

        Ok((commits, committed_subdags))
    }

    /// Fetches block headers needed for component reinitialization from the
    /// network. This is called when close_to_quorum mode is active and all
    /// pending fetches complete. Fetches headers for the maximum of
    /// cached_rounds, gc_depth * 2, leader_schedule_window, and
    /// commits_since_schedule_update to satisfy DagState cache, linearizer,
    /// and leader schedule recovery requirements.
    async fn fetch_headers_for_reinitialization(
        inner: Arc<Inner<C>>,
    ) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
        // We need headers for three purposes:
        // 1. DagState cache: at least cached_rounds commits back
        // 2. Linearizer recovery: at least gc_depth * 2 commits back
        // 3. Leader schedule recovery: at least leader_schedule_window commits back, or
        //    all commits since the last stored commit info
        //    (commits_since_schedule_update)
        // Fetch the maximum to satisfy all requirements
        let cached_rounds = inner.context.parameters.dag_state_cached_rounds;
        let gc_depth = inner.context.protocol_config.gc_depth();
        let leader_schedule_window = crate::leader_schedule::CONSENSUS_COMMITS_PER_SCHEDULE as u32;
        // Get block refs from recent commits stored during fast sync
        // TODO: The commits might not yet stored, but only fetched and pending
        // processing.
        let (commits_since_schedule_update, block_refs) = {
            let dag_state = inner.dag_state.read();
            let last_commit_index = dag_state.last_commit_index();
            let last_commit_info_index = dag_state.last_commit_info_index();
            let commits_since_schedule_update =
                last_commit_index.saturating_sub(last_commit_info_index);
            let num_commits = max(
                commits_since_schedule_update,
                max(leader_schedule_window, max(cached_rounds, gc_depth * 2)),
            );
            let block_refs = dag_state.get_block_refs_for_recent_commits(num_commits);
            (commits_since_schedule_update, block_refs)
        };

        let max_headers_per_fetch = inner.context.parameters.max_headers_per_commit_sync_fetch;

        info!(
            "[{}] Fetching {} block headers for reinitialization (cached_rounds={}, gc_depth*2={}, leader_schedule_window={}, commits_since_schedule_update={})",
            inner.sync_type.as_str(),
            block_refs.len(),
            cached_rounds,
            gc_depth * 2,
            leader_schedule_window,
            commits_since_schedule_update
        );

        // Shuffle target authorities for load balancing
        let mut target_authorities: Vec<_> = inner
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
            .collect();
        target_authorities.shuffle(&mut ThreadRng::default());

        // Fetch headers in chunks to avoid overwhelming the network
        let mut all_headers = Vec::new();
        for chunk in block_refs.chunks(max_headers_per_fetch) {
            let chunk_refs: Vec<_> = chunk.to_vec();

            // Try fetching from different authorities until successful
            let mut fetched = false;
            for &authority in &target_authorities {
                match tokio::time::timeout(
                    FETCH_HEADERS_TIMEOUT,
                    inner.network_client.fetch_block_headers(
                        authority,
                        chunk_refs.clone(),
                        vec![],
                        FETCH_HEADERS_TIMEOUT,
                    ),
                )
                .await
                {
                    Ok(Ok(serialized_headers)) => {
                        // Verify headers match requested refs
                        match verify_fetched_headers(authority, &chunk_refs, serialized_headers) {
                            Ok(headers) => {
                                info!(
                                    "[{}] Fetched {} headers from authority {}",
                                    inner.sync_type.as_str(),
                                    headers.len(),
                                    authority
                                );
                                all_headers.extend(headers);
                                fetched = true;
                                break;
                            }
                            Err(e) => {
                                warn!(
                                    "[{}] Failed to verify headers from {}: {}",
                                    inner.sync_type.as_str(),
                                    authority,
                                    e
                                );
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "[{}] Failed to fetch headers from {}: {}",
                            inner.sync_type.as_str(),
                            authority,
                            e
                        );
                    }
                    Err(_) => {
                        warn!(
                            "[{}] Timed out fetching headers from {}",
                            inner.sync_type.as_str(),
                            authority
                        );
                    }
                }
            }

            if !fetched {
                return Err(ConsensusError::FailedToFetchBlockHeaders {
                    num_requested: chunk_refs.len(),
                });
            }
        }

        info!(
            "[{}] Successfully fetched {} total block headers for reinitialization",
            inner.sync_type.as_str(),
            all_headers.len()
        );

        Ok(all_headers)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn pending_fetches(&self) -> BTreeSet<CommitRange> {
        self.pending_fetches.clone()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn fetched_ranges(&self) -> BTreeMap<CommitRange, (Vec<TrustedCommit>, Vec<CommittedSubDag>)> {
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
