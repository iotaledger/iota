// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
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
use tokio::sync::{Mutex, broadcast, mpsc::Sender, oneshot};
use tokio_util::sync::ReusableBoxFuture;
use tracing::{debug, info, warn};

use crate::{
    CommitIndex, Round, Transaction, VerifiedBlockHeader,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, GENESIS_ROUND, ShardWithProof,
        SignedBlockHeader, TransactionsCommitment, VerifiedBlock, VerifiedOwnShard,
        VerifiedTransactions,
    },
    block_verifier::BlockVerifier,
    commit::{CommitAPI as _, CommitRange, TrustedCommit},
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    cordial_knowledge::{
        AdditionalPartsForBundle, ConnectionKnowledgeMessage::TakeAdditionalPartForBundle,
        CordialKnowledgeHandle,
    },
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    encoder::ShardEncoder,
    error::{ConsensusError, ConsensusResult},
    network::{
        BlockBundle, BlockBundleStream, NetworkService, SerializedBlock, SerializedBlockBundle,
        SerializedBlockBundleParts, SerializedHeaderAndTransactions, SerializedTransactions,
    },
    shard_reconstructor::TransactionMessage,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    storage::Store,
    synchronizer::SynchronizerHandle,
    transactions_synchronizer::TransactionsSynchronizerHandle,
};

pub(crate) const COMMIT_LAG_MULTIPLIER: u32 = 5;

