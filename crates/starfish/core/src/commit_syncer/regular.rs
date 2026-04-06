// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use futures::{StreamExt as _, stream::FuturesOrdered};
use iota_metrics::spawn_logged_monitored_task;
use itertools::Itertools as _;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{
    runtime::Handle,
    sync::oneshot,
    task::JoinSet,
    time::{MissedTickBehavior, sleep},
};
use tracing::{debug, error, info, warn};

use crate::{
    CommitConsumerMonitor, CommitIndex,
    block_header::BlockRef,
    block_verifier::BlockVerifier,
    commit::{CertifiedCommit, CertifiedCommits, CommitAPI as _, CommitRange},
    commit_syncer::{
        CommitSyncType, CommitSyncerHandle, Inner, fetch_loop as shared_fetch_loop,
        handle_fetch_join_error, requeue_partial_range, schedule_commit_ranges,
        try_start_fetches as shared_try_start_fetches, verify_fetched_headers,
        verify_transactions_with_headers, verify_transactions_with_transactions_refs,
    },
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactionsV1, SerializedTransactionsV2},
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _},
};

pub(crate) struct RegularCommitSyncer<C: NetworkClient> {
    // States shared by scheduler and fetch tasks.

    // Shared components' wrapper.
    inner: Arc<Inner<C>>,

    // States only used by the scheduler.

