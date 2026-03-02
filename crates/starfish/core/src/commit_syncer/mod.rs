// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
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
use itertools::Itertools;
use parking_lot::RwLock;
#[cfg(not(test))]
use rand::{prelude::SliceRandom as _, rngs::ThreadRng};
use starfish_config::AuthorityIndex;
use tokio::{sync::oneshot, task::JoinHandle, time::sleep};
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
    transaction_ref::{GenericTransactionRef, TransactionRef},
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

    pub(crate) fn should_schedule(
        &self,
        gap: u32,
        commit_sync_gap_threshold: u32,
        consensus_fast_commit_sync: bool,
    ) -> bool {
        match self {
            // Fast syncer requires consensus_fast_commit_sync to be enabled
            CommitSyncType::Fast => consensus_fast_commit_sync && gap > commit_sync_gap_threshold,
            // Regular syncer handles all gaps when consensus_fast_commit_sync is disabled,
            // otherwise only handles small gaps
            CommitSyncType::Regular => {
                !consensus_fast_commit_sync || gap <= commit_sync_gap_threshold
            }
        }
    }
}

/// Verifies that fetched block headers match the requested block refs.
/// Returns verified headers or an error if count/reference mismatch.
pub(crate) fn verify_fetched_headers(
    peer: AuthorityIndex,
    request_block_refs: &[BlockRef],
    serialized_block_headers: Vec<Bytes>,
) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
    // 1. Verify count matches
    if request_block_refs.len() != serialized_block_headers.len() {
        return Err(ConsensusError::UnexpectedNumberOfHeadersFetched {
            authority: peer,
            requested: request_block_refs.len(),
            received_headers: serialized_block_headers.len(),
        });
    }

    // 2. Verify each header's reference matches requested
    serialized_block_headers
        .into_iter()
        .zip(request_block_refs)
        .map(|(serialized, requested_ref)| {
            let header = VerifiedBlockHeader::new_from_bytes(serialized)?;
            if *requested_ref != header.reference() {
                return Err(ConsensusError::UnexpectedBlockHeaderForCommit {
                    peer,
                    requested: *requested_ref,
                    received: header.reference(),
                });
            }
            Ok(header)
        })
        .collect()
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
    /// Calculates the threshold for unhandled commits to apply backpressure.
    /// When the gap between synced and scheduled commits exceeds this
    /// threshold, scheduling new fetches should pause to let the handler
    /// catch up.
    pub(crate) fn unhandled_commits_threshold(&self) -> CommitIndex {
        self.context.parameters.commit_sync_batch_size
            * (self.context.parameters.commit_sync_batches_ahead as u32)
    }

    /// Verifies the commits and also certifies them using the provided vote
    /// blocks for the last commit. The method returns the trusted commits
    /// and the verified voting block headers.
    pub(crate) fn verify_commits(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
        serialized_commits: Vec<Bytes>,
        serialized_vote_blocks_headers: Vec<Bytes>,
        max_commits: usize,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)> {
        // Validate response size - peer should not return more than max_commits
        if serialized_commits.len() > max_commits {
            return Err(ConsensusError::TooManyCommitsFromPeer {
                peer,
                count: serialized_commits.len() as CommitIndex,
                limit: max_commits as CommitIndex,
            });
        }

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
            commits.push((digest, commit));
        }
        let Some((end_commit_digest, end_commit)) = commits.last() else {
            return Err(ConsensusError::NoCommitReceived { peer });
        };

        // Parse and verify blocks. Then accumulate votes on the end commit.
        let end_commit_ref = CommitRef::new(end_commit.index(), *end_commit_digest);
        let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        let mut verified_voting_headers = Vec::new();
        for serialized_block_header in serialized_vote_blocks_headers {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)?;
            // The block signature needs to be verified.
            self.block_verifier.verify(&signed_block_header)?;
            for vote in signed_block_header.commit_votes() {
                if *vote == end_commit_ref {
                    stake_aggregator.add(signed_block_header.author(), &self.context.committee);
                }
            }
            // Store the verified voting block header
            verified_voting_headers.push(VerifiedBlockHeader::new_verified(
                signed_block_header,
                serialized_block_header,
            ));
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
        Ok((trusted_commits, verified_voting_headers))
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
            TransactionRef::new(block_ref, block_header.transactions_commitment()),
            Some(block_ref.digest),
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
            transaction_ref,
            None,
            inner_serialized_transactions,
        );

        verified_transactions_map.insert(
            GenericTransactionRef::TransactionRef(transaction_ref),
            verified_transactions,
        );
    }

    Ok(verified_transactions_map)
}

