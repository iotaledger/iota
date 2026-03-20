// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, pin::Pin, sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use consensus_config::AuthorityIndex;
use futures::{Stream, StreamExt, ready, stream, task};
use iota_macros::fail_point_async;
use parking_lot::RwLock;
use tokio::{sync::broadcast, time::sleep};
use tokio_util::sync::ReusableBoxFuture;
use tracing::{debug, info, warn};

use crate::{
    CommitIndex, Round,
    block::{BlockAPI as _, BlockRef, ExtendedBlock, GENESIS_ROUND, SignedBlock, VerifiedBlock},
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, TrustedCommit},
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{BlockStream, ExtendedSerializedBlock, NetworkService},
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    storage::Store,
    synchronizer::{MAX_ADDITIONAL_BLOCKS, SynchronizerHandle},
};

pub(crate) const COMMIT_LAG_MULTIPLIER: u32 = 5;

/// Authority's network service implementation, agnostic to the actual
/// networking stack used.
pub(crate) struct AuthorityService<C: CoreThreadDispatcher> {
    context: Arc<Context>,
    commit_vote_monitor: Arc<CommitVoteMonitor>,
    block_verifier: Arc<dyn BlockVerifier>,
    synchronizer: Arc<SynchronizerHandle>,
    core_dispatcher: Arc<C>,
    rx_block_broadcaster: broadcast::Receiver<ExtendedBlock>,
    subscription_counter: Arc<SubscriptionCounter>,
    dag_state: Arc<RwLock<DagState>>,
    store: Arc<dyn Store>,
}

impl<C: CoreThreadDispatcher> AuthorityService<C> {
    pub(crate) fn new(
        context: Arc<Context>,
        block_verifier: Arc<dyn BlockVerifier>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        synchronizer: Arc<SynchronizerHandle>,
        core_dispatcher: Arc<C>,
        rx_block_broadcaster: broadcast::Receiver<ExtendedBlock>,
        dag_state: Arc<RwLock<DagState>>,
        store: Arc<dyn Store>,
    ) -> Self {
        let subscription_counter = Arc::new(SubscriptionCounter::new(
            context.clone(),
            core_dispatcher.clone(),
        ));
        Self {
            context,
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher,
            rx_block_broadcaster,
            subscription_counter,
            dag_state,
            store,
        }
    }
}

#[async_trait]
impl<C: CoreThreadDispatcher> NetworkService for AuthorityService<C> {
    async fn handle_send_block(
        &self,
        peer: AuthorityIndex,
        serialized_block: ExtendedSerializedBlock,
    ) -> ConsensusResult<()> {
        fail_point_async!("consensus-rpc-response");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["AuthorityService::handle_stream"])
            .start_timer();
        let peer_hostname = &self.context.committee.authority(peer).hostname;

        // TODO: dedup block verifications, here and with fetched blocks.
        let signed_block: SignedBlock =
            bcs::from_bytes(&serialized_block.block).map_err(ConsensusError::MalformedBlock)?;

        // Reject blocks not produced by the peer.
        if peer != signed_block.author() {
            self.context
                .metrics
                .node_metrics
                .invalid_blocks
                .with_label_values(&[
                    peer_hostname.as_str(),
                    "handle_send_block",
                    "UnexpectedAuthority",
                ])
                .inc();
            let e = ConsensusError::UnexpectedAuthority(signed_block.author(), peer);
            info!("Block with wrong authority from {}: {}", peer, e);
            return Err(e);
        }
        let peer_hostname = &self.context.committee.authority(peer).hostname;

        // Reject blocks failing validations.
        if let Err(e) = self.block_verifier.verify(&signed_block) {
            self.context
                .metrics
                .node_metrics
                .invalid_blocks
                .with_label_values(&[peer_hostname.as_str(), "handle_send_block", e.name()])
                .inc();
            info!("Invalid block from {}: {}", peer, e);
            return Err(e);
        }
        let verified_block = VerifiedBlock::new_verified(signed_block, serialized_block.block);
        let block_ref = verified_block.reference();
        debug!("Received block {} via send block.", block_ref);

        let now = self.context.clock.timestamp_utc_ms();
        let forward_time_drift =
            Duration::from_millis(verified_block.timestamp_ms().saturating_sub(now));
        let latency_to_process_stream =
            Duration::from_millis(now.saturating_sub(verified_block.timestamp_ms()));
        self.context
            .metrics
            .node_metrics
            .latency_to_process_stream
            .with_label_values(&[peer_hostname.as_str()])
            .observe(latency_to_process_stream.as_secs_f64());