const MAX_FILTER_SIZE: u32 = 10000;

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
    synchronizer: Arc<SynchronizerHandle>,
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
        synchronizer: Arc<SynchronizerHandle>,
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
            synchronizer,
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

        let peer_hostname = &self.context.committee.authority(peer).hostname;
        let mut serialized_block_bundle_parts =
            SerializedBlockBundleParts::try_from(serialized_block_bundle)?;

        // 1. Create a verified block and make some preliminary checks
        let SerializedHeaderAndTransactions {
            serialized_block_header,
            serialized_transactions,
        } = SerializedHeaderAndTransactions::try_from(SerializedBlock {
            serialized_block: serialized_block_bundle_parts.serialized_block.clone(),
        })?;

        let signed_block_header: SignedBlockHeader =
            bcs::from_bytes(&serialized_block_header).map_err(ConsensusError::MalformedHeader)?;

        // Reject blocks not produced by the peer.
        if peer != signed_block_header.author() {
            self.context
                .metrics
                .node_metrics
                .invalid_block_headers
                .with_label_values(&[
                    peer_hostname.as_str(),
                    "handle_subscribed_block_bundle",
                    "UnexpectedAuthority",
                ])
                .inc();
            let e = ConsensusError::UnexpectedAuthority(signed_block_header.author(), peer);
            info!("Block with wrong authority from {}: {}", peer, e);
            return Err(e);
        }

        if let Err(e) = self.block_verifier.verify(&signed_block_header) {
            self.context
                .metrics
                .node_metrics
                .invalid_block_headers
                .with_label_values(&[
                    peer_hostname.as_str(),
                    "handle_subscribed_block_bundle",
                    e.clone().name(),
                ])
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
            verified_block_header.reference(),
            verified_block_header.transactions_commitment(),
            serialized_transactions,
        );
        let has_transactions = verified_transactions.has_transactions();
        let verified_block = VerifiedBlock::new(verified_block_header, verified_transactions);

        let block_ref = verified_block.reference();
        debug!("Received block {} via stream block bundle.", block_ref);

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

        // 4. Create block headers from bytes from a bundle
        // 4.a. Truncate headers in bundle to max_headers_per_bundle
        let mut serialized_headers =
            std::mem::take(&mut serialized_block_bundle_parts.serialized_headers);
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
                    .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
                    .inc();
                continue;
            }

            let signed_block_header: SignedBlockHeader =
                bcs::from_bytes(&serialized_header).map_err(ConsensusError::MalformedHeader)?;

            let header_round = signed_block_header.round();
            if header_round >= verified_block.round() {
                let e = Err(ConsensusError::TooBigHeaderRoundInABundle {
                    header_round,
                    block_round: verified_block.round(),
                });
                self.context
                    .metrics
                    .node_metrics
                    .invalid_headers_in_bundles
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        "handle_subscribed_block_bundle",
                        "invalid round in header",
                    ])
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
                    .invalid_headers_in_bundles
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        "handle_subscribed_block_bundle",
                        e.clone().name(),
                    ])
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
            .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
            .inc_by(additional_block_headers.len() as u64);

        // 5. Collect shards from a bundle and check their proofs.
        // 5.a. Truncate shards in bundle to max_shards_per_bundle.

        let mut serialized_shards =
            std::mem::take(&mut serialized_block_bundle_parts.serialized_shards);

        if serialized_shards.len() > self.context.parameters.max_shards_per_bundle {
            warn!("BlockBundle: {block_ref} exceeds max_shards_per_bundle.");
            serialized_shards.truncate(self.context.parameters.max_shards_per_bundle);
        }

        let mut verified_shards: Vec<ShardWithProof> = vec![];
        for serialized_shard in &serialized_shards {
            let shard: ShardWithProof =
                bcs::from_bytes(serialized_shard).map_err(ConsensusError::MalformedShard)?;

            if shard.block_ref.round >= verified_block.round() {
                let e = ConsensusError::TooBigShardRoundInABundle {
                    shard_round: shard.block_ref.round,
                    block_round: verified_block.round(),
                };
                self.context
                    .metrics
                    .node_metrics
                    .invalid_shard_in_bundles
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        "handle_subscribed_block_bundle",
                        e.clone().name(),
                    ])
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
                    round: shard.block_ref.round,
                };
                self.context
                    .metrics
                    .node_metrics
                    .invalid_shard_in_bundles
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        "handle_subscribed_block_bundle",
                        e.clone().name(),
                    ])
                    .inc();
                info!("Invalid shard from {}: {}", peer, e);
                return Err(e);
            }
        }
        self.context
            .metrics
            .node_metrics
            .valid_shards_in_bundles
            .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
            .inc_by(verified_shards.len() as u64);

        // 6. Observe headers and the block for the commit votes. When local commit is
        // lagging too much, commit sync loop will trigger fetching.
        for block_header in additional_block_headers.iter() {
            self.commit_vote_monitor.observe_block(block_header);
        }
        self.commit_vote_monitor.observe_block(&verified_block);

        // 7. Reject blocks when local commit index is lagging too far from quorum
        //    commit
        // index.
        //
        // IMPORTANT: this must be done after observing votes from the block, otherwise
        // observed quorum commit will no longer progress.

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

        // 8. Add digests to filter. Exclude from the vector those that are already
        //    inserted
        let mut digests_to_add_to_filter = vec![];
        for block_header in additional_block_headers.iter() {
            digests_to_add_to_filter.push(block_header.digest())
        }
        digests_to_add_to_filter.push(verified_block.digest());
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
            .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
            .inc_by(additional_block_headers.len() as u64);
        self.context
            .metrics
            .node_metrics
            .processed_duplicated_headers_in_bundles
            .with_label_values(&[peer_hostname.as_str(), "handle_subscribed_block_bundle"])
            .inc_by(digests_to_exclude.len() as u64);

        // 9. Prepare transaction messages for shard reconstructor and send them
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

        // 10.Add additional headers from bundle to dag, receive missing ancestors for
        // them. Normally, there should be no missing ancestors, as the headers are
        // sent in order of increasing rounds.
        let (mut missing_ancestors, mut missing_committed_txns) = self
            .core_dispatcher
            .add_block_headers(additional_block_headers.clone())
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        // 11. Add the block to dag, add its missing ancestors to the set
        let block_round = verified_block.round();
        let (missing_block_ancestors, missing_block_committed_transactions) = self
            .core_dispatcher
            .add_blocks(vec![verified_block])
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        missing_ancestors.extend(missing_block_ancestors);
        missing_committed_txns.extend(missing_block_committed_transactions);

        // 12. Add our shard from the received block and its proof to the dag_state
        // only if it contains transactions
        if has_transactions {
            let shard_for_core = ShardWithProof {
                shard: our_shard,
                transaction_commitment,
                proof: proof_for_shard,
                block_ref,
            };
            let serialized_shard_for_core: Bytes = bcs::to_bytes(&shard_for_core)
                .map_err(ConsensusError::SerializationFailure)?
                .into();
            let shard_for_core = VerifiedOwnShard {
                serialized_shard: serialized_shard_for_core,
                block_ref,
            };
            self.core_dispatcher
                .add_shards(vec![shard_for_core])
                .await
                .map_err(|_| ConsensusError::Shutdown)?;
        }

        // 13. Report useful info for cordial and connection knowledge
        self.cordial_knowledge
            .report_useful_authors(
                peer,
                &serialized_block_bundle_parts,
                &additional_block_headers,
                &missing_ancestors,
                block_round,
            )
            .await?;

        // 14. schedule the fetching of missing ancestors (if any) from this peer
        if !missing_ancestors.is_empty() {
            if let Err(err) = self
                .synchronizer
                .fetch_headers(missing_ancestors, peer)
                .await
            {
                warn!("Errored while trying to fetch missing ancestors via synchronizer: {err}");
            }
        }

        // 16. schedule the fetching of missing committed transactions (if any)
        if !missing_committed_txns.is_empty() {
            if let Err(err) = self
                .transactions_synchronizer
                .fetch_transactions(missing_committed_txns)
                .await
            {
                warn!(
                    "Errored while trying to fetch missing transactions via
             transactions synchronizer: {err}"
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
        let connection_knowledge_sender = self.cordial_knowledge.connection_knowledge_sender(peer);
        // Return a stream of blocks that first yields missed blocks as requested, then
        // new blocks.
        Ok(Box::pin(missed_blocks.chain({
            broadcasted_blocks.filter_map(move |block| {
                let connection_knowledge_sender = connection_knowledge_sender.clone();
                let context = context.clone();
                let ts = block.timestamp_ms();
                async move {
                    let (tx, rx) = oneshot::channel();

                    let msg = TakeAdditionalPartForBundle {
                        round_upper_bound_exclusive: block.round(),
                        respond_to: tx,
                    };

                    // Send message asynchronously to corresponding ConnectionKnowledge task
                    if let Err(e) = connection_knowledge_sender.send(vec![msg]).await {
                        warn!("Failed to send TakeAdditionalPartForBundle to Connection Knowledge: {e}");
                        return None;
                    }

                    // Await response from ConnectionKnowledge
                    let block_bundle = match rx.await {
                        Ok(AdditionalPartsForBundle {
                               headers,
                               shards,
                               useful_headers_authors_from_peer,
                               useful_shards_authors_from_peer,
                           }) => BlockBundle {
                            verified_block: block,
                            verified_headers: headers,
                            serialized_shards: shards,
                            useful_headers_authors: useful_headers_authors_from_peer,
                            useful_shards_authors: useful_shards_authors_from_peer,
                        },
                        Err(_) => {
                            warn!("Connection Knowledge oneshot dropped before response");
                            // Construct bundle (fill in your actual vars)
                            return None;
                        }
                    };


                    let now = context.clock.timestamp_utc_ms();
                    context.metrics.node_metrics.delay_in_sending_blocks.observe((now - ts) as f64);

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
        for block in &block_refs {
            if !self.context.committee.is_valid_index(block.author) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: block.author,
                    max: self.context.committee.size(),
                });
            }
            if block.round == GENESIS_ROUND {
                return Err(ConsensusError::UnexpectedGenesisHeaderRequested);
            }
        }

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

        // Get requested blocks from store.
        let blocks = if commit_sync_handle {
            // For commit sync, we respond with all blocks from the store
            self.dag_state
                .read()
                .get_block_headers(&block_refs)
                .into_iter()
                .flatten()
                .collect()
        } else {
            // For periodic or live synchronizer, we respond with requested blocks from the
            // store and with additional blocks from the cache
            block_refs.sort();
            block_refs.dedup();
            let dag_state = self.dag_state.read();
            let mut blocks = dag_state
                .get_block_headers(&block_refs)
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

                let missing_blocks = dag_state.get_cached_block_headers_in_range(
                    authority,
                    highest_accepted_round + 1,
                    lowest_missing_round,
                    self.context
                        .parameters
                        .max_headers_per_regular_sync_fetch
                        .saturating_sub(blocks.len()),
                );
                blocks.extend(missing_blocks);
                if blocks.len() >= self.context.parameters.max_headers_per_regular_sync_fetch {
                    blocks.truncate(self.context.parameters.max_headers_per_regular_sync_fetch);
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
    ) -> ConsensusResult<(Vec<TrustedCommit>, Vec<VerifiedBlockHeader>)> {
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
        let certifier_block_headers = self
            .store
            .read_block_headers(&certifier_block_refs)?
            .into_iter()
            .flatten()
            .collect();
        Ok((commits, certifier_block_headers))
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
        for authority in &authorities {
            if !self.context.committee.is_valid_index(*authority) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: *authority,
                    max: self.context.committee.size(),
                });
            }
        }

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

    async fn handle_get_latest_rounds(
        &self,
        _peer: AuthorityIndex,
    ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
        fail_point_async!("consensus-rpc-response");

        let mut highest_received_rounds = self.core_dispatcher.highest_received_rounds();

        let block_headers = self
            .dag_state
            .read()
            .get_last_cached_block_header_per_authority(Round::MAX);
        let highest_accepted_rounds = block_headers
            .into_iter()
            .map(|(block_headers, _)| block_headers.round())
            .collect::<Vec<_>>();

        // Own blocks do not go through the core dispatcher, so they need to be set
        // separately.
        highest_received_rounds[self.context.own_index] =
            highest_accepted_rounds[self.context.own_index];

        Ok((highest_received_rounds, highest_accepted_rounds))
    }

    async fn handle_fetch_transactions(
        &self,
        peer: AuthorityIndex,
        block_refs: Vec<BlockRef>,
    ) -> ConsensusResult<Vec<Bytes>> {
        fail_point_async!("consensus-rpc-response");

        if block_refs.is_empty() {
            return Ok(Vec::new());
        }

        if block_refs.len() > self.context.parameters.max_transactions_per_fetch {
            return Err(ConsensusError::TooManyFetchTransactionsRequested(peer));
        }

        // Some quick validation of the requested block refs
        for block in &block_refs {
            if !self.context.committee.is_valid_index(block.author) {
                return Err(ConsensusError::InvalidAuthorityIndex {
                    index: block.author,
                    max: self.context.committee.size(),
                });
            }
            if block.round == GENESIS_ROUND {
                return Err(ConsensusError::UnexpectedGenesisTransactionsRequested);
            }
        }

        // Get the transactions from the dag state
        let transactions = self.dag_state.read().get_transactions(&block_refs);

        // Return the serialized transactions
        let result = transactions
            .into_iter()
            .flatten()
            .map(|transaction| {
                Bytes::from(
                    bcs::to_bytes(&SerializedTransactions {
                        block_ref: transaction.block_ref(),
                        serialized_transactions: transaction.serialized().clone(),
                    })
                    .map_err(ConsensusError::SerializationFailure)
                    .expect("serialization should succeed"),
                )
            })
            .collect::<Vec<_>>();

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
        cmp::{max, min},
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::StreamExt;
    use iota_metrics::monitored_mpsc::unbounded_channel;
    use parking_lot::{Mutex, RwLock};
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
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        cordial_knowledge::{ConnectionKnowledgeMessage, CordialKnowledge},
        core::{Core, CoreSignals},
        core_thread::{CoreError, CoreThreadDispatcher, tests::MockCoreThreadDispatcher},
        dag_state::{DagState, TransactionSource},
        encoder::create_encoder,
        error::{ConsensusError, ConsensusResult},
        leader_schedule::LeaderSchedule,
        network::{
            BlockBundle, BlockBundleStream, NetworkClient, NetworkService, SerializedBlock,
            SerializedBlockBundle, SerializedBlockBundleParts, SerializedHeaderAndTransactions,
            SerializedTransactions,
        },
        storage::{Store, mem_store::MemStore},
        synchronizer::Synchronizer,
        test_dag_builder::DagBuilder,
        transaction::TransactionConsumer,
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

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_subscribed_block_bundle_time_drift() {
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_wrong_peer() {
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_wrong_transaction_commitment() {
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_with_bad_headers() {
        let committee_size = 4;
        let (context, _keys) = Context::new_for_test(committee_size);
        let context = Arc::new(context);
        let block_verifier = Arc::new(crate::block_verifier::NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_handle_get_latest_rounds() {
        // GIVEN
        let rounds = 15;
        let validators = 4;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: Mutex::new(vec![0; context.committee.size()]),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

        for round in 1..=rounds / 2 {
            core_dispatcher
                .add_block_headers(all_block_headers[round as usize].clone())
                .await
                .expect("block headers are expected to be added successfully");
        }

        let (received_rounds, accepted_rounds) = authority_service
            .handle_get_latest_rounds(AuthorityIndex::new_for_test(1))
            .await
            .unwrap();
        assert_eq!(
            received_rounds,
            [rounds / 2, rounds / 2, rounds / 2, rounds / 2]
        );
        assert_eq!(
            accepted_rounds,
            [rounds / 2, rounds / 2, rounds / 2, rounds / 2]
        );

        // Add header only for some validators so that received and accepted rounds are
        // different
        for round in rounds / 2 + 1..=rounds {
            let headers = &all_block_headers[round as usize];
            core_dispatcher
                .add_block_headers(headers[..2].to_vec())
                .await
                .expect("block headers are expected to be added successfully");
        }
        let (received_rounds, accepted_rounds) = authority_service
            .handle_get_latest_rounds(AuthorityIndex::new_for_test(1))
            .await
            .unwrap();
        assert_eq!(
            received_rounds,
            [rounds / 2 + 1, rounds, rounds / 2, rounds / 2]
        );
        assert_eq!(
            accepted_rounds,
            [rounds / 2 + 1, rounds / 2 + 1, rounds / 2, rounds / 2]
        );

        for round in rounds / 2 + 1..=rounds {
            let headers = &all_block_headers[round as usize];
            core_dispatcher
                .add_block_headers(headers[2..].to_vec())
                .await
                .expect("block headers are expected to be added successfully");
        }

        let (received_rounds, accepted_rounds) = authority_service
            .handle_get_latest_rounds(AuthorityIndex::new_for_test(1))
            .await
            .unwrap();
        assert_eq!(received_rounds, [rounds, rounds, rounds, rounds]);
        assert_eq!(accepted_rounds, [rounds, rounds, rounds, rounds]);
    }

    pub struct FakeCoreThreadDispatcher {
        core: Mutex<Core>,
        highest_received_rounds: Mutex<Vec<u32>>,
    }

    #[async_trait]
    impl CoreThreadDispatcher for FakeCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let mut guard = self.core.lock();
            let mut vec = self.highest_received_rounds.lock();
            for block in blocks.iter() {
                let entry = &mut vec[block.author()];
                *entry = max(*entry, block.round());
            }
            let _ = guard.add_blocks(blocks);
            Ok((BTreeSet::new(), BTreeMap::new()))
        }

        async fn add_block_headers(
            &self,
            block_headers: Vec<VerifiedBlockHeader>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let mut guard = self.core.lock();
            let mut vec = self.highest_received_rounds.lock();
            for block_header in block_headers.iter() {
                let entry = &mut vec[block_header.author()];
                *entry = max(*entry, block_header.round());
            }
            let _ = guard.add_block_headers(block_headers);
            Ok((BTreeSet::new(), BTreeMap::new()))
        }

        async fn add_transactions(
            &self,
            _transactions: Vec<VerifiedTransactions>,
            _source: TransactionSource,
        ) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn add_shards(&self, _shards: Vec<VerifiedOwnShard>) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!("Unimplemented")
        }

        async fn new_block(
            &self,
            _round: Round,
            _force: bool,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
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

        fn highest_received_rounds(&self) -> Vec<Round> {
            let guard = self.highest_received_rounds.lock();
            guard.clone()
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribed_block_bundle_with_additional_headers() {
        // GIVEN
        let rounds = 10;
        let validators = 10;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
                .add_block_headers(vec![all_headers[round as usize][0].clone()])
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

    #[tokio::test(flavor = "current_thread")]
    async fn test_handle_subscribe_bundle_without_additional_headers() {
        // GIVEN
        let rounds = 10;
        let validators = 10;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
                .add_block_headers(vec![all_headers[round as usize][0].clone()])
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

    #[tokio::test]
    async fn test_handle_subscribe_block_bundles_request() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let to_whom_authority = AuthorityIndex::new_for_test(1);
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
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
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
                .add_blocks(all_blocks[round as usize - 1].clone())
                .await
                .expect("blocks are expected to be added successfully");
            core_dispatcher
                .add_blocks(vec![all_blocks[round as usize][0].clone()])
                .await
                .expect("blocks are expected to be added successfully");
            sleep(Duration::from_millis(50)).await;
        }

        // Inject useful info
        let connection_knowledge_sender =
            cordial_knowledge.connection_knowledge_sender(to_whom_authority);
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
            useful_shards_from_peers: vec![None, Some(GENESIS_ROUND), None, Some(GENESIS_ROUND)],
        };
        let _ = connection_knowledge_sender.send(vec![msg]).await;

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
        for _ in 0..expected_number {
            if let Some(bundle) = stream.next().await {
                received_bundles.push(bundle);
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
                verified_block_header.reference(),
                verified_block_header.transactions_commitment(),
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
                .add_blocks(all_blocks[round as usize - 1].clone())
                .await
                .expect("blocks are expected to be added successfully");
            core_dispatcher
                .add_blocks(vec![all_blocks[round as usize][0].clone()])
                .await
                .expect("blocks are expected to be added successfully");
            sleep(Duration::from_millis(50)).await;
            tx_block_broadcast
                .send(all_blocks[round as usize][0].clone())
                .expect("We expect that block is sent successfully");
            sleep(Duration::from_millis(50)).await;
            if let Some(bundle) = stream.next().await {
                received_bundles.push(bundle);
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
                verified_block_header.reference(),
                verified_block_header.transactions_commitment(),
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
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

    #[tokio::test]
    async fn test_handle_fetch_commits() {
        // GIVEN
        let rounds = 15;
        let validators = 4;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(crate::block_verifier::test::TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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
                .add_block_headers(all_block_headers[round as usize].clone())
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
            .add_block_headers(new_block_headers.clone())
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
                .add_block_headers(new_block_headers.clone())
                .await
                .expect("block headers are expected to be added successfully");
        }

        let peer = context.committee.to_authority_index(1).unwrap();

        let result = authority_service
            .handle_fetch_commits(peer, range)
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

    #[tokio::test]
    async fn test_handle_fetch_transactions() {
        // GIVEN
        let rounds = 10;
        let validators = 4;
        let (context, key_pairs) = Context::new_for_test(validators);
        let context = Context {
            parameters: Parameters {
                max_transactions_per_fetch: 20,
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
        let store = Arc::new(MemStore::new());
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
            highest_received_rounds: vec![0; context.committee.size()].into(),
        });

        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let (tx_message_sender, _tx_message_receiver) = mpsc::channel(100);

        let network_client = Arc::new(FakeNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );

        let synchronizer = Synchronizer::start(
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
            synchronizer,
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

        let mut block_refs_to_request_first_batch: Vec<BlockRef> = (1..=rounds)
            .flat_map(|round| {
                all_block_headers[round as usize]
                    .iter()
                    .map(|bh| bh.reference())
            })
            .collect();

        let mut block_refs_to_request_second_batch: Vec<BlockRef> = (rounds + 1..=2 * rounds)
            .flat_map(|round| {
                all_block_headers[round as usize]
                    .iter()
                    .map(|bh| bh.reference())
            })
            .collect();

        let peer = context.committee.to_authority_index(1).unwrap();
        let err = authority_service
            .handle_fetch_transactions(peer, block_refs_to_request_first_batch.clone())
            .await
            .expect_err("Expected TooManyFetchTransactionsRequested error");

        assert!(matches!(err, ConsensusError::TooManyFetchTransactionsRequested(p) if p == peer));

        block_refs_to_request_first_batch.truncate(context.parameters.max_transactions_per_fetch);

        let serialized_transactions = authority_service
            .handle_fetch_transactions(peer, block_refs_to_request_first_batch.clone())
            .await
            .expect("Should return a valid vector of serialized transactions");

        // Verify that we received the correct number of requested transactions
        assert_eq!(
            serialized_transactions.len(),
            block_refs_to_request_first_batch.len(),
            "Should receive {} block transactions",
            block_refs_to_request_first_batch.len()
        );

        // Check the correctness of the received transactions
        for (i, serialized_transactions_bytes) in serialized_transactions.iter().enumerate() {
            // Deserialize and check transaction commitment
            let deserialized: SerializedTransactions =
                bcs::from_bytes(serialized_transactions_bytes)
                    .expect("deserialization should succeed");
            let block_ref = deserialized.block_ref;
            assert_eq!(block_ref, block_refs_to_request_first_batch[i]);
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

        block_refs_to_request_second_batch.truncate(context.parameters.max_transactions_per_fetch);

        let serialized_transactions = authority_service
            .handle_fetch_transactions(peer, block_refs_to_request_second_batch.clone())
            .await
            .expect("Should return an empty vector");

        // Verify that we received zero transactions since they are not present in the
        // dag
        assert!(serialized_transactions.is_empty());
    }
}
