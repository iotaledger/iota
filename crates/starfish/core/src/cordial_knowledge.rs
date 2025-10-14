use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::sync::Arc;
use ahash::{AHashMap, AHashSet};
use parking_lot::RwLock;
use tokio::sync::mpsc::{channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::task::{yield_now, JoinError};
use tracing::{debug, warn};
use starfish_config::AuthorityIndex;
use crate::block_header::BlockHeaderDigest;
use crate::{BlockHeaderAPI, BlockRef, Round, VerifiedBlockHeader};
use crate::context::Context;

/// Represents a subset of authorities using a bitmask.
/// Each bit in the `low` and `high` fields corresponds to an authority index.
/// The maximum number of authorities supported is 256 (0-255).
#[derive(Clone, Copy, Debug)]
pub(crate) struct SubsetAuthorities {
    low: u128,
    high: u128,
}

pub type Ancestors = Arc<[BlockRef]>;
impl SubsetAuthorities  {

    #[inline]
    pub fn new_with(author: usize, own: usize) -> Self {
        let mut s = Self { low: 0, high: 0 };
        s.insert(author);
        s.insert(own);
        s
    }

    /// Insert an authority into the subset. Returns true if the authority was not already present.
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
    /// Receives high-level updates from DAG state (new headers, new own shards, evictions)
    cordial_knowledge_receiver: UnboundedReceiver<CordialKnowledgeMessage>,
    /// Keeps track of the most recent DAG cordial
    /// knowledge (who knows which blocks) for each authority. This is a helper
    /// structure that is used primarily for traversing the recent DAG. This
    /// struct is evicted after flushing the dag state to storage and is not
    /// persisted. To access the cordial knowledge of a given block_ref, one
    /// shall retrieve it from `cordial_knowledge[block_ref.
    /// author][(block_ref.round, block_ref.digest)]`. The value is a tuple
    /// of (parents, who knows the block header).
    cordial_knowledge:
        Vec<VecDeque<(Round, AHashMap<BlockHeaderDigest, (Ancestors, SubsetAuthorities)>)>>,
    /// Per-connection message channels
    connections: Vec<Sender<Vec<ConnectionKnowledgeMessage>>>,
}

pub struct CordialKnowledgeHandle {
    cordial_knowledge_sender: UnboundedSender<CordialKnowledgeMessage>,
    connection_knowledge_senders: Vec<Sender<Vec<ConnectionKnowledgeMessage>>>,
    connection_handles: Mutex<Vec<Option<tokio::task::JoinHandle<()>>>>,
    join_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl CordialKnowledgeHandle{
    /// Get a sender to send messages to the CordialKnowledge task.
    pub fn cordial_knowledge_sender(&self) -> UnboundedSender<CordialKnowledgeMessage> {
        self.cordial_knowledge_sender.clone()
    }
    /// Get a sender to send messages to a specific ConnectionKnowledge task.
    pub fn connection_knowledge_senders(&self) -> Vec<Sender<Vec<ConnectionKnowledgeMessage>>> {
        self.connection_knowledge_senders.clone()
    }
    /// Gracefully stop the CordialKnowledge background task and all connection tasks.
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
                cordial_knowledge: vec![VecDeque::new(); num_authorities],
            },
            cordial_knowledge_sender,
            receivers,
        )
    }

    /// Start the CordialKnowledge task and return a handle to it.
    pub fn start(
        context: Arc<Context>,
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

        // Return handle with all pieces assembled
        Arc::new(CordialKnowledgeHandle {
            cordial_knowledge_sender: sender,
            connection_knowledge_senders: connection_knowledge_sender,
            connection_handles: Mutex::new(connection_handles),
            join_handle: Mutex::new(Some(join_handle)),
        })
    }

    /// Main async loop: receives high-level updates (headers, shards, evictions) from DAG state
    /// and updates global knowledge + notifies per-connection tasks.
    pub async fn run(
        mut self,
    ) {
        debug!("Cordial Knowledge main loop started");

        while let Some(message) = self.cordial_knowledge_receiver.recv().await {
            match message {
                CordialKnowledgeMessage::NewHeader(header) => {
                    self.handle_new_header(header);
                }

                CordialKnowledgeMessage::NewShard(block_ref) => {
                    self.handle_new_shard(block_ref);
                }

                CordialKnowledgeMessage::EvictBelow(rounds) => {
                    self.handle_evict_below(rounds);
                }
            }
            yield_now().await;
        }

        debug!("Cordial Knowledge main loop finished");
    }

    /// Called when a new verified block header is received.
    fn handle_new_header(&mut self, header: VerifiedBlockHeader) {
        self.update_cordial_knowledge(&header);
    }

    /// Called when a new *own shard* (produced locally) is added.
    fn handle_new_shard(&mut self, block_ref: BlockRef) {
        let msg = ConnectionKnowledgeMessage::NewShard { block_ref };
        for tx in &self.connections {
            let _ = tx.try_send(vec![msg.clone()]);
        }
    }

    /// Called when older rounds should be pruned globally.
    fn handle_evict_below(&mut self, rounds: Vec<Round>) {
        // Evict locally
        for (index, deque) in &mut self.cordial_knowledge.iter_mut().enumerate() {
            while let Some((front_round, _)) = deque.front() {
                if *front_round < rounds[index] {
                    deque.pop_front();
                } else {
                    break;
                }
            }
        }

        // Notify per-connection tasks about eviction
        self.notify_connection_tasks_for_eviction(rounds);
    }

    #[inline]
    fn notify_connection_tasks_for_eviction(&self, rounds: Vec<Round>) {
        let msg = ConnectionKnowledgeMessage::EvictBelow(rounds);
        for tx in &self.connections {
            let _ = tx.try_send(vec![msg.clone()]);
        }
    }

    /// Map a round to an index inside a per-author VecDeque<(Round, T)>.
    /// Returns None if `round` is outside the current rolling window stored in the deque.
    #[inline]
    fn round_to_index<T>(dq: &VecDeque<(Round, T)>, round: Round) -> Option<usize> {
        let (front_round, _) = dq.front()?;
        if round < *front_round {
            return None;
        }
        let idx = (round - *front_round) as usize;
        if idx < dq.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Ensure the author's deque contains `round`, extending **only that author's deque**
    /// forward as needed. Returns a mutable handle to
    /// the (Round, Map) entry for `round`.
    #[inline]
    fn ensure_author_round_map(
        &mut self,
        author_value: usize,
        round: Round,
    ) -> &mut (Round, AHashMap<BlockHeaderDigest, (Ancestors, SubsetAuthorities)>) {
        let dq = &mut self.cordial_knowledge[author_value];
        match dq.back() {
            // Empty -> push exactly this round
            None => {
                dq.push_back((round, AHashMap::default()));
            }
            Some((last_round, _)) => {
                if *last_round < round {
                    // Extend forward up to `round`
                    let mut r = *last_round + 1;
                    while r <= round {
                        dq.push_back((r, AHashMap::default()));
                        r += 1;
                    }
                }
            }
        }

        let idx = Self::round_to_index(dq, round)
            .expect("We should expect round to be within or equal to the deque window");
        &mut dq[idx]
    }

    /// Update cordial knowledge for exactly one new header.
    /// Assumes all parents are already stored somewhere in `recent_dag_cordial_knowledge`
    /// - Only grows the author's deque if needed.
    /// - For other authorities' "unknown headers" deques, we add the block only if
    ///   the round bucket already exists (no growth).
    fn update_cordial_knowledge(&mut self, header: &VerifiedBlockHeader) {
        let block_ref = header.reference();
        let block_author = block_ref.author.value();
        let round = block_ref.round;
        let digest = block_ref.digest;
        let own_index = self.context.own_index.value();
        let mut vec_knowledge_msgs: Vec<Vec<ConnectionKnowledgeMessage>> =
            vec![Vec::new(); self.context.committee.size()];

        //  === 1) Get (or create forward) the author's round bucket and insert if missing  ===
        let (_, round_map) = self.ensure_author_round_map(block_author, round);
        if round_map.contains_key(&digest) {
            // Already recorded — nothing else to do here.
            return;
        }

        let ancestors: Ancestors = Arc::from(header.ancestors());

        // (who_knows initially marks: author + self)
        let who_knows = SubsetAuthorities::new_with(block_author, own_index);
        round_map.insert(digest, (ancestors, who_knows));

        //  === 2) Mark this header as "unknown" for other authorities  ===
        let msg = ConnectionKnowledgeMessage::NewHeader { block_ref };
        for other_idx in 0..self.context.committee.size() {
            if other_idx == block_author || other_idx == own_index {
                continue;
            }
            vec_knowledge_msgs[other_idx].push(msg.clone());
        }

        //  === 3) Notify that the block_author now knows transaction data for certain blocks ===
        for acknowledgment in header.acknowledgments() {
            vec_knowledge_msgs[block_author].push(ConnectionKnowledgeMessage::RemoveShard {
                block_ref: *acknowledgment,
            });
        }


        // === 4) Traverse the DAG and update the knowledge of block author about the causal past ===
        // We do a DFS traversal using a stack (buffer).
        // For each parent, if the block_author does not know it yet, we mark it
        // as known by block_author, send a message to the corresponding connection,
        // and push the parent onto the stack for further traversal.
        let mut buffer = vec![block_ref];

        while let Some(traversed_ref) = buffer.pop() {
            let current_author = traversed_ref.author.value();
            let current_round = traversed_ref.round;
            let current_digest = traversed_ref.digest;

            // Locate the round bucket for this traversed block
            let deque = &mut self.cordial_knowledge[current_author];
            if let Some(index) = Self::round_to_index(deque, current_round) {
                // Found correct round bucket
                let (_r, map) = &mut deque[index];

                // Get this block’s entry
                let parents = match map.get(&current_digest) {
                    Some((ancestors, _)) => ancestors.clone(),
                    None => continue, // skip unknown block
                };

                // Iterate over the parents
                for parent in parents.iter() {
                    let parent_author = parent.author;
                    let parent_round = parent.round;
                    let parent_digest = parent.digest;

                    // Find the parent’s round bucket
                    let deque_parent_author = &mut self.cordial_knowledge[parent_author.value()];
                    if let Some(parent_index) = Self::round_to_index(deque_parent_author, parent_round) {
                        let (_, parents_of_parent_map) = &mut deque_parent_author[parent_index];

                        if let Some((_anc, who_knows_parent)) = parents_of_parent_map.get_mut(&parent_digest) {
                            // Insert new knowledge as block_author knows this parent
                            if who_knows_parent.insert(block_author) {
                                vec_knowledge_msgs[block_author].push(ConnectionKnowledgeMessage::RemoveHeader {
                                    block_ref: *parent,
                                });
                                // Push parent to buffer for further propagation
                                buffer.push(*parent);
                            }
                        }
                    }
                }
            }
        }
        self.send_knowledge_messages(vec_knowledge_msgs);
    }

    fn send_knowledge_messages(&self, msgs: Vec<Vec<ConnectionKnowledgeMessage>>) {
        for (idx, tx) in self.connections.iter().enumerate() {
            if !msgs[idx].is_empty() {
                let _ = tx.try_send(msgs[idx].clone());
            }
        }
    }


}

