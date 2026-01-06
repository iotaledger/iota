// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! CommitSyncer implements efficient synchronization of committed data.
//!
//! During the operation of a committee of authorities for consensus, one or
//! more authorities can fall behind the quorum in their received and accepted
//! blocks. This can happen due to network disruptions, host crash, or other
//! reasons. Authorities fell behind need to catch up to the quorum to be able
//! to vote on the latest leaders. So efficient synchronization is necessary
//! to minimize the impact of temporary disruptions and maintain smooth
//! operations of the network.
//! CommitSyncer achieves efficient synchronization by relying on the following:
//! when blocks are included in commits with >= 2f+1 certifiers by stake, these
//! blocks must have passed verifications on some honest validators, so
//! re-verifying them is unnecessary. In fact, the quorum certified commits
//! themselves can be trusted to be sent to IOTA directly, but for simplicity
//! this is not done. Blocks from trusted commits still go through Core and
//! committer.
//!
//! Another way CommitSyncer improves the efficiency of synchronization is
//! parallel fetching: commits have a simple dependency graph (linear), so it is
//! easy to fetch ranges of commits in parallel.
//!
//! Commit synchronization is an expensive operation, involving transferring
//! large amount of data via the network. And it is not on the critical path of
//! block processing. So the heuristics for synchronization, including triggers
//! and retries, should be chosen to favor throughput and efficient resource
//! usage, over faster reactions.

pub mod fast;
pub mod regular;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use iota_metrics::spawn_logged_monitored_task;
use itertools::Itertools;
use parking_lot::RwLock;
use rand::{prelude::SliceRandom as _, rngs::ThreadRng};
use starfish_config::AuthorityIndex;
use tokio::{
    sync::oneshot,
    task::{JoinHandle, JoinSet},
    time::{MissedTickBehavior, sleep},
};
use tracing::{info, warn};

use crate::{
    BlockRef, CommitConsumerMonitor, CommitIndex, Transaction, VerifiedBlockHeader,
    block_header::{
        BlockHeaderAPI, SignedBlockHeader, TransactionsCommitment, VerifiedTransactions,
    },
    block_verifier::BlockVerifier,
    commit::{Commit, CommitAPI as _, CommitDigest, CommitRange, CommitRef, TrustedCommit},
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    encoder::create_encoder,
    error::{ConsensusError, ConsensusResult},
    network::NetworkClient,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    transaction_ref::GenericTransactionRef,
};
pub(crate) enum CommitSyncType {
    Fast,
    Regular,
}

impl CommitSyncType {
    pub(crate) fn commit_sync_batch_size(&self, context: &Context) -> u32 {
        match self {
            CommitSyncType::Fast => context.parameters.fast_commit_sync_batch_size,
            CommitSyncType::Regular => context.parameters.commit_sync_batch_size,
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CommitSyncType::Fast => "fast_commit_sync",
            CommitSyncType::Regular => "commit_sync",
        }
    }

    pub(crate) fn should_schedule(&self, gap: u32, commit_sync_gap_threshold: u32) -> bool {
        match self {
            CommitSyncType::Fast => gap > commit_sync_gap_threshold,
            CommitSyncType::Regular => gap <= commit_sync_gap_threshold,
        }
    }
}

// Handle to stop the CommitSyncer loop.
pub(crate) struct CommitSyncerHandle {
    schedule_task: JoinHandle<()>,
    tx_shutdown: oneshot::Sender<()>,
}

impl CommitSyncerHandle {
    pub(crate) async fn stop(self) {
        let _ = self.tx_shutdown.send(());
        // Do not abort schedule task, which waits for fetches to shut down.
        if let Err(e) = self.schedule_task.await {
            if e.is_panic() {
                std::panic::resume_unwind(e.into_panic());
            }
        }
    }
}

pub(crate) struct Inner<C: NetworkClient> {
    pub(crate) context: Arc<Context>,
    pub(crate) core_thread_dispatcher: Arc<dyn CoreThreadDispatcher>,
    pub(crate) commit_vote_monitor: Arc<CommitVoteMonitor>,
    pub(crate) commit_consumer_monitor: Arc<CommitConsumerMonitor>,
    pub(crate) network_client: Arc<C>,
    pub(crate) block_verifier: Arc<dyn BlockVerifier>,
    pub(crate) dag_state: Arc<RwLock<DagState>>,
    pub(crate) sync_type: CommitSyncType,
}

