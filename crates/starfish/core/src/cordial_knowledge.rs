// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::max,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use ahash::{AHashMap, AHashSet};
use bytes::Bytes;
use iota_metrics::monitored_mpsc::{self, Receiver, Sender};
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{
    sync::{Mutex, mpsc::error::TrySendError},
    task::JoinError,
};
use tracing::{debug, warn};

use crate::{
    BlockHeaderAPI, BlockRef, Round, VerifiedBlockHeader,
    block_header::{BlockHeaderDigest, TransactionsCommitment, VerifiedBlock},
    context::Context,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{BlockBundle, SerializedBlockBundleParts},
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _},
};

/// Maximum round gap to consider a peer's useful shards/headers as still
/// relevant. 40 rounds correspond to at least 2 second due to the minimum block
/// delay
const MAX_ROUND_GAP_FOR_USEFUL_PARTS: Round = 40;
/// Capacity of the cordial knowledge channel. For normal operation with
/// 100 authorities, this allows buffering up to 5 seconds of headers at 20
/// blocks/sec. When the channel is full, the sender will skip sending new
/// messages.
const CORDIAL_KNOWLEDGE_CHANNEL_CAPACITY: usize = 10_000;
/// Eviction is performed every EVICTION_CHECK_INTERVAL processed messages.
/// This allows batching eviction checks instead of checking on every
/// message. For this operation, we don't need high precision, but we don't
/// skip evictions for too long either.
const EVICTION_CHECK_INTERVAL: usize = 10_000;

/// Represents a subset of authorities using a bitmask.
/// Each bit in the `low` and `high` fields corresponds to an authority index.
/// The maximum number of authorities supported is 256 (0-255).
#[derive(Clone, Copy, Debug)]
pub(crate) struct SubsetAuthorities {
    low: u128,
    high: u128,
}

pub type Ancestors = Arc<[BlockRef]>;
impl SubsetAuthorities {
    #[inline]
    pub fn new_with(author: usize, own: usize) -> Self {
        let mut s = Self { low: 0, high: 0 };
        s.insert(author);
        s.insert(own);
        s
    }

    /// Insert an authority into the subset. Returns true if the authority was
    /// not already present.
    #[inline]
    pub fn insert(&mut self, i: usize) -> bool {
        if i < 128 {
            let mask = 1u128 << i;
            let already_present = (self.low & mask) != 0;
            self.low |= mask;
            !already_present
        } else {
            let bit = i - 128;
            let mask = 1u128 << bit;
            let already_present = (self.high & mask) != 0;
            self.high |= mask;
            !already_present
        }
    }
}

/// Manages the global cordial knowledge state.
/// Receives high-level updates from DagState and AuthorityService and
/// notifies per-connection tasks.
pub(crate) struct CordialKnowledge {
    context: Arc<Context>,
    /// Receives high-level updates from DAG state (new headers, new own shards)
    /// and AuthorityService
    cordial_knowledge_receiver: Receiver<CordialKnowledgeMessage>,
    /// Receives eviction rounds from DagState (latest-only).
    eviction_rounds_receiver: tokio::sync::watch::Receiver<Vec<Round>>,
    /// Keeps track of the last round for which each peer's shards were
    /// considered useful to us. This is a global knowledge and is shared with
    /// all connection tasks. Initialized to None for all authorities and
    /// updated over time once AuthorityService reports useful shards from
    /// peers.
    last_useful_shards_from_peer_round: Vec<Option<Round>>,
    /// Keeps track of the most recent DAG cordial
    /// knowledge (who knows which blocks) for each authority. This is a helper
    /// structure that is used primarily for traversing the recent DAG. This
    /// struct is evicted after flushing the dag state to storage and is not
    /// persisted. To access the cordial knowledge of a given block_ref, one
    /// shall retrieve it from `cordial_knowledge[block_ref.
    /// author][block_ref.round][block_ref.digest]`. The provided value is a
    /// tuple of (ancestors, who knows the block header).
    cordial_knowledge:
        Vec<BTreeMap<Round, AHashMap<BlockHeaderDigest, (Ancestors, SubsetAuthorities)>>>,
    /// Each Connection Knowledge corresponds to one peer. Upon reception of a
    /// message from CordialKnowledge, we propagate the respected
    /// information for each connection.
    connection_knowledges: Vec<Arc<RwLock<ConnectionKnowledge>>>,
}

/// High-level messages sent to the CordialKnowledge task.
/// NewHeader, NewShard are received from DAG state.
/// UsefulShardsFromPeers is received from AuthorityService.
#[derive(Debug)]
pub enum CordialKnowledgeMessage {
    /// A new verified block header to integrate into cordial knowledge.
    /// Includes transaction commitments of all blocks acknowledged by this
    /// header.
    NewHeader {
        header: VerifiedBlockHeader,
        ack_transactions_commitments: Vec<Option<TransactionsCommitment>>,
    },
    /// A new verified own shard to integrate into cordial knowledge.
    NewShard(GenericTransactionRef),
    /// Update internal state about shards from which authorities are useful for
    /// the local node
    UsefulShardsFromPeers(BTreeMap<AuthorityIndex, Round>),
}

impl CordialKnowledgeMessage {
    /// Outputs the type of CordialKnowledgeMessage in a string slice format
    fn type_label(&self) -> &'static str {
        match self {
            CordialKnowledgeMessage::NewHeader { .. } => "New header",
            CordialKnowledgeMessage::NewShard(_) => "New shard",
            CordialKnowledgeMessage::UsefulShardsFromPeers(_) => "Useful authors for shards",
        }
    }
}

