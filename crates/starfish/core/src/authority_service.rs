// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::max,
    collections::{BTreeMap, VecDeque},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashSet;
use futures::{Stream, StreamExt, ready, stream, task};
use iota_macros::fail_point_async;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::sync::{Mutex, broadcast, mpsc::Sender};
use tokio_util::sync::ReusableBoxFuture;
use tracing::{debug, error, info, warn};

use crate::{
    CommitIndex, Round, Transaction, VerifiedBlockHeader,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, GENESIS_ROUND, ShardWithProof,
        ShardWithProofAPI, ShardWithProofV1, SignedBlockHeader, TransactionsCommitment,
        VerifiedBlock, VerifiedOwnShard, VerifiedTransactions,
    },
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, TrustedCommit},
    commit_syncer::CommitSyncType,
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    cordial_knowledge::CordialKnowledgeHandle,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    encoder::ShardEncoder,
    error::{ConsensusError, ConsensusResult},
    header_synchronizer::HeaderSynchronizerHandle,
    network::{
        BlockBundleStream, NetworkService, SerializedBlock, SerializedBlockBundle,
        SerializedBlockBundleParts, SerializedHeaderAndTransactions, SerializedTransactionsV1,
        SerializedTransactionsV2, TransactionFetchMode,
    },
    shard_reconstructor::TransactionMessage,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    storage::Store,
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _},
    transactions_synchronizer::TransactionsSynchronizerHandle,
};

pub(crate) const COMMIT_LAG_MULTIPLIER: u32 = 5;

const MAX_FILTER_SIZE: u32 = 100000;

struct FilterForHeaders {
    header_digests: DashSet<BlockHeaderDigest>,
    queue: Mutex<VecDeque<BlockHeaderDigest>>,
}

impl FilterForHeaders {
    fn new() -> Self {
        Self {
            header_digests: DashSet::new(),
            queue: Mutex::new(VecDeque::new()),
        }
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.header_digests.len()
    }

    async fn add_batch(&self, digests: Vec<BlockHeaderDigest>) -> Vec<BlockHeaderDigest> {
        let mut already_inserted = vec![];
        for digest in digests.iter() {
            if !self.header_digests.insert(*digest) {
                already_inserted.push(*digest);
            }
        }
        let mut queue = self.queue.lock().await;
        for digest in digests {
            queue.push_back(digest);
        }
        while queue.len() > MAX_FILTER_SIZE as usize {
            if let Some(removed) = queue.pop_front() {
                self.header_digests.remove(&removed);
            }
        }
        already_inserted
    }
    fn contains(&self, header_digest: &BlockHeaderDigest) -> bool {
        self.header_digests.contains(header_digest)
    }
}

/// Authority's network service implementation, agnostic to the actual
/// networking stack used.
pub(crate) struct AuthorityService<C: CoreThreadDispatcher> {
    context: Arc<Context>,
    commit_vote_monitor: Arc<CommitVoteMonitor>,
    block_verifier: Arc<dyn BlockVerifier>,
    synchronizer: Arc<HeaderSynchronizerHandle>,
    transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
    core_dispatcher: Arc<C>,
    rx_block_broadcaster: broadcast::Receiver<VerifiedBlock>,
    subscription_counter: Arc<SubscriptionCounter>,
    dag_state: Arc<RwLock<DagState>>,
    store: Arc<dyn Store>,
    /// A set contains BlockHeaderDigests for block headers, received from
    /// streaming. It is used to filter the headers if they are received
    /// multiple times. The size is limited by MAX_FILTER_SIZE, elements are
    /// evicted when the threshold is exceeded
    received_block_headers: FilterForHeaders,
    /// Sender to send received transaction messages to the shard reconstructor
    transaction_message_sender: Sender<Vec<TransactionMessage>>,
    /// CordialKnowledge allows to update cordial knowledge about the DAG (which
    /// blocks are know by which peer). In addition, one can retrieve some
    /// useful information such as which headers and shards are needed to a
    /// specific peer
    cordial_knowledge: Arc<CordialKnowledgeHandle>,
}

impl<C: CoreThreadDispatcher> AuthorityService<C> {
    pub(crate) fn new(
        context: Arc<Context>,
        block_verifier: Arc<dyn BlockVerifier>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        header_synchronizer: Arc<HeaderSynchronizerHandle>,
        transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
        core_dispatcher: Arc<C>,
        rx_block_broadcaster: broadcast::Receiver<VerifiedBlock>,
        dag_state: Arc<RwLock<DagState>>,
        store: Arc<dyn Store>,
        transaction_message_sender: Sender<Vec<TransactionMessage>>,
        cordial_knowledge: Arc<CordialKnowledgeHandle>,
    ) -> Self {
        let subscription_counter = Arc::new(SubscriptionCounter::new(
            context.clone(),
            core_dispatcher.clone(),
        ));

        Self {
            context,
            block_verifier,
            commit_vote_monitor,
            synchronizer: header_synchronizer,
            transactions_synchronizer,
            core_dispatcher,
            rx_block_broadcaster,
            subscription_counter,
            dag_state,
            store,
            received_block_headers: FilterForHeaders::new(),
            transaction_message_sender,
            cordial_knowledge,
        }
    }
    fn create_verified_block_and_shard(
        &self,
        peer: AuthorityIndex,
        peer_hostname: &str,
        serialized_block: Bytes,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> ConsensusResult<(VerifiedBlock, Option<ShardWithProof>)> {
        let SerializedHeaderAndTransactions {
            serialized_block_header,
            serialized_transactions,
        } = SerializedHeaderAndTransactions::try_from(SerializedBlock { serialized_block })?;

        let signed_block_header: SignedBlockHeader =
            bcs::from_bytes(&serialized_block_header).map_err(ConsensusError::MalformedHeader)?;

        // Reject blocks not produced by the peer.
        if peer != signed_block_header.author() {
            self.context
                .metrics
                .node_metrics
                .bundles_with_invalid_parts
                .with_label_values(&[peer_hostname, "header", "UnexpectedAuthority"])
                .inc();
            let e = ConsensusError::UnexpectedAuthority(signed_block_header.author(), peer);
            info!("Block with wrong authority from {}: {}", peer, e);
            return Err(e);
        }
        if let Err(e) = self.block_verifier.verify(&signed_block_header) {
            self.context
                .metrics
                .node_metrics
                .bundles_with_invalid_parts
                .with_label_values(&[peer_hostname, "header", e.name()])
                .inc();
            info!("Invalid block header from {}: {}", peer, e);
            return Err(e);
        }

        let (transaction_commitment, our_shard, proof_for_shard) = TransactionsCommitment::compute_merkle_root_shard_and_proof(
            &serialized_transactions,
            &self.context,
            encoder,
        )
            .expect("we should expect correct computation of the transactions commitment, our shard and its proof");
        if signed_block_header.transactions_commitment() != transaction_commitment {
            return Err(ConsensusError::TransactionCommitmentFailure {
                round: signed_block_header.round(),
                author: signed_block_header.author(),
                peer,
            });
        }

        let verified_block_header =
            VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
        let transactions: Vec<Transaction> = bcs::from_bytes(&serialized_transactions)
            .map_err(ConsensusError::MalformedTransactions)?;

        self.block_verifier
            .check_and_verify_transactions(&transactions)?;

        let verified_transactions = VerifiedTransactions::new(
            transactions,
            verified_block_header.transaction_ref(),
            Some(verified_block_header.digest()),
            serialized_transactions,
        );
        let has_transactions = verified_transactions.has_transactions();
        let verified_block = VerifiedBlock::new(verified_block_header, verified_transactions);
        let block_ref = verified_block.reference();
        debug!("Received block {} via stream block bundle.", block_ref);
        let shard_for_core = if has_transactions {
            Some(ShardWithProof::new(
                our_shard,
                proof_for_shard,
                block_ref,
                transaction_commitment,
                self.context.protocol_config.consensus_fast_commit_sync(),
            ))
        } else {
            None
        };
        Ok((verified_block, shard_for_core))
    }

    fn extract_additional_block_headers_from_bundle(
        &self,
        peer: AuthorityIndex,
        peer_hostname: &str,
        mut serialized_headers: Vec<Bytes>,
        block_ref: BlockRef,
    ) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
        let block_round = block_ref.round;
        if serialized_headers.len() > self.context.parameters.max_headers_per_bundle {
            warn!("BlockBundle: {block_ref} exceeds max_headers_per_bundle.");
            serialized_headers.truncate(self.context.parameters.max_headers_per_bundle);
        };

        let mut additional_block_headers = vec![];
        for serialized_header in serialized_headers {
            let digest = VerifiedBlockHeader::compute_digest(&serialized_header);
            if self.received_block_headers.contains(&digest) {
                self.context
                    .metrics
                    .node_metrics
                    .filtered_headers_in_bundles
                    .with_label_values(&[peer_hostname, "handle_subscribed_block_bundle"])
                    .inc();
                continue;
            }

            let signed_block_header: SignedBlockHeader =
                bcs::from_bytes(&serialized_header).map_err(ConsensusError::MalformedHeader)?;

            let header_round = signed_block_header.round();
            if header_round >= block_round {
                let e = Err(ConsensusError::TooBigHeaderRoundInABundle {
                    header_round,
                    block_round,
                });
                self.context
                    .metrics
                    .node_metrics
                    .bundles_with_invalid_parts
                    .with_label_values(&[peer_hostname, "header", "invalid round in header"])
                    .inc();
                info!(
                    "Invalid additional block header from {}: {}",
                    peer,
                    e.as_ref().unwrap_err()
                );
                return e;
            }

            if let Err(e) = self.block_verifier.verify(&signed_block_header) {
                self.context
                    .metrics
                    .node_metrics
                    .bundles_with_invalid_parts
                    .with_label_values(&[peer_hostname, "header", e.name()])
                    .inc();
                info!("Invalid additional block header from {}: {}", peer, e);
                return Err(e);
            }

            let verified_block_header = VerifiedBlockHeader::new_verified_with_digest(
                signed_block_header,
                serialized_header,
                digest,
            );

            additional_block_headers.push(verified_block_header);
        }
        self.context
            .metrics
            .node_metrics
            .valid_headers_in_bundles
            .with_label_values(&[peer_hostname, "handle_subscribed_block_bundle"])
            .inc_by(additional_block_headers.len() as u64);
        Ok(additional_block_headers)
    }
    fn extract_shards_from_bundle(
        &self,
        peer: AuthorityIndex,
        peer_hostname: &str,
        mut serialized_shards: Vec<Bytes>,
        block_ref: BlockRef,
    ) -> ConsensusResult<Vec<ShardWithProof>> {
        let block_round = block_ref.round;
        if serialized_shards.len() > self.context.parameters.max_shards_per_bundle {
            warn!("BlockBundle: {block_ref} exceeds max_shards_per_bundle.");
            serialized_shards.truncate(self.context.parameters.max_shards_per_bundle);
        }

        let mut verified_shards: Vec<ShardWithProof> = vec![];
        for serialized_shard in &serialized_shards {
            let shard: ShardWithProof =
                if !self.context.protocol_config.consensus_fast_commit_sync() {
                    // For backward compatibility, we still support ShardWithProofV1 during the
                    // epoch during which nodes are upgraded to a new software version. Peers
                    // running an old version will still send ShardWithProofV1 without the enum
                    // wrapping. We can remove this support after we are sure
                    // all peers have been updated to send versioned ShardWithProof.
                    let shard_v1: ShardWithProofV1 = bcs::from_bytes(serialized_shard)
                        .map_err(ConsensusError::MalformedShard)?;
                    ShardWithProof::V1(shard_v1)
                } else {
                    bcs::from_bytes(serialized_shard).map_err(ConsensusError::MalformedShard)?
                };

            if shard.round() >= block_round {
                let e = ConsensusError::TooBigShardRoundInABundle {
                    shard_round: shard.round(),
                    block_round,
                };
                self.context
                    .metrics
                    .node_metrics
                    .bundles_with_invalid_parts
                    .with_label_values(&[peer_hostname, "shard", e.name()])
                    .inc();
                info!("Invalid shard from {}: {}", peer, e);
                return Err(e);
            }

            let proof_check = TransactionsCommitment::check_merkle_proof(
                shard.clone(),
                self.context.committee.size(),
                peer.value(),
            );
            if proof_check {
                verified_shards.push(shard);
            } else {
                let e = ConsensusError::IncorrectShardProof {
                    peer,
                    round: shard.round(),
                };
                self.context
                    .metrics
                    .node_metrics
                    .bundles_with_invalid_parts
                    .with_label_values(&[peer_hostname, "shard", e.name()])
                    .inc();
                info!("Invalid shard from {}: {}", peer, e);
                return Err(e);
            }
        }
        self.context
            .metrics
            .node_metrics
            .valid_shards_in_bundles
            .with_label_values(&[peer_hostname, "handle_subscribed_block_bundle"])
            .inc_by(verified_shards.len() as u64);
        Ok(verified_shards)
    }
    fn ensure_commit_lag_within_threshold(&self, block_ref: BlockRef) -> ConsensusResult<()> {
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
        Ok(())
    }
    async fn add_digests_to_filter(
        &self,
        peer_hostname: &str,
        additional_block_headers: &mut Vec<VerifiedBlockHeader>,
        block_ref: BlockRef,
    ) {
        let mut digests_to_add_to_filter = vec![];
        for block_header in additional_block_headers.iter() {
            digests_to_add_to_filter.push(block_header.digest())
        }
        digests_to_add_to_filter.push(block_ref.digest);
        let digests_to_exclude = self
            .received_block_headers
            .add_batch(digests_to_add_to_filter)
            .await;
        // Exclude digests that are already in the filter from the additional headers
        // We rely on the fact that digests_to_exclude is a subsequence of
        // additional_block_headers
        let mut index = 0;
        additional_block_headers.retain(|block_header| {
            if index < digests_to_exclude.len()
                && block_header.digest() == digests_to_exclude[index]
            {
                index += 1;
                false
            } else {
                true
            }
        });
        self.context
            .metrics
            .node_metrics
            .received_unique_headers_from_bundles
            .with_label_values(&[peer_hostname, "handle_subscribed_block_bundle"])
            .inc_by(additional_block_headers.len() as u64);
        self.context
            .metrics
            .node_metrics
            .processed_duplicated_headers_in_bundles
            .with_label_values(&[peer_hostname, "handle_subscribed_block_bundle"])
            .inc_by(digests_to_exclude.len() as u64);
        for header in additional_block_headers.iter() {
            self.context
                .metrics
                .node_metrics
                .additional_headers_round_gap
                .observe(block_ref.round.saturating_sub(header.round()) as f64);
        }
    }

