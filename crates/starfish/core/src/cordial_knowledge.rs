use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

use ahash::{AHashMap, AHashSet};
use bytes::Bytes;
use iota_macros::fail_point_async;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{
    sync::{
        Mutex,
        mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel},
        oneshot,
    },
    task::JoinError,
};
use tracing::{debug, warn};

use crate::{
    BlockHeaderAPI, BlockRef, Round, VerifiedBlockHeader,
    block_header::{BlockHeaderDigest, GENESIS_ROUND},
    context::Context,
    dag_state::DagState,
    network::SerializedBlockBundleParts,
};

const MAX_ROUND_GAP_FOR_USEFUL_SHARDS: Round = 5;
const MAX_ROUND_GAP_FOR_USEFUL_HEADERS: Round = 50;

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

pub(crate) struct CordialKnowledge {
    context: Arc<Context>,
    /// Receives high-level updates from DAG state (new headers, new own shards,
    /// evictions)
    cordial_knowledge_receiver: UnboundedReceiver<CordialKnowledgeMessage>,
    last_useful_shards_from_peer_round: Vec<Round>,
    /// Keeps track of the most recent DAG cordial
    /// knowledge (who knows which blocks) for each authority. This is a helper
    /// structure that is used primarily for traversing the recent DAG. This
    /// struct is evicted after flushing the dag state to storage and is not
    /// persisted. To access the cordial knowledge of a given block_ref, one
    /// shall retrieve it from `cordial_knowledge[block_ref.
    /// author][(block_ref.round, block_ref.digest)]`. The value is a tuple
    /// of (parents, who knows the block header).
    cordial_knowledge: Vec<
        BTreeMap<
            Round,
            AHashMap<BlockHeaderDigest, (Ancestors, SubsetAuthorities)>,
        >,
    >,
    /// Per-connection message channels
    connections: Vec<Sender<Vec<ConnectionKnowledgeMessage>>>,
}