        if !self
            .context
            .protocol_config
            .consensus_median_timestamp_with_checkpoint_enforcement()
        {
            // Reject block with timestamp too far in the future.
            if forward_time_drift > self.context.parameters.max_forward_time_drift {
                self.context
                    .metrics
                    .node_metrics
                    .rejected_future_blocks
                    .with_label_values(&[peer_hostname])
                    .inc();
                debug!(
                    "Block {:?} timestamp ({} > {}) is too far in the future, rejected.",
                    block_ref,
                    verified_block.timestamp_ms(),
                    now,
                );
                return Err(ConsensusError::BlockRejected {
                    block_ref,
                    reason: format!(
                        "Block timestamp is too far in the future: {} > {}",
                        verified_block.timestamp_ms(),
                        now
                    ),
                });
            }

            // Wait until the block's timestamp is current.
            if forward_time_drift > Duration::ZERO {
                self.context
                    .metrics
                    .node_metrics
                    .block_timestamp_drift_ms
                    .with_label_values(&[peer_hostname.as_str(), "handle_send_block"])
                    .inc_by(forward_time_drift.as_millis() as u64);
                debug!(
                    "Block {:?} timestamp ({} > {}) is in the future, waiting for {}ms",
                    block_ref,
                    verified_block.timestamp_ms(),
                    now,
                    forward_time_drift.as_millis(),
                );
                sleep(forward_time_drift).await;
            }
        } else {
            self.context
                .metrics
                .node_metrics
                .block_timestamp_drift_ms
                .with_label_values(&[peer_hostname.as_str(), "handle_send_block"])
                .inc_by(forward_time_drift.as_millis() as u64);
        }

        // Observe the block for the commit votes. When local commit is lagging too
        // much, commit sync loop will trigger fetching.
        self.commit_vote_monitor.observe_block(&verified_block);

        // Reject blocks when local commit index is lagging too far from quorum commit
        // index.
        //
        // IMPORTANT: this must be done after observing votes from the block, otherwise
        // observed quorum commit will no longer progress.
        //
        // Since the main issue with too many suspended blocks is memory usage not CPU,
        // it is ok to reject after block verifications instead of before.
        let last_commit_index = self.dag_state.read().last_commit_index();
        let quorum_commit_index = self.commit_vote_monitor.quorum_commit_index();
        // The threshold to ignore block should be larger than commit_sync_batch_size,
        // to avoid excessive block rejections and synchronizations.
        if last_commit_index
            + self.context.parameters.commit_sync_batch_size * COMMIT_LAG_MULTIPLIER
            < quorum_commit_index
        {
            self.context
                .metrics
                .node_metrics
                .rejected_blocks
                .with_label_values(&["commit_lagging"])
                .inc();
            debug!(
                "Block {:?} is rejected because last commit index is lagging quorum commit index too much ({} < {})",
                block_ref, last_commit_index, quorum_commit_index,
            );
            return Err(ConsensusError::BlockRejected {
                block_ref,
                reason: format!(
                    "Last commit index is lagging quorum commit index too much ({last_commit_index} < {quorum_commit_index})",
                ),
            });
        }

        self.context
            .metrics
            .node_metrics
            .verified_blocks
            .with_label_values(&[peer_hostname])
            .inc();

        let missing_ancestors = self
            .core_dispatcher
            .add_blocks(vec![verified_block])
            .await
            .map_err(|_| ConsensusError::Shutdown)?;
        if !missing_ancestors.is_empty() {
            // schedule the fetching of them from this peer
            if let Err(err) = self
                .synchronizer
                .fetch_blocks(missing_ancestors, peer)
                .await
            {
                warn!("Errored while trying to fetch missing ancestors via synchronizer: {err}");
            }
        }

        // After processing the block, process the excluded ancestors

        let mut excluded_ancestors = serialized_block
            .excluded_ancestors
            .into_iter()
            .map(|serialized| bcs::from_bytes::<BlockRef>(&serialized))
            .collect::<Result<Vec<BlockRef>, bcs::Error>>()
            .map_err(ConsensusError::MalformedBlock)?;

        let excluded_ancestors_limit = self.context.committee.size() * 2;
        if excluded_ancestors.len() > excluded_ancestors_limit {
            debug!(
                "Dropping {} excluded ancestor(s) from {} {} due to size limit",
                excluded_ancestors.len() - excluded_ancestors_limit,
                peer,
                peer_hostname,
            );
            excluded_ancestors.truncate(excluded_ancestors_limit);
        }

        self.context
            .metrics
            .node_metrics
            .network_received_excluded_ancestors_from_authority
            .with_label_values(&[peer_hostname])
            .inc_by(excluded_ancestors.len() as u64);