    /// Finds the highest commit index in the commit range up to search_up_to
    /// that can be certified with available votes. Returns the highest
    /// certifiable commit index and the block refs (votes) that certify it,
    /// or None if no certifiable commit is found.
    fn find_highest_certifiable_commit_in_range(
        &self,
        commit_range: &CommitRange,
        mut search_up_to: CommitIndex,
        commit_sync_type: &CommitSyncType,
    ) -> ConsensusResult<Option<(CommitIndex, Vec<BlockRef>)>> {
        loop {
            // Find the highest index with at least some votes, up to our search bound.
            let Some(index_with_votes) = self
                .store
                .read_highest_commit_index_with_votes(search_up_to)?
            else {
                // No votes found for any index in the range.
                return Ok(None);
            };

            // If the index with votes is below our commit range start, there are no
            // certifiable commits.
            if index_with_votes < commit_range.start() {
                return Ok(None);
            }

            let votes = self.store.read_commit_votes(index_with_votes)?;
            let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            for v in &votes {
                stake_aggregator.add(v.author, &self.context.committee);
            }
            if stake_aggregator.reached_threshold(&self.context.committee) {
                self.context
                    .metrics
                    .node_metrics
                    .commit_sync_fetch_commits_handler_uncertified_skipped
                    .with_label_values(&[commit_sync_type.as_str()])
                    .inc_by((search_up_to - index_with_votes) as u64);
                return Ok(Some((index_with_votes, votes)));
            } else {
                debug!(
                    "Commit {} votes did not reach quorum to certify, {} < {}, skipping",
                    index_with_votes,
                    stake_aggregator.stake(),
                    stake_aggregator.threshold(&self.context.committee)
                );
                self.context
                    .metrics
                    .node_metrics
                    .commit_sync_fetch_commits_handler_uncertified_skipped
                    .with_label_values(&[commit_sync_type.as_str()])
                    .inc_by((search_up_to - index_with_votes + 1) as u64);
                // Continue searching from index_with_votes - 1
                search_up_to = index_with_votes.saturating_sub(1);
                if search_up_to < commit_range.start() {
                    return Ok(None);
                }
            }
        }
    }

    /// Finds the lowest commit index from search_from that can be certified
    /// with available votes. Returns the lowest certifiable commit index and
    /// the block refs (votes) that certify it, or None if no certifiable
    /// commit is found.
    fn find_lowest_certifiable_commit_from(
        &self,
        search_from: CommitIndex,
        search_up_to: CommitIndex,
        commit_sync_type: &CommitSyncType,
    ) -> ConsensusResult<Option<(CommitIndex, Vec<BlockRef>)>> {
        let mut current_search_from = search_from;
        loop {
            let Some(index_with_votes) = self
                .store
                .read_lowest_commit_index_with_votes(current_search_from)?
            else {
                return Ok(None);
            };

            if index_with_votes > search_up_to {
                return Ok(None);
            }

            let votes = self.store.read_commit_votes(index_with_votes)?;
            let mut stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            for v in &votes {
                stake_aggregator.add(v.author, &self.context.committee);
            }
            if stake_aggregator.reached_threshold(&self.context.committee) {
                self.context
                    .metrics
                    .node_metrics
                    .commit_sync_fetch_commits_handler_uncertified_skipped
                    .with_label_values(&[commit_sync_type.as_str()])
                    .inc_by((index_with_votes - current_search_from) as u64);
                return Ok(Some((index_with_votes, votes)));
            } else {
                debug!(
                    "Commit {} votes did not reach quorum to certify, {} < {}, skipping",
                    index_with_votes,
                    stake_aggregator.stake(),
                    stake_aggregator.threshold(&self.context.committee)
                );
                self.context
                    .metrics
                    .node_metrics
                    .commit_sync_fetch_commits_handler_uncertified_skipped
                    .with_label_values(&[commit_sync_type.as_str()])
                    .inc_by((index_with_votes - current_search_from + 1) as u64);
                // Continue searching from index_with_votes + 1
                current_search_from = index_with_votes.saturating_add(1);
            }
        }
    }
}