pub struct CordialKnowledgeHandle {
    cordial_knowledge_sender: UnboundedSender<CordialKnowledgeMessage>,
    connection_knowledge_senders: Vec<Sender<Vec<ConnectionKnowledgeMessage>>>,
    connection_handles: Mutex<Vec<Option<tokio::task::JoinHandle<()>>>>,
    join_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl CordialKnowledgeHandle {
    /// Get a sender to send messages to the CordialKnowledge task.
    pub fn cordial_knowledge_sender(&self) -> UnboundedSender<CordialKnowledgeMessage> {
        self.cordial_knowledge_sender.clone()
    }
    /// Get a sender to send messages to a specific ConnectionKnowledge task.
    pub fn connection_knowledge_senders(&self) -> Vec<Sender<Vec<ConnectionKnowledgeMessage>>> {
        self.connection_knowledge_senders.clone()
    }
    /// Gracefully stop the CordialKnowledge background task and all connection
    /// tasks.
    pub async fn stop(&self) -> Result<(), JoinError> {
        // Stop main CordialKnowledge loop
        let mut guard = self.join_handle.lock().await;

        if let Some(main_handle) = guard.take() {
            main_handle.abort();
            match main_handle.await {
                Ok(_) => (),
                Err(e) if e.is_cancelled() => (),
                Err(e) => return Err(e),
            }
        }

        // --- Stop all per-connection tasks ---
        let mut conn_guard = self.connection_handles.lock().await;
        for handle_opt in conn_guard.iter_mut() {
            if let Some(handle) = handle_opt.take() {
                handle.abort();
                match handle.await {
                    Ok(_) => (),
                    Err(e) if e.is_cancelled() => (),
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }
}

impl CordialKnowledge {
    pub fn new(
        context: Arc<Context>,
    ) -> (
        Self,
        UnboundedSender<CordialKnowledgeMessage>,
        Vec<Receiver<Vec<ConnectionKnowledgeMessage>>>,
    ) {
        let num_authorities = context.committee.size();

        // Main unbounded channel for high-level DAG updates
        let (cordial_knowledge_sender, cordial_knowledge_receiver): (
            UnboundedSender<CordialKnowledgeMessage>,
            UnboundedReceiver<CordialKnowledgeMessage>,
        ) = unbounded_channel();

        // Bounded per-connection channels for controlled flow
        let mut connections = Vec::new();
        let mut receivers = Vec::new();

        for _ in 0..num_authorities {
            let (connection_sender, connection_receiver): (
                Sender<Vec<ConnectionKnowledgeMessage>>,
                Receiver<Vec<ConnectionKnowledgeMessage>>,
            ) = channel(512);

            connections.push(connection_sender);
            receivers.push(connection_receiver);
        }

        (
            Self {
                context,
                connections,
                cordial_knowledge_receiver,
                cordial_knowledge: vec![BTreeMap::new(); num_authorities],
                last_useful_shards_from_peer_round: vec![GENESIS_ROUND; num_authorities],
            },
            cordial_knowledge_sender,
            receivers,
        )
    }

    /// Start the CordialKnowledge task and return a handle to it.
    pub fn start(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Arc<CordialKnowledgeHandle> {
        // Build main CordialKnowledge and associated channels
        let (cordial_knowledge, sender, receivers) = CordialKnowledge::new(context.clone());
        let num_authorities = context.committee.size();

        let connection_knowledge_sender = cordial_knowledge.connections.clone();

        // Spawn one ConnectionKnowledge task per authority
        let mut connection_handles = Vec::with_capacity(num_authorities);

        for (authority_index, receiver) in receivers.into_iter().enumerate() {
            let connection_knowledge = ConnectionKnowledge::new(
                context.clone(),
                dag_state.clone(),
                authority_index,
                receiver,
            );

            // Spawn async run() for each peer connection
            let task_handle = tokio::spawn(async move {
                connection_knowledge.run().await;
            });

            connection_handles.push(Some(task_handle));
        }

        // Spawn the main CordialKnowledge loop
        let join_handle = tokio::spawn(async move {
            cordial_knowledge.run().await;
        });

        dag_state
            .write()
            .set_cordial_knowledge_sender(sender.clone());

        // Return handle with all pieces assembled
        Arc::new(CordialKnowledgeHandle {
            cordial_knowledge_sender: sender,
            connection_knowledge_senders: connection_knowledge_sender,
            connection_handles: Mutex::new(connection_handles),
            join_handle: Mutex::new(Some(join_handle)),
        })
    }

    /// Main async loop: receives high-level updates (headers, shards,
    /// evictions) from DAG state and updates global knowledge + notifies
    /// per-connection tasks.
    pub async fn run(mut self) {
        fail_point_async!("consensus-rpc-response");

        debug!("Cordial Knowledge main loop started");

        loop {
            match self.cordial_knowledge_receiver.recv().await {
                Some(msg) => {
                    // Handle the first received message
                    self.process_message(msg).await;

                    // Report the buffer size after processing the first message
                    let buffer_size = self.cordial_knowledge_receiver.len() + 1;
                    self.context
                        .metrics
                        .node_metrics
                        .cordial_knowledge_buffer_size
                        .set(buffer_size as i64);

                    // Drain the rest of the buffer without awaiting
                    while let Ok(msg) = self.cordial_knowledge_receiver.try_recv() {
                        self.process_message(msg).await;
                    }

                    // Yield to give other tasks a chance before looping again
                    tokio::task::yield_now().await;
                }
                None => {
                    debug!("Cordial Knowledge channel closed; exiting loop");
                    break;
                }
            }
        }

        debug!("Cordial Knowledge main loop finished");
    }

    async fn process_message(&mut self, cordial_knowledge_message: CordialKnowledgeMessage) {
        debug!(
            "Processing Cordial Message: {:?}",
            cordial_knowledge_message
        );
        match cordial_knowledge_message {
            CordialKnowledgeMessage::NewHeader(header) => {
                self.handle_new_header(header).await;
            }
            CordialKnowledgeMessage::NewShard(block_ref) => {
                self.handle_new_shard(block_ref).await;
            }
            CordialKnowledgeMessage::EvictBelow(round) => {
                self.handle_evict_below(round).await;
            }
            CordialKnowledgeMessage::UsefulShardsFromPeers(useful_shards_from_peer) => {
                self.handle_useful_shards_from(useful_shards_from_peer)
                    .await;
            }
        }
        self.context
            .metrics
            .node_metrics
            .cordial_knowledge_processed_messages
            .inc();
    }

    async fn handle_useful_shards_from(
        &mut self,
        useful_shards_from_peer: BTreeMap<AuthorityIndex, Round>,
    ) {
        for (authority, round) in useful_shards_from_peer {
            if round > self.last_useful_shards_from_peer_round[authority] {
                self.last_useful_shards_from_peer_round[authority] = round;
            }
        }
        self.disseminate_useful_info_to_connection_tasks().await;
    }

    async fn disseminate_useful_info_to_connection_tasks(&mut self) {
        for connection_sender in &self.connections {
            let msg = ConnectionKnowledgeMessage::UsefulInfo {
                useful_shards_from_peers: self.last_useful_shards_from_peer_round.clone(),
                useful_headers_from_peer: BTreeMap::new(),
                useful_headers_to_peer: BTreeMap::new(),
                useful_shards_to_peer: BTreeMap::new(),
            };
            if let Err(e) = connection_sender.send(vec![msg]).await {
                warn!("Failed to send useful info to connection task: {}", e);
            }
        }
    }

    /// Called when a new verified block header is received.
    async fn handle_new_header(&mut self, header: VerifiedBlockHeader) {
        self.update_cordial_knowledge(&header).await;
    }

    /// Called when a new *own shard* (created locally) is added to dag state.
    async fn handle_new_shard(&mut self, block_ref: BlockRef) {
        for tx in &self.connections {
            let msg = ConnectionKnowledgeMessage::NewShard { block_ref };
            let _ = tx.send(vec![msg]).await;
        }
    }

    /// Called when older rounds should be pruned globally.
    async fn handle_evict_below(&mut self, rounds: Vec<Round>) {
        // Evict locally
        for (index, btree_map) in &mut self.cordial_knowledge.iter_mut().enumerate() {
            let split_round = rounds[index];
            *btree_map = btree_map.split_off(&split_round);
            self.context
                .metrics
                .node_metrics
                .cordial_knowledge_rounds
                .with_label_values(&[&index.to_string()])
                .set(btree_map.len() as i64);
        }
        let largest_round = self.cordial_knowledge[self.context.own_index].keys().max().cloned()
            .unwrap_or(GENESIS_ROUND);
        let useful_shards_from_peer_count = self
            .last_useful_shards_from_peer_round
            .iter()
            .filter(|&&r| r + MAX_ROUND_GAP_FOR_USEFUL_HEADERS >= largest_round)
            .count();
        self.context.metrics.node_metrics.cordial_knowledge_useful_shards.set(useful_shards_from_peer_count as i64);

        // Notify per-connection tasks about eviction
        self.notify_connection_tasks_for_eviction(rounds).await;
    }
    #[inline]
    async fn notify_connection_tasks_for_eviction(&self, rounds: Vec<Round>) {
        for tx in &self.connections {
            let msg = ConnectionKnowledgeMessage::EvictBelow(rounds.clone());
            let _ = tx.send(vec![msg]).await;
        }
    }

    /// Update cordial knowledge for exactly one new header.
    /// Assumes all parents are already stored somewhere in
    /// `recent_dag_cordial_knowledge`
    /// - Only grows the author's deque if needed.
    /// - For other authorities' "unknown headers" deques, we add the block only
    ///   if the round bucket already exists (no growth).
    async fn update_cordial_knowledge(&mut self, header: &VerifiedBlockHeader) {
        let block_ref = header.reference();
        let block_author = block_ref.author.value();
        let block_round = block_ref.round;
        let block_digest = block_ref.digest;
        let own_index = self.context.own_index.value();

        // Pre-allocate message buffers
        let mut vec_knowledge_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            (0..self.context.committee.size()).map(|_| Vec::new()).collect();

        // === 1) Ensure we have a round map for this author and insert the block ===
        let btree_map = &mut self.cordial_knowledge[block_author];
        let round_map = btree_map.entry(block_round).or_insert_with(AHashMap::new);

        // Already recorded — nothing else to do.
        if round_map.contains_key(&block_digest) {
            return;
        }

        // Insert block into cordial knowledge
        let ancestors: Ancestors = Arc::from(header.ancestors());
        let who_knows_this_block = SubsetAuthorities::new_with(block_author, own_index);
        round_map.insert(block_digest, (ancestors.clone(), who_knows_this_block));

        // === 2) Notify all *other* authorities (except self and block_author) ===
        for (authority, msgs) in vec_knowledge_msgs.iter_mut().enumerate() {
            if authority == block_author || authority == own_index {
                continue;
            }
            msgs.push(ConnectionKnowledgeMessage::NewHeader { block_ref });
        }

        // === 3) The block_author now acknowledges previously known transactions ===
        for acknowledgment in header.acknowledgments() {
            vec_knowledge_msgs[block_author].push(ConnectionKnowledgeMessage::RemoveShard {
                block_ref: *acknowledgment,
            });
        }

        // === 4) Traversing back and marking the causal past as known by block_author ===
        let mut stack = vec![block_ref];
        while let Some(current_ref) = stack.pop() {
            let current_author = current_ref.author.value();
            let current_round = current_ref.round;
            let current_digest = current_ref.digest;

            // ---- Get parents of current block ----
            let parents_buf: Vec<BlockRef> = {
                let author_map = &self.cordial_knowledge[current_author];
                let Some(current_round_map) = author_map.get(&current_round) else {
                    continue;
                };
                let Some((parents, _)) = current_round_map.get(&current_digest) else {
                    continue;
                };
                parents.iter().copied().collect()
            };

            // Traverse parents
            for parent_ref in parents_buf {
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
                                    block_ref: parent_ref,
                                },
                            );
                            stack.push(parent_ref);
                        }
                    }
                }
            }
        }


        // === 5) Send all accumulated knowledge messages ===
        self.send_connection_knowledge_messages(vec_knowledge_msgs).await;
    }