#[derive(Clone, Debug)]
pub enum ConnectionKnowledgeMessage {
    /// A new block header was added globally.
    NewHeader {
        block_ref: BlockRef,
    },
    /// Remove a block header from the "unknown" set .
    RemoveHeader {
        block_ref: BlockRef,
    },
    /// A new shard was added globally.
    NewShard {
        block_ref: BlockRef,
    },
    /// Remove a header from the "unknown" set.
    RemoveShard {
        block_ref: BlockRef,
    },
    /// Take useful headers for the given authorities, up to the given round (exclusive).
    TakeUsefulHeaders {
        round_upper_bound_exclusive: Round,
        useful_authorities: Vec<usize>,
        respond_to: Sender<Vec<BlockRef>>,
    },
    /// Take useful shards for the given authorities, up to the given round (exclusive).
    TakeUsefulShards {
        round_upper_bound_exclusive: Round,
        useful_authorities: Vec<usize>,
        respond_to: Sender<Vec<BlockRef>>,
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
}


/// Manages the knowledge state for a single connection to a peer.
/// Receives updates from the global cordial knowledge
pub struct ConnectionKnowledge {
    context: Arc<Context>,
    peer_index: usize,
    headers_not_known: Vec<VecDeque<(Round, AHashSet<BlockRef>)>>,
    shards_not_known: Vec<VecDeque<(Round, AHashSet<BlockRef>)>>,
    /// Receives updates from the global cordial knowledge
    receiver: Receiver<Vec<ConnectionKnowledgeMessage>>,
}

impl ConnectionKnowledge {
    pub fn new(
        context: Arc<Context>,
        peer_index: usize,
        receiver: Receiver<Vec<ConnectionKnowledgeMessage>>,
    ) -> Self {
        let num_authorities = context.committee.size();
        let headers_not_known = Vec::with_capacity(num_authorities);
        let shards_not_known = Vec::with_capacity(num_authorities);
        Self {
            context,
            peer_index,
            headers_not_known,
            shards_not_known,
            receiver,
        }
    }