#[async_trait]
impl<C: CoreThreadDispatcher> NetworkService for AuthorityService<C> {
    async fn handle_subscribed_block_bundle(
        &self,
        peer: AuthorityIndex,
        serialized_block_bundle: SerializedBlockBundle,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
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
        let mut serialized_block_bundle_parts =
            SerializedBlockBundleParts::try_from(serialized_block_bundle)?;

        // 1. Create a verified block and make some preliminary checks
        let (verified_block, shard_for_core) = self.create_verified_block_and_shard(
            peer,
            peer_hostname,
            serialized_block_bundle_parts.serialized_block.clone(),
            encoder,
        )?;
        let block_ref = verified_block.reference();
        let transaction_ref = verified_block.transaction_ref();
        let gen_transaction_ref = if self.context.protocol_config.consensus_fast_commit_sync() {
            GenericTransactionRef::from(transaction_ref)
        } else {
            GenericTransactionRef::from(block_ref)
        };
        // 2. Record timestamp drift metric (NEW mode - no waiting or rejection)
        let now = self.context.clock.timestamp_utc_ms();
        let forward_time_drift =
            Duration::from_millis(verified_block.timestamp_ms().saturating_sub(now));
        self.context
            .metrics
            .node_metrics
            .block_timestamp_drift_ms
            .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
            .inc_by(forward_time_drift.as_millis() as u64);
        let latency_to_process_stream =
            Duration::from_millis(now.saturating_sub(verified_block.timestamp_ms()));
        self.context
            .metrics
            .node_metrics
            .latency_to_process_stream
            .with_label_values(&[peer_hostname.as_str()])
            .observe(latency_to_process_stream.as_secs_f64());

        // 3. Create block headers from bytes from a bundle

        let serialized_headers =
            std::mem::take(&mut serialized_block_bundle_parts.serialized_headers);
        let mut additional_block_headers = self.extract_additional_block_headers_from_bundle(
            peer,
            peer_hostname,
            serialized_headers,
            block_ref,
        )?;

        // 4. Collect shards from a bundle and check their proofs.

        let serialized_shards =
            std::mem::take(&mut serialized_block_bundle_parts.serialized_shards);
        let verified_shards =
            self.extract_shards_from_bundle(peer, peer_hostname, serialized_shards, block_ref)?;

        // 5. Observe headers and the block for the commit votes. When local commit is
        // lagging too much, commit sync loop will trigger fetching.
        for block_header in additional_block_headers.iter() {
            self.commit_vote_monitor.observe_block(block_header);
        }
        self.commit_vote_monitor.observe_block(&verified_block);

        // 6. Reject blocks when local commit index is lagging too far from quorum
        //    commit index.
        //
        // IMPORTANT: this must be done after observing votes from the block, otherwise
        // observed quorum commit will no longer progress.
        self.ensure_commit_lag_within_threshold(block_ref)?;

        self.context
            .metrics
            .node_metrics
            .verified_blocks
            .with_label_values(&[peer_hostname])
            .inc();

        // 7. Add digests to filter. Exclude from the vector those that are already
        //    inserted
        self.add_digests_to_filter(peer_hostname, &mut additional_block_headers, block_ref)
            .await;

        // 8. Prepare transaction messages for shard reconstructor and send them
        let transaction_messages = TransactionMessage::create_transaction_messages(
            &verified_block,
            &verified_shards,
            peer.value(),
        );
        if let Err(e) = self
            .transaction_message_sender
            .send(transaction_messages)
            .await
        {
            warn!("Failed to send transaction messages to shard reconstructor: {e}");
        }

        // 9. Add additional headers from bundle to dag, receive missing ancestors for
        // them. Normally, there should be no missing ancestors, as the headers are
        // sent in order of increasing rounds.
        let (mut missing_ancestors, mut missing_committed_txns) = self
            .core_dispatcher
            .add_block_headers(
                additional_block_headers.clone(),
                DataSource::BlockBundleStream,
            )
            .await
            .map_err(|_| ConsensusError::Shutdown)?;
        self.context
            .metrics
            .node_metrics
            .missing_ancestors_from_streaming
            .with_label_values(&["headers"])
            .observe(missing_ancestors.len() as f64);

        // 10. Add the block to dag, add its missing ancestors to the set
        let (missing_block_ancestors, missing_block_committed_transactions) = self
            .core_dispatcher
            .add_blocks(vec![verified_block], DataSource::BlockStreaming)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;
        self.context
            .metrics
            .node_metrics
            .missing_ancestors_from_streaming
            .with_label_values(&["block"])
            .observe(missing_block_ancestors.len() as f64);

        missing_ancestors.extend(missing_block_ancestors);
        missing_committed_txns.extend(missing_block_committed_transactions);

        for missing_block_ref in missing_ancestors.iter() {
            self.context
                .metrics
                .node_metrics
                .missing_ancestors_from_streaming_round_gap
                .observe(block_ref.round as f64 - missing_block_ref.round as f64);
        }

        // 11. Add our shard from the received block and its proof to the dag_state
        // only if it contains transactions
        if let Some(shard_for_core) = shard_for_core {
            let serialized_shard_for_core: Bytes = match shard_for_core {
                // For backward compatibility, we still support ShardWithProofV1 during the
                // epoch during which nodes are upgraded to a new software version. Because of
                // peers running an old version will still need to send
                // ShardWithProofV1 without the enum wrapping. We can remove this
                // support after we are sure all peers have been updated to send
                // versioned ShardWithProof.
                ShardWithProof::V1(shard_v1)
                    if !self.context.protocol_config.consensus_fast_commit_sync() =>
                {
                    bcs::to_bytes(&shard_v1)
                        .map_err(ConsensusError::SerializationFailure)?
                        .into()
                }
                _ => bcs::to_bytes(&shard_for_core)
                    .map_err(ConsensusError::SerializationFailure)?
                    .into(),
            };
            let shard_for_core = VerifiedOwnShard {
                serialized_shard: serialized_shard_for_core,
                gen_transaction_ref,
            };
            self.core_dispatcher
                .add_shards(vec![shard_for_core])
                .await
                .map_err(|_| ConsensusError::Shutdown)?;
        }

        // 12. Report useful info for cordial and connection knowledge
        let block_round = block_ref.round;
        self.cordial_knowledge.report_useful_authors(
            peer,
            &serialized_block_bundle_parts,
            &additional_block_headers,
            &missing_ancestors,
            block_round,
        )?;

        // 13. schedule the fetching of missing ancestors (if any) from this peer
        if !missing_ancestors.is_empty() {
            if let Err(err) = self
                .synchronizer
                .fetch_headers(missing_ancestors, peer)
                .await
            {
                warn!("Errored while trying to fetch missing ancestors via synchronizer: {err}");
            }
        }

        // 14. schedule the fetching of missing committed transactions (if any)
        if !missing_committed_txns.is_empty() {
            if let Err(err) = self
                .transactions_synchronizer
                .fetch_transactions(missing_committed_txns)
                .await
            {
                warn!(
                    "Errored while trying to fetch missing transactions via transactions synchronizer: {err}"
                );
            }
        }
        Ok(())
    }

    async fn handle_subscribe_block_bundles_request(
        &self,
        peer: AuthorityIndex,
        last_received: Round,
    ) -> ConsensusResult<BlockBundleStream> {
        fail_point_async!("consensus-rpc-response");

        let dag_state = self.dag_state.read();
        // Find recent own blocks that have not been received by the peer.
        // If last_received is a valid and more blocks have been proposed since then,
        // this call is guaranteed to return at least some recent blocks, which
        // will help with liveness.
        let missed_blocks = stream::iter(
            dag_state
                .get_own_cached_blocks(last_received + 1)
                .into_iter()
                .filter_map(|block| match SerializedBlockBundle::try_from(block) {
                    Ok(block_bundle) => Some(block_bundle),
                    Err(e) => {
                        tracing::error!("Failed to serialize block bundle from cache: {e}");
                        None
                    }
                }),
        );

        let broadcasted_blocks = BroadcastedBlockStream::new(
            peer,
            self.rx_block_broadcaster.resubscribe(),
            self.subscription_counter.clone(),
        );
        let context = self.context.clone();
        let connection_knowledge = self.cordial_knowledge.connection_knowledge(peer);
        // Return a stream of blocks that first yields missed blocks as requested, then
        // new blocks.
        Ok(Box::pin(missed_blocks.chain({
            broadcasted_blocks.filter_map(move |block| {
                let context = context.clone();
                let connection_knowledge = connection_knowledge.clone();
                async move {
                    let ts = block.timestamp_ms();

                    let block_bundle = {
                        let mut conn = connection_knowledge.write();
                        conn.create_bundle(block)
                    };

                    let now = context.clock.timestamp_utc_ms();
                    context
                        .metrics
                        .node_metrics
                        .delay_in_sending_blocks
                        .observe((now - ts) as f64);

                    match SerializedBlockBundle::try_from(block_bundle) {
                        Ok(serialized_block_bundle) => Some(serialized_block_bundle),
                        Err(e) => {
                            tracing::error!("Failed to serialize block bundle from broadcast: {e}");
                            None
                        }
                    }
                }
            })
        })))
    }

    /// Handles two types of fetch headers requests:
    /// 1. Missing block headers for regular sync:
    ///    - uses highest_accepted_rounds.
    ///    - at most max_blocks_per_regular_sync blocks should be returned.
    /// 2. Committed block headers for commit sync:
    ///    - does not use highest_accepted_rounds.
    ///    - at most max_blocks_per_commit_sync blocks should be returned.
    async fn handle_fetch_headers(
        &self,
        peer: AuthorityIndex,
        mut block_refs: Vec<BlockRef>,
        highest_accepted_rounds: Vec<Round>,
    ) -> ConsensusResult<Vec<Bytes>> {
        fail_point_async!("consensus-rpc-response");

        // Some quick validation of the requested block refs
        ConsensusError::quick_validation_requested_block_refs(
            &block_refs,
            peer,
            &self.context.committee,
        )?;

        if !highest_accepted_rounds.is_empty()
            && highest_accepted_rounds.len() != self.context.committee.size()
        {
            return Err(ConsensusError::InvalidSizeOfHighestAcceptedRounds(
                highest_accepted_rounds.len(),
                self.context.committee.size(),
            ));
        }

        // This method is used for both commit sync and periodic/live synchronizer.
        // For commit sync, we do not use highest_accepted_rounds and the fetch size is
        // larger.
        let commit_sync_handle = highest_accepted_rounds.is_empty();

        // For commit sync, the fetch size is larger. For periodic/live synchronizer,
        // the fetch size is smaller. Instead of rejecting the request, we truncate
        // the size to allow an easy update of this parameter in the future.
        let max_fetch_size = if commit_sync_handle {
            self.context.parameters.max_headers_per_commit_sync_fetch
        } else {
            self.context.parameters.max_headers_per_regular_sync_fetch
        };

        if block_refs.len() > max_fetch_size {
            warn!(
                "Truncated fetch headers request from {} to {} blocks for peer {}",
                block_refs.len(),
                max_fetch_size,
                peer
            );
            block_refs.truncate(max_fetch_size);
        }

        // Get requested block headers from store.
        let serialized_headers = if commit_sync_handle {
            // For commit sync, optimize by fetching from store for headers below GC round
            let gc_round = self.dag_state.read().gc_round_for_last_solid_commit();

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

            let mut headers: Vec<Option<Bytes>> = vec![None; block_refs.len()];

            // Read headers below GC from store
            if !below_gc_refs.is_empty() {
                for (idx, header) in below_gc_indices
                    .iter()
                    .zip(self.store.read_serialized_block_headers(&below_gc_refs)?)
                {
                    headers[*idx] = header;
                }
            }

            // Read headers at-or-above GC from dag_state
            if !above_gc_refs.is_empty() {
                for (idx, header) in above_gc_indices.iter().zip(
                    self.dag_state
                        .read()
                        .get_serialized_block_headers(&above_gc_refs),
                ) {
                    headers[*idx] = header;
                }
            }

            headers.into_iter().flatten().collect()
        } else {
            // For periodic or live synchronizer, we respond with requested blocks from the
            // store and with additional blocks from the cache
            block_refs.sort();
            block_refs.dedup();
            let dag_state = self.dag_state.read();
            let mut headers = dag_state
                .get_serialized_block_headers(&block_refs)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            // Get additional blocks for authorities with missing block, if they are
            // available in cache. Compute the lowest missing round per
            // requested authority.
            let mut lowest_missing_rounds = BTreeMap::<AuthorityIndex, Round>::new();
            for block_ref in block_refs.iter() {
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

                let missing_headers = dag_state.get_cached_block_headers_in_range(
                    authority,
                    highest_accepted_round + 1,
                    lowest_missing_round,
                    self.context
                        .parameters
                        .max_headers_per_regular_sync_fetch
                        .saturating_sub(headers.len()),
                );
                let serialized_missing_headers: Vec<_> = missing_headers
                    .into_iter()
                    .map(|header| header.serialized().clone())
                    .collect();
                headers.extend(serialized_missing_headers);
                if headers.len() >= self.context.parameters.max_headers_per_regular_sync_fetch {
                    headers.truncate(self.context.parameters.max_headers_per_regular_sync_fetch);
                    break;
                }
            }

            headers
        };
        Ok(serialized_headers)
    }