    async fn send_connection_knowledge_messages(&self, msgs: Vec<Vec<ConnectionKnowledgeMessage>>) {
        for (index, msg) in msgs.into_iter().enumerate() {
            if !msg.is_empty() {
                let _ = self.connections[index].send(msg).await;
            }
        }
    }

    pub async fn report_useful_info(
        connection_knowledge_sender: &Sender<Vec<ConnectionKnowledgeMessage>>,
        cordial_knowledge_sender: &UnboundedSender<CordialKnowledgeMessage>,
        serialized_block_bundle_parts: &SerializedBlockBundleParts,
        additional_block_headers: &[VerifiedBlockHeader],
        missing_ancestors: &BTreeSet<BlockRef>,
        block_round: Round,
    ) {
        let useful_headers_authors = additional_block_headers
            .iter()
            .map(|block_header| block_header.author())
            .chain(missing_ancestors.iter().map(|block_ref| block_ref.author))
            .collect::<BTreeSet<_>>();

        let mut useful_shard_authors: BTreeMap<AuthorityIndex, Round> = BTreeMap::new();
        for header in additional_block_headers {
            let author = header.author();
            let round = header.round();

            // Insert or update if newer round
            useful_shard_authors
                .entry(author)
                .and_modify(|was_round| *was_round = (*was_round).max(round))
                .or_insert(round);
        }

        // Extract authorities this peer finds useful for cordial dissemination
        let useful_headers_to_peer = serialized_block_bundle_parts.useful_headers_authors();
        let useful_headers_to_peer = useful_headers_to_peer
            .iter()
            .map(|&a| (a, block_round))
            .collect::<BTreeMap<_, _>>();
        let useful_shards_to_peer = serialized_block_bundle_parts.useful_shards_authors();
        let useful_shards_to_peer = useful_shards_to_peer
            .iter()
            .map(|&a| (a, block_round))
            .collect::<BTreeMap<_, _>>();
        let useful_headers_from_peer =  useful_headers_authors
            .into_iter()
            .map(|a| (a, block_round))
            .collect();

        // Notify connection knowledge about useful headers and shards to/from this peer
        let connection_knowledge_message = ConnectionKnowledgeMessage::UsefulInfo {
            useful_headers_to_peer,
            useful_shards_to_peer,
            useful_headers_from_peer,
            useful_shards_from_peers: vec![],
        };
        let _ = connection_knowledge_sender
            .send(vec![connection_knowledge_message])
            .await;

        // Notify global cordial knowledge about useful shards from this peer
        let cordial_knowledge_message =
            CordialKnowledgeMessage::UsefulShardsFromPeers(useful_shard_authors);
        let _ = cordial_knowledge_sender.send(cordial_knowledge_message);
    }
}