    #[inline]
    fn round_to_index<T>(deque: &VecDeque<(Round, T)>, round: Round) -> Option<usize> {
        let (front_round, _) = deque.front()?;
        if round < *front_round { return None; }
        let index = (round - *front_round) as usize;
        (index < deque.len()).then_some(index)
    }

    /// Find the minimum front round among the given authorities’ deques.
    /// Returns `None` if all selected deques are empty.
    #[inline]
    fn min_front_round<'a, T>(
        all_deques: &Vec<VecDeque<(Round, T)>>,
        authorities: impl IntoIterator<Item = &'a usize>,
    ) -> Option<Round> {
        let mut min_round: Option<Round> = None;
        for authority in authorities {
            if let Some((round, _)) = all_deques[*authority].front() {
                min_round = Some(min_round.map_or(*round, |m| m.min(*round)));
            }
        }
        min_round
    }

    #[inline]
    fn ensure_round_in_deque(
        deque: &mut VecDeque<(Round, AHashSet<BlockRef>)>,
        target_round: Round,
    ) -> Option<usize>    {

        // 1. If deque is empty, initialize with this round
        if deque.is_empty() {
            deque.push_back((target_round, AHashSet::default()));
            return Some(0);
        }

        let front_round = deque.front().unwrap().0;
        let back_round = deque.back().unwrap().0;

        // 2. If round is older than the current window → skip (already evicted)
        if target_round < front_round {
            return None;
        }

        // 3. Extend forward up to the requested round
        if target_round > back_round {
            for current_round in (back_round + 1)..=target_round {
                deque.push_back((current_round, AHashSet::default()));
            }
        }

        // 4. Compute index for this round
        let idx = (target_round - front_round) as usize;
        Some(idx)
    }
    /// Block ref selection:
    ///  - iterate rounds from the earliest available among `useful_authorities`
    ///    up to (but excluding) `round_upper_bound_exclusive`,
    ///  - for each round, scan only the given `useful_authorities`,
    ///  - collect up to `max_headers_per_bundle`,
    ///  - then remove them from this connection’s unknown sets.
    fn take_useful_block_refs_round(
        &mut self,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
    ) -> Vec<BlockRef> {
        let max_take = self.context.parameters.max_headers_per_bundle;

        // Nothing to do
        if useful_authorities.is_empty() || max_take == 0 {
            return Vec::new();
        }

        // Start from the earliest front round across the selected authorities.
        let Some(min_round) = Self::min_front_round(&self.headers_not_known, useful_authorities.iter()) else {
            return Vec::new();
        };

        let mut current_round = min_round;

        let mut taken: Vec<BlockRef> = Vec::with_capacity(max_take);

        'outer: while current_round < round_upper_bound_exclusive {
            for authority in useful_authorities {
                let deque = &self.headers_not_known[*authority];

                // If this authority has this round bucket, take from it.
                if let Some(index) = Self::round_to_index(deque, current_round) {
                    // Iterate the set without mutating it yet.
                    for block_ref in deque[index].1.iter() {
                        taken.push(*block_ref);
                        if taken.len() >= max_take {
                            break 'outer;
                        }
                    }
                }
            }
            // advance round
            current_round = current_round.saturating_add(1);
        }