        for excluded_ancestor in &excluded_ancestors {
            let excluded_ancestor_hostname = &self
                .context
                .committee
                .authority(excluded_ancestor.author)
                .hostname;
            self.context
                .metrics
                .node_metrics
                .network_excluded_ancestors_count_by_authority
                .with_label_values(&[excluded_ancestor_hostname])
                .inc();
        }

        let missing_excluded_ancestors = self
            .core_dispatcher
            .check_block_refs(excluded_ancestors)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        if !missing_excluded_ancestors.is_empty() {
            self.context
                .metrics
                .node_metrics
                .network_excluded_ancestors_sent_to_fetch
                .with_label_values(&[peer_hostname])
                .inc_by(missing_excluded_ancestors.len() as u64);

            let synchronizer = self.synchronizer.clone();
            tokio::spawn(async move {
                // schedule the fetching of them from this peer in the background
                if let Err(err) = synchronizer
                    .fetch_blocks(missing_excluded_ancestors, peer)
                    .await
                {
                    warn!(
                        "Errored while trying to fetch missing excluded ancestors via synchronizer: {err}"
                    );
                }
            });
        }

        Ok(())
    }

    async fn handle_subscribe_blocks(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockStream> {
        fail_point_async!("consensus-rpc-response");

        let dag_state = self.dag_state.read();
        // Find recent own blocks that have not been received by the peer.
        // If last_received is a valid and more blocks have been proposed since then,
        // this call is guaranteed to return at least some recent blocks, which
        // will help with liveness.
        let missed_blocks = stream::iter(
            dag_state
                .get_cached_blocks(self.context.own_index, last_received + 1)
                .into_iter()
                .map(|block| ExtendedSerializedBlock {
                    block: block.serialized().clone(),
                    excluded_ancestors: vec![],
                }),
        );

        let broadcasted_blocks = BroadcastedBlockStream::new(
            peer,
            self.rx_block_broadcaster.resubscribe(),
            self.subscription_counter.clone(),
        );

        // Return a stream of blocks that first yields missed blocks as requested, then
        // new blocks.
        Ok(Box::pin(missed_blocks.chain(
            broadcasted_blocks.map(ExtendedSerializedBlock::from),
        )))
    }

    // Handles two types of requests:
    // 1. Missing block for block sync:
    //    - uses highest_accepted_rounds.
    //    - at most max_blocks_per_sync blocks should be returned.
    // 2. Committed block for commit sync:
    //    - does not use highest_accepted_rounds.
    //    - at most max_blocks_per_fetch blocks should be returned.
    async fn handle_fetch_blocks(
        &self,
        peer: AuthorityIndex,
        mut block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>> {
        // This method is used for both commit sync and periodic/live synchronizer.
        // For commit sync, we do not use highest_accepted_rounds and the fetch size is
        // larger.
        let commit_sync_handle = highest_accepted_rounds.is_empty();

        fail_point_async!("consensus-rpc-response");

        // Some quick validation of the requested block refs
        for block in &block_refs {
            if !self.context.committee.is_valid_index(block.author) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: block.author,
                    max: self.context.committee.size(),
                });
            }
            if block.round == GENESIS_ROUND {
                return Err(ConsensusError::UnexpectedGenesisBlockRequested);
            }
        }

        if !self.context.protocol_config.consensus_batched_block_sync() {
            if block_refs.len() > self.context.parameters.max_blocks_per_fetch {
                return Err(ConsensusError::TooManyFetchBlocksRequested(peer));
            }

            if !commit_sync_handle && highest_accepted_rounds.len() != self.context.committee.size()
            {
                return Err(ConsensusError::InvalidSizeOfHighestAcceptedRounds(
                    highest_accepted_rounds.len(),
                    self.context.committee.size(),
                ));
            }

            // For now ask dag state directly
            let blocks = self.dag_state.read().get_blocks(&block_refs);

            // Now check if an ancestor's round is higher than the one that the peer has. If
            // yes, then serve that ancestor blocks up to `MAX_ADDITIONAL_BLOCKS`.
            let mut ancestor_blocks = vec![];
            if !commit_sync_handle {
                let all_ancestors = blocks
                    .iter()
                    .flatten()
                    .flat_map(|block| block.ancestors().to_vec())
                    .filter(|block_ref| highest_accepted_rounds[block_ref.author] < block_ref.round)
                    .take(MAX_ADDITIONAL_BLOCKS)
                    .collect::<Vec<_>>();

                if !all_ancestors.is_empty() {
                    ancestor_blocks = self.dag_state.read().get_blocks(&all_ancestors);
                }
            }

            // Return the serialised blocks & the ancestor blocks
            let result = blocks
                .into_iter()
                .chain(ancestor_blocks)
                .flatten()
                .map(|block| block.serialized().clone())
                .collect::<Vec<_>>();

            return Ok(result);
        }

        // For commit sync, the fetch size is larger. For periodic/live synchronizer,
        // the fetch size is smaller.else { Instead of rejecting the request, we
        // truncate the size to allow an easy update of this parameter in the future.
        if commit_sync_handle {
            block_refs.truncate(self.context.parameters.max_blocks_per_fetch);
        } else {
            block_refs.truncate(self.context.parameters.max_blocks_per_sync);
        }

        // Get requested blocks from store.
        let blocks = if commit_sync_handle {
            // For commit sync, optimize by fetching from store for blocks below GC round
            let gc_round = self.dag_state.read().gc_round();

            // Separate indices for below/above GC while preserving original order
            let mut below_gc_indices = Vec::new();
            let mut above_gc_indices = Vec::new();
            let mut below_gc_refs = Vec::new();
            let mut above_gc_refs = Vec::new();
            for (i, block_ref) in block_refs.iter().enumerate() {
                if block_ref.round < gc_round {
                    below_gc_indices.push(i);
                    below_gc_refs.push(*block_ref);
                } else {
                    above_gc_indices.push(i);
                    above_gc_refs.push(*block_ref);
                }
            }

            let mut blocks: Vec<Option<VerifiedBlock>> = vec![None; block_refs.len()];

            // Fetch blocks below GC from store
            if !below_gc_refs.is_empty() {
                for (idx, block) in below_gc_indices
                    .iter()
                    .zip(self.store.read_blocks(&below_gc_refs)?)
                {
                    blocks[*idx] = block;
                }
            }

            // Fetch blocks at-or-above GC from dag_state
            if !above_gc_refs.is_empty() {
                for (idx, block) in above_gc_indices
                    .iter()
                    .zip(self.dag_state.read().get_blocks(&above_gc_refs))
                {
                    blocks[*idx] = block;
                }
            }

            blocks.into_iter().flatten().collect()
        } else {
            // For periodic or live synchronizer, we respond with requested blocks from the
            // store and with additional blocks from the cache
            block_refs.sort();
            block_refs.dedup();
            let dag_state = self.dag_state.read();
            let mut blocks = dag_state
                .get_blocks(&block_refs)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            // Get additional blocks for authorities with missing block, if they are
            // available in cache. Compute the lowest missing round per
            // requested authority.
            let mut lowest_missing_rounds = BTreeMap::<AuthorityIndex, Round>::new();
            for block_ref in blocks.iter().map(|b| b.reference()) {
                let entry = lowest_missing_rounds
                    .entry(block_ref.author)
                    .or_insert(block_ref.round);
                *entry = (*entry).min(block_ref.round);
            }

            // Retrieve additional blocks per authority, from peer's highest accepted round
            // + 1 to lowest missing round (exclusive) per requested authority. Start with
            //   own blocks.
            let own_index = self.context.own_index;

            // Collect and sort so own_index comes first
            let mut ordered_missing_rounds: Vec<_> = lowest_missing_rounds.into_iter().collect();
            ordered_missing_rounds.sort_by_key(|(auth, _)| if *auth == own_index { 0 } else { 1 });

            for (authority, lowest_missing_round) in ordered_missing_rounds {
                let highest_accepted_round = highest_accepted_rounds[authority];
                if highest_accepted_round >= lowest_missing_round {
                    continue;
                }

                let missing_blocks = dag_state.get_cached_blocks_in_range(
                    authority,
                    highest_accepted_round + 1,
                    lowest_missing_round,
                    self.context
                        .parameters
                        .max_blocks_per_sync
                        .saturating_sub(blocks.len()),
                );
                blocks.extend(missing_blocks);
                if blocks.len() >= self.context.parameters.max_blocks_per_sync {
                    blocks.truncate(self.context.parameters.max_blocks_per_sync);
                    break;
                }
            }

            blocks
        };

        // Return the serialized blocks
        let bytes = blocks
            .into_iter()
            .map(|block| block.serialized().clone())
            .collect::<Vec<_>>();
        Ok(bytes)
    }

    async fn handle_fetch_commits(
        &self,
        _peer: AuthorityIndex,
        commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlock>)> {
        fail_point_async!("consensus-rpc-response");

        // Compute an inclusive end index and bound the maximum number of commits
        // scanned.
        let inclusive_end = commit_range.end().min(
            commit_range.start() + self.context.parameters.commit_sync_batch_size as CommitIndex
                - 1,
        );
        let mut commits = self
            .store
            .scan_commits((commit_range.start()..=inclusive_end).into())?;
        let mut certifier_block_refs = vec![];
        'commit: while let Some(c) = commits.last() {
            let index = c.index();
            let votes = self.store.read_commit_votes(index)?;
            let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            for v in &votes {
                stake_aggregator.add(v.author, &self.context.committee);
            }
            if stake_aggregator.reached_threshold(&self.context.committee) {
                certifier_block_refs = votes;
                break 'commit;
            } else {
                debug!(
                    "Commit {} votes did not reach quorum to certify, {} < {}, skipping",
                    index,
                    stake_aggregator.stake(),
                    stake_aggregator.threshold(&self.context.committee)
                );
                self.context
                    .metrics
                    .node_metrics
                    .commit_sync_fetch_commits_handler_uncertified_skipped
                    .inc();
                commits.pop();
            }
        }
        let certifier_blocks = self
            .store
            .read_blocks(&certifier_block_refs)?
            .into_iter()
            .flatten()
            .collect();
        Ok((commits, certifier_blocks))
    }

    async fn handle_fetch_latest_blocks(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>> {
        fail_point_async!("consensus-rpc-response");

        if authorities.len() > self.context.committee.size() {
            return Err(ConsensusError::TooManyAuthoritiesProvided(peer));
        }

        // Ensure that those are valid authorities
        for authority in &authorities {
            if !self.context.committee.is_valid_index(*authority) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: *authority,
                    max: self.context.committee.size(),
                });
            }
        }

        // Read from the dag state to find the latest blocks.
        // TODO: at the moment we don't look into the block manager for suspended
        // blocks. Ideally we want in the future if we think we would like to
        // tackle the majority of cases.
        let mut blocks = vec![];
        let dag_state = self.dag_state.read();
        for authority in authorities {
            let block = dag_state.get_last_block_for_authority(authority);

            debug!("Latest block for {authority}: {block:?} as requested from {peer}");

            // no reason to serve back the genesis block - it's equal as if it has not
            // received any block
            if block.round() != GENESIS_ROUND {
                blocks.push(block);
            }
        }

        // Return the serialised blocks
        let result = blocks
            .into_iter()
            .map(|block| block.serialized().clone())
            .collect::<Vec<_>>();

        Ok(result)
    }

    async fn handle_get_latest_rounds(
        &self,
        _peer: AuthorityIndex,
    ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
        fail_point_async!("consensus-rpc-response");

        let mut highest_received_rounds = self.core_dispatcher.highest_received_rounds();

        let blocks = self
            .dag_state
            .read()
            .get_last_cached_block_per_authority(Round::MAX);
        let highest_accepted_rounds = blocks
            .into_iter()
            .map(|(block, _)| block.round())
            .collect::<Vec<_>>();

        // Own blocks do not go through the core dispatcher, so they need to be set
        // separately.
        highest_received_rounds[self.context.own_index] =
            highest_accepted_rounds[self.context.own_index];

        Ok((highest_received_rounds, highest_accepted_rounds))
    }
}