#[derive(Debug)]
pub enum ConnectionKnowledgeMessage {
    /// A new block header was added globally.
    NewHeader { block_ref: BlockRef },
    /// Remove a block header from the "unknown" set .
    RemoveHeader { block_ref: BlockRef },
    /// A new shard was added globally.
    NewShard { block_ref: BlockRef },
    /// Remove a header from the "unknown" set.
    RemoveShard { block_ref: BlockRef },
    /// Update useful info about which authorities are useful to/from the peer.
    UsefulInfo {
        useful_headers_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_headers_from_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_from_peers: Vec<Round>,
    },
    /// Take useful headers and shards for authorities, up to the given round
    /// (exclusive).
    TakeAdditionalPartForBundle {
        round_upper_bound_exclusive: Round,
        respond_to: oneshot::Sender<AdditionalPartsForBundle>,
    },

    /// Global eviction (prune below round)
    EvictBelow(Vec<Round>),
}

#[derive(Debug)]
pub enum CordialKnowledgeMessage {
    /// A new verified block header to integrate into cordial knowledge.
    NewHeader(VerifiedBlockHeader),
    /// A new verified own shard to integrate into cordial knowledge.
    NewShard(BlockRef),
    /// Evict old rounds globally.
    EvictBelow(Vec<Round>),
    /// Update internal state about shards from which authorities are useful for
    /// us
    UsefulShardsFromPeers(BTreeMap<AuthorityIndex, Round>),
}