impl<C: NetworkClient> Inner<C> {
    /// Verifies the commits and also certifies them using the provided vote
    /// blocks for the last commit. The method returns the trusted commits
    /// and the votes as verified blocks.
    pub(crate) fn verify_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        serialized_commits: Vec<Bytes>,
        serialized_vote_blocks_headers: Vec<Bytes>,
    ) -> ConsensusResult<Vec<TrustedCommit>> {
        // Parse and verify commits.
        let mut commits = Vec::new();
        for serialized in &serialized_commits {
            let commit: Commit =
                bcs::from_bytes(serialized).map_err(ConsensusError::MalformedCommit)?;
            let digest = TrustedCommit::compute_digest(serialized);
            if commits.is_empty() {
                // start is inclusive, so first commit must be at the start index.
                if commit.index() != commit_range.start() {
                    return Err(ConsensusError::UnexpectedStartCommit {
                        peer,
                        start: commit_range.start(),
                        commit: Box::new(commit),
                    });
                }
            } else {
                // Verify next commit increments index and references the previous digest.
                let (last_commit_digest, last_commit): &(CommitDigest, Commit) =
                    commits.last().unwrap();
                if commit.index() != last_commit.index() + 1
                    || &commit.previous_digest() != last_commit_digest
                {
                    return Err(ConsensusError::UnexpectedCommitSequence {
                        peer,
                        prev_commit: Box::new(last_commit.clone()),
                        curr_commit: Box::new(commit),
                    });
                }
            }
            // Do not process more commits past the end index.
            if commit.index() > commit_range.end() {
                break;
            }
            commits.push((digest, commit));
        }
        let Some((end_commit_digest, end_commit)) = commits.last() else {
            return Err(ConsensusError::NoCommitReceived { peer });
        };

        // Parse and verify blocks. Then accumulate votes on the end commit.
        let end_commit_ref = CommitRef::new(end_commit.index(), *end_commit_digest);
        let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for serialized_block_header in serialized_vote_blocks_headers.into_iter() {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)?;
            // The block signature needs to be verified.
            self.block_verifier.verify(&signed_block_header)?;
            for vote in signed_block_header.commit_votes() {
                if *vote == end_commit_ref {
                    stake_aggregator.add(signed_block_header.author(), &self.context.committee);
                }
            }
        }

        // Check if the end commit has enough votes.
        if !stake_aggregator.reached_threshold(&self.context.committee) {
            return Err(ConsensusError::NotEnoughCommitVotes {
                stake: stake_aggregator.stake(),
                peer,
                commit: Box::new(end_commit.clone()),
            });
        }

        let trusted_commits = commits
            .into_iter()
            .zip(serialized_commits)
            .map(|((_d, c), s)| TrustedCommit::new_trusted(c, s))
            .collect();
        Ok(trusted_commits)
    }
}

/// Verifies transactions against their block headers and returns a map of
/// BlockRef to VerifiedTransactions.
pub(crate) fn verify_transactions_with_headers(
    context: Arc<Context>,
    peer: AuthorityIndex,
    serialized_transactions: BTreeMap<GenericTransactionRef, Bytes>,
    block_headers: BTreeMap<BlockRef, VerifiedBlockHeader>,
) -> ConsensusResult<BTreeMap<GenericTransactionRef, VerifiedTransactions>> {
    let mut verified_transactions_map = BTreeMap::new();
    let mut encoder = create_encoder(&context);
    for (committed_transactions_ref, inner_serialized_transactions) in serialized_transactions {
        let block_ref = match committed_transactions_ref {
            GenericTransactionRef::BlockRef(br) => br,
            _ => {
                return Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: false,
                    expected_variant: "BlockRef",
                    received_variant: "TransactionRef",
                });
            }
        };
        // Step 1: Get the block header and verify that the transactions commitment
        // matches. This ensures the transactions we received are exactly
        // the ones that were included in the block when it was created.
        let block_header = block_headers
            .get(&block_ref)
            .expect("header for fetched transactions must exist");

        if block_header.transactions_commitment()
            != TransactionsCommitment::compute_transactions_commitment(
                &inner_serialized_transactions,
                &context,
                &mut encoder,
            )
            .expect("correct computation of the transactions commitment should be successful")
        {
            return Err(ConsensusError::TransactionCommitmentFailure {
                round: block_ref.round,
                author: block_ref.author,
                peer,
            });
        }

        // Step 2: Deserialize the actual transactions vector.
        let transactions: Vec<Transaction> = bcs::from_bytes(&inner_serialized_transactions)
            .map_err(ConsensusError::MalformedTransactions)?;

        // Step 3: Create a VerifiedTransactions instance and insert into map
        let verified_transactions = VerifiedTransactions::new(
            transactions,
            block_ref,
            block_header.transactions_commitment(),
            inner_serialized_transactions,
        );

        verified_transactions_map.insert(
            GenericTransactionRef::BlockRef(block_ref),
            verified_transactions,
        );
    }

    Ok(verified_transactions_map)
}