struct Counter {
    count: usize,
    subscriptions_by_authority: Vec<usize>,
}

/// Atomically counts the number of active subscriptions to the block broadcast
/// stream, and dispatch commands to core based on the changes.
struct SubscriptionCounter {
    context: Arc<Context>,
    counter: parking_lot::Mutex<Counter>,
    dispatcher: Arc<dyn CoreThreadDispatcher>,
}

impl SubscriptionCounter {
    fn new(context: Arc<Context>, dispatcher: Arc<dyn CoreThreadDispatcher>) -> Self {
        // Set the subscribed peers by default to 0
        for (_, authority) in context.committee.authorities() {
            context
                .metrics
                .node_metrics
                .subscribed_by
                .with_label_values(&[authority.hostname.as_str()])
                .set(0);
        }

        Self {
            counter: parking_lot::Mutex::new(Counter {
                count: 0,
                subscriptions_by_authority: vec![0; context.committee.size()],
            }),
            dispatcher,
            context,
        }
    }

    fn increment(&self, peer: AuthorityIndex) -> Result<(), ConsensusError> {
        let mut counter = self.counter.lock();
        counter.count += 1;
        let original_subscription_by_peer = counter.subscriptions_by_authority[peer];
        counter.subscriptions_by_authority[peer] += 1;
        let mut total_stake = 0;
        for (authority_index, _) in self.context.committee.authorities() {
            if counter.subscriptions_by_authority[authority_index] >= 1
                || self.context.own_index == authority_index
            {
                total_stake += self.context.committee.stake(authority_index);
            }
        }
        // Stake of subscriptions before a new peer was subscribed
        let previous_stake = if original_subscription_by_peer == 0 {
            total_stake - self.context.committee.stake(peer)
        } else {
            total_stake
        };

        let peer_hostname = &self.context.committee.authority(peer).hostname;
        self.context
            .metrics
            .node_metrics
            .subscribed_by
            .with_label_values(&[peer_hostname])
            .set(1);
        // If the subscription count reaches quorum, notify the dispatcher and get ready
        // to propose blocks.
        if !self.context.committee.reached_quorum(previous_stake)
            && self.context.committee.reached_quorum(total_stake)
        {
            self.dispatcher
                .set_quorum_subscribers_exists(true)
                .map_err(|_| ConsensusError::Shutdown)?;
        }
        // Drop the counter after sending the command to the dispatcher
        drop(counter);
        Ok(())
    }