/// Handle to the CordialKnowledge task, allowing interaction and graceful
/// shutdown.
pub struct CordialKnowledgeHandle {
    cordial_knowledge_sender: Sender<CordialKnowledgeMessage>,
    connection_knowledges: Vec<Arc<RwLock<ConnectionKnowledge>>>,
    cordial_knowledge_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl CordialKnowledgeHandle {
    /// Outputs specific ConnectionKnowledge corresponding to a given
    /// AuthorityIndex.
    pub fn connection_knowledge(
        &self,
        authority_index: AuthorityIndex,
    ) -> Arc<RwLock<ConnectionKnowledge>> {
        self.connection_knowledges[authority_index].clone()
    }

    /// Gracefully stop the CordialKnowledge background task and all connection
    /// tasks.
    pub async fn stop(&self) -> Result<(), JoinError> {
        // Stop main CordialKnowledge loop
        let mut guard = self.cordial_knowledge_handle.lock().await;

        if let Some(main_handle) = guard.take() {
            main_handle.abort();
            match main_handle.await {
                Ok(_) => (),
                Err(e) if e.is_cancelled() => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    /// Report from AuthorityService useful information about headers and
    /// shards to global knowledge and connection knowledge.
    pub fn report_useful_authors(
        &self,
        peer: AuthorityIndex,
        serialized_block_bundle_parts: &SerializedBlockBundleParts,
        additional_block_headers: &[VerifiedBlockHeader],
        missing_ancestors: &BTreeSet<BlockRef>,
        block_round: Round,
    ) -> ConsensusResult<()> {
        let cordial_knowledge_sender = &self.cordial_knowledge_sender;
        // Extract authorities this peer has useful headers from
        let useful_headers_authors_from_peer = additional_block_headers
            .iter()
            .map(|block_header| block_header.author())
            .chain(missing_ancestors.iter().map(|block_ref| block_ref.author))
            .collect::<BTreeSet<_>>();
        let useful_headers_from_peer = useful_headers_authors_from_peer
            .into_iter()
            .map(|a| (a, block_round))
            .collect();

        // Extract authorities this peer has useful shards from
        let mut useful_shard_authors: BTreeMap<AuthorityIndex, Round> = BTreeMap::new();
        // Since headers showed up in the filter before the corresponding full blocks
        // we consider all authors of additional headers as useful shard authors too.
        for header in additional_block_headers {
            let author = header.author();
            let round = header.round();

            // Insert or update if newer round
            useful_shard_authors
                .entry(author)
                .and_modify(|was_round| *was_round = (*was_round).max(round))
                .or_insert(round);
        }

        // Extract authorities this peer finds useful for cordial dissemination from our
        // side
        let useful_headers_to_peer = serialized_block_bundle_parts.useful_headers_authors();
        let useful_headers_to_peer = useful_headers_to_peer
            .iter()
            .map(|&a| (a, block_round))
            .collect::<BTreeMap<_, _>>();
        // Extract authorities this peer finds useful shards from our side
        let useful_shards_to_peer = serialized_block_bundle_parts.useful_shards_authors();
        let useful_shards_to_peer = useful_shards_to_peer
            .iter()
            .map(|&a| (a, block_round))
            .collect::<BTreeMap<_, _>>();

        // Notify connection knowledge about useful headers and shards to/from this peer
        let connection_knowledge_message = ConnectionKnowledgeMessage::UsefulAuthors {
            useful_headers_to_peer,
            useful_shards_to_peer,
            useful_headers_from_peer,
            useful_shards_from_peer: vec![None; self.connection_knowledges.len()],
        };
        {
            let mut connection_knowledge_guard = self.connection_knowledges[peer].write();
            connection_knowledge_guard.process_one_message(connection_knowledge_message);
        }
        // Notify global cordial knowledge about useful shards from this peer
        if !useful_shard_authors.is_empty() {
            let cordial_knowledge_message =
                CordialKnowledgeMessage::UsefulShardsFromPeers(useful_shard_authors);
            if let Err(TrySendError::Closed(_)) =
                cordial_knowledge_sender.try_send(cordial_knowledge_message)
            {
                return Err(ConsensusError::Shutdown);
            }
        }

        Ok(())
    }
}

impl CordialKnowledge {
    /// Create a new CordialKnowledge instance along with its associated
    /// channels.
    fn new(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> (
        Self,
        Vec<Arc<RwLock<ConnectionKnowledge>>>,
        Sender<CordialKnowledgeMessage>,
        tokio::sync::watch::Sender<Vec<Round>>,
    ) {
        let num_authorities = context.committee.size();

        // Main bounded channel for high-level DAG updates (monitored for metrics)
        let (cordial_knowledge_sender, cordial_knowledge_receiver): (
            Sender<CordialKnowledgeMessage>,
            Receiver<CordialKnowledgeMessage>,
        ) = monitored_mpsc::channel("cordial_knowledge", CORDIAL_KNOWLEDGE_CHANNEL_CAPACITY);
        let (eviction_rounds_sender, eviction_rounds_receiver) =
            tokio::sync::watch::channel(Vec::new());

        let mut connection_knowledges = Vec::with_capacity(num_authorities);

        for peer_index in 0..num_authorities {
            let peer = AuthorityIndex::from(peer_index as u8);
            let connection_knowledge =
                ConnectionKnowledge::new(context.clone(), peer, dag_state.clone());

            let connection_knowledge = Arc::new(RwLock::new(connection_knowledge));

            connection_knowledges.push(connection_knowledge);
        }

        (
            Self {
                context,
                cordial_knowledge_receiver,
                eviction_rounds_receiver,
                cordial_knowledge: vec![BTreeMap::new(); num_authorities],
                last_useful_shards_from_peer_round: vec![None; num_authorities],
                connection_knowledges: connection_knowledges.clone(),
            },
            connection_knowledges,
            cordial_knowledge_sender,
            eviction_rounds_sender,
        )
    }

    /// Start the CordialKnowledge task and all ConnectionKnowledge tasks.
    /// Updates the DAG state with the sender to the CordialKnowledge task.
    /// Return a handle to these tasks.
    pub fn start(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Arc<CordialKnowledgeHandle> {
        // Build main CordialKnowledge and associated channels
        let (
            cordial_knowledge,
            connection_knowledges,
            cordial_knowledge_sender,
            eviction_rounds_sender,
        ) = CordialKnowledge::new(context, dag_state.clone());
        // Spawn the main CordialKnowledge loop
        let cordial_knowledge_handle = tokio::spawn(async move {
            cordial_knowledge.run().await;
        });

        dag_state.write().set_cordial_knowledge_senders(
            cordial_knowledge_sender.clone(),
            eviction_rounds_sender,
        );

        // Return handle with all pieces assembled
        Arc::new(CordialKnowledgeHandle {
            cordial_knowledge_sender,
            connection_knowledges,
            cordial_knowledge_handle: Mutex::new(Some(cordial_knowledge_handle)),
        })
    }

    /// Main async loop: receives high-level updates (headers, shards)
    /// from DAG state and updates global knowledge + notifies per-connection
    /// tasks. Evictions are checked periodically via a watch channel.
    async fn run(mut self) {
        debug!("Cordial Knowledge main loop started");
        let mut processed_since_eviction = 0usize;

        loop {
            match self.cordial_knowledge_receiver.recv().await {
                Some(msg) => {
                    let mut batch = vec![msg];
                    while let Ok(msg) = self.cordial_knowledge_receiver.try_recv() {
                        batch.push(msg);
                    }
                    processed_since_eviction = processed_since_eviction.saturating_add(batch.len());
                    // Report the buffer size
                    self.context
                        .metrics
                        .node_metrics
                        .cordial_knowledge_message_batch_size
                        .observe(batch.len() as f64);
                    let mut vec_connection_knowledge_msgs_batch: Vec<Vec<_>> =
                        (0..self.context.committee.size())
                            .map(|_| Vec::new())
                            .collect();

                    for msg in batch {
                        if let Some(vec_connection_knowledge_msgs) = self.process_message(msg) {
                            for (index, msgs) in
                                vec_connection_knowledge_msgs.into_iter().enumerate()
                            {
                                vec_connection_knowledge_msgs_batch[index].extend(msgs);
                            }
                        }
                    }

                    if processed_since_eviction >= EVICTION_CHECK_INTERVAL {
                        self.append_eviction_msgs_if_changed(
                            &mut vec_connection_knowledge_msgs_batch,
                        );
                        self.report_sizes();
                        processed_since_eviction = 0;
                    }

                    for (index, msgs) in vec_connection_knowledge_msgs_batch.into_iter().enumerate()
                    {
                        if !msgs.is_empty() {
                            let mut guard = self.connection_knowledges[index].write();
                            guard.process_vec_messages(msgs);
                        }
                    }
                }
                None => {
                    debug!("Cordial Knowledge channel closed; exiting loop");
                    break;
                }
            }
        }

        debug!("Cordial Knowledge main loop finished");
    }

    fn append_eviction_msgs_if_changed(
        &mut self,
        vec_connection_knowledge_msgs_batch: &mut [Vec<ConnectionKnowledgeMessage>],
    ) {
        if !self.eviction_rounds_receiver.has_changed().unwrap_or(false) {
            return;
        }
        let evicted_rounds = self.eviction_rounds_receiver.borrow_and_update().clone();
        if evicted_rounds.len() != self.context.committee.size() {
            warn!(
                "Eviction rounds length {} does not match committee size {}; skipping eviction",
                evicted_rounds.len(),
                self.context.committee.size()
            );
            return;
        }
        if let Some(vec_connection_knowledge_msgs) = self.handle_evict_below(evicted_rounds) {
            for (index, msgs) in vec_connection_knowledge_msgs.into_iter().enumerate() {
                vec_connection_knowledge_msgs_batch[index].extend(msgs);
            }
        }
    }

    /// Processes a single high-level cordial knowledge message.
    fn process_message(
        &mut self,
        cordial_knowledge_message: CordialKnowledgeMessage,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        // Report the type of message
        self.context
            .metrics
            .node_metrics
            .cordial_knowledge_processed_messages
            .with_label_values(&[cordial_knowledge_message.type_label()])
            .inc();

        // Handle the cordial knowledge message depending on its type

        match cordial_knowledge_message {
            CordialKnowledgeMessage::NewHeader {
                header,
                ack_transactions_commitments,
            } => self.update_cordial_knowledge(&header, &ack_transactions_commitments),
            CordialKnowledgeMessage::NewShard(gen_tx_ref) => {
                self.prepare_new_shard_msgs(gen_tx_ref)
            }
            CordialKnowledgeMessage::UsefulShardsFromPeers(useful_shards_from_peer) => {
                self.handle_useful_shards_from(useful_shards_from_peer)
            }
        }
    }

    // Helper function to update authority rounds if the new round is greater
    fn update_authority_rounds_if_greater(
        target: &mut [Option<Round>],
        updates: BTreeMap<AuthorityIndex, Round>,
    ) -> bool {
        let mut changed = false;
        for (authority, new_round) in updates {
            if let Some(existing_round) = &mut target[authority.value()] {
                if new_round > *existing_round {
                    *existing_round = new_round;
                    changed = true;
                }
            } else {
                target[authority.value()] = Some(new_round);
                changed = true;
            }
        }
        changed
    }

    /// Update global knowledge about shards from which authors will be useful
    /// for us
    fn handle_useful_shards_from(
        &mut self,
        useful_shards_from_peer: BTreeMap<AuthorityIndex, Round>,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        if Self::update_authority_rounds_if_greater(
            &mut self.last_useful_shards_from_peer_round,
            useful_shards_from_peer,
        ) {
            self.prepare_useful_shards_from_peers_msgs()
        } else {
            None
        }
    }

    /// Prepare useful authors message for each connection knowledge.
    fn prepare_useful_shards_from_peers_msgs(
        &mut self,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        let mut vec_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            Vec::with_capacity(self.cordial_knowledge.len());
        for index in 0..self.cordial_knowledge.len() {
            if index == self.context.own_index.value() {
                vec_msgs.push(vec![]);
                continue;
            }
            let msg = ConnectionKnowledgeMessage::UsefulAuthors {
                useful_shards_from_peer: self.last_useful_shards_from_peer_round.clone(),
                useful_headers_from_peer: BTreeMap::new(),
                useful_headers_to_peer: BTreeMap::new(),
                useful_shards_to_peer: BTreeMap::new(),
            };
            vec_msgs.push(vec![msg]);
        }
        Some(vec_msgs)
    }

    /// Called when a new own shard (created locally) is added to dag state.
    fn prepare_new_shard_msgs(
        &mut self,
        gen_transaction_ref: GenericTransactionRef,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        let mut vec_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            Vec::with_capacity(self.cordial_knowledge.len());
        for index in 0..self.cordial_knowledge.len() {
            // Don't send own shard to the author of the block and local node
            if index == gen_transaction_ref.author().value()
                || index == self.context.own_index.value()
            {
                vec_msgs.push(vec![]);
                continue;
            }
            let msg = ConnectionKnowledgeMessage::NewShard {
                gen_tx_ref: gen_transaction_ref,
            };
            vec_msgs.push(vec![msg]);
        }
        Some(vec_msgs)
    }

    /// Called when older rounds should be pruned globally.
    fn handle_evict_below(
        &mut self,
        evicted_rounds: Vec<Round>,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        // Evict locally
        for (index, btree_map) in &mut self.cordial_knowledge.iter_mut().enumerate() {
            // Increase by 1 for splitting as the evicted rounds are gone from memory
            let split_round = evicted_rounds[index] + 1;
            // Remove everything strictly below this round
            *btree_map = btree_map.split_off(&split_round);
        }

        // Prepare message for per-connection knowledge about eviction
        self.prepare_evict_msgs(evicted_rounds)
    }
    #[inline]
    fn prepare_evict_msgs(
        &self,
        rounds: Vec<Round>,
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        let mut vec_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            Vec::with_capacity(self.cordial_knowledge.len());
        for _ in 0..self.cordial_knowledge.len() {
            let msg = ConnectionKnowledgeMessage::EvictBelow(rounds.clone());
            vec_msgs.push(vec![msg]);
        }
        Some(vec_msgs)
    }

    /// Report current sizes of cordial knowledge data structures.
    fn report_sizes(&self) {
        let metrics = &self.context.metrics.node_metrics;

        let global_entries: usize = self
            .cordial_knowledge
            .iter()
            .map(|m| m.values().map(|v| v.len()).sum::<usize>())
            .sum();
        metrics.cordial_knowledge_entries.set(global_entries as i64);

        let mut total_headers_not_known: usize = 0;
        let mut total_shards_not_known: usize = 0;
        for ck in &self.connection_knowledges {
            let guard = ck.read();
            let (headers, shards) = guard.sizes();
            total_headers_not_known += headers;
            total_shards_not_known += shards;
        }
        metrics
            .cordial_knowledge_headers_not_known
            .set(total_headers_not_known as i64);
        metrics
            .cordial_knowledge_shards_not_known
            .set(total_shards_not_known as i64);
    }

    /// Update cordial knowledge for exactly one new header.
    /// Assumes all parents are already stored somewhere in
    /// `recent_dag_cordial_knowledge` (if not, they will be skipped).
    /// We traverse back the causal past of the new header and mark all
    /// ancestors as known by the block author. All acknowledged blocks are
    /// marked as known by the block author as well.
    /// At the end, we notify all connections about new
    /// knowledge changes.
    fn update_cordial_knowledge(
        &mut self,
        header: &VerifiedBlockHeader,
        ack_transactions_commitments: &[Option<TransactionsCommitment>],
    ) -> Option<Vec<Vec<ConnectionKnowledgeMessage>>> {
        let block_ref = header.reference();
        let block_author = block_ref.author.value();
        let block_round = block_ref.round;
        let block_digest = block_ref.digest;
        let own_index = self.context.own_index.value();

        // Pre-allocate message buffers
        let mut vec_knowledge_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            (0..self.context.committee.size())
                .map(|_| Vec::new())
                .collect();

        // 1) Ensure we have a round map for this author and insert the block if new
        let btree_map = &mut self.cordial_knowledge[block_author];
        let round_map = btree_map.entry(block_round).or_default();

        // Already recorded — nothing else to do.
        if round_map.contains_key(&block_digest) {
            return None;
        }

        // Insert block into cordial knowledge
        let ancestors: Ancestors = Arc::from(header.ancestors());
        let who_knows_this_block = SubsetAuthorities::new_with(block_author, own_index);
        round_map.insert(block_digest, (ancestors, who_knows_this_block));

        // 2) Notify all *other* authorities (except self and block_author) about new
        //    header
        for (authority, msgs) in vec_knowledge_msgs.iter_mut().enumerate() {
            // don't send shard to self nor to the author of the block
            if authority == block_author || authority == own_index {
                continue;
            }
            msgs.push(ConnectionKnowledgeMessage::NewHeader { block_ref });
        }

        // 3) The block_author now acknowledges previously known transactions
        // Use the provided transaction commitments to create the proper
        // GenericTransactionRef variant
        let consensus_fast_commit_sync = self.context.protocol_config.consensus_fast_commit_sync();
        for (acknowledgment, &transactions_commitment) in header
            .acknowledgments()
            .iter()
            .zip(ack_transactions_commitments.iter())
        {
            let gen_tx_ref = if consensus_fast_commit_sync {
                if let Some(transactions_commitment) = transactions_commitment {
                    GenericTransactionRef::TransactionRef(crate::transaction_ref::TransactionRef {
                        round: acknowledgment.round,
                        author: acknowledgment.author,
                        transactions_commitment,
                    })
                } else {
                    continue;
                }
            } else {
                GenericTransactionRef::BlockRef(*acknowledgment)
            };

            vec_knowledge_msgs[block_author]
                .push(ConnectionKnowledgeMessage::RemoveShard { gen_tx_ref });
        }

        // 4) Traversing back and marking the causal past as known by block_author
        let mut stack = vec![block_ref];
        while let Some(current_ref) = stack.pop() {
            let current_author = current_ref.author.value();
            let current_round = current_ref.round;
            let current_digest = current_ref.digest;

            // ---- Get parents of current block ----
            let parents_buf: Ancestors = {
                let author_map = &self.cordial_knowledge[current_author];
                let Some(current_round_map) = author_map.get(&current_round) else {
                    continue;
                };
                let Some((parents, _)) = current_round_map.get(&current_digest) else {
                    continue;
                };
                parents.clone()
            };

            // Traverse parents
            for parent_ref in parents_buf.iter() {
                let parent_author = parent_ref.author.value();
                let parent_round = parent_ref.round;
                let parent_digest = parent_ref.digest;

                let parent_author_map = &mut self.cordial_knowledge[parent_author];

                if let Some(parent_round_map) = parent_author_map.get_mut(&parent_round) {
                    if let Some((_, who_knows_parent)) = parent_round_map.get_mut(&parent_digest) {
                        // Mark that block_author now knows this parent
                        if who_knows_parent.insert(block_author) {
                            vec_knowledge_msgs[block_author].push(
                                ConnectionKnowledgeMessage::RemoveHeader {
                                    block_ref: *parent_ref,
                                },
                            );
                            stack.push(*parent_ref);
                        }
                    }
                }
            }
        }
        Some(vec_knowledge_msgs)
    }
}

/// Messages sent to a ConnectionKnowledge task to update its state.
#[derive(Debug)]
pub enum ConnectionKnowledgeMessage {
    /// A new block header was added globally.
    NewHeader { block_ref: BlockRef },
    /// Remove a block header from the "unknown" set .
    RemoveHeader { block_ref: BlockRef },
    /// A new shard was added globally.
    NewShard { gen_tx_ref: GenericTransactionRef },
    /// Remove a header from the "unknown" set.
    RemoveShard { gen_tx_ref: GenericTransactionRef },
    /// Update useful info about which authorities are useful to/from the peer.
    UsefulAuthors {
        useful_headers_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_headers_from_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_from_peer: Vec<Option<Round>>,
    },
    /// Global eviction (prune below round)
    EvictBelow(Vec<Round>),
}

/// Manages the knowledge state for a single connection to a peer.
/// Receives updates from the global cordial knowledge
pub struct ConnectionKnowledge {
    context: Arc<Context>,
    peer: AuthorityIndex,
    dag_state: Arc<RwLock<DagState>>,
    /// Keeps track of which headers are not known by the peer yet.
    headers_not_known: Vec<BTreeMap<Round, AHashSet<BlockRef>>>,
    /// Keeps track of which shards are not known by the peer yet.
    shards_not_known: Vec<BTreeMap<Round, AHashSet<GenericTransactionRef>>>,
    /// Last rounds for (potentially) useful shards that can be sent to this
    /// peer
    last_useful_shards_to_peer_round: Vec<Option<Round>>,
    /// Last rounds for (potentially) useful headers that can be sent to this
    /// peer
    last_useful_headers_to_peer_round: Vec<Option<Round>>,
    /// Last rounds for potentially useful shards that could be received from
    /// this peer
    last_useful_shards_from_peer_round: Vec<Option<Round>>,
    /// Last rounds for (potentially) useful headers that could be received from
    /// this peer
    last_useful_headers_from_peer_round: Vec<Option<Round>>,
}

impl ConnectionKnowledge {
    pub fn new(
        context: Arc<Context>,
        peer: AuthorityIndex,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Self {
        let num_authorities = context.committee.size();

        Self {
            dag_state,
            peer,
            last_useful_headers_to_peer_round: vec![None; num_authorities],
            last_useful_shards_to_peer_round: vec![None; num_authorities],
            last_useful_headers_from_peer_round: vec![None; num_authorities],
            last_useful_shards_from_peer_round: vec![None; num_authorities],
            context,
            headers_not_known: vec![BTreeMap::new(); num_authorities],
            shards_not_known: vec![BTreeMap::new(); num_authorities],
        }
    }

    /// Processes a vector of ConnectionKnowledge messages
    fn process_vec_messages(&mut self, msgs: Vec<ConnectionKnowledgeMessage>) {
        for msg in msgs {
            self.process_one_message(msg);
        }
    }
    /// Take useful refs (headers or shards) for the given authorities
    /// up to the given round (exclusive), up to max_take total.
    /// Generic function that works with both BlockRef and
    /// GenericTransactionRef.
    fn take_useful_refs_round<T>(
        maps: &mut [BTreeMap<Round, AHashSet<T>>],
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
        max_take: usize,
        get_author: impl Fn(&T) -> usize,
        get_round: impl Fn(&T) -> Round,
    ) -> Vec<T>
    where
        T: Copy + Eq + std::hash::Hash,
    {
        if useful_authorities.is_empty() || max_take == 0 {
            return Vec::new();
        }

        // Find the smallest existing round among all useful authorities.
        let min_round = useful_authorities
            .iter()
            .filter_map(|&auth| maps[auth].keys().next().copied())
            .min();

        let Some(mut current_round) = min_round else {
            return Vec::new();
        };

        let mut taken = Vec::with_capacity(max_take);

        'outer: while current_round < round_upper_bound_exclusive {
            for &authority in useful_authorities {
                let map = &maps[authority];
                if let Some(refs_from_authority_in_round) = map.get(&current_round) {
                    for &item_ref in refs_from_authority_in_round {
                        taken.push(item_ref);
                        if taken.len() >= max_take {
                            break 'outer;
                        }
                    }
                }
            }
            current_round += 1;
        }

        // Remove the taken refs from the corresponding authorities
        for item_ref in &taken {
            let authority = get_author(item_ref);
            let round = get_round(item_ref);
            if let Some(refs_from_authority_in_round) = maps[authority].get_mut(&round) {
                refs_from_authority_in_round.remove(item_ref);
                // Remove empty rounds to keep map small
                if refs_from_authority_in_round.is_empty() {
                    maps[authority].remove(&round);
                }
            }
        }

        taken
    }

    /// Take useful header block refs from the given authorities up to the given
    /// round (exclusive).
    fn take_useful_header_block_refs_round(
        &mut self,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
    ) -> Vec<BlockRef> {
        let max_take = self.context.parameters.max_headers_per_bundle;
        Self::take_useful_refs_round(
            &mut self.headers_not_known,
            round_upper_bound_exclusive,
            useful_authorities,
            max_take,
            |block_ref| block_ref.author.value(),
            |block_ref| block_ref.round,
        )
    }

    /// Take useful shard block refs from the given authorities up to the given
    /// round (exclusive).
    fn take_useful_shard_block_refs_round(
        &mut self,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
    ) -> Vec<GenericTransactionRef> {
        let max_take = self.context.parameters.max_shards_per_bundle;
        Self::take_useful_refs_round(
            &mut self.shards_not_known,
            round_upper_bound_exclusive,
            useful_authorities,
            max_take,
            |gen_tx_ref| gen_tx_ref.author().value(),
            |gen_tx_ref| gen_tx_ref.round(),
        )
    }

    /// Evict all connection knowledge below the given rounds (exclusive)
    fn evict_below(&mut self, evicted_rounds: Vec<Round>) {
        for (index, map) in self.headers_not_known.iter_mut().enumerate() {
            let threshold_round = evicted_rounds[index] + 1;
            // Keep only entries >= threshold
            *map = map.split_off(&threshold_round);
        }

        for (index, map) in self.shards_not_known.iter_mut().enumerate() {
            let threshold_round = evicted_rounds[index] + 1;
            *map = map.split_off(&threshold_round);
        }
    }

    /// Processes a batch of knowledge updates for this connection.
    /// The only async message is `TakeAdditionalPartForBundle`, which awaits
    /// and provides the additional parts for the bundle
    pub fn process_one_message(&mut self, message: ConnectionKnowledgeMessage) {
        match message {
            ConnectionKnowledgeMessage::NewHeader { block_ref } => {
                self.handle_new_header(block_ref);
            }
            ConnectionKnowledgeMessage::RemoveHeader { block_ref } => {
                self.handle_remove_header(block_ref);
            }
            ConnectionKnowledgeMessage::NewShard { gen_tx_ref } => {
                self.handle_new_shard(gen_tx_ref);
            }
            ConnectionKnowledgeMessage::RemoveShard { gen_tx_ref } => {
                self.handle_remove_shard(gen_tx_ref);
            }
            ConnectionKnowledgeMessage::EvictBelow(rounds) => {
                self.evict_below(rounds);
            }
            ConnectionKnowledgeMessage::UsefulAuthors {
                useful_headers_to_peer,
                useful_shards_to_peer,
                useful_headers_from_peer,
                useful_shards_from_peer,
            } => {
                self.handle_useful_authors(
                    useful_headers_to_peer,
                    useful_shards_to_peer,
                    useful_headers_from_peer,
                    useful_shards_from_peer,
                );
            }
        }
    }

    /// Handle useful info update from global CordialKnowledge or
    /// AuthorityService.
    fn handle_useful_authors(
        &mut self,
        useful_headers_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_headers_from_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_from_peer: Vec<Option<Round>>,
    ) {
        // Update local state
        self.handle_useful_headers_to(useful_headers_to_peer);
        self.handle_useful_shards_to(useful_shards_to_peer);
        self.handle_useful_headers_from(useful_headers_from_peer);
        self.handle_useful_shards_from(useful_shards_from_peer);
    }

    /// Update last useful shards from peer rounds
    fn handle_useful_shards_from(&mut self, useful_shards_from_peer_round: Vec<Option<Round>>) {
        for (index, opt_round) in useful_shards_from_peer_round.into_iter().enumerate() {
            if let Some(new_round) = opt_round {
                if let Some(old_round) = &mut self.last_useful_shards_from_peer_round[index] {
                    *old_round = max(*old_round, new_round);
                } else {
                    self.last_useful_shards_from_peer_round[index] = Some(new_round);
                }
            }
        }
    }

    /// Update last rounds of useful headers from peer. Iterate over the given
    /// map (authority, round) and update only if the new round is greater.
    fn handle_useful_headers_from(
        &mut self,
        authorities_with_round: BTreeMap<AuthorityIndex, Round>,
    ) {
        CordialKnowledge::update_authority_rounds_if_greater(
            &mut self.last_useful_headers_from_peer_round,
            authorities_with_round,
        );
    }

    /// Update last rounds of useful shards to peer. Iterate over the given map
    /// (authority, round) and update only if the new round is greater.
    fn handle_useful_shards_to(&mut self, authorities_with_round: BTreeMap<AuthorityIndex, Round>) {
        CordialKnowledge::update_authority_rounds_if_greater(
            &mut self.last_useful_shards_to_peer_round,
            authorities_with_round,
        );
    }

    /// Update last rounds of useful headers to peer. Iterate over the given map
    /// (authority, round) and update only if the new round is greater.
    fn handle_useful_headers_to(
        &mut self,
        authorities_with_round: BTreeMap<AuthorityIndex, Round>,
    ) {
        CordialKnowledge::update_authority_rounds_if_greater(
            &mut self.last_useful_headers_to_peer_round,
            authorities_with_round,
        );
    }

    /// Used by AuthorityService to create a block bundle
    /// to send to the peer.
    pub fn create_bundle(&mut self, block: VerifiedBlock) -> BlockBundle {
        let block_round = block.round();
        // Try to update ancestors as they may still be pending updates.
        // These headers will also be updated via cordial knowledge messages and may be
        // sent again in the future. We consider this overhead negligible.
        for ancestor_block_ref in block.ancestors() {
            self.handle_new_header(*ancestor_block_ref);
        }
        // 1. Own headers and shards for round up to round_upper_bound_exclusive should
        //    be marked as known
        let own_index = self.context.own_index;
        let mut rounds = vec![Round::MIN; self.context.committee.size()];
        rounds[own_index] = block_round; // We are supposed to send own block of this round in a bundle when calling this function with this parameter

        self.evict_below(rounds);

        // 2. Identify useful authorities for headers and take the corresponding headers
        //    from the DAG state
        let useful_headers_authors_to_peer: Vec<usize> = self
            .last_useful_headers_to_peer_round
            .iter()
            .enumerate()
            .filter(|(_authority_index, &opt_round)| {
                if let Some(round) = opt_round {
                    round.saturating_add(MAX_ROUND_GAP_FOR_USEFUL_PARTS) >= block_round
                } else {
                    false
                }
            })
            .map(|(authority_index, _opt_round)| authority_index)
            .collect();

        let useful_headers_block_refs_to_peer =
            self.take_useful_header_block_refs_round(block_round, &useful_headers_authors_to_peer);

        let useful_headers_to_peer: Vec<VerifiedBlockHeader> = {
            let dag_state_read = self.dag_state.read();
            dag_state_read
                .get_cached_block_headers(&useful_headers_block_refs_to_peer)
                .into_iter()
                .flatten() // Filter out None values
                .collect()
        };

        // 3. Identify useful authorities for shards and take the corresponding shards
        //    from the DAG state
        let useful_shards_authors_to_peer: Vec<usize> = self
            .last_useful_shards_to_peer_round
            .iter()
            .enumerate()
            .filter(|(_authority_index, &opt_round)| {
                if let Some(round) = opt_round {
                    round.saturating_add(MAX_ROUND_GAP_FOR_USEFUL_PARTS) >= block_round
                } else {
                    false
                }
            })
            .map(|(authority_index, _opt_round)| authority_index)
            .collect();

        let useful_shards_block_refs_to_peer =
            self.take_useful_shard_block_refs_round(block_round, &useful_shards_authors_to_peer);
        let useful_shards_to_peer: Vec<Bytes> = {
            let dag_state_read = self.dag_state.read();
            dag_state_read
                .get_cached_shards(&useful_shards_block_refs_to_peer)
                .into_iter()
                .flatten() // Filter out None values
                .collect()
        };

        // 4. Get useful header authors from peer.
        // Authority is (potentially) useful if the
        // last known useful round + MAX_ROUND_GAP_FOR_USEFUL_PARTS >=
        // round_upper_bound_exclusive
        let useful_headers_authors_from_peer = self
            .last_useful_headers_from_peer_round
            .iter()
            .enumerate()
            .filter(|(_authority_index, &opt_round)| {
                if let Some(round) = opt_round {
                    round.saturating_add(MAX_ROUND_GAP_FOR_USEFUL_PARTS) >= block_round
                } else {
                    false
                }
            })
            .map(|(authority_index, _opt_round)| AuthorityIndex::from(authority_index as u8))
            .collect::<BTreeSet<AuthorityIndex>>();

        // 5. Get useful shard authors from peer
        let useful_shards_authors_from_peer = self
            .last_useful_shards_from_peer_round
            .iter()
            .enumerate()
            .filter(|(_authority_index, &opt_round)| {
                if let Some(round) = opt_round {
                    round.saturating_add(MAX_ROUND_GAP_FOR_USEFUL_PARTS) >= block_round
                } else {
                    false
                }
            })
            .map(|(authority_index, _opt_round)| AuthorityIndex::from(authority_index as u8))
            .collect::<BTreeSet<AuthorityIndex>>();

        // Report useful authors
        let peer_hostname = self.context.authority_hostname(self.peer);
        for author in &useful_headers_authors_from_peer {
            let author_hostname = self.context.authority_hostname(*author);
            self.context
                .metrics
                .node_metrics
                .cordial_knowledge_useful_headers_authors
                .with_label_values(&[peer_hostname, author_hostname])
                .inc();
        }

        for author in &useful_shards_authors_from_peer {
            let author_hostname = self.context.authority_hostname(*author);
            self.context
                .metrics
                .node_metrics
                .cordial_knowledge_useful_shards_authors
                .with_label_values(&[author_hostname])
                .inc();
        }

        BlockBundle {
            verified_block: block,
            verified_headers: useful_headers_to_peer,
            serialized_shards: useful_shards_to_peer,
            useful_headers_authors: useful_headers_authors_from_peer,
            useful_shards_authors: useful_shards_authors_from_peer,
        }
    }

    /// Handles adding a new header to the set of potentially unknown headers
    fn handle_new_header(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();

        // Insert the block into the set for that (authority, round)
        self.headers_not_known[authority]
            .entry(round)
            .or_default()
            .insert(block_ref);
    }

    /// Handles adding a new shard to the set of potentially unknown shards.
    fn handle_new_shard(&mut self, gen_tx_ref: GenericTransactionRef) {
        let round = gen_tx_ref.round();
        let authority = gen_tx_ref.author().value();

        self.shards_not_known[authority]
            .entry(round)
            .or_default()
            .insert(gen_tx_ref);
    }

    /// Returns (total_headers_not_known, total_shards_not_known) entry counts.
    fn sizes(&self) -> (usize, usize) {
        let headers: usize = self
            .headers_not_known
            .iter()
            .map(|m| m.values().map(|s| s.len()).sum::<usize>())
            .sum();
        let shards: usize = self
            .shards_not_known
            .iter()
            .map(|m| m.values().map(|s| s.len()).sum::<usize>())
            .sum();
        (headers, shards)
    }

    /// Handles removing a header that this peer now knows.
    fn handle_remove_header(&mut self, block_ref: BlockRef) {
        let authority = block_ref.author.value();
        let round = block_ref.round;

        if let Some(set) = self.headers_not_known[authority].get_mut(&round) {
            set.remove(&block_ref);
            // Optional: remove empty round entries to keep map clean
            if set.is_empty() {
                self.headers_not_known[authority].remove(&round);
            }
        }
    }

    /// Handles removing a shard that this peer now knows.
    fn handle_remove_shard(&mut self, gen_tx_ref: GenericTransactionRef) {
        let authority = gen_tx_ref.author().value();
        let round = gen_tx_ref.round();

        if let Some(set) = self.shards_not_known[authority].get_mut(&round) {
            set.remove(&gen_tx_ref);
            if set.is_empty() {
                self.shards_not_known[authority].remove(&round);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::RwLock;
    use tokio::time::sleep;

    use super::*;
    use crate::{
        TestBlockHeader,
        block_header::{GENESIS_ROUND, VerifiedBlock, VerifiedOwnShard},
        context::Context,
        dag_state::{DagState, DataSource},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
    };

    /// Test that cordial knowledge correctly tracks blocks from a byzantine
    /// validator that does not disseminate its blocks until a certain round.
    #[tokio::test]
    async fn test_cordial_knowledge_bundle_with_byzantine() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let validators = 4;
        let our_index = AuthorityIndex::new_for_test(0);
        let to_whom_index = AuthorityIndex::new_for_test(1);
        let byzantine_index = AuthorityIndex::new_for_test(3);
        let (context, _key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        // Set up DAG with blocks from all validators.
        // Validator D does not disseminate its blocks, so they are not referenced.
        // Validator A will learn about D's blocks only at round 6.
        // After that, A should be able to send all D's blocks to B.
        let dag_str = "DAG {
                Round 0 : { 4 },
                Round 1 :  { * },
                Round 2 : {
                    A -> [-D1],
                    B -> [-D1],
                    C -> [-D1],
                    D -> [*],
                },
                Round 3 : {
                    A -> [-D2],
                    B -> [-D2],
                    C -> [-D2],
                    D -> [*],
                },
                Round 4 : {
                    A -> [-D3],
                    B -> [-D3],
                    C -> [-D3],
                    D -> [*],
                },
                Round 5 : {
                    A -> [-D4],
                    B -> [-D4],
                    C -> [-D4],
                    D -> [*],
                },
                Round 6 : {
                    A -> [*],
                    B -> [-D5],
                    C -> [-D5],
                    D -> [*],
                },
                Round 7 : { * },
             }";
        let final_round = 6;
        let result = parse_dag(dag_str);
        assert!(result.is_ok());

        let dag_builder = result.unwrap();

        // Get all blocks by rounds
        let mut all_blocks: Vec<Vec<VerifiedBlock>> = vec![];
        for round in 0..=final_round {
            all_blocks.push(dag_builder.blocks(round..=round));
        }

        // Report useful info to connection knowledge corresponding to to_whom_index
        let connection_knowledge = cordial_knowledge.connection_knowledges[to_whom_index].clone();
        // Inject useful info for connection knowledge of peer 1 (B)
        // A says that C and D are useful for headers and shards when receiving from B
        // B says that A and C are useful for headers and shards when sending from A
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

        // get all blocks of D. They will be injected to dag state at final_round
        let d_blocks = all_blocks
            .iter()
            .flat_map(|blocks| blocks.iter().filter(|b| b.author() == byzantine_index))
            .cloned()
            .collect::<Vec<VerifiedBlock>>();
        // Add block to DAG state and automatically update cordial knowledge
        for round in 1..=final_round - 1 {
            if round == final_round - 1 {
                // Add D's blocks to DAG state only at final_round-1
                for block in d_blocks.iter() {
                    let VerifiedBlock {
                        verified_block_header,
                        verified_transactions,
                    } = block.clone();
                    dag_state
                        .write()
                        .accept_block_header(verified_block_header, DataSource::Test);
                    let gen_transaction_ref =
                        if context.protocol_config.consensus_fast_commit_sync() {
                            GenericTransactionRef::TransactionRef(
                                verified_transactions.transaction_ref(),
                            )
                        } else {
                            GenericTransactionRef::BlockRef(
                                verified_transactions.block_ref().expect(
                                    "block_ref must be present in non-transaction-ref path",
                                ),
                            )
                        };
                    let shard_for_core = VerifiedOwnShard {
                        serialized_shard: Bytes::from([0u8; 32].to_vec()), /* put some dummy
                                                                            * shard data */
                        gen_transaction_ref,
                    };
                    dag_state.write().add_shard(shard_for_core);
                }
            }
            // add all blocks of this round and our block of next round to dag state
            for block in all_blocks[round as usize]
                .iter()
                .filter(|b| b.author() != our_index && b.author() != byzantine_index)
                .chain(std::iter::once(&all_blocks[round as usize + 1][our_index]))
            {
                let VerifiedBlock {
                    verified_block_header,
                    verified_transactions,
                } = block.clone();
                dag_state
                    .write()
                    .accept_block_header(verified_block_header, DataSource::Test);
                let gen_transaction_ref = if context.protocol_config.consensus_fast_commit_sync() {
                    GenericTransactionRef::TransactionRef(verified_transactions.transaction_ref())
                } else {
                    GenericTransactionRef::BlockRef(
                        verified_transactions
                            .block_ref()
                            .expect("block_ref must be present in non-transaction-ref path"),
                    )
                };
                let shard_for_core = VerifiedOwnShard {
                    serialized_shard: Bytes::from([0u8; 32].to_vec()), // put some dummy shard data
                    gen_transaction_ref,
                };
                dag_state.write().add_shard(shard_for_core);
            }
            sleep(std::time::Duration::from_millis(10)).await; // give some time for cordial knowledge to update
            // By default, for MAX_ROUND_GAP_FOR_USEFUL_PARTS rounds, all unknown
            // shards/headers are useful
            let block_bundle = {
                connection_knowledge
                    .write()
                    .create_bundle(all_blocks[round as usize + 1][our_index].clone())
            };
            let BlockBundle {
                verified_headers: headers,
                serialized_shards: shards,
                ..
            } = block_bundle;
            // In rounds 1..final_round, A should not know any of D's blocks, so no headers
            // or shards should be sent to B.
            if round < final_round - 1 {
                // Only headers of C's block of previous round should be sent
                assert_eq!(
                    headers.len(),
                    1,
                    "In round {round}, unexpected headers found: {headers:?}",
                );
                assert_eq!(
                    headers[0].digest(),
                    all_blocks[round as usize][2].verified_block_header.digest()
                );
                assert_eq!(
                    shards.len(),
                    1,
                    "In round {round}, unexpected shards found: {shards:?}",
                );
            } else {
                // In round 6, A should know about D's blocks and send them all to B
                let d_headers_in_bundle: Vec<&VerifiedBlockHeader> = headers
                    .iter()
                    .filter(|h| h.author() == byzantine_index)
                    .collect();
                assert_eq!(d_headers_in_bundle.len(), final_round as usize - 1); // All 5 headers of D's blocks
                // Validator A sends to B all 5 shards of D's blocks and 1 header/shard of C's
                // block of round 5
                assert_eq!(
                    headers.len(),
                    final_round as usize,
                    "In round {round}, unexpected headers found: {headers:?}",
                );
                assert_eq!(shards.len(), final_round as usize);
            }
        }
    }

    /// Test that connection knowledge correctly takes additional parts for
    /// a bundle based on useful authorities info.
    #[tokio::test]
    async fn test_connection_knowledge_take_additional_parts() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let validators = 4;
        let our_index = AuthorityIndex::new_for_test(0);
        let to_whom_index = AuthorityIndex::new_for_test(1);
        let (context, key_pairs) = Context::new_for_test(validators);
        let protocol_keypairs = key_pairs.iter().map(|kp| kp.1.clone()).collect();
        let context = Arc::new(context);
        let final_round: Round = MAX_ROUND_GAP_FOR_USEFUL_PARTS / 2;
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let cordial_knowledge = CordialKnowledge::start(context.clone(), dag_state.clone());
        // Report useful info to connection knowledge corresponding to to_whom_index
        let connection_knowledge = cordial_knowledge.connection_knowledges[to_whom_index].clone();
        // Inject useful info
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
        // Build DAG with blocks from all validators up to final_round and add to
        // dag_state
        let mut dag_builder =
            DagBuilder::new(context.clone()).set_protocol_keypair(protocol_keypairs);
        dag_builder
            .layers(1..=final_round)
            .build()
            .persist_layers(dag_state.clone());
        sleep(std::time::Duration::from_millis(1)).await;
        // create dummy own verified block for next round to create a bundle
        let verified_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new(final_round + 1, our_index.value() as u8).build(),
        );
        let bundle = {
            connection_knowledge
                .write()
                .create_bundle(verified_block.clone())
        };
        let BlockBundle {
            verified_headers: headers,
            useful_shards_authors: useful_headers_authors_from_peer,
            useful_headers_authors: useful_shards_authors_from_peer,
            ..
        } = bundle;
        // Only headers and shards from authorities 2 and 3 should be included
        assert_eq!(headers.len(), 2);
        assert!(
            headers
                .iter()
                .all(|h| h.author() != our_index || h.author() == to_whom_index)
        );
        assert_eq!(
            useful_headers_authors_from_peer,
            BTreeSet::from([1, 3].map(AuthorityIndex::new_for_test))
        );
        assert_eq!(
            useful_shards_authors_from_peer,
            BTreeSet::from([1, 3].map(AuthorityIndex::new_for_test))
        );
        // Repeat the request, should get no headers this time
        // create dummy own verified block for next round to create a bundle
        let verified_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new(final_round + 1, our_index.value() as u8).build(),
        );
        let bundle = {
            connection_knowledge
                .write()
                .create_bundle(verified_block.clone())
        };
        let BlockBundle {
            verified_headers: headers,
            ..
        } = bundle;
        assert_eq!(headers.len(), 0);

        // Add more rounds to DAG
        let last_round = final_round + MAX_ROUND_GAP_FOR_USEFUL_PARTS;
        dag_builder
            .layers(final_round + 1..=last_round)
            .build()
            .persist_layers(dag_state.clone());
        sleep(std::time::Duration::from_millis(1)).await;

        // Make a request for a last round, should get no headers, no shards and no
        // useful authorities as the last useful rounds are beyond
        // MAX_ROUND_GAP_FOR_USEFUL_PARTS from last_round
        // create dummy own verified block for next round to create a bundle
        let verified_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new(last_round + 1, our_index.value() as u8).build(),
        );
        let bundle = { connection_knowledge.write().create_bundle(verified_block) };
        let BlockBundle {
            verified_headers: headers,
            serialized_shards: shards,
            useful_shards_authors: useful_headers_authors_from_peer,
            useful_headers_authors: useful_shards_authors_from_peer,
            ..
        } = bundle;
        assert!(headers.is_empty());
        assert!(shards.is_empty());
        assert!(useful_headers_authors_from_peer.is_empty());
        assert!(useful_shards_authors_from_peer.is_empty());
    }
}