/// Verifies transactions against their transaction refs and returns a map of
/// BlockRef to VerifiedTransactions.
pub(crate) fn verify_transactions_with_transactions_refs(
    context: &Arc<Context>,
    peer: AuthorityIndex,
    serialized_transactions: BTreeMap<GenericTransactionRef, Bytes>,
) -> ConsensusResult<BTreeMap<GenericTransactionRef, VerifiedTransactions>> {
    let mut verified_transactions_map = BTreeMap::new();
    let mut encoder = create_encoder(context);
    for (committed_transactions_ref, inner_serialized_transactions) in serialized_transactions {
        let transaction_ref = match committed_transactions_ref {
            GenericTransactionRef::TransactionRef(tx_ref) => tx_ref,
            _ => {
                return Err(ConsensusError::TransactionRefVariantMismatch {
                    protocol_flag_enabled: true,
                    expected_variant: "TransactionRef",
                    received_variant: "BlockRef",
                });
            }
        };
        let block_ref = BlockRef {
            round: transaction_ref.round,
            author: transaction_ref.author,
            digest: transaction_ref.block_digest,
        };
        // Step 1: Verify that the transaction commitment matches.
        if transaction_ref.transactions_commitment
            != TransactionsCommitment::compute_transactions_commitment(
                &inner_serialized_transactions,
                context,
                &mut encoder,
            )
            .expect("correct computation of the transactions commitment should be successful")
        {
            return Err(ConsensusError::TransactionCommitmentFailure {
                round: transaction_ref.round,
                author: transaction_ref.author,
                peer,
            });
        }

        // Step 2: Deserialize the actual transactions vector.
        let transactions: Vec<Transaction> = bcs::from_bytes(&inner_serialized_transactions)
            .map_err(ConsensusError::MalformedTransactions)?;

        // Step 3: Create a VerifiedTransactions instance and insert into map
        let verified_transactions = VerifiedTransactions::new(
            transactions,
            block_ref,
            transaction_ref.transactions_commitment,
            inner_serialized_transactions,
        );

        verified_transactions_map.insert(
            GenericTransactionRef::TransactionRef(transaction_ref),
            verified_transactions,
        );
    }

    Ok(verified_transactions_map)
}