    fn decrement(&self, peer: AuthorityIndex) -> Result<(), ConsensusError> {
        let mut counter = self.counter.lock();
        counter.count -= 1;
        let original_subscription_by_peer = counter.subscriptions_by_authority[peer];
        counter.subscriptions_by_authority[peer] -= 1;
        let mut total_stake = 0;
        for (authority_index, _) in self.context.committee.authorities() {
            if counter.subscriptions_by_authority[authority_index] >= 1
                || self.context.own_index == authority_index
            {
                total_stake += self.context.committee.stake(authority_index);
            }
        }
        // Stake of subscriptions before a peer was dropped
        let previous_stake = if original_subscription_by_peer == 1 {
            total_stake + self.context.committee.stake(peer)
        } else {
            total_stake
        };

        if counter.subscriptions_by_authority[peer] == 0 {
            let peer_hostname = &self.context.committee.authority(peer).hostname;
            self.context
                .metrics
                .node_metrics
                .subscribed_by
                .with_label_values(&[peer_hostname])
                .set(0);
        }

        // If the subscription count drops below quorum, notify the dispatcher to stop
        // proposing blocks.
        if self.context.committee.reached_quorum(previous_stake)
            && !self.context.committee.reached_quorum(total_stake)
        {
            self.dispatcher
                .set_quorum_subscribers_exists(false)
                .map_err(|_| ConsensusError::Shutdown)?;
        }
        // Drop the counter after sending the command to the dispatcher
        drop(counter);
        Ok(())
    }
}