    /// Handles fetch requests for commit data and their certifying block
    /// headers.
    // The range for returned trusted commits starts at the same index, but the end
    // can be different, bigger for fast sync and smaller for regular.
    async fn handle_fetch_commits(
        &self,
        _peer: AuthorityIndex,
        commit_range: CommitRange,
        commit_sync_type: CommitSyncType,
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)> {
        fail_point_async!("consensus-rpc-response");

        // TODO: This gate can be removed once consensus_fast_commit_sync is enabled on
        // all networks. Fast commit sync type is controlled by the client, so
        // we need to validate that the protocol supports it before processing.
        if matches!(commit_sync_type, CommitSyncType::Fast)
            && !self.context.protocol_config.consensus_fast_commit_sync()
        {
            return Err(ConsensusError::FastCommitSyncNotEnabled);
        }

        // Bound the range based on sync type.
        let batch_size = commit_sync_type.commit_sync_batch_size(&self.context);
        let inclusive_bound = commit_range
            .end()
            .min(commit_range.start() + batch_size as CommitIndex - 1);

        // Find certifiable commit based on sync type
        let find_certifiable_commit = |commit_sync_type: &CommitSyncType| -> ConsensusResult<Option<(CommitIndex, Vec<BlockRef>)>> {
            match commit_sync_type {
                CommitSyncType::Regular => self.find_highest_certifiable_commit_in_range(&commit_range, inclusive_bound, commit_sync_type),
                CommitSyncType::Fast => {
                    let search_up_to = inclusive_bound + batch_size as CommitIndex;
                    self.find_lowest_certifiable_commit_from(inclusive_bound, search_up_to, commit_sync_type)
                }
            }
        };

        let Some((new_commit_inclusive_end, certifier_block_refs)) =
            find_certifiable_commit(&commit_sync_type)?
        else {
            return Ok((vec![], vec![]));
        };
        let commit_range_length = new_commit_inclusive_end - commit_range.start() + 1;
        match commit_sync_type {
            CommitSyncType::Regular => {
                if commit_range_length > batch_size as CommitIndex {
                    error!(
                        "Commit range exceeded limit after scanning during regular sync: {} > {}",
                        commit_range_length, batch_size
                    );
                    return Err(ConsensusError::CommitRangeExceededAfterScanning {
                        count: commit_range_length,
                        limit: batch_size as CommitIndex,
                        sync_type: "regular",
                    });
                }
            }
            CommitSyncType::Fast => {
                if commit_range_length > 2 * batch_size as CommitIndex {
                    error!(
                        "Commit range exceeded limit after scanning during fast sync: {} > {}",
                        commit_range_length,
                        2 * batch_size
                    );
                    return Err(ConsensusError::CommitRangeExceededAfterScanning {
                        count: commit_range_length,
                        limit: 2 * batch_size as CommitIndex,
                        sync_type: "fast",
                    });
                }
            }
        }

        // Then scan commits up to the certifiable index
        let commits = self
            .store
            .scan_commits((commit_range.start()..=new_commit_inclusive_end).into())?;
        // Try reading from voting block headers storage first, then fallback to regular
        // block headers for any that weren't found.
        let voting_headers = self
            .store
            .read_voting_block_headers(&certifier_block_refs)?;

        // Collect refs that weren't found in voting storage
        let missing_refs: Vec<BlockRef> = certifier_block_refs
            .iter()
            .zip(voting_headers.iter())
            .filter_map(|(r, h)| if h.is_none() { Some(*r) } else { None })
            .collect();

        // Track metrics for voting storage hits vs fallbacks
        let voting_hits = voting_headers.iter().filter(|h| h.is_some()).count();
        self.context
            .metrics
            .node_metrics
            .commit_sync_voting_block_headers_hits
            .inc_by(voting_hits as u64);

        // Read missing headers from regular block storage
        let fallback_headers = if !missing_refs.is_empty() {
            self.context
                .metrics
                .node_metrics
                .commit_sync_voting_block_headers_fallbacks
                .inc_by(missing_refs.len() as u64);
            self.store.read_verified_block_headers(&missing_refs)?
        } else {
            vec![]
        };

        // Merge results: use voting headers where available, fallback headers otherwise
        let mut fallback_iter = fallback_headers.into_iter();
        let certifier_block_headers: Vec<VerifiedBlockHeader> = voting_headers
            .into_iter()
            .zip(certifier_block_refs.iter())
            .map(|(h, block_ref)| {
                h.or_else(|| fallback_iter.next().flatten()).ok_or(
                    ConsensusError::MissingVotingBlockHeaderInStorage {
                        block_ref: *block_ref,
                    },
                )
            })
            .collect::<ConsensusResult<Vec<_>>>()?;

        Ok((commits, certifier_block_headers))
    }

    async fn handle_fetch_commits_and_transactions(
        &self,
        peer: AuthorityIndex,
        commit_range: CommitRange,
    ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
        fail_point_async!("consensus-rpc-response");

        // TODO: This gate can be removed once consensus_fast_commit_sync is enabled on
        // all networks. This endpoint is gated by the
        // consensus_fast_commit_sync feature flag as it is more expensive than
        // just fetching commits or headers.
        if !self.context.protocol_config.consensus_fast_commit_sync() {
            return Err(ConsensusError::FastCommitSyncNotEnabled);
        }

        let (commits, certifier_block_headers) = self
            .handle_fetch_commits(peer, commit_range, CommitSyncType::Fast)
            .await?;

        let transaction_refs: Vec<GenericTransactionRef> = commits
            .iter()
            .flat_map(|commit| commit.committed_transactions())
            .collect();

        let serialized_transactions = self
            .handle_fetch_transactions(peer, transaction_refs, TransactionFetchMode::FastCommitSync)
            .await?;

        let serialized_commits: Vec<Bytes> = commits
            .into_iter()
            .map(|c| c.serialized().clone())
            .collect();

        let serialized_headers: Vec<Bytes> = certifier_block_headers
            .into_iter()
            .map(|h| h.serialized().clone())
            .collect();

        Ok((
            serialized_commits,
            serialized_headers,
            serialized_transactions,
        ))
    }

    async fn handle_fetch_latest_block_headers(
        &self,
        peer: AuthorityIndex,
        authorities: Vec<AuthorityIndex>,
    ) -> ConsensusResult<Vec<Bytes>> {
        fail_point_async!("consensus-rpc-response");

        if authorities.len() > self.context.committee.size() {
            return Err(ConsensusError::TooManyAuthoritiesProvided(peer));
        }

        // Ensure that those are valid authorities
        ConsensusError::quick_validation_authority_indices(&authorities, &self.context.committee)?;

        // Read from the dag state to find the latest block headers.
        // TODO: at the moment we don't look into the block manager for suspended
        // block headers. Ideally we want in the future if we think we would like to
        // tackle the majority of cases.
        let mut block_headers = vec![];
        let dag_state = self.dag_state.read();
        for authority in authorities {
            let block_header = dag_state.get_last_block_header_for_authority(authority);

            debug!(
                "Latest block header for {authority}: {block_header:?} as requested from {peer}"
            );

            // no reason to serve back the genesis block - it's equal as if it has not
            // received any block
            if block_header.round() != GENESIS_ROUND {
                block_headers.push(block_header);
            }
        }

        // Return the serialised blocks
        let result = block_headers
            .into_iter()
            .map(|block_header| block_header.serialized().clone())
            .collect::<Vec<_>>();

        Ok(result)
    }