        // Remove the selected ones from local unknown sets.
        for block_ref in &taken {
            let authority = block_ref.author.value();
            let deque = &mut self.headers_not_known[authority];
            if let Some(index) = Self::round_to_index(deque, block_ref.round) {
                deque[index].1.remove(block_ref);
            }
        }

        taken
    }

    /// Same as `take_useful_block_refs_round` but for shards.
    fn take_useful_shard_block_refs_round(
        &mut self,
        round_upper_bound_exclusive: Round,
        useful_authorities: &[usize],
    ) -> Vec<BlockRef> {
        let max_take = self.context.parameters.max_shards_per_bundle;

        if useful_authorities.is_empty() || max_take == 0 {
            return Vec::new();
        }

        let Some(min_round) = Self::min_front_round(&self.shards_not_known, useful_authorities.iter()) else {
            return Vec::new();
        };

        let mut current_round = min_round;

        let mut taken: Vec<BlockRef> = Vec::with_capacity(max_take);

        'outer: while current_round < round_upper_bound_exclusive {
            for authority in useful_authorities {
                let deque = &self.shards_not_known[*authority];

                if let Some(index) = Self::round_to_index(deque, current_round) {
                    for block_ref in deque[index].1.iter() {
                        taken.push(*block_ref);
                        if taken.len() >= max_take {
                            break 'outer;
                        }
                    }
                }
            }
            // Advance round
            current_round = current_round.saturating_add(1);
        }

        // Remove the selected ones from unknown sets
        for block_ref in &taken {
            let authority = block_ref.author.value();
            let deque = &mut self.shards_not_known[authority];
            if let Some(index) = Self::round_to_index(deque, block_ref.round) {
                deque[index].1.remove(block_ref);
            }
        }

        taken
    }

    fn evict_below(&mut self, rounds: Vec<Round>) {
        for (index, deque) in self.headers_not_known.iter_mut().enumerate() {
            while let Some((front_round, _)) = deque.front() {
                if *front_round < rounds[index] {
                    deque.pop_front();
                } else {
                    break;
                }
            }
        }
        for (index, deque) in self.shards_not_known.iter_mut().enumerate() {
            while let Some((front_round, _)) = deque.front() {
                if *front_round < rounds[index] {
                    deque.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Async task loop — just receives messages and dispatches to processing logic.
    pub async fn run(mut self) {
        tracing::debug!(
            "Connection Knowledge started for peer {}",
            self.peer_index
        );

        while let Some(knowledge_msgs) = self.receiver.recv().await {
            for knowledge_msg in knowledge_msgs {
                self.process_message(knowledge_msg).await;
            }
            tokio::task::yield_now().await;
        }

        tracing::debug!(
            "Connection Knowledge loop ended for peer {}",
            self.peer_index
        );
    }

    /// Processes a batch of knowledge updates synchronously (non-async).
    /// This isolates all mutation logic so it can be tested without async context.
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
            ConnectionKnowledgeMessage::TakeUsefulHeaders {
                round_upper_bound_exclusive,
                useful_authorities,
                respond_to,
            } => {
                let taken = self.take_useful_block_refs_round(
                    round_upper_bound_exclusive,
                    &useful_authorities,
                );
                if let Err(e) = respond_to.send(taken).await {
                    warn!("Failed to respond to useful headers: {} for connection task with peer index {}", e, self.peer_index);
                }
            }
            ConnectionKnowledgeMessage::TakeUsefulShards {
                round_upper_bound_exclusive,
                useful_authorities,
                respond_to,
            } => {
                let taken = self.take_useful_shard_block_refs_round(
                    round_upper_bound_exclusive,
                    &useful_authorities,
                );
                if let Err(e) = respond_to.send(taken).await {
                    warn!("Failed to respond to useful shards: {} for connection task with peer index {}", e, self.peer_index);
                }
            }
        }
    }

    /// Handles adding a new block to the unknown set.
    fn handle_new_header(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();
        if let Some(index) = Self::ensure_round_in_deque(
            &mut self.headers_not_known[authority],
            round,
        ) {
            let (_r, set) = &mut self.headers_not_known[authority][index];
            set.insert(block_ref);
        }
    }


    /// Handles adding a new shard to the unknown set.
    fn handle_new_shard(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();

        if let Some(index) = Self::ensure_round_in_deque(
            &mut self.shards_not_known[authority],
            round,
        ) {
            let (_r, set) = &mut self.shards_not_known[authority][index];
            set.insert(block_ref);
        }
    }

    /// Handles removing a header that this peer now knows.
    fn handle_remove_header(&mut self, block_ref: BlockRef) {
        let authority = block_ref.author.value();
        let round = block_ref.round;
        if let Some(index) = Self::round_to_index(&self.headers_not_known[authority], round) {
            let (_r, set) = &mut self.headers_not_known[authority][index];
            set.remove(&block_ref);
        }
    }

    /// Handles removing a shard that this peer now knows.
    fn handle_remove_shard(&mut self, block_ref: BlockRef) {
        let round = block_ref.round;
        let authority = block_ref.author.value();
        if let Some(index) = Self::round_to_index(&self.shards_not_known[authority], round) {
            let (_r, set) = &mut self.shards_not_known[authority][index];
            set.remove(&block_ref);
        }
    }
}