/// Each broadcasted block stream wraps a broadcast receiver for blocks.
/// It yields blocks that are broadcasted after the stream is created.
type BroadcastedBlockStream = BroadcastStream<ExtendedBlock>;

/// Adapted from `tokio_stream::wrappers::BroadcastStream`. The main difference
/// is that this tolerates lags with only logging, without yielding errors.
struct BroadcastStream<T> {
    peer: AuthorityIndex,
    // Stores the receiver across poll_next() calls.
    inner: ReusableBoxFuture<
        'static,
        (
            Result<T, broadcast::error::RecvError>,
            broadcast::Receiver<T>,
        ),
    >,
    // Counts total subscriptions / active BroadcastStreams.
    subscription_counter: Arc<SubscriptionCounter>,
}

impl<T: 'static + Clone + Send> BroadcastStream<T> {
    pub fn new(
        peer: AuthorityIndex,
        rx: broadcast::Receiver<T>,
        subscription_counter: Arc<SubscriptionCounter>,
    ) -> Self {
        if let Err(err) = subscription_counter.increment(peer) {
            match err {
                ConsensusError::Shutdown => {}
                _ => panic!("Unexpected error: {err}"),
            }
        }
        Self {
            peer,
            inner: ReusableBoxFuture::new(make_recv_future(rx)),
            subscription_counter,
        }
    }
}

impl<T: 'static + Clone + Send> Stream for BroadcastStream<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let peer = self.peer;
        let maybe_item = loop {
            let (result, rx) = ready!(self.inner.poll(cx));
            self.inner.set(make_recv_future(rx));

            match result {
                Ok(item) => break Some(item),
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Block BroadcastedBlockStream {} closed", peer);
                    break None;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        "Block BroadcastedBlockStream {} lagged by {} messages",
                        peer, n
                    );
                    continue;
                }
            }
        };
        task::Poll::Ready(maybe_item)
    }
}

impl<T> Drop for BroadcastStream<T> {
    fn drop(&mut self) {
        if let Err(err) = self.subscription_counter.decrement(self.peer) {
            match err {
                ConsensusError::Shutdown => {}
                _ => panic!("Unexpected error: {err}"),
            }
        }
    }
}

async fn make_recv_future<T: Clone>(
    mut rx: broadcast::Receiver<T>,
) -> (
    Result<T, broadcast::error::RecvError>,
    broadcast::Receiver<T>,
) {
    let result = rx.recv().await;
    (result, rx)
}