    async fn handle_fetch_transactions(
        &self,
        peer: AuthorityIndex,
        mut committed_transactions_refs: Vec<GenericTransactionRef>,
        fetch_mode: TransactionFetchMode,
    ) -> ConsensusResult<Vec<Bytes>> {
        fail_point_async!("consensus-rpc-response");

        if committed_transactions_refs.is_empty() {
            return Ok(Vec::new());
        }

        // Apply truncation based on fetch mode
        match fetch_mode {
            TransactionFetchMode::FastCommitSync => {
                // TODO: This gate can be removed once consensus_fast_commit_sync is enabled on
                // all networks. FastCommitSync mode is controlled by the
                // client, so we need to validate that the protocol supports it
                // before processing. No truncation for fast commit sync - all
                // transactions referenced by commits must be fetched.
                if !self.context.protocol_config.consensus_fast_commit_sync() {
                    return Err(ConsensusError::FastCommitSyncNotEnabled);
                }
            }
            TransactionFetchMode::TransactionSync => {
                let max_transactions = max(
                    self.context
                        .parameters
                        .max_transactions_per_commit_sync_fetch,
                    self.context
                        .parameters
                        .max_transactions_per_regular_sync_fetch,
                );

                if committed_transactions_refs.len() > max_transactions {
                    committed_transactions_refs.truncate(max_transactions);
                }
            }
        }

        // Some quick validation of the requested transactions refs
        ConsensusError::quick_validation_requested_tx_refs(
            &committed_transactions_refs,
            peer,
            &self.context.committee,
        )?;

        // Optimize by reading from store for transactions below GC round
        let gc_round = self.dag_state.read().gc_round_for_last_solid_commit();

        // Partition committed_transactions_refs into those below and at-or-above GC
        // round
        let (below_gc, above_gc): (Vec<_>, Vec<_>) = committed_transactions_refs
            .iter()
            .cloned()
            .partition(|gen_tx_ref| gen_tx_ref.round() < gc_round);

        // Fetch transactions below GC from store
        let store_transactions = if !below_gc.is_empty() {
            self.store
                .read_serialized_transactions(&below_gc)?
                .into_iter()
                .zip(below_gc)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        // Fetch transactions at-or-above GC from dag_state
        let dag_transactions = if !above_gc.is_empty() {
            self.dag_state
                .read()
                .get_serialized_transactions(&above_gc)
                .into_iter()
                .zip(above_gc)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        // Combine and serialize the results
        let mut result = Vec::new();
        for (opt_serialized_tx, gen_ref) in store_transactions
            .into_iter()
            .chain(dag_transactions.into_iter())
        {
            if let Some(serialized_tx) = opt_serialized_tx {
                let serialized = if !self.context.protocol_config.consensus_fast_commit_sync() {
                    if let GenericTransactionRef::BlockRef(block_ref) = gen_ref {
                        bcs::to_bytes(&SerializedTransactionsV1 {
                            block_ref,
                            serialized_transactions: serialized_tx,
                        })
                        .map_err(ConsensusError::SerializationFailure)?
                    } else {
                        return Err(ConsensusError::TransactionRefVariantMismatch {
                            protocol_flag_enabled: false,
                            expected_variant: "BlockRef",
                            received_variant: gen_ref.variant_name(),
                        });
                    }
                } else if let GenericTransactionRef::TransactionRef(transaction_ref) = gen_ref {
                    bcs::to_bytes(&SerializedTransactionsV2 {
                        transaction_ref,
                        serialized_transactions: serialized_tx,
                    })
                    .map_err(ConsensusError::SerializationFailure)?
                } else {
                    return Err(ConsensusError::TransactionRefVariantMismatch {
                        protocol_flag_enabled: true,
                        expected_variant: "TransactionRef",
                        received_variant: gen_ref.variant_name(),
                    });
                };
                result.push(Bytes::from(serialized));
            }
        }

        Ok(result)
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

        if counter.subscriptions_by_authority[peer] == 0 {
            panic!("Subscription count for peer {peer} is already zero, cannot decrement");
        }
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
type BroadcastedBlockStream = BroadcastStream<VerifiedBlock>;

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
                    info!("BroadcastedBlockStream {} closed", peer);
                    break None;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("BroadcastedBlockStream {} lagged by {} messages", peer, n);
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

#[cfg(test)]
mod tests {
    use std::{
        cmp::min,
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::StreamExt;
    use iota_metrics::monitored_mpsc::unbounded_channel;
    use parking_lot::{Mutex, RwLock};
    use rstest::rstest;
    use starfish_config::{AuthorityIndex, Parameters};
    use tokio::{
        sync::{broadcast, mpsc},
        time::sleep,
    };

    use crate::{
        CommitConsumer, Round, Transaction, TransactionClient,
        authority_service::{
            AuthorityService, BroadcastedBlockStream, MAX_FILTER_SIZE, SubscriptionCounter,
        },
        block_header::{
            BlockHeaderAPI, BlockRef, GENESIS_ROUND, SignedBlockHeader, TestBlockHeader,
            TransactionsCommitment, VerifiedBlock, VerifiedBlockHeader, VerifiedOwnShard,
            VerifiedTransactions,
        },
        block_manager::BlockManager,
        block_verifier::SignedBlockVerifier,
        commit::{CertifiedCommits, CommitRange},
        commit_observer::CommitObserver,
        commit_syncer::CommitSyncType,
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        cordial_knowledge::{ConnectionKnowledgeMessage, CordialKnowledge},
        core::{Core, CoreSignals, ReasonToCreateBlock},
        core_thread::{CoreError, CoreThreadDispatcher, tests::MockCoreThreadDispatcher},
        dag_state::{DagState, DataSource},
        encoder::create_encoder,
        error::{ConsensusError, ConsensusResult},
        header_synchronizer::HeaderSynchronizer,
        leader_schedule::LeaderSchedule,
        network::{
            BlockBundle, BlockBundleStream, NetworkClient, NetworkService, SerializedBlock,
            SerializedBlockBundle, SerializedBlockBundleParts, SerializedHeaderAndTransactions,
            SerializedTransactionsV1, SerializedTransactionsV2, TransactionFetchMode,
        },
        storage::{Store, WriteBatch, mem_store::MemStore},
        test_dag_builder::DagBuilder,
        transaction::TransactionConsumer,
        transaction_ref::GenericTransactionRef,
        transactions_synchronizer::TransactionsSynchronizer,
    };

    #[derive(Default)]
    struct FakeNetworkClient {}

    #[async_trait]
    impl NetworkClient for FakeNetworkClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
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

        async fn fetch_latest_block_headers(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
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

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<GenericTransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_subscribed_block_bundle_time_drift(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let (mut context, _keys) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        // Test that block with timestamp drift to the future is not rejected.
        let now = context.clock.timestamp_utc_ms();
        let max_drift = context.parameters.max_forward_time_drift;
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(1, 0, &context, &mut encoder)
                .set_timestamp_ms(now + max_drift.as_millis() as u64 + 1)
                .build(),
        );

        let serialized_block_bundle = SerializedBlockBundle::try_from(input_block.clone()).unwrap();

        tokio::spawn(async move {
            authority_service
                .handle_subscribed_block_bundle(
                    context.committee.to_authority_index(0).unwrap(),
                    serialized_block_bundle,
                    &mut encoder,
                )
                .await
                .unwrap();
        });

        // Give it some time to process
        sleep(max_drift / 2).await;

        let blocks = core_dispatcher.get_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], input_block);
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_wrong_peer(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let (mut context, _keys) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(1, 0, &context, &mut encoder).build(),
        );

        let service = authority_service.clone();
        let serialized_block_bundle = SerializedBlockBundle::try_from(input_block.clone()).unwrap();

        // Test sending a block from wrong peer

        let result = authority_service
            .handle_subscribed_block_bundle(
                context.committee.to_authority_index(1).unwrap(),
                serialized_block_bundle.clone(),
                &mut encoder,
            )
            .await;

        if let Err(ConsensusError::UnexpectedAuthority { .. }) = result {
            // everything is fine
        } else {
            panic!("Expected UnexpectedAuthority error, got {result:?}");
        }

        // Now send from correct peer
        tokio::spawn(async move {
            service
                .handle_subscribed_block_bundle(
                    context.committee.to_authority_index(0).unwrap(),
                    serialized_block_bundle,
                    &mut encoder,
                )
                .await
                .unwrap();
        });
        sleep(Duration::from_millis(200)).await; // wait for the block to be processed
        let blocks = core_dispatcher.get_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], input_block);
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_wrong_transaction_commitment(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let (mut context, _keys) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(1, 0, &context, &mut encoder)
                .set_commitment(
                    TransactionsCommitment::compute_transactions_commitment(
                        &Bytes::from_static(b"dummy data"),
                        &context,
                        &mut encoder,
                    )
                    .unwrap(),
                )
                .build(),
        );

        let serialized_block_bundle = SerializedBlockBundle::try_from(input_block.clone()).unwrap();

        // Test sending a block with wrong transaction commitment
        let result = authority_service
            .handle_subscribed_block_bundle(
                context.committee.to_authority_index(0).unwrap(),
                serialized_block_bundle,
                &mut encoder,
            )
            .await;

        if let Err(ConsensusError::TransactionCommitmentFailure { .. }) = result {
            // everything is fine
        } else {
            panic!("Expected TransactionCommitmentFailure error, got {result:?}",);
        }
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_with_bad_headers(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        let committee_size = 4;
        let (mut context, _keys) = Context::new_for_test(committee_size);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(1, 0, &context, &mut encoder).build(),
        );
        let headers = (0..context.parameters.max_headers_per_bundle)
            .map(|i| {
                VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new_with_commitment(
                        (i / committee_size + 1) as u32,
                        (i % committee_size) as u8,
                        &context,
                        &mut encoder,
                    )
                    .build(),
                )
            })
            .collect::<Vec<_>>();

        let service = authority_service.clone();

        let block_bundle_with_big_rounds = BlockBundle {
            verified_block: input_block.clone(),
            verified_headers: headers.clone(),
            serialized_shards: vec![],
            useful_headers_authors: (0u8..(committee_size as u8)).map(Into::into).collect(),
            useful_shards_authors: (0u8..(committee_size as u8)).map(Into::into).collect(),
        };
        let serialized_block_bundle_with_big_round = SerializedBlockBundle::try_from(
            SerializedBlockBundleParts::try_from(block_bundle_with_big_rounds).unwrap(),
        )
        .unwrap();

        // Send a bundle with too many headers
        let result = authority_service
            .handle_subscribed_block_bundle(
                context.committee.to_authority_index(0).unwrap(),
                serialized_block_bundle_with_big_round,
                &mut encoder,
            )
            .await;

        if let Err(ConsensusError::TooBigHeaderRoundInABundle { .. }) = result {
            // everything is fine
        } else {
            panic!("Expected TooBigHeaderRoundInABundle error, got {result:?}",);
        }

        // Create a block with a big round
        let input_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new_with_commitment(
                context.parameters.max_headers_per_bundle as u32 + 1,
                0,
                &context,
                &mut encoder,
            )
            .build(),
        );

        let block_bundle = BlockBundle {
            verified_block: input_block.clone(),
            verified_headers: headers.clone(),
            serialized_shards: vec![],
            useful_headers_authors: (0u8..(committee_size as u8)).map(Into::into).collect(),
            useful_shards_authors: (0u8..(committee_size as u8)).map(Into::into).collect(),
        };
        let serialized_block_bundle = SerializedBlockBundle::try_from(
            SerializedBlockBundleParts::try_from(block_bundle).unwrap(),
        )
        .unwrap();

