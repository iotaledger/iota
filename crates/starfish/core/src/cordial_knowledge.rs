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
    authority_set::AuthoritySet,
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
pub(crate) const MAX_ROUND_GAP_FOR_USEFUL_PARTS: Round = 40;
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

pub type Ancestors = Arc<[BlockRef]>;

/// One author whose headers peers recently supplied or referenced as missing.
/// Those peers are asked to keep including the author's headers.
#[derive(Clone, Default)]
struct MissingAuthor {
    /// Latest local own-block round when any peer supplied or referenced one of
    /// this author's headers.
    last_useful_round: Round,
    /// Peers that supplied or referenced one of this author's headers, stamped
    /// with the latest local own-block round when they did. Never the author.
    useful_peers: BTreeMap<AuthorityIndex, Round>,
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
    cordial_knowledge: Vec<BTreeMap<Round, AHashMap<BlockHeaderDigest, (Ancestors, AuthoritySet)>>>,
    /// Each Connection Knowledge corresponds to one peer. Upon reception of a
    /// message from CordialKnowledge, we propagate the respected
    /// information for each connection.
    connection_knowledges: Vec<Arc<RwLock<ConnectionKnowledge>>>,
    /// Authors whose headers peers recently supplied or referenced as missing,
    /// indexed by author.
    missing_authors: Vec<Option<MissingAuthor>>,
    /// Highest local own-block round seen. It timestamps useful-header reports,
    /// so peer block rounds cannot leave requests active for too long.
    latest_own_block_round: Round,
    /// Whether headers are currently requested from each peer. This ensures an
    /// empty set is sent once when the last request is removed.
    has_useful_headers_from_peer: Vec<bool>,
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
    /// Authors whose headers one peer supplied or referenced as missing.
    UsefulHeadersFromPeer {
        peer: AuthorityIndex,
        authors: BTreeSet<AuthorityIndex>,
    },
}