/// Generic fetch loop that retries fetching data from available authorities
/// until a request succeeds. This is shared between RegularCommitSyncer and
/// FastCommitSyncer.
///
/// # Type Parameters
/// - `C`: Network client type
/// - `T`: Fetched data type (CertifiedCommits for regular, (Vec<TrustedCommit>,
///   Vec<CommittedSubDag>) for fast)
/// - `F`: Fetch function type
/// - `Fut`: Future returned by fetch function
///
/// # Parameters
/// - `inner`: Shared context and dependencies
/// - `commit_range`: The range of commits to fetch
/// - `fetch_timeout_multiplier`: Multiplier for timeout calculation (4 for
///   regular, 2 for fast)
/// - `fetch_once_fn`: Implementation-specific fetch function
///
/// # Returns
/// Tuple of (end_commit_index, fetched_data)
#[cfg_attr(test, tracing::instrument(skip_all, fields(authority = %inner.context.own_index)))]
pub(crate) async fn fetch_loop<C, T, F, Fut>(
    inner: Arc<Inner<C>>,
    commit_range: CommitRange,
    fetch_timeout_multiplier: u32,
    fetch_once_fn: F,
) -> (CommitIndex, T)
where
    C: NetworkClient,
    T: Send,
    F: Fn(Arc<Inner<C>>, AuthorityIndex, CommitRange, Duration) -> Fut,
    Fut: std::future::Future<Output = ConsensusResult<T>> + Send,
{
    // Individual request base timeout.
    #[cfg(not(test))]
    const TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(test)]
    const TIMEOUT: Duration = Duration::from_millis(500);
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
        #[cfg(not(test))]
        target_authorities.shuffle(&mut ThreadRng::default());
        target_authorities.truncate(MAX_NUM_TARGETS);
        // Increase timeout multiplier for each loop until MAX_TIMEOUT_MULTIPLIER.
        timeout_multiplier = (timeout_multiplier + 1).min(MAX_TIMEOUT_MULTIPLIER);
        let request_timeout = TIMEOUT * timeout_multiplier;

        let fetch_timeout = request_timeout * fetch_timeout_multiplier;
        // Try fetching from the selected target authority.
        for authority in target_authorities {
            match tokio::time::timeout(
                fetch_timeout,
                fetch_once_fn(
                    inner.clone(),
                    authority,
                    commit_range.clone(),
                    request_timeout,
                ),
            )
            .await
            {
                Ok(Ok(data)) => {
                    info!(
                        "[{}] Finished fetching commits in {commit_range:?}",
                        inner.sync_type.as_str()
                    );
                    return (commit_range.end(), data);
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
                        .with_label_values(&[hostname.as_str(), error, inner.sync_type.as_str()])
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

/// Generic function to start pending fetches while respecting parallelism
/// limits. This is shared between RegularCommitSyncer and FastCommitSyncer.
///
/// # Parameters
/// - `inner`: Shared context and dependencies
/// - `pending_fetches`: Set of commit ranges pending fetch
/// - `fetched_ranges_count`: Number of fetched ranges waiting to be processed
/// - `inflight_fetches_count`: Number of currently in-flight fetch tasks
/// - `synced_commit_index`: Latest synced commit index
/// - `spawn_fn`: Closure to spawn a new fetch task
///
/// # Returns
/// Updated counts after spawning new fetches
pub(crate) fn try_start_fetches<C, F>(
    inner: &Arc<Inner<C>>,
    pending_fetches: &mut BTreeSet<CommitRange>,
    fetched_ranges_count: usize,
    inflight_fetches_count: usize,
    synced_commit_index: CommitIndex,
    mut spawn_fn: F,
) -> (usize, usize)
where
    C: NetworkClient,
    F: FnMut(CommitRange),
{
    // Cap parallel fetches based on configured limit and committee size, to avoid
    // overloading the network. Also when there are too many fetched block headers
    // that cannot be sent to Core before an earlier fetch has not finished,
    // reduce parallelism so the earlier fetch can retry on a better host and
    // succeed.
    let target_parallel_fetches = inner
        .context
        .parameters
        .commit_sync_parallel_fetches
        .min(inner.context.committee.size() * 2 / 3)
        .min(
            inner
                .context
                .parameters
                .commit_sync_batches_ahead
                .saturating_sub(fetched_ranges_count),
        )
        .max(1);

    let mut new_inflight_count = inflight_fetches_count;

    // Start new fetches if there are pending batches and available slots.
    loop {
        if new_inflight_count >= target_parallel_fetches {
            break;
        }
        if !pending_fetches.is_empty() {
            info!(
                "[{}] Pending fetches: {:?}, target parallel fetches: {}, inflight fetch number: {}",
                inner.sync_type.as_str(),
                pending_fetches,
                target_parallel_fetches,
                new_inflight_count
            );
        }
        let Some(commit_range) = pending_fetches.pop_first() else {
            break;
        };
        spawn_fn(commit_range);
        new_inflight_count += 1;
    }

    let metrics = &inner.context.metrics.node_metrics;
    let sync_label = inner.sync_type.as_str();
    metrics
        .commit_sync_inflight_fetches
        .with_label_values(&[sync_label])
        .set(new_inflight_count as i64);
    metrics
        .commit_sync_pending_fetches
        .with_label_values(&[sync_label])
        .set(pending_fetches.len() as i64);
    metrics
        .commit_sync_highest_synced_index
        .with_label_values(&[sync_label])
        .set(synced_commit_index as i64);

    (new_inflight_count, pending_fetches.len())
}

// =============================================================================
// Shared helper functions for scheduling and error handling
// =============================================================================

/// Result of scheduling commit range fetches.
pub(crate) struct ScheduleResult {
    /// The new highest scheduled commit index (if any ranges were scheduled).
    pub new_highest_scheduled: Option<CommitIndex>,
    /// The commit ranges that were scheduled for fetching.
    pub ranges_scheduled: Vec<CommitRange>,
}

/// Creates commit range batches for fetching, respecting backpressure.
///
/// This function is shared between RegularCommitSyncer and FastCommitSyncer.
/// It calculates which commit ranges should be fetched next based on:
/// - The current gap between local and quorum commit indices
/// - Backpressure from unhandled commits
///
/// # Parameters
/// - `inner`: Shared context and dependencies
/// - `fetch_after_index`: Start fetching from commits after this index
/// - `quorum_commit_index`: The commit index that quorum has reached
/// - `highest_handled_index`: The highest commit index that has been processed
/// - `unhandled_commits_threshold`: Threshold for applying backpressure
///
/// # Returns
/// A `ScheduleResult` with the new highest scheduled index and ranges to fetch.
pub(crate) fn schedule_commit_ranges<C: NetworkClient>(
    inner: &Inner<C>,
    fetch_after_index: CommitIndex,
    quorum_commit_index: CommitIndex,
    highest_handled_index: CommitIndex,
    unhandled_commits_threshold: CommitIndex,
) -> ScheduleResult {
    let step = inner.sync_type.commit_sync_batch_size(&inner.context);
    let mut result = ScheduleResult {
        new_highest_scheduled: None,
        ranges_scheduled: Vec::new(),
    };

    for prev_end in (fetch_after_index..=quorum_commit_index).step_by(step as usize) {
        let range_start = prev_end + 1;
        let range_end = prev_end + step;

        // Don't schedule incomplete batches
        if quorum_commit_index < range_end {
            break;
        }

        // Apply backpressure if handler is lagging
        if highest_handled_index + unhandled_commits_threshold < range_end {
            warn!(
                "[{}] Skip scheduling new commit fetches: handler lagging. \
                 highest_handled={}, threshold={}",
                inner.sync_type.as_str(),
                highest_handled_index,
                unhandled_commits_threshold
            );
            break;
        }

        result
            .ranges_scheduled
            .push((range_start..=range_end).into());
        result.new_highest_scheduled = Some(range_end);
    }

    result
}

/// Handles JoinError from fetch tasks.
///
/// # Returns
/// `true` if the syncer should shutdown, `false` otherwise.
pub(crate) fn handle_fetch_join_error(
    error: &tokio::task::JoinError,
    sync_type: &CommitSyncType,
) -> bool {
    if error.is_panic() {
        // Re-panic in the main task
        return true;
    }
    warn!(
        "[{}] Fetch cancelled. Shutting down: {}",
        sync_type.as_str(),
        error
    );
    true // Signal to shutdown
}

/// Re-queues unfetched portion of a commit range for retry.
///
/// When a fetch returns partial results (fewer commits than requested),
/// this function queues the remaining range for another fetch attempt.
pub(crate) fn requeue_partial_range(
    pending_fetches: &mut BTreeSet<CommitRange>,
    commit_end: CommitIndex,
    target_end: CommitIndex,
) {
    if commit_end < target_end {
        pending_fetches.insert((commit_end + 1..=target_end).into());
    }
}