/// Manages the knowledge state for a single connection to a peer.
/// Receives updates from the global cordial knowledge
pub struct ConnectionKnowledge {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    peer_index: usize,
    headers_not_known: Vec<BTreeMap<Round, AHashSet<BlockRef>>>,
    shards_not_known: Vec<BTreeMap<Round, AHashSet<BlockRef>>>,
    last_useful_shards_to_peer_round: Vec<Round>,
    last_useful_headers_to_peer_round: Vec<Round>,
    last_useful_shards_from_peer_round: Vec<Round>,
    last_useful_headers_from_peer_round: Vec<Round>,
    /// Receives updates from the global cordial knowledge
    receiver: Receiver<Vec<ConnectionKnowledgeMessage>>,
}

#[derive(Debug)]
pub(crate) struct AdditionalPartsForBundle {
    pub headers: Vec<VerifiedBlockHeader>,
    pub shards: Vec<Bytes>,
    pub useful_headers_authors_from_peer: BTreeSet<AuthorityIndex>,
    pub useful_shards_authors_from_peer: BTreeSet<AuthorityIndex>,
}

impl ConnectionKnowledge {
    pub fn new(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        peer_index: usize,
        receiver: Receiver<Vec<ConnectionKnowledgeMessage>>,
    ) -> Self {
        let num_authorities = context.committee.size();
        let headers_not_known = vec![BTreeMap::new(); num_authorities];
        let shards_not_known = vec![BTreeMap::new(); num_authorities];
        Self {
            dag_state,
            last_useful_headers_to_peer_round: vec![GENESIS_ROUND; num_authorities],
            last_useful_shards_to_peer_round: vec![GENESIS_ROUND; num_authorities],
            last_useful_headers_from_peer_round: vec![GENESIS_ROUND; num_authorities],
            last_useful_shards_from_peer_round: vec![GENESIS_ROUND; num_authorities],
            context,
            peer_index,
            headers_not_known,
            shards_not_known,
            receiver,
        }
    }
    fn take_useful_refs_round(
        maps: &mut Vec<BTreeMap<Round, AHashSet<BlockRef>>>,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
        max_take: usize,
    ) -> Vec<BlockRef> {
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
                if let Some(blocks) = map.get(&current_round) {
                    for &block_ref in blocks {
                        taken.push(block_ref);
                        if taken.len() >= max_take {
                            break 'outer;
                        }
                    }
                }
            }
            current_round = current_round.saturating_add(1);
        }

        // Remove the taken blocks from the corresponding authorities
        for block_ref in &taken {
            let authority = block_ref.author.value();
            if let Some(set) = maps[authority].get_mut(&block_ref.round) {
                set.remove(block_ref);
                // Optional cleanup: remove empty rounds to keep map small
                if set.is_empty() {
                    maps[authority].remove(&block_ref.round);
                }
            }
        }

        taken
    }

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
        )
    }

    fn take_useful_shard_block_refs_round(
        &mut self,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
    ) -> Vec<BlockRef> {
        let max_take = self.context.parameters.max_shards_per_bundle;
        Self::take_useful_refs_round(
            &mut self.shards_not_known,
            round_upper_bound_exclusive,
            useful_authorities,
            max_take,
        )
    }



    fn evict_below(&mut self, rounds_exclusive: Vec<Round>) {
        for (index, map) in self.headers_not_known.iter_mut().enumerate() {
            let threshold_round = rounds_exclusive[index];
            // Keep only entries >= threshold
            *map = map.split_off(&threshold_round);
        }

        for (index, map) in self.shards_not_known.iter_mut().enumerate() {
            let threshold_round = rounds_exclusive[index];
            *map = map.split_off(&threshold_round);
        }
    }


    /// Async task loop — just receives messages and dispatches to processing
    /// logic.
    pub async fn run(mut self) {
        debug!("Connection Knowledge started for peer {}", self.peer_index);

        while let Some(knowledge_msgs) = self.receiver.recv().await {
            debug!("Received knowledge message: {:?}", knowledge_msgs);
            for knowledge_msg in knowledge_msgs {
                self.process_message(knowledge_msg).await;
            }
            tokio::task::yield_now().await;
        }

        debug!(
            "Connection Knowledge loop ended for peer {}",
            self.peer_index
        );
    }

    /// Processes a batch of knowledge updates synchronously (non-async).
    /// This isolates all mutation logic so it can be tested without async
    /// context.
    async fn process_message(&mut self, message: ConnectionKnowledgeMessage) {
        match message {
            ConnectionKnowledgeMessage::NewHeader { block_ref } => {
                self.handle_new_header(block_ref);
            }
            ConnectionKnowledgeMessage::RemoveHeader { block_ref } => {
                self.handle_remove_header(block_ref);
            }
            ConnectionKnowledgeMessage::NewShard { block_ref } => {
                self.handle_new_shard(block_ref);
            }
            ConnectionKnowledgeMessage::RemoveShard { block_ref } => {
                self.handle_remove_shard(block_ref);
            }
            ConnectionKnowledgeMessage::EvictBelow(rounds) => {
                self.evict_below(rounds);
            }
            ConnectionKnowledgeMessage::UsefulInfo {
                useful_headers_to_peer,
                useful_shards_to_peer,
                useful_headers_from_peer,
                useful_shards_from_peers: useful_shards_from_peer,
            } => {
                self.handle_useful_info(
                    useful_headers_to_peer,
                    useful_shards_to_peer,
                    useful_headers_from_peer,
                    useful_shards_from_peer,
                );
            }
            ConnectionKnowledgeMessage::TakeAdditionalPartForBundle {
                round_upper_bound_exclusive,
                respond_to,
            } => {
                self.handle_take_additional_parts_for_bundle(
                    round_upper_bound_exclusive,
                    respond_to,
                )
                .await;
            }
        }
    }
    fn handle_useful_info(
        &mut self,
        useful_headers_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_to_peer: BTreeMap<AuthorityIndex, Round>,
        useful_headers_from_peer: BTreeMap<AuthorityIndex, Round>,
        useful_shards_from_peer: Vec<Round>,
    ) {
        // Update local state
        self.handle_useful_headers_to(useful_headers_to_peer);
        self.handle_useful_shards_to(useful_shards_to_peer);
        self.handle_useful_headers_from(useful_headers_from_peer);
        self.handle_useful_shards_from(useful_shards_from_peer);
    }

    fn handle_useful_shards_from(&mut self, useful_shards_from_peer_round: Vec<Round>) {
        self.last_useful_shards_from_peer_round = useful_shards_from_peer_round;
    }

    fn handle_useful_headers_from(
        &mut self,
        authorities_with_round: BTreeMap<AuthorityIndex, Round>,
    ) {
        for (authority, round) in authorities_with_round {
            if round > self.last_useful_headers_from_peer_round[authority] {
                self.last_useful_headers_from_peer_round[authority] = round;
            }
        }
    }

    fn handle_useful_shards_to(&mut self, authorities_with_round: BTreeMap<AuthorityIndex, Round>) {
        for (authority, round) in authorities_with_round {
            if round > self.last_useful_shards_to_peer_round[authority] {
                self.last_useful_shards_to_peer_round[authority] = round;
            }
        }
    }

    fn handle_useful_headers_to(
        &mut self,
        authorities_with_round: BTreeMap<AuthorityIndex, Round>,
    ) {
        for (authority, round) in authorities_with_round {
            if round > self.last_useful_headers_to_peer_round[authority] {
                self.last_useful_headers_to_peer_round[authority] = round;
            }
        }
    }

    async fn handle_take_additional_parts_for_bundle(
        &mut self,
        round_upper_bound_exclusive: Round,
        respond_to: oneshot::Sender<AdditionalPartsForBundle>,
    ) {
        // 1. Own headers and shards for round up to round_upper_bound_exclusive should
        //    be marked as known
        let own_index = self.context.own_index;
        let mut rounds = vec![Round::MIN; self.context.committee.size()];
        rounds[own_index] = round_upper_bound_exclusive + 1; // We are supposed to send own block of this round in a bundle when calling this function with this parameter

        self.evict_below(rounds);
        // 2. Identify useful authorities for headers and take the corresponding headers
        //    from the DAG state
        let useful_headers_authors_to_peer: Vec<usize> = self
            .last_useful_headers_to_peer_round
            .iter()
            .enumerate()
            .filter_map(|(i, &r)| {
                if r + MAX_ROUND_GAP_FOR_USEFUL_HEADERS >= round_upper_bound_exclusive {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        debug!(
            "Useful header authors: {:?}",
            useful_headers_authors_to_peer
        );

        let useful_headers_block_refs_to_peer = self.take_useful_header_block_refs_round(
            round_upper_bound_exclusive,
            &useful_headers_authors_to_peer,
        );

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
            .filter_map(|(i, &r)| {
                if r + MAX_ROUND_GAP_FOR_USEFUL_SHARDS >= round_upper_bound_exclusive {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        let useful_shards_block_refs_to_peer = self.take_useful_shard_block_refs_round(
            round_upper_bound_exclusive,
            &useful_shards_authors_to_peer,
        );
        let useful_shards: Vec<Bytes> = {
            let dag_state_read = self.dag_state.read();
            dag_state_read
                .get_cached_shards(&useful_shards_block_refs_to_peer)
                .into_iter()
                .flatten() // Filter out None values
                .collect()
        };
        // 4. Get useful header authors from peer
        let useful_headers_authors_from_peer = self
            .last_useful_headers_from_peer_round
            .iter()
            .enumerate()
            .filter_map(|(i, &r)| {
                if r + MAX_ROUND_GAP_FOR_USEFUL_HEADERS >= round_upper_bound_exclusive {
                    Some(AuthorityIndex::from(i as u8))
                } else {
                    None
                }
            })
            .collect::<BTreeSet<AuthorityIndex>>();
        // 5. Get useful shard authors from peer
        let useful_shards_authors_from_peer = self
            .last_useful_shards_from_peer_round
            .iter()
            .enumerate()
            .filter_map(|(i, &r)| {
                if r + MAX_ROUND_GAP_FOR_USEFUL_SHARDS >= round_upper_bound_exclusive {
                    Some(AuthorityIndex::from(i as u8))
                } else {
                    None
                }
            })
            .collect::<BTreeSet<AuthorityIndex>>();

        // 6. Build a response message and send it back
        let message = AdditionalPartsForBundle {
            headers: useful_headers_to_peer,
            shards: useful_shards,
            useful_headers_authors_from_peer,
            useful_shards_authors_from_peer,
        };

        respond_to.send(message).ok();
    }

    /// Handles adding a new block to the unknown set.
    fn handle_new_header(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();

        // Insert the block into the set for that (authority, round)
        self.headers_not_known[authority]
            .entry(round)
            .or_default()
            .insert(block_ref);
    }

    /// Handles adding a new shard to the unknown set.
    fn handle_new_shard(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();

        self.shards_not_known[authority]
            .entry(round)
            .or_default()
            .insert(block_ref);
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
    fn handle_remove_shard(&mut self, block_ref: BlockRef) {
        let authority = block_ref.author.value();
        let round = block_ref.round;

        if let Some(set) = self.shards_not_known[authority].get_mut(&round) {
            set.remove(&block_ref);
            if set.is_empty() {
                self.shards_not_known[authority].remove(&round);
            }
        }
    }

}