impl CordialKnowledgeMessage {
    /// Outputs the type of CordialKnowledgeMessage in a string slice format
    fn type_label(&self) -> &'static str {
        match self {
            CordialKnowledgeMessage::NewHeader { .. } => "New header",
            CordialKnowledgeMessage::NewShard(_) => "New shard",
            CordialKnowledgeMessage::UsefulShardsFromPeers(_) => "Useful authors for shards",
            CordialKnowledgeMessage::UsefulHeadersFromPeer { .. } => "Useful headers from peer",
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

        // Accepted headers remain useful input even when they arrive before the
        // block that references them and therefore prevent a missing ancestor.
        let useful_header_authors = additional_block_headers
            .iter()
            .map(|header| header.author())
            .chain(missing_ancestors.iter().map(|block_ref| block_ref.author))
            .collect::<BTreeSet<_>>();
        if !useful_header_authors.is_empty() {
            let cordial_knowledge_message = CordialKnowledgeMessage::UsefulHeadersFromPeer {
                peer,
                authors: useful_header_authors,
            };
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
                missing_authors: vec![None; num_authorities],
                latest_own_block_round: Round::MIN,
                has_useful_headers_from_peer: vec![false; num_authorities],
            },
            connection_knowledges,
            cordial_knowledge_sender,
            eviction_rounds_sender,
        )
    }

    /// A CordialKnowledge driven by calling its handlers directly instead of
    /// running the task.
    #[cfg(test)]
    fn new_for_test(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> (Self, Vec<Arc<RwLock<ConnectionKnowledge>>>) {
        let (cordial_knowledge, connection_knowledges, _sender, _eviction_sender) =
            Self::new(context, dag_state);
        (cordial_knowledge, connection_knowledges)
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

                    let own_block_round_before_batch = self.latest_own_block_round;
                    for msg in batch {
                        if let Some(vec_connection_knowledge_msgs) = self.process_message(msg) {
                            for (index, msgs) in
                                vec_connection_knowledge_msgs.into_iter().enumerate()
                            {
                                vec_connection_knowledge_msgs_batch[index].extend(msgs);
                            }
                        }
                    }

                    // Requested authors can be sent only with an own block, so
                    // refresh each peer's requests after processing a newer one.
                    if self.latest_own_block_round > own_block_round_before_batch {
                        self.append_useful_headers_from_peer_msgs(
                            &mut vec_connection_knowledge_msgs_batch,
                        );
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
            CordialKnowledgeMessage::UsefulHeadersFromPeer { peer, authors } => {
                self.handle_useful_headers_from_peer(peer, authors);
                None
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

    /// Record the authors whose headers one peer supplied or referenced as
    /// missing.
    fn handle_useful_headers_from_peer(
        &mut self,
        peer: AuthorityIndex,
        authors: BTreeSet<AuthorityIndex>,
    ) {
        let own_index = self.context.own_index;
        let latest_own_block_round = self.latest_own_block_round;
        for author in authors {
            if author == own_index {
                continue;
            }
            let state = self.missing_authors[author].get_or_insert_default();
            state.last_useful_round = latest_own_block_round;
            // A connection to the author never holds that author's own headers,
            // so it can never push them.
            if peer != author && peer != own_index {
                state.useful_peers.insert(peer, latest_own_block_round);
            }
        }
    }

    /// Refresh which authors each peer is asked to supply. Accepted headers and
    /// references to missing headers both keep a request active.
    fn append_useful_headers_from_peer_msgs(
        &mut self,
        vec_connection_knowledge_msgs_batch: &mut [Vec<ConnectionKnowledgeMessage>],
    ) {
        // The healthy steady state: nothing to ask and nothing to clear, and the
        // gauge was zeroed by the pass that emptied the last of it.
        if self.missing_authors.iter().all(Option::is_none)
            && !self.has_useful_headers_from_peer.iter().any(|asked| *asked)
        {
            return;
        }

        let own_index = self.context.own_index;
        let latest_own_block_round = self.latest_own_block_round;
        let is_stale = |round: Round| {
            latest_own_block_round.saturating_sub(round) > MAX_ROUND_GAP_FOR_USEFUL_PARTS
        };

        let mut missing_authors: i64 = 0;
        let mut useful_headers_from_peer: Vec<BTreeMap<AuthorityIndex, Round>> =
            vec![BTreeMap::new(); self.context.committee.size()];
        for (author_index, state) in self.missing_authors.iter_mut().enumerate() {
            if state
                .as_ref()
                .is_some_and(|missing| is_stale(missing.last_useful_round))
            {
                *state = None;
            }
            let Some(missing) = state else { continue };
            missing.useful_peers.retain(|_, round| !is_stale(*round));
            missing_authors += 1;
            let author = AuthorityIndex::from(author_index as u8);
            for peer in missing.useful_peers.keys() {
                useful_headers_from_peer[peer.value()].insert(author, latest_own_block_round);
            }
        }

        for (peer_index, authors) in useful_headers_from_peer.into_iter().enumerate() {
            if peer_index == own_index.value() {
                continue;
            }
            let non_empty = !authors.is_empty();
            if non_empty || self.has_useful_headers_from_peer[peer_index] {
                vec_connection_knowledge_msgs_batch[peer_index].push(
                    ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(authors),
                );
            }
            self.has_useful_headers_from_peer[peer_index] = non_empty;
        }

        self.context
            .metrics
            .node_metrics
            .cordial_knowledge_missing_authors
            .set(missing_authors);
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

        if block_author == own_index {
            self.latest_own_block_round = max(self.latest_own_block_round, block_round);
        }

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
        let who_knows_this_block = AuthoritySet::new_with(block_ref.author, self.context.own_index);
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

        // 3) The block_author now acknowledges previously known transactions,
        // using the provided transaction commitments.
        for (acknowledgment, &transactions_commitment) in header
            .acknowledgments()
            .iter()
            .zip(ack_transactions_commitments.iter())
        {
            let Some(transactions_commitment) = transactions_commitment else {
                continue;
            };
            let gen_tx_ref =
                GenericTransactionRef::TransactionRef(crate::transaction_ref::TransactionRef {
                    round: acknowledgment.round,
                    author: acknowledgment.author,
                    transactions_commitment,
                });

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
                        if who_knows_parent.insert(block_ref.author) {
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
    /// Replace the authors this peer is asked to push headers of. The cordial
    /// knowledge task is their only source.
    SetUsefulHeadersFromPeer(BTreeMap<AuthorityIndex, Round>),
    /// Update useful info about which authorities are useful to/from the peer.
    UsefulAuthors {
        useful_headers_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_to_peer: BTreeMap<AuthorityIndex, Round>,
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
            ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(authorities_with_round) => {
                self.set_useful_headers_from(authorities_with_round);
            }
            ConnectionKnowledgeMessage::UsefulAuthors {
                useful_headers_to_peer,
                useful_shards_to_peer,
                useful_shards_from_peer,
            } => {
                self.handle_useful_authors(
                    useful_headers_to_peer,
                    useful_shards_to_peer,
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
        useful_shards_from_peer: Vec<Option<Round>>,
    ) {
        // Update local state
        self.handle_useful_headers_to(useful_headers_to_peer);
        self.handle_useful_shards_to(useful_shards_to_peer);
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

    /// Replace the rounds of useful headers from peer, so an author the peer is
    /// no longer asked for stops at once instead of ageing out.
    fn set_useful_headers_from(&mut self, authorities_with_round: BTreeMap<AuthorityIndex, Round>) {
        self.last_useful_headers_from_peer_round.fill(None);
        for (authority, round) in authorities_with_round {
            self.last_useful_headers_from_peer_round[authority] = Some(round);
        }
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
        // The global knowledge updates for these ancestors may still be in flight,
        // so record them here to have them available for this bundle: each becomes
        // the header disseminated as an additional header for its slot. For an
        // equivocating author this overrides a header recorded for that slot
        // earlier, which is fine, as that one is still obtainable by reference.
        for ancestor_block_ref in block.ancestors() {
            self.set_referenced_header(*ancestor_block_ref);
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

    /// Handles adding a new header to the set of potentially unknown headers.
    /// Keeps at most one header per slot, since a bundle carries only one: the
    /// receiver drops additional headers of a slot as spam protection. An
    /// equivocating header a peer needs is obtained by reference instead.
    fn handle_new_header(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();

        let refs_at_slot = self.headers_not_known[authority].entry(round).or_default();
        if refs_at_slot.is_empty() {
            refs_at_slot.insert(block_ref);
        }
    }

    /// Records `block_ref` as the header to offer for its slot, replacing any
    /// header tracked there. Used for the ancestors of a block we are about to
    /// send: the peer needs them to accept it.
    fn set_referenced_header(&mut self, block_ref: BlockRef) {
        let refs_at_slot = self.headers_not_known[block_ref.author.value()]
            .entry(block_ref.round)
            .or_default();
        refs_at_slot.clear();
        refs_at_slot.insert(block_ref);
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

    fn cordial_knowledge_for_test(
        validators: usize,
    ) -> (CordialKnowledge, Vec<Arc<RwLock<ConnectionKnowledge>>>) {
        let (context, _key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        CordialKnowledge::new_for_test(context, dag_state)
    }

    /// Runs one recompute and returns the per-connection messages it prepared.
    fn recompute(cordial_knowledge: &mut CordialKnowledge) -> Vec<Vec<ConnectionKnowledgeMessage>> {
        let mut batch: Vec<Vec<ConnectionKnowledgeMessage>> =
            (0..cordial_knowledge.context.committee.size())
                .map(|_| Vec::new())
                .collect();
        cordial_knowledge.append_useful_headers_from_peer_msgs(&mut batch);
        batch
    }

    /// The authors a message tells one connection to ask its peer for.
    fn asked_authors(msgs: &[ConnectionKnowledgeMessage]) -> Option<BTreeSet<AuthorityIndex>> {
        msgs.iter().find_map(|msg| match msg {
            ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(set) => {
                Some(set.keys().copied().collect())
            }
            _ => None,
        })
    }

    fn report_missing(cordial_knowledge: &mut CordialKnowledge, peer: u8, missing_author: u8) {
        cordial_knowledge.handle_useful_headers_from_peer(
            AuthorityIndex::new_for_test(peer),
            BTreeSet::from([AuthorityIndex::new_for_test(missing_author)]),
        );
    }

    fn asks_nothing(connection_knowledge: &Arc<RwLock<ConnectionKnowledge>>) -> bool {
        connection_knowledge
            .read()
            .last_useful_headers_from_peer_round
            .iter()
            .all(|round| round.is_none())
    }

    /// Peers that referenced a missing block of an author are asked to push its
    /// headers, and only the cordial knowledge task writes that to a
    /// connection.
    #[tokio::test]
    async fn test_referencing_a_missing_block_asks_the_peer() {
        let (mut cordial_knowledge, connection_knowledges) = cordial_knowledge_for_test(6);
        cordial_knowledge.latest_own_block_round = 10;
        let author = AuthorityIndex::new_for_test(5);

        report_missing(&mut cordial_knowledge, 1, 5);
        report_missing(&mut cordial_knowledge, 4, 5);
        assert!(connection_knowledges.iter().all(asks_nothing));

        let batch = recompute(&mut cordial_knowledge);

        assert_eq!(asked_authors(&batch[1]), Some(BTreeSet::from([author])));
        assert_eq!(asked_authors(&batch[4]), Some(BTreeSet::from([author])));
        assert!(batch[2].is_empty());
        assert!(batch[3].is_empty());
        connection_knowledges[1]
            .write()
            .process_vec_messages(batch.into_iter().nth(1).unwrap());
        assert_eq!(
            connection_knowledges[1]
                .read()
                .last_useful_headers_from_peer_round[author],
            Some(10)
        );
    }

    /// An accepted header renews the request even when it prevents the
    /// referencing block from reporting a missing ancestor.
    #[tokio::test]
    async fn test_accepted_header_keeps_peer_asked() {
        let peer = AuthorityIndex::new_for_test(1);
        let author = AuthorityIndex::new_for_test(3);
        let (context, _key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (
            mut cordial_knowledge,
            connection_knowledges,
            cordial_knowledge_sender,
            _eviction_rounds_sender,
        ) = CordialKnowledge::new(context, dag_state);
        let handle = CordialKnowledgeHandle {
            cordial_knowledge_sender,
            connection_knowledges,
            cordial_knowledge_handle: Mutex::new(None),
        };

        cordial_knowledge.latest_own_block_round = 10;
        report_missing(
            &mut cordial_knowledge,
            peer.value() as u8,
            author.value() as u8,
        );
        recompute(&mut cordial_knowledge);

        cordial_knowledge.latest_own_block_round = 10 + MAX_ROUND_GAP_FOR_USEFUL_PARTS;
        let accepted = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(10 + MAX_ROUND_GAP_FOR_USEFUL_PARTS, author.value() as u8).build(),
        );
        handle
            .report_useful_authors(
                peer,
                &SerializedBlockBundleParts {
                    serialized_block: Bytes::new(),
                    serialized_headers: vec![],
                    serialized_shards: vec![],
                    useful_headers_authors_bitmask: AuthoritySet::default(),
                    useful_shards_authors_bitmask: AuthoritySet::default(),
                },
                &[accepted],
                &BTreeSet::new(),
                7,
            )
            .unwrap();

        assert!(asks_nothing(&handle.connection_knowledges[peer]));
        while let Ok(message) = cordial_knowledge.cordial_knowledge_receiver.try_recv() {
            let _ = cordial_knowledge.process_message(message);
        }

        cordial_knowledge.latest_own_block_round = 10 + MAX_ROUND_GAP_FOR_USEFUL_PARTS + 1;
        let batch = recompute(&mut cordial_knowledge);
        assert_eq!(asked_authors(&batch[peer]), Some(BTreeSet::from([author])));
    }

    /// An author cannot push its own headers, and the local node is never a
    /// peer, so neither is ever asked.
    #[tokio::test]
    async fn test_author_and_own_index_are_never_asked() {
        let (mut cordial_knowledge, _connection_knowledges) = cordial_knowledge_for_test(5);
        let own_index = cordial_knowledge.context.own_index;
        cordial_knowledge.latest_own_block_round = 10;
        let author = AuthorityIndex::new_for_test(3);

        // The author is the only peer that referenced its own missing block, and
        // our own blocks are reported missing too.
        report_missing(&mut cordial_knowledge, 3, 3);
        report_missing(&mut cordial_knowledge, 3, own_index.value() as u8);

        assert!(cordial_knowledge.missing_authors[own_index].is_none());
        let state = cordial_knowledge.missing_authors[author].as_ref().unwrap();
        assert!(!state.useful_peers.contains_key(&author));
        let batch = recompute(&mut cordial_knowledge);
        assert!(batch.iter().all(|msgs| msgs.is_empty()));
    }

    /// Once no peer supplies or references the author's headers for a while,
    /// every connection that was asking is cleared exactly once.
    #[tokio::test]
    async fn test_asking_stops_without_useful_header_input() {
        let (mut cordial_knowledge, connection_knowledges) = cordial_knowledge_for_test(6);
        cordial_knowledge.latest_own_block_round = 10;
        let author = AuthorityIndex::new_for_test(5);

        report_missing(&mut cordial_knowledge, 1, 5);
        report_missing(&mut cordial_knowledge, 2, 5);
        let batch = recompute(&mut cordial_knowledge);
        for (index, msgs) in batch.into_iter().enumerate() {
            connection_knowledges[index]
                .write()
                .process_vec_messages(msgs);
        }
        assert!(!asks_nothing(&connection_knowledges[1]));

        cordial_knowledge.latest_own_block_round = 10 + MAX_ROUND_GAP_FOR_USEFUL_PARTS + 1;
        let mut batch = recompute(&mut cordial_knowledge);

        assert!(cordial_knowledge.missing_authors[author].is_none());
        for index in [1, 2] {
            assert_eq!(asked_authors(&batch[index]), Some(BTreeSet::new()));
            let msgs = std::mem::take(&mut batch[index]);
            connection_knowledges[index]
                .write()
                .process_vec_messages(msgs);
            assert!(asks_nothing(&connection_knowledges[index]));
        }

        // Nothing left to clear, so a further recompute says nothing.
        let batch = recompute(&mut cordial_knowledge);
        assert!(batch.iter().all(|msgs| msgs.is_empty()));
    }

    /// A peer stops being asked once its last useful input is old, even while
    /// another peer keeps the author active.
    #[tokio::test]
    async fn test_asking_stops_when_one_peer_input_ages() {
        let (mut cordial_knowledge, _connection_knowledges) = cordial_knowledge_for_test(6);
        let author = AuthorityIndex::new_for_test(5);

        cordial_knowledge.latest_own_block_round = 10;
        report_missing(&mut cordial_knowledge, 1, 5);
        cordial_knowledge.latest_own_block_round = 30;
        report_missing(&mut cordial_knowledge, 2, 5);
        recompute(&mut cordial_knowledge);

        cordial_knowledge.latest_own_block_round = 10 + MAX_ROUND_GAP_FOR_USEFUL_PARTS + 1;
        let batch = recompute(&mut cordial_knowledge);

        assert_eq!(asked_authors(&batch[1]), Some(BTreeSet::new()));
        assert_eq!(asked_authors(&batch[2]), Some(BTreeSet::from([author])));
    }

    /// Rounds recorded for a missing author come from the local clock, not from
    /// the round of the peer's block that reported it.
    #[tokio::test]
    async fn test_rounds_are_stamped_from_the_local_clock() {
        let (mut cordial_knowledge, _connection_knowledges) = cordial_knowledge_for_test(5);
        cordial_knowledge.latest_own_block_round = 42;
        let author = AuthorityIndex::new_for_test(4);

        report_missing(&mut cordial_knowledge, 1, 4);

        let state = cordial_knowledge.missing_authors[author].as_ref().unwrap();
        assert_eq!(state.last_useful_round, 42);
        assert_eq!(state.useful_peers[&AuthorityIndex::new_for_test(1)], 42);
        let batch = recompute(&mut cordial_knowledge);
        match &batch[1][..] {
            [ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(set)] => {
                assert_eq!(set[&author], 42);
            }
            other => panic!("unexpected messages: {other:?}"),
        }
    }

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
        let store = Arc::new(MemStore::new());
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
        let result = parse_dag(dag_str, false);
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
            useful_shards_from_peer: vec![None, Some(GENESIS_ROUND), None, Some(GENESIS_ROUND)],
        };
        {
            let mut connection_knowledge = connection_knowledge.write();
            connection_knowledge.process_one_message(msg);
            connection_knowledge.process_one_message(
                ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(BTreeMap::from([
                    (AuthorityIndex::new_for_test(1), GENESIS_ROUND),
                    (AuthorityIndex::new_for_test(3), GENESIS_ROUND),
                ])),
            );
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
                    let gen_transaction_ref = GenericTransactionRef::TransactionRef(
                        verified_transactions.transaction_ref(),
                    );
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
                let gen_transaction_ref =
                    GenericTransactionRef::TransactionRef(verified_transactions.transaction_ref());
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

    /// When an author equivocates, the header offered for that slot is the one
    /// the block we are sending references, not whichever reached us first.
    #[tokio::test]
    async fn test_referenced_ancestor_wins_the_slot() {
        telemetry_subscribers::init_for_testing();

        let validators = 4;
        let our_index = AuthorityIndex::new_for_test(0);
        let to_whom_index = AuthorityIndex::new_for_test(1);
        let equivocating_index = AuthorityIndex::new_for_test(2);
        let (context, _key_pairs) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let cordial_knowledge = CordialKnowledge::start(context, dag_state.clone());
        let connection_knowledge = cordial_knowledge.connection_knowledges[to_whom_index].clone();

        // Only the equivocating author's headers are useful to this peer, so the
        // bundle content is unambiguous.
        connection_knowledge.write().process_one_message(
            ConnectionKnowledgeMessage::UsefulAuthors {
                useful_headers_to_peer: BTreeMap::from([(equivocating_index, GENESIS_ROUND)]),
                useful_shards_to_peer: BTreeMap::new(),
                useful_shards_from_peer: vec![None; validators],
            },
        );

        // Two headers for slot (equivocating author, round 1), distinguished by
        // timestamp so their digests differ. Both are accepted, as they would be
        // when one arrives by stream and the other by a requested fetch.
        let first_arrival = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(1, equivocating_index.value() as u8)
                .set_timestamp_ms(1000)
                .build(),
        );
        let referenced = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(1, equivocating_index.value() as u8)
                .set_timestamp_ms(2000)
                .build(),
        );
        assert_ne!(first_arrival.reference(), referenced.reference());
        for header in [&first_arrival, &referenced] {
            dag_state
                .write()
                .accept_block_header(header.clone(), DataSource::Test);
        }

        // The slot is filled by arrival order first.
        connection_knowledge
            .write()
            .process_one_message(ConnectionKnowledgeMessage::NewHeader {
                block_ref: first_arrival.reference(),
            });

        // Our own block references the other header of that slot, so the peer
        // needs that one to accept the block.
        let own_block = VerifiedBlock::new_for_test(
            TestBlockHeader::new(2, our_index.value() as u8)
                .set_ancestors(vec![referenced.reference()])
                .build(),
        );
        let BlockBundle {
            verified_headers, ..
        } = connection_knowledge.write().create_bundle(own_block);

        assert_eq!(
            verified_headers
                .iter()
                .map(|header| header.reference())
                .collect::<Vec<_>>(),
            vec![referenced.reference()],
            "the bundle must offer the referenced header, not the first-arrived one"
        );
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
        let store = Arc::new(MemStore::new());
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
            useful_shards_from_peer: vec![None, Some(GENESIS_ROUND), None, Some(GENESIS_ROUND)],
        };
        {
            let mut connection_knowledge = connection_knowledge.write();
            connection_knowledge.process_one_message(msg);
            connection_knowledge.process_one_message(
                ConnectionKnowledgeMessage::SetUsefulHeadersFromPeer(BTreeMap::from([
                    (AuthorityIndex::new_for_test(1), GENESIS_ROUND),
                    (AuthorityIndex::new_for_test(3), GENESIS_ROUND),
                ])),
            );
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