#[async_trait::async_trait]
pub trait CommitSyncer<C: NetworkClient>: Send + Sync + 'static + Sized {
    type FetchedData: Send + Sync;

    fn inner(&self) -> &Arc<Inner<C>>;
    fn inflight_fetches(&mut self) -> &mut JoinSet<(u32, Self::FetchedData)>;
    fn inflight_fetches_len(&self) -> usize;
    fn pending_fetches_len(&self) -> usize;

    fn highest_scheduled_index(&mut self) -> &mut Option<CommitIndex>;

    fn synced_commit_index(&mut self) -> &mut CommitIndex;

    fn pending_fetches(&mut self) -> &mut BTreeSet<CommitRange>;

    fn fetched_ranges(&mut self) -> &mut BTreeMap<CommitRange, Self::FetchedData>;

    fn unhandled_commits_threshold(&self) -> CommitIndex {
        self.inner().context.parameters.commit_sync_batch_size
            * (self.inner().context.parameters.commit_sync_batches_ahead as u32)
    }

    #[cfg(test)]
    fn highest_fetched_commit_index(&self) -> CommitIndex;
    async fn handle_fetch_result(
        &mut self,
        target_end: CommitIndex,
        fetched_data_set: Self::FetchedData,
    );
    async fn fetch_once(
        inner: Arc<Inner<C>>,
        target_authority: AuthorityIndex,
        commit_range: CommitRange,
        timeout: Duration,
    ) -> ConsensusResult<Self::FetchedData>;

    fn start(self) -> CommitSyncerHandle {
        let (tx_shutdown, rx_shutdown) = oneshot::channel();
        let schedule_task = spawn_logged_monitored_task!(self.schedule_loop(rx_shutdown,));
        CommitSyncerHandle {
            schedule_task,
            tx_shutdown,
        }
    }

    async fn fetch_loop(
        inner: Arc<Inner<C>>,
        commit_range: CommitRange,
    ) -> (CommitIndex, Self::FetchedData) {
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

    fn try_start_fetches(&mut self) {
        // Cap parallel fetches based on configured limit and committee size, to avoid
        // overloading the network. Also when there are too many fetched block headers
        // that cannot be sent to Core before an earlier fetch has not finished,
        // reduce parallelism so the earlier fetch can retry on a better host and
        // succeed.
        let target_parallel_fetches = self
            .inner()
            .context
            .parameters
            .commit_sync_parallel_fetches
            .min(self.inner().context.committee.size() * 2 / 3)
            .min(
                self.inner()
                    .context
                    .parameters
                    .commit_sync_batches_ahead
                    .saturating_sub(self.fetched_ranges().len()),
            )
            .max(1);
        // Start new fetches if there are pending batches and available slots.
        loop {
            let inner = self.inner().clone();
            let inflight_fetches_len = self.inflight_fetches_len();

            if inflight_fetches_len >= target_parallel_fetches {
                break;
            }
            if !self.pending_fetches().is_empty() {
                info!(
                    "[{}] Pending fetches: {:?}, target parallel fetches: {}, inflight fetch number: {}",
                    inner.sync_type.as_str(),
                    self.pending_fetches(),
                    target_parallel_fetches,
                    inflight_fetches_len
                );
            }
            let Some(commit_range) = self.pending_fetches().pop_first() else {
                break;
            };
            (*self.inflight_fetches()).spawn(Self::fetch_loop(inner, commit_range));
        }

        let metrics = &self.inner().context.metrics.node_metrics;
        let sync_label = self.inner().sync_type.as_str();
        metrics
            .commit_sync_inflight_fetches
            .with_label_values(&[sync_label])
            .set(self.inflight_fetches_len() as i64);
        metrics
            .commit_sync_pending_fetches
            .with_label_values(&[sync_label])
            .set(self.pending_fetches_len() as i64);
        metrics
            .commit_sync_highest_synced_index
            .with_label_values(&[sync_label])
            .set(*self.synced_commit_index() as i64);
    }
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
                Some(result) = self.inflight_fetches().join_next(), if !self.inflight_fetches().is_empty() => {
                    if let Err(e) = result {
                        if e.is_panic() {
                            std::panic::resume_unwind(e.into_panic());
                        }
                        warn!("[{}] Fetch cancelled. FastCommitSyncer shutting down: {}", self.inner().sync_type.as_str(), e);
                        // If any fetch is cancelled or panicked, try to shutdown and exit the loop.
                        self.inflight_fetches().shutdown().await;
                        return;
                    }
                    let (target_end, committed_subdags) = result.unwrap();
                    self.handle_fetch_result(target_end, committed_subdags).await;
                }
                _ = &mut rx_shutdown => {
                    // Shutdown requested.
                    info!("[{}] FastCommitSyncer shutting down ...", self.inner().sync_type.as_str());
                    self.inflight_fetches().shutdown().await;
                    return;
                }
            }

            self.try_start_fetches();
        }
    }

    fn try_schedule_once(&mut self) {
        let quorum_commit_index = self.inner().commit_vote_monitor.quorum_commit_index();
        let local_commit_index = self.inner().dag_state.read().last_commit_index();

        // Skip scheduling depending on sync type and gap threshold.
        let gap = quorum_commit_index.saturating_sub(local_commit_index);
        if !self.inner().sync_type.should_schedule(
            gap,
            self.inner().context.parameters.commit_sync_gap_threshold,
        ) {
            return;
        }

        let metrics = &self.inner().context.metrics.node_metrics;
        metrics
            .commit_sync_quorum_index
            .set(quorum_commit_index as i64);
        metrics
            .commit_sync_local_index
            .set(local_commit_index as i64);
        let highest_handled_index = self
            .inner()
            .commit_consumer_monitor
            .highest_handled_commit();
        let highest_scheduled_index = self.highest_scheduled_index().unwrap_or(0);
        // Update synced_commit_index periodically to make sure it is not smaller than
        // local commit index.
        *self.synced_commit_index() = (*self.synced_commit_index()).max(local_commit_index);
        let unhandled_commits_threshold = self.unhandled_commits_threshold();

        // TODO: cleanup inflight fetches that are no longer needed.
        let fetch_after_index =
            (*self.synced_commit_index()).max(self.highest_scheduled_index().unwrap_or(0));
        // When the node is falling behind, schedule pending fetches which will be
        // executed on later.
        let step = self
            .inner()
            .sync_type
            .commit_sync_batch_size(&self.inner().context);

        info!(
            "[{}] Checking to schedule fetches: synced_commit_index={}, highest_handled_index={}, highest_scheduled_index={}, quorum_commit_index={}, unhandled_commits_threshold={}, fetch_after_index={}, step={}",
            self.inner().sync_type.as_str(),
            self.synced_commit_index(),
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
                    self.inner().sync_type.as_str(),
                    highest_handled_index,
                    highest_scheduled_index
                );
                break;
            }
            info!(
                "[{}] Scheduling fetch for commit range {}..={}",
                self.inner().sync_type.as_str(),
                range_start,
                range_end
            );
            self.pending_fetches()
                .insert((range_start..=range_end).into());
            // quorum_commit_index should be non-decreasing, so highest_scheduled_index
            // should not decrease either.
            *self.highest_scheduled_index() = Some(range_end);
        }
    }
}