    // Inflight requests to fetch commits from different authorities.
    inflight_fetches: JoinSet<(u32, CertifiedCommits)>,
    // Additional ranges of commits to fetch.
    pending_fetches: BTreeSet<CommitRange>,
    // Fetched commits and blocks by commit range.
    fetched_ranges: BTreeMap<CommitRange, CertifiedCommits>,
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

impl<C: NetworkClient> RegularCommitSyncer<C> {
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
            sync_type: CommitSyncType::Regular,
        });
        let synced_commit_index = inner.dag_state.read().last_commit_index();
        RegularCommitSyncer {
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
                    let (target_end, commits) = result.unwrap();
                    self.handle_fetch_result(target_end, commits).await;
                }
                _ = &mut rx_shutdown => {
                    // Shutdown requested.
                    info!("[{}] CommitSyncer shutting down ...", self.inner.sync_type.as_str());
                    self.inflight_fetches.shutdown().await;
                    return;
                }
            }

            self.try_start_fetches();
        }
    }

    fn try_schedule_once(&mut self) {
        let quorum_commit_index = self.inner.commit_vote_monitor.quorum_commit_index();
        let dag_state_commit_index = self.inner.dag_state.read().last_commit_index();

        // Skip scheduling depending on sync type and gap threshold.
        let gap = quorum_commit_index.saturating_sub(dag_state_commit_index);
        if !self.inner.sync_type.should_schedule(
            gap,
            self.inner.context.parameters.commit_sync_gap_threshold,
            self.inner
                .context
                .protocol_config
                .consensus_fast_commit_sync()
                && self.inner.context.parameters.enable_fast_commit_syncer,
        ) {
            return;
        }

        let metrics = &self.inner.context.metrics.node_metrics;
        metrics
            .commit_sync_quorum_index
            .set(quorum_commit_index as i64);
        metrics
            .commit_sync_local_index
            .set(dag_state_commit_index as i64);
        let highest_handled_index = self.inner.commit_consumer_monitor.highest_handled_commit();
        let highest_scheduled_index = self.highest_scheduled_index.unwrap_or(0);
        // Update synced_commit_index periodically to make sure it is not smaller than
        // local commit index.
        self.synced_commit_index = self.synced_commit_index.max(dag_state_commit_index);
        let unhandled_commits_threshold = self.inner.unhandled_commits_threshold();

        // TODO: cleanup inflight fetches that are no longer needed.
        let fetch_after_index = self
            .synced_commit_index
            .max(self.highest_scheduled_index.unwrap_or(0));

        info!(
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
            info!(
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

    async fn handle_fetch_result(
        &mut self,
        target_end: CommitIndex,
        certified_commits: CertifiedCommits,
    ) {
        assert!(!certified_commits.commits().is_empty());

        let (total_blocks_fetched, total_headers_size_bytes, total_transactions_size_bytes) =
            certified_commits.commits().iter().fold(
                (0, 0, 0),
                |(blocks, header_bytes, transaction_bytes), c| {
                    (
                        blocks + c.block_headers().len(),
                        header_bytes
                            + c.block_headers()
                                .iter()
                                .map(|b| b.serialized().len())
                                .sum::<usize>() as u64,
                        transaction_bytes
                            + c.transactions()
                                .iter()
                                .map(|b| b.serialized().len())
                                .sum::<usize>() as u64,
                    )
                },
            );

        let metrics = &self.inner.context.metrics.node_metrics;
        let sync_label = self.inner.sync_type.as_str();
        metrics
            .commit_sync_fetched_commits
            .with_label_values(&[sync_label])
            .inc_by(certified_commits.commits().len() as u64);
        metrics
            .commit_sync_fetched_block_headers
            .inc_by(total_blocks_fetched as u64);
        metrics
            .commit_sync_total_fetched_block_headers_size
            .inc_by(total_headers_size_bytes);
        metrics
            .commit_sync_total_fetched_transactions_size
            .with_label_values(&[sync_label])
            .inc_by(total_transactions_size_bytes);

        let (commit_start, commit_end) = (
            certified_commits.commits().first().unwrap().index(),
            certified_commits.commits().last().unwrap().index(),
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
            self.fetched_ranges
                .insert((commit_start..=commit_end).into(), certified_commits);
        }
        // Try to process as many fetched blocks as possible.
        while let Some((fetched_commit_range, _commits)) = self.fetched_ranges.first_key_value() {
            // Only pop fetched_ranges if there is no gap with blocks already synced.
            // Note: start, end and synced_commit_index are all inclusive.
            let (fetched_commit_range, commits) =
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
                "[{}] Fetched certified block headers for commit range {:?}: {}",
                self.inner.sync_type.as_str(),
                fetched_commit_range,
                commits
                    .commits()
                    .iter()
                    .flat_map(|c| c.block_headers())
                    .map(|b| b.reference().to_string())
                    .join(","),
            );

            // Compare transactions available in CertifiedCommits with
            // committed_transactions in TrustedCommits
            let mut expected_transactions = BTreeSet::new();
            let mut available_transactions = BTreeSet::new();

            for certified_commit in commits.commits() {
                // Collect committed_transactions from the TrustedCommit
                for gen_tx_ref in certified_commit.committed_transactions() {
                    expected_transactions.insert(gen_tx_ref);
                }

                // Collect available transactions from VerifiedTransactions
                for verified_txns in certified_commit.transactions() {
                    let gen_tx_ref = if self
                        .inner
                        .context
                        .protocol_config
                        .consensus_fast_commit_sync()
                    {
                        GenericTransactionRef::TransactionRef(verified_txns.transaction_ref())
                    } else {
                        let Some(block_ref) = verified_txns.block_ref() else {
                            error!(
                                "block_ref unavailable for transactions in non-transaction-ref path"
                            );
                            continue;
                        };
                        GenericTransactionRef::BlockRef(block_ref)
                    };
                    available_transactions.insert(gen_tx_ref);
                }
            }

            // Find missing transactions
            let missing_transactions: Vec<_> = expected_transactions
                .difference(&available_transactions)
                .collect();

            if !missing_transactions.is_empty() {
                warn!(
                    "[{}] Missing {} out of {} transactions after fetching commit range {:?}: {:?}",
                    sync_label,
                    missing_transactions.len(),
                    expected_transactions.len(),
                    fetched_commit_range,
                    missing_transactions,
                );
            }

            // If core thread cannot handle the incoming blocks, it is ok to block here.
            // Also it is possible to have missing ancestors because an equivocating
            // validator may produce blocks that are not included in commits but
            // are ancestors to other blocks. Synchronizer is needed to fill in
            // the missing ancestors in this case.
            match self
                .inner
                .core_thread_dispatcher
                .add_certified_commits(commits)
                .await
            {
                Ok((missing_headers, missing_committed_txns)) => {
                    if !missing_headers.is_empty() {
                        warn!(
                            "[{}] Fetched block headers have missing ancestors: {:?} for commit range {:?}",
                            sync_label, missing_headers, fetched_commit_range
                        );
                    }
                    for block_ref in missing_headers {
                        let hostname = &self
                            .inner
                            .context
                            .committee
                            .authority(block_ref.author)
                            .hostname;
                        metrics
                            .commit_sync_fetch_missing_block_headers
                            .with_label_values(&[hostname])
                            .inc();
                    }
                    if !missing_committed_txns.is_empty() {
                        warn!(
                            "[{}] Missing committed transactions after adding commit range {:?} to DAG State : {}",
                            sync_label,
                            fetched_commit_range,
                            missing_committed_txns
                                .keys()
                                .map(|b| b.to_string())
                                .join(","),
                        );
                        for (gen_tran_ref, _ack_authorities) in missing_committed_txns {
                            let hostname = &self
                                .inner
                                .context
                                .committee
                                .authority(gen_tran_ref.author())
                                .hostname;
                            metrics
                                .commit_sync_fetch_missing_transactions
                                .with_label_values(&[hostname.as_str(), sync_label])
                                .inc();
                        }
                    }
                }
                Err(e) => {
                    info!(
                        "[{}] Failed to add blocks, shutting down: {}",
                        sync_label, e
                    );
                    return;
                }
            };

            // Once commits and blocks are sent to Core, ratchet up synced_commit_index
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
    ) -> (CommitIndex, CertifiedCommits) {
        // Regular syncer uses 4x timeout multiplier to account for:
        // - Fetching commits and commit certifying block headers
        // - Fetching block headers referenced by the commits
        // - Time spent on pipelining requests
        // - Headroom to allow fetch_once() to timeout gracefully
        shared_fetch_loop(inner, commit_range, 4, Self::fetch_once).await
    }

    // Fetches commits and blocks from a single authority. At a high level, first
    // the commits are fetched and verified. After that, blocks referenced in
    // the certified commits are fetched and sent to Core for processing.
    async fn fetch_once(
        inner: Arc<Inner<C>>,
        target_authority: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<CertifiedCommits> {
        // Maximum delay between consecutive pipelined requests, to avoid
        // overwhelming the peer while still maintaining reasonable throughput.
        const MAX_PIPELINE_DELAY: Duration = Duration::from_secs(1);

        let _timer = inner
            .context
            .metrics
            .node_metrics
            .commit_sync_fetch_once_latency
            .with_label_values(&[inner.sync_type.as_str()])
            .start_timer();
        let consensus_fast_commit_sync = inner.context.protocol_config.consensus_fast_commit_sync();

        // 1. Fetch commits in the commit range from the target authority.
        let (serialized_commits, serialized_voting_block_headers) = inner
            .network_client
            .fetch_commits(target_authority, commit_range.clone(), timeout)
            .await?;

        // 2. Verify the response contains block headers that can certify the last
        //    returned commit,
        // and the returned commits are chained by digest, so earlier commits are
        // certified as well.
        let batch_size = inner.sync_type.commit_sync_batch_size(&inner.context) as usize;
        let (commits, _) = Handle::current()
            .spawn_blocking({
                let inner = inner.clone();
                move || {
                    inner.verify_commits(
                        target_authority,
                        commit_range,
                        serialized_commits,
                        serialized_voting_block_headers,
                        batch_size,
                    )
                }
            })
            .await
            .expect("Spawn blocking should not fail")?;

        // 3. Fetch block headers referenced by the commits, from the same authority.
        let mut block_refs: Vec<_> = commits
            .iter()
            .flat_map(|c| c.block_headers())
            .cloned()
            .collect();

        // 3a. Collect all committed transaction block refs from commits
        let committed_tx_refs: Vec<GenericTransactionRef> = commits
            .iter()
            .flat_map(|c| c.committed_transactions())
            .collect();

        if !consensus_fast_commit_sync {
            // 3b. Identify which committed transaction blocks are NOT in the committed
            // blocks list and add them to block_refs so they get fetched together.
            // If consensus_fast_commit_sync is true, then we fetch these transactions
            // separately without fetching headers, so in this case we don't need to do
            // anything here
            let block_refs_set: BTreeSet<_> = block_refs.iter().cloned().collect();
            let missing_tx_header_refs: ConsensusResult<Vec<BlockRef>> = committed_tx_refs
                .iter()
                .filter_map(|tx_ref| match tx_ref {
                    GenericTransactionRef::BlockRef(br) => {
                        if !block_refs_set.contains(br) {
                            Some(Ok(*br))
                        } else {
                            None
                        }
                    }
                    _ => Some(Err(ConsensusError::TransactionRefVariantMismatch {
                        protocol_flag_enabled: false,
                        expected_variant: "BlockRef",
                        received_variant: "TransactionRef",
                    })),
                })
                .collect();
            let missing_tx_header_refs = missing_tx_header_refs?;

            // Merge missing transaction headers into the main block_refs list
            block_refs.extend(missing_tx_header_refs);
        }

        let num_chunks = block_refs
            .len()
            .div_ceil(inner.context.parameters.max_headers_per_commit_sync_fetch)
            as u32;
        let mut requests: FuturesOrdered<_> = block_refs
            .chunks(inner.context.parameters.max_headers_per_commit_sync_fetch)
            .enumerate()
            .map(|(i, request_block_refs)| {
                let inner = inner.clone();
                async move {
                    // 4. Send out pipelined fetch requests to avoid overloading the target
                    //    authority.
                    let individual_delay = (timeout / num_chunks).min(MAX_PIPELINE_DELAY);
                    sleep(individual_delay * i as u32).await;
                    // TODO: add some retries.
                    let serialized_block_headers = inner
                        .network_client
                        .fetch_block_headers(
                            target_authority,
                            request_block_refs.to_vec(),
                            vec![],
                            timeout,
                        )
                        .await?;
                    // 5. Verify headers: count matches and each reference matches requested.
                    verify_fetched_headers(
                        target_authority,
                        request_block_refs,
                        serialized_block_headers,
                    )
                }
            })
            .collect();

        // 8. Create transaction fetch requests (will be processed concurrently with
        //    headers)
        let mut transaction_requests: FuturesOrdered<_> = if !committed_tx_refs.is_empty() {
            let num_tx_chunks = committed_tx_refs.len().div_ceil(
                inner
                    .context
                    .parameters
                    .max_transactions_per_commit_sync_fetch,
            ) as u32;
            committed_tx_refs
                .chunks(
                    inner
                        .context
                        .parameters
                        .max_transactions_per_commit_sync_fetch,
                )
                .enumerate()
                .map(|(i, request_block_refs)| {
                    let inner = inner.clone();
                    async move {
                        // 9. Send out pipelined fetch requests to avoid overloading the target
                        //    authority. Offset by half delay to interleave with header requests.
                        let individual_delay =
                            (timeout / num_tx_chunks.max(1)).min(MAX_PIPELINE_DELAY);
                        sleep(individual_delay * i as u32 + individual_delay / 2).await;
                        let serialized_transactions = inner
                            .network_client
                            .fetch_transactions(
                                target_authority,
                                request_block_refs.to_vec(),
                                timeout,
                            )
                            .await?;

                        // 10. Verify that the number of returned transactions is not greater than
                        //     the number of requested transactions. It's OK if not all requested
                        //     transactions are returned as long as the peer returns all the
                        //     headers. We don't want to fail the whole fetch in this case.
                        //     TransactionSynchronizer will take care of fetching missing
                        //     transactions later.
                        if request_block_refs.len() < serialized_transactions.len() {
                            return Err(ConsensusError::TooManyFetchedTransactionsReturned(
                                target_authority,
                            ));
                        }
                        let requested_block_refs_set: BTreeSet<_> =
                            request_block_refs.iter().cloned().collect();
                        // Deserialize to extract BlockRef and build a map directly
                        let mut result = BTreeMap::new();
                        for serialized_bytes in serialized_transactions {
                            let (committed_transaction_ref, serialized_transactions) =
                                if !consensus_fast_commit_sync {
                                    let serialized_tx: SerializedTransactionsV1 =
                                        bcs::from_bytes(&serialized_bytes)
                                            .map_err(ConsensusError::MalformedTransactions)?;

                                    // 11. Verify the returned transactions match the requested
                                    //     block refs.
                                    let committed_transaction_ref =
                                        GenericTransactionRef::BlockRef(serialized_tx.block_ref);
                                    if !requested_block_refs_set
                                        .contains(&committed_transaction_ref)
                                    {
                                        return Err(
                                            ConsensusError::UnexpectedTransactionForCommit {
                                                peer: target_authority,
                                                received: committed_transaction_ref,
                                            },
                                        );
                                    }
                                    (
                                        committed_transaction_ref,
                                        serialized_tx.serialized_transactions,
                                    )
                                } else {
                                    let serialized_tx: SerializedTransactionsV2 =
                                        bcs::from_bytes(&serialized_bytes)
                                            .map_err(ConsensusError::MalformedTransactions)?;

                                    // 11. Verify the returned transactions match the requested
                                    //     transaction refs.
                                    let committed_transaction_ref =
                                        GenericTransactionRef::TransactionRef(
                                            serialized_tx.transaction_ref,
                                        );
                                    if !requested_block_refs_set
                                        .contains(&committed_transaction_ref)
                                    {
                                        return Err(
                                            ConsensusError::UnexpectedTransactionForCommit {
                                                peer: target_authority,
                                                received: committed_transaction_ref,
                                            },
                                        );
                                    }
                                    (
                                        committed_transaction_ref,
                                        serialized_tx.serialized_transactions,
                                    )
                                };

                            result.insert(committed_transaction_ref, serialized_transactions);
                        }

                        Ok::<BTreeMap<GenericTransactionRef, Bytes>, ConsensusError>(result)
                    }
                })
                .collect()
        } else {
            FuturesOrdered::new()
        };

        // 12. Process header and transaction requests concurrently
        let mut fetched_block_headers = BTreeMap::new();
        let mut fetched_transactions = BTreeMap::new();

        loop {
            tokio::select! {
                Some(result) = requests.next() => {
                    for block_header in result? {
                        fetched_block_headers.insert(block_header.reference(), block_header);
                    }
                }
                Some(result) = transaction_requests.next() => {
                    fetched_transactions.extend(result?);
                }
                else => break,
            }
        }

        // 13. Verify transactions
        let mut transactions_map = if !fetched_transactions.is_empty() {
            if !inner.context.protocol_config.consensus_fast_commit_sync() {
                Handle::current()
                    .spawn_blocking({
                        let context = inner.context.clone();
                        let fetched_block_headers_clone = fetched_block_headers.clone();
                        move || {
                            verify_transactions_with_headers(
                                context,
                                target_authority,
                                fetched_transactions,
                                fetched_block_headers_clone,
                            )
                        }
                    })
                    .await
                    .expect("Spawn blocking should not fail")?
            } else {
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
            }
        } else {
            BTreeMap::new()
        };

        // 14. Now create the Certified commits by assigning the block headers and
        //     transactions to each commit and retaining the commit votes history.
        let mut certified_commits = Vec::new();
        for commit in &commits {
            let block_headers = commit
                .block_headers()
                .iter()
                .map(|block_ref| {
                    fetched_block_headers
                        .remove(block_ref)
                        // safe to call .expect here as we make sure beforehand that all headers
                        // from the commit are fetched
                        .expect("Block should exist")
                })
                .collect::<Vec<_>>();

            // Collect transactions for this commit
            let commit_transactions = commit
                .committed_transactions()
                .iter()
                .filter_map(|tx_ref| transactions_map.remove(tx_ref))
                .collect::<Vec<_>>();

            certified_commits.push(CertifiedCommit::new_certified(
                commit.clone(),
                block_headers,
                commit_transactions,
            ));
        }

        Ok(CertifiedCommits::new(certified_commits))
    }

    #[cfg(test)]
    fn unhandled_commits_threshold(&self) -> CommitIndex {
        self.inner.unhandled_commits_threshold()
    }

    #[cfg(test)]
    fn pending_fetches(&self) -> BTreeSet<CommitRange> {
        self.pending_fetches.clone()
    }

    #[cfg(test)]
    fn fetched_ranges(&self) -> BTreeMap<CommitRange, CertifiedCommits> {
        self.fetched_ranges.clone()
    }

    #[cfg(test)]
    fn highest_scheduled_index(&self) -> Option<CommitIndex> {
        self.highest_scheduled_index
    }

    #[cfg(test)]
    fn highest_fetched_commit_index(&self) -> CommitIndex {
        self.highest_fetched_commit_index
    }

    #[cfg(test)]
    fn synced_commit_index(&self) -> CommitIndex {
        self.synced_commit_index
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use bytes::Bytes;
    use parking_lot::RwLock;
    use starfish_config::{AuthorityIndex, Parameters};

    use crate::{
        CommitConsumerMonitor, CommitDigest, CommitRef, Round,
        block_header::{BlockRef, TestBlockHeader, VerifiedBlockHeader},
        block_verifier::NoopBlockVerifier,
        commit::CommitRange,
        commit_syncer::regular::RegularCommitSyncer,
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        core_thread::tests::MockCoreThreadDispatcher,
        dag_state::DagState,
        error::ConsensusResult,
        network::{BlockBundleStream, NetworkClient},
        storage::{Store, mem_store::MemStore},
        transaction_ref::GenericTransactionRef,
    };

    #[derive(Default)]
    struct FakeNetworkClient {}

    #[async_trait::async_trait]
    impl NetworkClient for FakeNetworkClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<GenericTransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        // Returns a vector of serialized block headers
        async fn fetch_block_headers(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!();
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_commits_and_transactions(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_block_headers(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn commit_syncer_start_and_pause_scheduling() {
        // SETUP
        let (context, _) = Context::new_for_test(4);
        // Use smaller batches and fetch limits for testing.
        let context = Context {
            own_index: AuthorityIndex::new_for_test(3),
            parameters: Parameters {
                commit_sync_batch_size: 5,
                commit_sync_batches_ahead: 5,
                commit_sync_parallel_fetches: 5,
                max_headers_per_commit_sync_fetch: 5,
                ..context.parameters
            },
            ..context
        };
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_thread_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(FakeNetworkClient::default());
        let store: Arc<dyn Store> = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let commit_consumer_monitor = Arc::new(CommitConsumerMonitor::new(0));

        let mut commit_syncer = RegularCommitSyncer::new(
            context,
            core_thread_dispatcher,
            commit_vote_monitor.clone(),
            commit_consumer_monitor.clone(),
            network_client,
            block_verifier,
            dag_state,
        );

        // Check initial state.
        assert!(commit_syncer.pending_fetches().is_empty());
        assert!(commit_syncer.fetched_ranges().is_empty());
        assert!(commit_syncer.highest_scheduled_index().is_none());
        assert_eq!(commit_syncer.highest_fetched_commit_index(), 0);
        assert_eq!(commit_syncer.synced_commit_index(), 0);

        // Observe round 15 blocks voting for commit 10 from authorities 0 to 2 in
        // CommitVoteMonitor
        for i in 0..3 {
            let test_block = TestBlockHeader::new(15, i)
                .set_commit_votes(vec![CommitRef::new(10, CommitDigest::MIN)])
                .build();
            let block = VerifiedBlockHeader::new_for_test(test_block);
            commit_vote_monitor.observe_block(&block);
        }

        // Fetches should be scheduled after seeing progress of other validators.
        commit_syncer.try_schedule_once();

        // Verify state.
        assert_eq!(commit_syncer.pending_fetches().len(), 2);
        assert!(commit_syncer.fetched_ranges().is_empty());
        assert_eq!(commit_syncer.highest_scheduled_index(), Some(10));
        assert_eq!(commit_syncer.highest_fetched_commit_index(), 0);
        assert_eq!(commit_syncer.synced_commit_index(), 0);

        // Observe round 40 blocks voting for commit 35 from authorities 0 to 2 in
        // CommitVoteMonitor
        for i in 0..3 {
            let test_block = TestBlockHeader::new(40, i)
                .set_commit_votes(vec![CommitRef::new(35, CommitDigest::MIN)])
                .build();
            let block = VerifiedBlockHeader::new_for_test(test_block);
            commit_vote_monitor.observe_block(&block);
        }

        // Fetches should be scheduled until the unhandled commits threshold.
        commit_syncer.try_schedule_once();

        // Verify commit syncer is paused after scheduling 15 commits to index 25.
        assert_eq!(commit_syncer.unhandled_commits_threshold(), 25);
        assert_eq!(commit_syncer.highest_scheduled_index(), Some(25));
        let pending_fetches = commit_syncer.pending_fetches();
        assert_eq!(pending_fetches.len(), 5);

        // Indicate commit index 25 is consumed, and try to schedule again.
        commit_consumer_monitor.set_highest_handled_commit(25);
        commit_syncer.try_schedule_once();

        // Verify commit syncer schedules fetches up to index 35.
        assert_eq!(commit_syncer.highest_scheduled_index(), Some(35));
        let pending_fetches = commit_syncer.pending_fetches();
        assert_eq!(pending_fetches.len(), 7);

        // Verify contiguous ranges are scheduled.
        for (range, start) in pending_fetches.iter().zip((1..35).step_by(5)) {
            assert_eq!(range.start(), start);
            assert_eq!(range.end(), start + 4);
        }
    }
}