        // Send a correct bundle
        tokio::spawn(async move {
            service
                .handle_subscribed_block_bundle(
                    context.committee.to_authority_index(0).unwrap(),
                    serialized_block_bundle,
                    &mut encoder,
                )
                .await
                .unwrap();
        });
        sleep(Duration::from_millis(200)).await; // wait for the block to be processed
        let blocks = core_dispatcher.get_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], input_block);
        let block_headers = core_dispatcher.get_block_headers();
        assert_eq!(block_headers.len(), headers.len());
        assert_eq!(block_headers, headers);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_fetch_latest_block_headers() {
        // GIVEN
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            true,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
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
            .handle_fetch_latest_block_headers(
                AuthorityIndex::new_for_test(1),
                authorities_to_request,
            )
            .await;

        // THEN
        let serialised_block_headers = results.unwrap();
        for serialised_block_header in serialised_block_headers {
            let signed_block: SignedBlockHeader =
                bcs::from_bytes(&serialised_block_header).expect("Error while deserialising block");
            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block, serialised_block_header);

            assert_eq!(verified_block_header.round(), 10);
        }
    }

    pub struct FakeCoreThreadDispatcher {
        core: Mutex<Core>,
    }

    #[async_trait]
    impl CoreThreadDispatcher for FakeCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
            source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let mut guard = self.core.lock();
            let _ = guard.add_blocks(blocks, source);
            Ok((BTreeSet::new(), BTreeMap::new()))
        }

        async fn add_block_headers(
            &self,
            block_headers: Vec<VerifiedBlockHeader>,
            source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let mut guard = self.core.lock();
            let _ = guard.add_block_headers(block_headers, source);
            Ok((BTreeSet::new(), BTreeMap::new()))
        }

        async fn add_transactions(
            &self,
            _transactions: Vec<VerifiedTransactions>,
            _source: DataSource,
        ) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn add_shards(&self, _shards: Vec<VerifiedOwnShard>) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!("Unimplemented")
        }

        async fn add_subdags_from_fast_sync(
            &self,
            _output: crate::commit_syncer::fast::FastSyncOutput,
        ) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn reinitialize_components(
            &self,
            _block_headers: Vec<crate::block_header::VerifiedBlockHeader>,
        ) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn new_block(
            &self,
            _round: Round,
            _reason: ReasonToCreateBlock,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn get_missing_block_headers(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            // do nothing
            Ok(BTreeMap::new())
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }
    }
    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_with_additional_headers(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        // GIVEN
        let rounds = 10;
        let validators = 10;
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );
        // we set sync_last_known_own_block to true and last known proposed round to
        // rounds+5 so that core doesn't start to create its own new blocks,
        // that would be different from the blocks created in dag builder
        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );
        core.set_last_known_proposed_round(rounds + 5);

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        let protocol_keypairs = key_pairs.iter().map(|kp| kp.1.clone()).collect();
        let mut dag_builder =
            DagBuilder::new(context.clone()).set_protocol_keypair(protocol_keypairs);
        dag_builder.layers(1..=rounds).build();
        let mut all_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        let mut all_transactions: Vec<Vec<VerifiedTransactions>> = vec![];
        for round in 0..=rounds {
            all_headers.push(dag_builder.block_headers(round..=round));
            all_transactions.push(dag_builder.transactions(round..=round));
        }
        for round in 1..=rounds {
            core_dispatcher
                .add_block_headers(
                    vec![all_headers[round as usize][0].clone()],
                    DataSource::Test,
                )
                .await
                .expect("blocks header is expected to be added successfully");
            for peer in 1..validators {
                let mut headers = if round > 1 {
                    all_headers[round as usize - 1].clone()
                } else {
                    vec![]
                };
                let block = VerifiedBlock {
                    verified_block_header: all_headers[round as usize][peer].clone(),
                    verified_transactions: all_transactions[round as usize][peer].clone(),
                };
                if round > 1 {
                    headers.remove(peer);
                }
                let block_bundle = BlockBundle {
                    verified_block: block,
                    verified_headers: headers,
                    serialized_shards: vec![],
                    useful_headers_authors: (0u8..(context.committee.size() as u8))
                        .map(Into::into)
                        .collect(),
                    useful_shards_authors: (0u8..(context.committee.size() as u8))
                        .map(Into::into)
                        .collect(),
                };
                let serialized_block_bundle = SerializedBlockBundle::try_from(
                    SerializedBlockBundleParts::try_from(block_bundle).unwrap(),
                )
                .unwrap();
                authority_service
                    .handle_subscribed_block_bundle(
                        context.committee.to_authority_index(peer).unwrap(),
                        serialized_block_bundle,
                        &mut encoder,
                    )
                    .await
                    .expect("bundle is expected to be processed successfully");
            }
            for (authority_index, _) in context.committee.authorities() {
                let block = dag_state
                    .read()
                    .get_last_block_header_for_authority(authority_index);

                assert_eq!(block.round(), round);
            }
            assert_eq!(
                authority_service.received_block_headers.size(),
                min(validators * round as usize - 1, MAX_FILTER_SIZE as usize)
            )
        }
    }

    #[rstest]
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribe_bundle_without_additional_headers(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        // GIVEN
        let rounds = 10;
        let validators = 10;
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );
        // we set sync_last_known_own_block to true and last known proposed round to
        // rounds+5 so that core doesn't start to create its own new blocks,
        // that would be different from the blocks created in dag builder
        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );
        core.set_last_known_proposed_round(rounds + 5);

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        let protocol_keypairs = key_pairs.iter().map(|kp| kp.1.clone()).collect();
        let mut dag_builder =
            DagBuilder::new(context.clone()).set_protocol_keypair(protocol_keypairs);
        dag_builder.layers(1..=rounds).build();
        let mut all_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        let mut all_transactions: Vec<Vec<VerifiedTransactions>> = vec![];
        for round in 0..=rounds {
            all_headers.push(dag_builder.block_headers(round..=round));
            all_transactions.push(dag_builder.transactions(round..=round));
        }
        for round in 1..=rounds {
            core_dispatcher
                .add_block_headers(
                    vec![all_headers[round as usize][0].clone()],
                    DataSource::Test,
                )
                .await
                .expect("blocks header is expected to be added successfully");
            for peer in 1..validators {
                let block = VerifiedBlock {
                    verified_block_header: all_headers[round as usize][peer].clone(),
                    verified_transactions: all_transactions[round as usize][peer].clone(),
                };
                let block_bundle = BlockBundle {
                    verified_block: block,
                    verified_headers: vec![],
                    serialized_shards: vec![],
                    useful_headers_authors: (0u8..(context.committee.size() as u8))
                        .map(Into::into)
                        .collect(),
                    useful_shards_authors: (0u8..(context.committee.size() as u8))
                        .map(Into::into)
                        .collect(),
                };
                let serialized_block_bundle = SerializedBlockBundle::try_from(
                    SerializedBlockBundleParts::try_from(block_bundle).unwrap(),
                )
                .unwrap();
                authority_service
                    .handle_subscribed_block_bundle(
                        context.committee.to_authority_index(peer).unwrap(),
                        serialized_block_bundle,
                        &mut encoder,
                    )
                    .await
                    .expect("bundle is expected to be processed successfully");
            }
            for (authority_index, _) in context.committee.authorities() {
                let block = dag_state
                    .read()
                    .get_last_block_header_for_authority(authority_index);

                assert_eq!(block.round(), round);
            }
        }
    }

    #[tokio::test]
    async fn test_broadcast_stream_receives_and_closes() {
        let (tx, rx) = broadcast::channel(10);
        let subscription_counter = Arc::new(SubscriptionCounter::new(
            Arc::new(Context::new_for_test(4).0),
            Arc::new(MockCoreThreadDispatcher::default()),
        ));
        let peer = AuthorityIndex::new_for_test(0);

        let mut stream = BroadcastedBlockStream::new(peer, rx, subscription_counter);

        // Send a block
        let verified_block = VerifiedBlock::new_for_test(TestBlockHeader::new(1, 0).build());
        tx.send(verified_block.clone()).unwrap();

        // Should receive the block
        let received = stream.next().await;
        assert_eq!(received, Some(verified_block));

        // Drop the sender to close the channel
        drop(tx);

        // Stream should end (return None)
        let received = stream.next().await;
        assert_eq!(received, None);
    }

    #[rstest]
    #[tokio::test]
    async fn test_handle_subscribe_block_bundles_request(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let to_whom_authority = AuthorityIndex::new_for_test(1);
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        // Create a broadcast channel for new blocks
        let (tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create the authority service
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge.clone(),
        ));
        let mut encoder = create_encoder(&context);

        // Set up DAG with blocks
        let protocol_keypairs = key_pairs.iter().map(|kp| kp.1.clone()).collect();
        let mut dag_builder =
            DagBuilder::new(context.clone()).set_protocol_keypair(protocol_keypairs);
        dag_builder.layers(1..=rounds).build();

        // Get all blocks
        let mut all_blocks: Vec<Vec<VerifiedBlock>> = vec![];
        for round in 0..=rounds {
            all_blocks.push(dag_builder.blocks(round..=round));
        }

        let first_batch_end_exclusive = 5;
        for round in 1..first_batch_end_exclusive {
            core_dispatcher
                .add_blocks(all_blocks[round as usize - 1].clone(), DataSource::Test)
                .await
                .expect("blocks are expected to be added successfully");
            core_dispatcher
                .add_blocks(
                    vec![all_blocks[round as usize][0].clone()],
                    DataSource::Test,
                )
                .await
                .expect("blocks are expected to be added successfully");
            sleep(Duration::from_millis(50)).await;
        }

        // Inject useful info
        let connection_knowledge = cordial_knowledge.connection_knowledge(to_whom_authority);
        let msg = ConnectionKnowledgeMessage::UsefulAuthors {
            useful_headers_to_peer: BTreeMap::from([
                (AuthorityIndex::new_for_test(2), GENESIS_ROUND),
                (AuthorityIndex::new_for_test(3), GENESIS_ROUND),
            ]),
            useful_shards_to_peer: BTreeMap::from([
                (AuthorityIndex::new_for_test(2), GENESIS_ROUND),
                (AuthorityIndex::new_for_test(3), GENESIS_ROUND),
            ]),
            useful_headers_from_peer: BTreeMap::from([
                (AuthorityIndex::new_for_test(1), GENESIS_ROUND),
                (AuthorityIndex::new_for_test(3), GENESIS_ROUND),
            ]),
            useful_shards_from_peer: vec![None, Some(GENESIS_ROUND), None, Some(GENESIS_ROUND)],
        };
        {
            connection_knowledge.write().process_one_message(msg);
        }
        // WHEN
        // Call handle_subscribe_block_bundles_request with last_received = 2
        let last_received_round = 2;
        let block_bundle_stream = authority_service
            .handle_subscribe_block_bundles_request(to_whom_authority, last_received_round)
            .await
            .expect("Should return a valid stream");

        // Convert the stream to a vector for testing
        let mut stream = Box::pin(block_bundle_stream);
        let mut received_bundles = Vec::new();

        // Collect expected blocks from the first batch
        let expected_number = first_batch_end_exclusive - 1 - last_received_round;
        for i in 0..expected_number {
            match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
                Ok(Some(bundle)) => received_bundles.push(bundle),
                Ok(None) => panic!("Stream ended at bundle {} of {}", i, expected_number),
                Err(_) => panic!("Timeout at bundle {} of {}", i, expected_number),
            }
        }

        // THEN
        // Verify that we received expected blocks (rounds
        // last_received_round-first_batch)

        assert_eq!(
            received_bundles.len() as u32,
            expected_number,
            "Should receive {expected_number} missed blocks",
        );

        // Check the correctness of the received blocks
        for (i, bundle) in received_bundles.into_iter().enumerate() {
            let serialized_block_and_headers =
                SerializedBlockBundleParts::try_from(bundle).unwrap();
            let SerializedHeaderAndTransactions {
                serialized_block_header,
                serialized_transactions,
            } = SerializedHeaderAndTransactions::try_from(SerializedBlock {
                serialized_block: serialized_block_and_headers.serialized_block,
            })
            .unwrap();

            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)
                .unwrap();
            assert_eq!(
                signed_block_header.transactions_commitment(),
                TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions,
                    &context,
                    &mut encoder
                )
                .unwrap()
            );

            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
            let transactions: Vec<Transaction> = bcs::from_bytes(&serialized_transactions)
                .map_err(ConsensusError::MalformedTransactions)
                .unwrap();
            let verified_transactions = VerifiedTransactions::new(
                transactions,
                verified_block_header.transaction_ref(),
                Some(verified_block_header.digest()),
                serialized_transactions,
            );
            let verified_block = VerifiedBlock::new(verified_block_header, verified_transactions);
            assert_eq!(
                verified_block.round(),
                (i as u32) + last_received_round + 1,
                "Block should be from round {}",
                (i as u32) + last_received_round + 1
            );
            assert_eq!(
                verified_block,
                all_blocks[i + (last_received_round + 1) as usize][0],
            );
        }
        received_bundles = vec![];

        for round in first_batch_end_exclusive..=rounds {
            core_dispatcher
                .add_blocks(all_blocks[round as usize - 1].clone(), DataSource::Test)
                .await
                .expect("blocks are expected to be added successfully");
            core_dispatcher
                .add_blocks(
                    vec![all_blocks[round as usize][0].clone()],
                    DataSource::Test,
                )
                .await
                .expect("blocks are expected to be added successfully");
            sleep(Duration::from_millis(50)).await;
            tx_block_broadcast
                .send(all_blocks[round as usize][0].clone())
                .expect("We expect that block is sent successfully");
            sleep(Duration::from_millis(50)).await;
            match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
                Ok(Some(bundle)) => received_bundles.push(bundle),
                Ok(None) => panic!("Stream ended at round {}", round),
                Err(_) => panic!(
                    "Timeout at round {}, got {} bundles",
                    round,
                    received_bundles.len()
                ),
            }
        }

        // Check blocks from the second batch
        for (i, bundle) in received_bundles.into_iter().enumerate() {
            let serialized_block_bundle_parts =
                SerializedBlockBundleParts::try_from(bundle).unwrap();
            let SerializedHeaderAndTransactions {
                serialized_block_header,
                serialized_transactions,
            } = SerializedHeaderAndTransactions::try_from(SerializedBlock {
                serialized_block: serialized_block_bundle_parts.serialized_block,
            })
            .unwrap();

            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)
                .unwrap();
            assert_eq!(
                signed_block_header.transactions_commitment(),
                TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions,
                    &context,
                    &mut encoder
                )
                .unwrap()
            );

            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
            let transactions: Vec<Transaction> = bcs::from_bytes(&serialized_transactions)
                .map_err(ConsensusError::MalformedTransactions)
                .unwrap();
            let verified_transactions = VerifiedTransactions::new(
                transactions,
                verified_block_header.transaction_ref(),
                Some(verified_block_header.digest()),
                serialized_transactions,
            );
            let verified_block = VerifiedBlock::new(verified_block_header, verified_transactions);
            assert_eq!(
                verified_block.round(),
                (i as u32) + first_batch_end_exclusive,
                "Block should be from round {}",
                (i as u32) + first_batch_end_exclusive
            );
            assert_eq!(
                verified_block,
                all_blocks[i + first_batch_end_exclusive as usize][0],
            );

            let mut authorities = vec![];
            for serialized_header in serialized_block_bundle_parts.serialized_headers {
                let signed_header: SignedBlockHeader = bcs::from_bytes(&serialized_header)
                    .map_err(ConsensusError::MalformedHeader)
                    .unwrap();
                assert_eq!(
                    verified_block.round(),
                    signed_header.round() + 1,
                    "Headers should be from the previous round"
                );
                authorities.push(signed_header.author());
            }
            authorities.sort();
            assert_eq!(
                authorities,
                vec![
                    AuthorityIndex::new_for_test(2),
                    AuthorityIndex::new_for_test(3)
                ],
                "We should have pushed headers from other authorities in round {:?}",
                verified_block.round(),
            );
        }
    }

    #[tokio::test]
    async fn test_handle_fetch_headers_commit_sync() {
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Context {
            parameters: Parameters {
                max_headers_per_commit_sync_fetch: 20,
                ..context.parameters
            },
            ..context
        };
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create the authority service
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
        ));

        // Set up DAG with blocks
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=rounds).build();
        dag_builder.persist_all_blocks(dag_state.clone());

        // Get all block headers
        let mut all_block_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        for round in 0..=rounds {
            all_block_headers.push(dag_builder.block_headers(round..=round));
        }

        let block_refs_to_request: Vec<BlockRef> = (1..=rounds)
            .flat_map(|round| {
                all_block_headers[round as usize]
                    .iter()
                    .map(|bh| bh.reference())
            })
            .collect();

        let peer = context.committee.to_authority_index(1).unwrap();
        let truncated_headers = authority_service
            .handle_fetch_headers(peer, block_refs_to_request.clone(), vec![])
            .await
            .expect("Should return a valid vector of serialized block headers");

        // Verify that we received requested block headers
        assert_eq!(
            truncated_headers.len(),
            context.parameters.max_headers_per_commit_sync_fetch,
            "Should receive {} block headers",
            context.parameters.max_headers_per_commit_sync_fetch
        );

        // Check the correctness of the received blocks
        for (i, serialized_block_header) in truncated_headers.into_iter().enumerate() {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)
                .unwrap();
            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
            assert_eq!(verified_block_header.reference(), block_refs_to_request[i],);
        }
    }

    #[tokio::test]
    async fn test_handle_fetch_headers_regular_sync() {
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Context {
            parameters: Parameters {
                max_headers_per_regular_sync_fetch: 20,
                ..context.parameters
            },
            ..context
        };
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create the authority service
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
        ));

        // Set up DAG with blocks
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=rounds).build();
        dag_builder.persist_all_blocks(dag_state.clone());

        // Get all block headers
        let mut all_block_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        for round in 0..=rounds {
            all_block_headers.push(dag_builder.block_headers(round..=round));
        }

        let mut block_refs_to_request: Vec<BlockRef> = (5..=rounds)
            .flat_map(|round| {
                all_block_headers[round as usize]
                    .iter()
                    .map(|bh| bh.reference())
            })
            .collect();

        let peer = context.committee.to_authority_index(1).unwrap();
        let err = authority_service
            .handle_fetch_headers(peer, block_refs_to_request.clone(), vec![1; validators + 1])
            .await
            .expect_err("Expected InvalidSizeOfHighestAcceptedRounds error");

        assert!(matches!(
            err,
            ConsensusError::InvalidSizeOfHighestAcceptedRounds(..)
        ));
        let truncated_headers = authority_service
            .handle_fetch_headers(peer, block_refs_to_request.clone(), vec![1; validators])
            .await
            .expect("Should return a valid vector of serialized block headers");

        // Verify that we received requested block headers
        assert_eq!(
            truncated_headers.len(),
            context.parameters.max_headers_per_regular_sync_fetch,
            "Should receive {} block headers",
            context.parameters.max_headers_per_regular_sync_fetch
        );

        // Check the correctness of the received blocks
        for (i, serialized_block_header) in truncated_headers.into_iter().enumerate() {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)
                .unwrap();
            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
            assert_eq!(verified_block_header.reference(), block_refs_to_request[i],);
        }

        // check that missing headers from previous rounds would be added
        block_refs_to_request.truncate(context.parameters.max_headers_per_regular_sync_fetch / 2);

        let serialized_block_headers = authority_service
            .handle_fetch_headers(peer, block_refs_to_request.clone(), vec![1; validators])
            .await
            .expect("Should return a valid vector of serialized block headers");

        // Verify that we received requested block headers and additional from previous
        // rounds
        assert!(
            serialized_block_headers.len() > block_refs_to_request.len(),
            "Should receive more block headers than requested",
        );
    }

    #[rstest]
    #[tokio::test]
    async fn test_handle_fetch_commits(#[values(false, true)] consensus_fast_commit_sync: bool) {
        // GIVEN
        let rounds = 15;
        let validators = 4;
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        // we set sync_last_known_own_block to true and last known proposed round to
        // rounds+5 so that core doesn't start to create its own new blocks,
        // that would be different from the blocks created in dag builder
        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );
        core.set_last_known_proposed_round(rounds + 5);

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create the authority service
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store.clone(),
            tx_message_sender,
            cordial_knowledge,
        ));

        // Set up DAG with blocks
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=rounds).build();
        // dag_builder.persist_all_blocks(dag_state.clone());

        // Get all block headers
        let mut all_block_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        for round in 0..=rounds {
            all_block_headers.push(dag_builder.block_headers(round..=round));
        }

        for round in 1..=rounds {
            core_dispatcher
                .add_block_headers(all_block_headers[round as usize].clone(), DataSource::Test)
                .await
                .expect("block headers are expected to be added successfully");
        }

        // Manually construct headers with commit votes
        let range = CommitRange::new(1..=rounds);
        let commits = store.scan_commits(range.clone()).unwrap();
        let mut commit_refs = vec![];
        for commit in commits {
            let commit_ref = commit.reference();
            commit_refs.push(commit_ref);
        }
        let mut new_block_headers = vec![];
        let refs_to_headers_from_prev_round = all_block_headers[rounds as usize]
            .iter()
            .map(|header| header.reference())
            .collect::<Vec<_>>();
        for validator in 0..validators {
            let test_block_header = TestBlockHeader::new(rounds + 1, validator as u8)
                .set_commit_votes(commit_refs.clone())
                .set_ancestors(refs_to_headers_from_prev_round.clone())
                .set_timestamp_ms(
                    (rounds as u64 + 1) * 1000 + (validator + rounds as usize + 1) as u64,
                )
                .build();
            let verified_block_header = VerifiedBlockHeader::new_for_test(test_block_header);
            new_block_headers.push(verified_block_header);
        }
        all_block_headers.push(new_block_headers.clone());
        core_dispatcher
            .add_block_headers(new_block_headers.clone(), DataSource::Test)
            .await
            .expect("block headers are expected to be added successfully");

        // create headers for several more rounds so that new headers with commit votes
        // are committed
        for round in rounds + 2..rounds + 5 {
            let mut new_block_headers = vec![];
            let refs_to_headers_from_prev_round = all_block_headers[round as usize - 1]
                .iter()
                .map(|header| header.reference())
                .collect::<Vec<_>>();
            for validator in 0..validators {
                let test_block_header = TestBlockHeader::new(round, validator as u8)
                    .set_ancestors(refs_to_headers_from_prev_round.clone())
                    .set_timestamp_ms(round as u64 * 1000 + (validator + round as usize + 1) as u64)
                    .build();
                let verified_block_header = VerifiedBlockHeader::new_for_test(test_block_header);
                new_block_headers.push(verified_block_header);
            }
            all_block_headers.push(new_block_headers.clone());
            core_dispatcher
                .add_block_headers(new_block_headers.clone(), DataSource::Test)
                .await
                .expect("block headers are expected to be added successfully");
        }

        let peer = context.committee.to_authority_index(1).unwrap();

        let result = authority_service
            .handle_fetch_commits(peer, range, CommitSyncType::Regular)
            .await
            .unwrap();

        assert_eq!(
            result.0.len() as u32,
            rounds - 2,
            "Should return commits for range 1..={}, but returned {}",
            rounds - 2,
            result.0.len() as u32
        );
    }

    #[rstest]
    #[tokio::test]
    async fn test_handle_fetch_transactions(
        #[values(false, true)] consensus_fast_commit_sync: bool,
    ) {
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Context {
            parameters: Parameters {
                max_transactions_per_regular_sync_fetch: 20,
                max_transactions_per_commit_sync_fetch: 10,
                enable_fast_commit_syncer: consensus_fast_commit_sync,
                ..context.parameters
            },
            ..context
        };
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create the authority service
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store,
            tx_message_sender,
            cordial_knowledge,
        ));
        let mut encoder = create_encoder(&context);

        // Set up DAG with blocks
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=rounds).build();
        dag_builder.persist_all_blocks(dag_state.clone());
        dag_builder.layers(rounds + 1..=2 * rounds).build();
        // Get all block headers
        let mut all_block_headers: Vec<Vec<VerifiedBlockHeader>> = vec![];
        for round in 0..=2 * rounds {
            all_block_headers.push(dag_builder.block_headers(round..=round));
        }

        let mut block_refs_to_request_first_batch: Vec<GenericTransactionRef> = (1..=rounds)
            .flat_map(|round| {
                all_block_headers[round as usize].iter().map(|bh| {
                    if consensus_fast_commit_sync {
                        GenericTransactionRef::TransactionRef(bh.transaction_ref())
                    } else {
                        GenericTransactionRef::from(bh.reference())
                    }
                })
            })
            .collect();

        let mut block_refs_to_request_second_batch: Vec<GenericTransactionRef> = (rounds + 1
            ..=2 * rounds)
            .flat_map(|round| {
                all_block_headers[round as usize].iter().map(|bh| {
                    if consensus_fast_commit_sync {
                        GenericTransactionRef::TransactionRef(bh.transaction_ref())
                    } else {
                        GenericTransactionRef::from(bh.reference())
                    }
                })
            })
            .collect();

        let peer = context.committee.to_authority_index(1).unwrap();
        let serialized_transactions = authority_service
            .handle_fetch_transactions(
                peer,
                block_refs_to_request_first_batch.clone(),
                TransactionFetchMode::TransactionSync,
            )
            .await
            .expect("We should expect a correct return of serialized transactions");

        block_refs_to_request_first_batch
            .truncate(context.parameters.max_transactions_per_regular_sync_fetch);
        // Verify that we received the correct number of requested transactions
        assert_eq!(
            serialized_transactions.len(),
            block_refs_to_request_first_batch.len(),
            "Should receive {} block transactions",
            block_refs_to_request_first_batch.len()
        );

        // Check the correctness of the received transactions
        for (i, serialized_transactions_bytes) in serialized_transactions.iter().enumerate() {
            if consensus_fast_commit_sync {
                // Deserialize V2 format with TransactionRef
                let deserialized: SerializedTransactionsV2 =
                    bcs::from_bytes(serialized_transactions_bytes)
                        .expect("deserialization should succeed");
                let transaction_ref = deserialized.transaction_ref;

                // Verify it matches the expected ref
                assert_eq!(
                    GenericTransactionRef::TransactionRef(transaction_ref),
                    block_refs_to_request_first_batch[i]
                );

                let serialized_transactions = deserialized.serialized_transactions;
                // Verify the transaction commitment matches
                assert_eq!(
                    transaction_ref.transactions_commitment,
                    TransactionsCommitment::compute_transactions_commitment(
                        &serialized_transactions,
                        &context,
                        &mut encoder
                    )
                    .unwrap()
                );
            } else {
                // Deserialize V1 format with BlockRef
                let deserialized: SerializedTransactionsV1 =
                    bcs::from_bytes(serialized_transactions_bytes)
                        .expect("deserialization should succeed");
                let block_ref = deserialized.block_ref;
                assert_eq!(
                    GenericTransactionRef::from(block_ref),
                    block_refs_to_request_first_batch[i]
                );
                let serialized_transactions = deserialized.serialized_transactions;
                let block_header = all_block_headers[block_ref.round as usize]
                    .iter()
                    .find(|header| header.reference() == block_ref)
                    .expect("We expect to find the header with such block_ref");
                assert_eq!(
                    block_header.transactions_commitment(),
                    TransactionsCommitment::compute_transactions_commitment(
                        &serialized_transactions,
                        &context,
                        &mut encoder
                    )
                    .unwrap()
                );
            }
        }

        block_refs_to_request_second_batch
            .truncate(context.parameters.max_transactions_per_regular_sync_fetch);

        let serialized_transactions = authority_service
            .handle_fetch_transactions(
                peer,
                block_refs_to_request_second_batch.clone(),
                TransactionFetchMode::TransactionSync,
            )
            .await
            .expect("Should return an empty vector");

        // Verify that we received zero transactions since they are not present in the
        // dag
        assert!(serialized_transactions.is_empty());
    }

    /// Tests that handle_fetch_headers preserves the original request order
    /// of block refs when they span the GC boundary — i.e. some are fetched
    /// from the persistent store (below GC) and others from in-memory
    /// dag_state (at or above GC). The interleaved input order must be
    /// maintained in the response.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_fetch_headers_commit_sync_order_across_gc_boundary() {
        // GIVEN
        let rounds = 20;
        let validators = 4;
        let gc_depth = 5;
        let (mut context, key_pairs) = Context::new_for_test(validators);
        context.protocol_config.set_gc_depth_for_testing(gc_depth);
        context.parameters.max_headers_per_commit_sync_fetch = 100;
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, _signal_receivers) = CoreSignals::new(context.clone());
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs[context.own_index.value()].1.clone(),
            dag_state.clone(),
            true,
        );

        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher {
            core: Mutex::new(core),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let header_synchronizer = HeaderSynchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            header_synchronizer,
            transactions_synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state.clone(),
            store.clone(),
            tx_message_sender,
            cordial_knowledge,
        ));

        // Build DAG and persist all blocks to dag_state
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=rounds).build();
        dag_builder.persist_all_blocks(dag_state.clone());

        // Also write all block headers to the store so below-GC refs can be found
        let all_headers: Vec<VerifiedBlockHeader> = dag_builder.block_headers(1..=rounds);
        store
            .write(
                WriteBatch::new(vec![], all_headers, vec![], vec![], vec![], Some(false)),
                context.clone(),
            )
            .expect("Failed to write block headers to store");

        // Set last_solid_subdag_base so gc_round_for_last_solid_commit() is ~10.
        // gc_round = leader_round.saturating_sub(gc_depth * 2) = 20 - 10 = 10
        let leader_ref = dag_builder
            .block_headers(rounds..=rounds)
            .first()
            .unwrap()
            .reference();
        dag_state
            .write()
            .update_last_solid_subdag_base(crate::commit::SubDagBase {
                leader: leader_ref,
                headers: vec![],
                committed_header_refs: vec![],
                timestamp_ms: 0,
                commit_ref: crate::commit::CommitRef::new(1, crate::commit::CommitDigest::MIN),
                reputation_scores_desc: vec![],
            });

        let gc_round = dag_state.read().gc_round_for_last_solid_commit();
        assert!(
            gc_round > GENESIS_ROUND && gc_round < rounds,
            "GC round {gc_round} should be between genesis and max round"
        );

        // Collect block headers per round for easy access
        let mut headers_by_round: Vec<Vec<VerifiedBlockHeader>> =
            vec![vec![]; (rounds + 1) as usize];
        for round in 1..=rounds {
            headers_by_round[round as usize] = dag_builder.block_headers(round..=round);
        }

        // Create interleaved block_refs that alternate between below-GC and above-GC
        // rounds. E.g., [round 3 auth 0, round 15 auth 1, round 5 auth 2, round 12 auth
        // 3, ...]
        let below_gc_rounds: Vec<Round> = (1..gc_round).collect();
        let above_gc_rounds: Vec<Round> = (gc_round..=rounds).collect();
        let mut interleaved_refs = Vec::new();
        let max_pairs = min(below_gc_rounds.len(), above_gc_rounds.len());
        for i in 0..max_pairs {
            let below_round = below_gc_rounds[i];
            let auth_idx = i % validators;
            if auth_idx < headers_by_round[below_round as usize].len() {
                interleaved_refs.push(headers_by_round[below_round as usize][auth_idx].reference());
            }
            let above_round = above_gc_rounds[i];
            let auth_idx2 = (i + 1) % validators;
            if auth_idx2 < headers_by_round[above_round as usize].len() {
                interleaved_refs
                    .push(headers_by_round[above_round as usize][auth_idx2].reference());
            }
        }

        // Verify that we have refs from both sides of the GC boundary
        assert!(
            interleaved_refs.iter().any(|r| r.round < gc_round),
            "Should have refs below GC round"
        );
        assert!(
            interleaved_refs.iter().any(|r| r.round >= gc_round),
            "Should have refs above GC round"
        );

        // WHEN: call handle_fetch_headers with empty highest_accepted_rounds (commit
        // sync path)
        let peer = context.committee.to_authority_index(1).unwrap();
        let returned_headers = authority_service
            .handle_fetch_headers(peer, interleaved_refs.clone(), vec![])
            .await
            .expect("Should return valid serialized block headers");

        // THEN: each returned header should match the corresponding input ref at the
        // same index
        assert_eq!(
            returned_headers.len(),
            interleaved_refs.len(),
            "Should receive all requested headers"
        );
        for (i, serialized_block_header) in returned_headers.into_iter().enumerate() {
            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)
                .unwrap();
            let verified_block_header =
                VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
            assert_eq!(
                verified_block_header.reference(),
                interleaved_refs[i],
                "Header at index {i} should match requested ref. \
                 Expected {:?}, got {:?}",
                interleaved_refs[i],
                verified_block_header.reference()
            );
        }
    }
}