// TODO: add a unit test for BroadcastStream.

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use consensus_config::AuthorityIndex;
    use parking_lot::{Mutex, RwLock};
    use rstest::rstest;
    use tokio::{sync::broadcast, time::sleep};

    use crate::{
        Round,
        authority_service::AuthorityService,
        block::{BlockAPI, BlockRef, GENESIS_ROUND, SignedBlock, TestBlock, VerifiedBlock},
        commit::{CertifiedCommits, CommitDigest, CommitRange, TrustedCommit},
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        core_thread::{CoreError, CoreThreadDispatcher},
        dag_state::DagState,
        error::ConsensusResult,
        network::{BlockStream, ExtendedSerializedBlock, NetworkClient, NetworkService},
        round_prober::QuorumRound,
        storage::{Store, WriteBatch, mem_store::MemStore},
        synchronizer::Synchronizer,
        test_dag_builder::DagBuilder,
    };

    pub(crate) struct FakeCoreThreadDispatcher {
        blocks: Mutex<Vec<VerifiedBlock>>,
    }

    impl FakeCoreThreadDispatcher {
        pub(crate) fn new() -> Self {
            Self {
                blocks: Mutex::new(vec![]),
            }
        }

        fn get_blocks(&self) -> Vec<VerifiedBlock> {
            self.blocks.lock().clone()
        }
    }

    #[async_trait]
    impl CoreThreadDispatcher for FakeCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            let block_refs = blocks.iter().map(|b| b.reference()).collect();
            self.blocks.lock().extend(blocks);
            Ok(block_refs)
        }

        async fn check_block_refs(
            &self,
            _block_refs: Vec<BlockRef>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            Ok(BTreeSet::new())
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            todo!()
        }

        async fn new_block(&self, _round: Round, _force: bool) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_missing_blocks(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            Ok(Default::default())
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            todo!()
        }

        fn set_propagation_delay_and_quorum_rounds(
            &self,
            _delay: Round,
            _received_quorum_rounds: Vec<QuorumRound>,
            _accepted_quorum_rounds: Vec<QuorumRound>,
        ) -> Result<(), CoreError> {
            todo!()
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            todo!()
        }

        fn highest_received_rounds(&self) -> Vec<Round> {
            todo!()
        }
    }

    #[derive(Default)]
    pub(crate) struct FakeNetworkClient {}

    #[async_trait]
    impl NetworkClient for FakeNetworkClient {
        const SUPPORT_STREAMING: bool = false;

        async fn send_block(
            &self,
            _peer: AuthorityIndex,
            _block: &VerifiedBlock,
            _timeout: Duration,
        ) -> ConsensusResult<()> {
            unimplemented!("Unimplemented")
        }

        async fn subscribe_blocks(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockStream> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_blocks(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_blocks(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn get_latest_rounds(
            &self,
            _peer: AuthorityIndex,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
            unimplemented!("Unimplemented")
        }
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_send_block(#[values(false, true)] median_based_timestamp: bool) {
        let (mut context, _keys) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_median_timestamp_with_checkpoint_enforcement_for_testing(
                median_based_timestamp,
            );
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher::new());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let synchronizer = Synchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
        ));

        // Test delaying blocks with time drift.
        let now = context.clock.timestamp_utc_ms();
        let max_drift = context.parameters.max_forward_time_drift;
        let input_block = VerifiedBlock::new_for_test(
            TestBlock::new(9, 0)
                .set_timestamp_ms(now + max_drift.as_millis() as u64)
                .build(),
        );

        let service = authority_service.clone();
        let serialized = ExtendedSerializedBlock {
            block: input_block.serialized().clone(),
            excluded_ancestors: vec![],
        };

        tokio::spawn(async move {
            service
                .handle_send_block(context.committee.to_authority_index(0).unwrap(), serialized)
                .await
                .unwrap();
        });

        sleep(max_drift / 2).await;

        if !median_based_timestamp {
            assert!(core_dispatcher.get_blocks().is_empty());
            sleep(max_drift).await;
        }

        let blocks = core_dispatcher.get_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], input_block);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_fetch_latest_blocks() {
        // GIVEN
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher::new());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let synchronizer = Synchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            true,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
        ));

        // Create some blocks for a few authorities. Create some equivocations as well
        // and store in dag state.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=10)
            .authorities(vec![AuthorityIndex::new_for_test(2)])
            .equivocate(1)
            .build()
            .persist_layers(dag_state);

        // WHEN
        let authorities_to_request = vec![
            AuthorityIndex::new_for_test(1),
            AuthorityIndex::new_for_test(2),
        ];
        let results = authority_service
            .handle_fetch_latest_blocks(AuthorityIndex::new_for_test(1), authorities_to_request)
            .await;

        // THEN
        let serialised_blocks = results.unwrap();
        for serialised_block in serialised_blocks {
            let signed_block: SignedBlock =
                bcs::from_bytes(&serialised_block).expect("Error while deserialising block");
            let verified_block = VerifiedBlock::new_verified(signed_block, serialised_block);

            assert_eq!(verified_block.round(), 10);
        }
    }

    /// Tests that handle_fetch_blocks preserves the original request order
    /// of block refs when they span the GC boundary — i.e. some are fetched
    /// from the persistent store (below GC) and others from in-memory
    /// dag_state (at or above GC). The interleaved input order must be
    /// maintained in the response.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_fetch_blocks_commit_sync_order_across_gc_boundary() {
        // GIVEN
        let rounds = 20;
        let gc_depth = 5;
        let (mut context, _keys) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_batched_block_sync_for_testing(true);
        context.protocol_config.set_gc_depth_for_testing(gc_depth);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher::new());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let synchronizer = Synchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            true,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store.clone(),
        ));

        // Build DAG and persist all blocks to dag_state
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=rounds)
            .build()
            .persist_layers(dag_state.clone());

        // Also write all blocks to the store so below-GC refs can be found
        let all_blocks = dag_builder.blocks(1..=rounds);
        store
            .write(WriteBatch::new(all_blocks, vec![], vec![], vec![]))
            .expect("Failed to write blocks to store");

        // Set last_commit so gc_round() = leader_round - gc_depth = 15 - 5 = 10
        let leader_round = 15;
        let leader_ref = dag_builder
            .blocks(leader_round..=leader_round)
            .first()
            .unwrap()
            .reference();
        let commit =
            TrustedCommit::new_for_test(1, CommitDigest::MIN, 0, leader_ref, vec![leader_ref]);
        dag_state.write().set_last_commit(commit);

        let gc_round = dag_state.read().gc_round();
        assert!(
            gc_round > GENESIS_ROUND && gc_round < rounds,
            "GC round {gc_round} should be between genesis and max round"
        );

        // Collect blocks per round for easy access
        let mut blocks_by_round: Vec<Vec<VerifiedBlock>> = vec![vec![]; (rounds + 1) as usize];
        for round in 1..=rounds {
            blocks_by_round[round as usize] = dag_builder.blocks(round..=round);
        }

        // Create interleaved block_refs that alternate between below-GC and above-GC
        let below_gc_rounds: Vec<Round> = (1..gc_round).collect();
        let above_gc_rounds: Vec<Round> = (gc_round..=rounds).collect();
        let validators = context.committee.size();
        let mut interleaved_refs = Vec::new();
        let max_pairs = std::cmp::min(below_gc_rounds.len(), above_gc_rounds.len());
        for i in 0..max_pairs {
            let below_round = below_gc_rounds[i];
            let auth_idx = i % validators;
            if auth_idx < blocks_by_round[below_round as usize].len() {
                interleaved_refs.push(blocks_by_round[below_round as usize][auth_idx].reference());
            }
            let above_round = above_gc_rounds[i];
            let auth_idx2 = (i + 1) % validators;
            if auth_idx2 < blocks_by_round[above_round as usize].len() {
                interleaved_refs.push(blocks_by_round[above_round as usize][auth_idx2].reference());
            }
        }

        // Verify we have refs from both sides of the GC boundary
        assert!(
            interleaved_refs.iter().any(|r| r.round < gc_round),
            "Should have refs below GC round"
        );
        assert!(
            interleaved_refs.iter().any(|r| r.round >= gc_round),
            "Should have refs above GC round"
        );

        // WHEN: call handle_fetch_blocks with empty highest_accepted_rounds (commit
        // sync path)
        let peer = context.committee.to_authority_index(1).unwrap();
        let returned_blocks = authority_service
            .handle_fetch_blocks(peer, interleaved_refs.clone(), vec![])
            .await
            .expect("Should return valid serialized blocks");

        // THEN: each returned block should match the corresponding input ref
        assert_eq!(
            returned_blocks.len(),
            interleaved_refs.len(),
            "Should receive all requested blocks"
        );
        for (i, serialized_block) in returned_blocks.into_iter().enumerate() {
            let signed_block: SignedBlock =
                bcs::from_bytes(&serialized_block).expect("Error while deserialising block");
            let verified_block = VerifiedBlock::new_verified(signed_block, serialized_block);
            assert_eq!(
                verified_block.reference(),
                interleaved_refs[i],
                "Block at index {i} should match requested ref. \
                 Expected {:?}, got {:?}",
                interleaved_refs[i],
                verified_block.reference()
            );
        }
    }
}
