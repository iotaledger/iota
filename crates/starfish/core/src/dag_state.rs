// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::{max, min},
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    mem,
    ops::Bound::{Excluded, Included, Unbounded},
    panic,
    sync::Arc,
    vec,
};

use itertools::Itertools as _;
use starfish_config::AuthorityIndex;
use tokio::time::Instant;
use tracing::{debug, error, info};

use crate::{
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, Slot,
        VerifiedBlock, VerifiedBlockHeader, VerifiedTransactions, genesis_blocks,
    },
    commit::{
        CommitAPI as _, CommitDigest, CommitIndex, CommitInfo, CommitRef, CommitVote,
        GENESIS_COMMIT_INDEX, SubDagBase, TrustedCommit, load_pending_subdag_from_store,
    },
    context::Context,
    leader_scoring::{ReputationScores, ScoringSubdag},
    linearizer::MAX_LINEARIZER_DEPTH,
    storage::{Store, WriteBatch},
    threshold_clock::ThresholdClock,
};

/// Acknowledgment depth is the maximum number of rounds from current round
/// for which acknowledgments are kept in memory and can be injected in a new
/// block.
// TODO: make it derivable from the protocol parameters
pub(crate) const MAX_TRANSACTIONS_ACK_DEPTH: Round = 50;
pub(crate) const MAX_HEADERS_PER_BUNDLE: usize = 150;

/// DagState provides the API to write and read accepted blocks from the DAG.
/// Only uncommitted and last committed blocks are cached in memory.
/// The rest of blocks are stored on disk.
/// Refs to cached blocks and additional refs are cached as well, to speed up
/// existence checks.
///
/// Note: DagState should be wrapped with Arc<parking_lot::RwLock<_>>, to allow
/// concurrent access from multiple components.
pub(crate) struct DagState {
    context: Arc<Context>,

    // The genesis blocks
    genesis: BTreeMap<BlockRef, VerifiedBlock>,

    // Contains recent block headers within CACHED_ROUNDS from the last traversed round per
    // authority. Note: all uncommitted block headers are kept in memory.
    recent_block_headers: BTreeMap<BlockRef, VerifiedBlockHeader>,

    // Contains recent transactions. It contains MAX_TRANSACTIONS_ACK_DEPTH+MAX_LINEARIZER_DEPTH
    // from the round of the last consumed commit. Note: all transactions in blocks below that
    // round are evicted from memory.
    recent_transactions: BTreeMap<BlockRef, VerifiedTransactions>,

    // Indexes recent block headers refs by their authorities.
    // Vec position corresponds to the authority index.
    recent_headers_refs_by_authority: Vec<BTreeSet<BlockRef>>,

    // Keeps track of the threshold clock for proposing blocks.
    threshold_clock: ThresholdClock,

    // Keeps track of the highest round that has been evicted for each authority. Any block header
    // that are of round <= evict_round should be considered evicted, and if any exist we
    // should not consider the causally complete in the order they appear. The `evicted_rounds`
    // size should be the same as the committee size.
    evicted_rounds: Vec<Round>,

    // Highest round of blocks accepted.
    highest_accepted_round: Round,

    // Last consensus commit of the dag.
    last_commit: Option<TrustedCommit>,

    // Last wall time when commit round advanced. Does not persist across restarts.
    last_commit_round_advancement_time: Option<std::time::Instant>,

    // Round of the last committed leader which created a commit with available transactions. Does
    // not persist across restarts and after recovery. All transactions below this round minus
    // MAX_TRANSACTIONS_ACK_DEPTH minus MAX_LINEARIZER_DEPTH are evicted from memory.
    last_available_commit_leader_round: Option<Round>,

    // Rounds for latest blocks traversed by linearizer per authority.
    last_committed_rounds: Vec<Round>,

    /// The committed subdags that have been scored but scores have not been
    /// used for leader schedule yet.
    scoring_subdag: ScoringSubdag,

    // Commit votes pending to be included in new blocks.
    // TODO: limit to 1st commit per round with multi-leader.
    pending_commit_votes: VecDeque<CommitVote>,

    // Acknowledgments pending to be included in new blocks. These represent votes indicating
    // availability of transaction data from the corresponding blocks
    pending_acknowledgments: BTreeSet<BlockRef>,

    // TODO: add metrics for recent_dag_cordial_knowledge and block_headers_not_known_by_authority
    // and pending_acknowledgments Keeps track of the most recent BlockHeaderDAG cordial
    // knowledge (who knows which blocks) for each authority. This is a helper structure that
    // is used primarily for traversing the recent DAG. This struct is evicted after flushing
    // the dag state to storage and is not persisted.
    // To access the cordial knowledge of a given block_ref, one shall retrieve it from
    // `recent_dag_cordial_knowledge[block_ref.author][(block_ref.round, block_ref.digest)]`.
    // The value is a tuple of (parents, who knows the block header).
    recent_dag_cordial_knowledge:
        Vec<BTreeMap<(Round, BlockHeaderDigest), (Vec<BlockRef>, HashSet<AuthorityIndex>)>>,
    // Keeps tracks of block headers that are not known by the authority.
    // Is used to ensure that we send block headers that are really needed
    // to the authority, and not the ones that they already know.
    block_headers_not_known_by_authority: Vec<BTreeSet<BlockRef>>,

    // Transactions to be flushed to storage.
    transactions_to_write: Vec<VerifiedTransactions>,
    block_headers_to_write: Vec<VerifiedBlockHeader>,
    commits_to_write: Vec<TrustedCommit>,

    // Buffer the reputation scores & last_committed_rounds to be flushed with the
    // next dag state flush. This is okay because we can recover reputation scores
    // & last_committed_rounds from the commits as needed.
    commit_info_to_write: Vec<(CommitRef, CommitInfo)>,

    // Persistent storage for blocks, commits and other consensus data.
    store: Arc<dyn Store>,

    // The number of cached rounds
    cached_rounds: Round,
}

impl DagState {
    /// Initializes DagState from storage.
    pub(crate) fn new(context: Arc<Context>, store: Arc<dyn Store>) -> Self {
        let cached_rounds = context.parameters.dag_state_cached_rounds as Round;
        let num_authorities = context.committee.size();

        let genesis = genesis_blocks(context.clone())
            .into_iter()
            .map(|block| (block.reference(), block))
            .collect();

        let threshold_clock = ThresholdClock::new(1, context.clone());

        let last_commit = store
            .read_last_commit()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));

        let commit_info = store
            .read_last_commit_info()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));
        let (mut last_committed_rounds, commit_recovery_start_index) =
            if let Some((commit_ref, commit_info)) = commit_info {
                tracing::info!("Recovering committed state from {commit_ref} {commit_info:?}");
                (commit_info.committed_rounds, commit_ref.index + 1)
            } else {
                tracing::info!("Found no stored CommitInfo to recover from");
                (vec![0; num_authorities], GENESIS_COMMIT_INDEX + 1)
            };

        let mut unscored_committed_subdags = Vec::new();
        let mut scoring_subdag = ScoringSubdag::new(context.clone());

        if let Some(last_commit) = last_commit.as_ref() {
            store
                .scan_commits((commit_recovery_start_index..=last_commit.index()).into())
                .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e))
                .iter()
                .for_each(|commit| {
                    for block_ref in commit.blocks() {
                        last_committed_rounds[block_ref.author] =
                            max(last_committed_rounds[block_ref.author], block_ref.round);
                    }

                    let committed_subdag =
                        load_pending_subdag_from_store(store.as_ref(), commit.clone(), vec![]); // We don't need to recover reputation scores for unscored_committed_subdags
                    unscored_committed_subdags.push(committed_subdag.base);
                });
        }

        tracing::info!(
            "DagState was initialized with the following state: \
            {last_commit:?}; {last_committed_rounds:?}; {} unscored committed subdags;",
            unscored_committed_subdags.len()
        );

        scoring_subdag.add_subdags(std::mem::take(&mut unscored_committed_subdags));

        let mut state = Self {
            context,
            genesis,
            recent_block_headers: BTreeMap::new(),
            recent_transactions: BTreeMap::new(),
            recent_headers_refs_by_authority: vec![BTreeSet::new(); num_authorities],
            threshold_clock,
            highest_accepted_round: 0,
            last_commit: last_commit.clone(),
            last_commit_round_advancement_time: None,
            last_committed_rounds: last_committed_rounds.clone(),
            last_available_commit_leader_round: None, /* Later the commit observer might update
                                                       * this value during recovery process. */
            pending_commit_votes: VecDeque::new(),
            transactions_to_write: vec![],
            block_headers_to_write: vec![],
            commits_to_write: vec![],
            commit_info_to_write: vec![],
            pending_acknowledgments: BTreeSet::new(),
            recent_dag_cordial_knowledge: vec![BTreeMap::new(); num_authorities],
            block_headers_not_known_by_authority: vec![BTreeSet::new(); num_authorities],
            scoring_subdag,
            store: store.clone(),
            cached_rounds,
            evicted_rounds: vec![0; num_authorities],
        };

        for (i, round) in last_committed_rounds.into_iter().enumerate() {
            let authority_index = state.context.committee.to_authority_index(i).unwrap();
            let (block_headers, transactions_by_author, eviction_round) = {
                let eviction_round = Self::eviction_round(round, cached_rounds);
                // TODO: scan for blocks?
                let block_headers = state
                    .store
                    .scan_block_headers_by_author(authority_index, eviction_round + 1)
                    .expect("Database error");
                let transactions_by_author = state
                    .store
                    .scan_transactions_by_author(authority_index, eviction_round + 1)
                    .expect("Database error");
                (block_headers, transactions_by_author, eviction_round)
            };

            state.evicted_rounds[authority_index] = eviction_round;

            // Update the block metadata for the authority.
            for block_header in &block_headers {
                state.update_block_metadata(block_header);
            }
            for transactions in &transactions_by_author {
                state.update_transaction_metadata(transactions);
            }

            info!(
                "Recovered block headers {}: {:?}",
                authority_index,
                block_headers
                    .iter()
                    .map(|b| b.reference())
                    .collect::<Vec<BlockRef>>()
            );
        }
        state
    }

    /// Accepts a block header into DagState and keeps it in memory.
    pub(crate) fn accept_block_header(&mut self, block_header: VerifiedBlockHeader) {
        assert_ne!(
            block_header.round(),
            0,
            "Genesis block should not be accepted into DAG."
        );

        let block_ref = block_header.reference();
        if self.contains_block_header(&block_ref) {
            return;
        }

        let now = self.context.clock.timestamp_utc_ms();
        if block_header.timestamp_ms() > now {
            panic!(
                "Block {:?} cannot be accepted! Block timestamp {} is greater than local timestamp {}.",
                block_header,
                block_header.timestamp_ms(),
                now,
            );
        }

        // TODO: Move this check to core
        // Ensure we don't write multiple blocks per slot for our own index
        if block_ref.author == self.context.own_index {
            let existing_blocks = self.get_uncommitted_blocks_at_slot(block_ref.into());
            assert!(
                existing_blocks.is_empty(),
                "Block Rejected! Attempted to add block header {block_header:#?} to own slot where \
                block(s) {existing_blocks:#?} already exists."
            );
        }
        self.update_block_metadata(&block_header);
        debug!(
            "block header {} pushed to write to store batch by {}",
            block_header, self.context.own_index
        );
        self.block_headers_to_write.push(block_header);
        let source = if self.context.own_index == block_ref.author {
            "own"
        } else {
            "others"
        };
        // TODO: rename to accepted block headers?
        self.context
            .metrics
            .node_metrics
            .accepted_blocks
            .with_label_values(&[source])
            .inc();
    }

    pub(crate) fn add_transactions(&mut self, transactions: VerifiedTransactions) {
        let block_ref = transactions.block_ref();
        self.recent_transactions
            .insert(block_ref, transactions.clone());

        // If a block is not very old, add it to pending acknowledgments
        let clock_round = self.threshold_clock_round();
        let min_round: Round = clock_round.saturating_sub(MAX_TRANSACTIONS_ACK_DEPTH);

        if block_ref.round >= min_round {
            self.add_pending_acknowledgment(block_ref);
        }
        self.transactions_to_write.push(transactions);
    }

    pub fn update_last_available_commit_leader_round(&mut self, round: Round) {
        debug!(
            "Last commit with available transactions has leader at round {}",
            round
        );
        self.last_available_commit_leader_round = Some(round);
    }

    /// Updates internal metadata for a block.
    fn update_block_metadata(&mut self, block_header: &VerifiedBlockHeader) {
        let block_ref = block_header.reference();
        self.recent_block_headers
            .insert(block_ref, block_header.clone());
        self.recent_headers_refs_by_authority[block_ref.author].insert(block_ref);
        self.threshold_clock.add_block(block_ref);
        self.highest_accepted_round = max(self.highest_accepted_round, block_header.round());
        self.context
            .metrics
            .node_metrics
            .highest_accepted_round
            .set(self.highest_accepted_round as i64);

        let highest_accepted_round_for_author = self.recent_headers_refs_by_authority
            [block_ref.author]
            .last()
            .map(|block_ref| block_ref.round)
            .expect("There should be by now at least one block ref");
        let hostname = &self.context.committee.authority(block_ref.author).hostname;
        self.context
            .metrics
            .node_metrics
            .highest_accepted_authority_round
            .with_label_values(&[hostname])
            .set(highest_accepted_round_for_author as i64);
        self.update_cordial_knowledge(block_header);
    }

    fn update_transaction_metadata(&mut self, transaction: &VerifiedTransactions) {
        self.recent_transactions
            .insert(transaction.block_ref(), transaction.clone());
    }

    // Function updates who knows which BlockHeaders what after receiving a new
    // block header, which is accepted. In particular, it traverses the DAG from
    // the block header and updates the knowledge of the given authority.
    fn update_cordial_knowledge(&mut self, block_header: &VerifiedBlockHeader) {
        let block_reference = block_header.reference();
        let block_author = block_reference.author;
        let round_digest = (block_reference.round, block_reference.digest);

        // Collect parents of the block header, which are not genesis
        let parents = block_header
            .ancestors()
            .iter()
            .filter(|parent| parent.round > GENESIS_ROUND)
            .cloned()
            .collect::<Vec<_>>();

        // If the block header is in the recent_dag_cordial_knowledge, then
        // don't update if it is already there
        if self.recent_dag_cordial_knowledge[block_author.value()].contains_key(&round_digest) {
            return;
        }

        // update information about block_reference
        self.recent_dag_cordial_knowledge[block_author.value()].insert(
            round_digest,
            (
                parents,
                vec![block_reference.author, self.context.own_index]
                    .into_iter()
                    .collect::<HashSet<_>>(),
            ),
        );

        // Assume that only this authority and the author know the block header
        for authority_index in 0..self.block_headers_not_known_by_authority.len() {
            if authority_index == self.context.own_index.value()
                || authority_index == block_reference.author.value()
            {
                continue;
            }
            self.block_headers_not_known_by_authority[authority_index].insert(block_reference);
        }

        // traverse the DAG from block_reference and update the blocks known by the
        // author of this block
        let mut buffer = vec![block_reference];

        while let Some(traversed_block_reference) = buffer.pop() {
            let traversed_block_author = traversed_block_reference.author;
            let traversed_block_round_digest = (
                traversed_block_reference.round,
                traversed_block_reference.digest,
            );
            let (parents, _) = self.recent_dag_cordial_knowledge[traversed_block_author.value()]
                .get(&traversed_block_round_digest)
                .expect("We should expect having an element with given BlockRef")
                .clone();
            for parent in parents {
                let traversed_parent_author = parent.author;
                let traversed_parent_round_digest = (parent.round, parent.digest);

                if self.recent_dag_cordial_knowledge[traversed_parent_author.value()]
                    .contains_key(&traversed_parent_round_digest)
                {
                    let (_, who_knows_given_parent) = self.recent_dag_cordial_knowledge
                        [traversed_parent_author.value()]
                    .get_mut(&traversed_parent_round_digest)
                    .expect("We should have this value as it is checked");
                    if who_knows_given_parent.insert(block_author) {
                        self.block_headers_not_known_by_authority[block_author.value()]
                            .remove(&parent);
                        buffer.push(parent);
                    }
                }
            }
        }
    }

    /// Accepts a block header into DagState and keeps it in memory.
    pub(crate) fn accept_block_headers(&mut self, blocks: Vec<VerifiedBlockHeader>) {
        debug!(
            "Accepting blocks: {}",
            blocks.iter().map(|b| b.reference().to_string()).join(",")
        );
        for block in blocks {
            self.accept_block_header(block);
        }
    }

    /// Gets a transaction by checking cached recent transactions then storage.
    /// Returns None when the transaction is not found.
    pub(crate) fn get_transaction(&self, reference: &BlockRef) -> Option<VerifiedTransactions> {
        self.get_transactions(&[*reference])
            .pop()
            .expect("Exactly one element should be returned")
    }

    /// Gets transactions by checking cached recent transactions in memory, then
    /// storage. An element is None when the corresponding transaction is not
    /// found.
    pub(crate) fn get_transactions(
        &self,
        block_refs: &[BlockRef],
    ) -> Vec<Option<VerifiedTransactions>> {
        let mut transactions = vec![None; block_refs.len()];
        let mut missing = Vec::new();

        for (index, block_ref) in block_refs.iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Allow the caller to handle the invalid genesis ancestor error.
                if let Some(transaction) = self
                    .genesis
                    .get(block_ref)
                    .map(|block| block.verified_transactions.clone())
                {
                    transactions[index] = Some(transaction);
                }
                continue;
            }
            if let Some(transaction) = self.recent_transactions.get(block_ref) {
                transactions[index] = Some(transaction.clone());
                continue;
            }
            missing.push((index, block_ref));
        }

        if missing.is_empty() {
            return transactions;
        }

        let missing_refs = missing
            .iter()
            .map(|(_, block_ref)| **block_ref)
            .collect::<Vec<_>>();
        let store_results = self
            .store
            .read_transactions(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));
        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["get_transactions"])
            .inc();

        for ((index, _), result) in missing.into_iter().zip(store_results.into_iter()) {
            transactions[index] = result;
        }

        transactions
    }

    /// Gets a block by reconstructing it from its header and transactions.
    /// Returns None when the block is not found.
    pub(crate) fn get_block(&self, reference: &BlockRef) -> Option<VerifiedBlock> {
        let header = self.get_block_header(reference)?;
        let transactions = self.get_transaction(reference)?;
        Some(VerifiedBlock::new(header, transactions))
    }

    /// Gets blocks by reconstructing them from their headers and transactions.
    /// Returns None for elements where the block is not found.
    pub(crate) fn get_blocks(&self, block_refs: &[BlockRef]) -> Vec<Option<VerifiedBlock>> {
        let headers = self.get_block_headers(block_refs);
        let transactions = self.get_transactions(block_refs);

        headers
            .into_iter()
            .zip(transactions)
            .map(|(header, transaction)| match (header, transaction) {
                (Some(header), Some(transaction)) => Some(VerifiedBlock::new(header, transaction)),
                _ => None,
            })
            .collect()
    }

    /// Gets a block header by checking cached recent blocks then storage.
    /// Returns None when the block is not found.
    pub(crate) fn get_block_header(&self, reference: &BlockRef) -> Option<VerifiedBlockHeader> {
        self.get_block_headers(&[*reference])
            .pop()
            .expect("Exactly one element should be returned")
    }

    /// Gets block headers by checking genesis, cached recent block headers in
    /// memory, then storage. An element is None when the corresponding
    /// block header is not found.
    pub(crate) fn get_block_headers(
        &self,
        block_refs: &[BlockRef],
    ) -> Vec<Option<VerifiedBlockHeader>> {
        let mut block_headers: Vec<Option<VerifiedBlockHeader>> = vec![None; block_refs.len()];
        let mut missing_headers = Vec::new();
        for (index, block_ref) in block_refs.iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Allow the caller to handle the invalid genesis ancestor error.
                if let Some(block) = self.genesis.get(block_ref) {
                    block_headers[index] = Some((**block).clone());
                }
                continue;
            }
            if let Some(block) = self.recent_block_headers.get(block_ref) {
                block_headers[index] = Some(block.clone());
                continue;
            }
            missing_headers.push((index, block_ref));
        }

        if missing_headers.is_empty() {
            return block_headers;
        }

        let missing_refs = missing_headers
            .iter()
            .map(|(_, block_ref)| **block_ref)
            .collect::<Vec<_>>();
        let store_results = self
            .store
            .read_block_headers(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));
        // TODO:similar metric for header reads count
        // self.context
        // .metrics
        // .node_metrics
        // .dag_state_store_read_count
        // .with_label_values(&["get_blocks"])
        // .inc();

        for ((index, _), result) in missing_headers.into_iter().zip(store_results.into_iter()) {
            block_headers[index] = result;
        }

        block_headers
    }

    /// Gets all uncommitted blocks in a slot.
    /// Uncommitted blocks must exist in memory, so only in-memory blocks are
    /// checked.
    pub(crate) fn get_uncommitted_blocks_at_slot(&self, slot: Slot) -> Vec<VerifiedBlockHeader> {
        // TODO: either panic below when the slot is at or below the last committed
        // round, or support reading from storage while limiting storage reads
        // to edge cases.

        let mut blocks = vec![];
        for (_block_ref, block) in self.recent_block_headers.range((
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MIN,
            )),
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MAX,
            )),
        )) {
            blocks.push(block.clone())
        }
        blocks
    }

    /// Gets all uncommitted blocks in a round.
    /// Uncommitted blocks must exist in memory, so only in-memory blocks are
    /// checked.
    pub(crate) fn get_uncommitted_blocks_at_round(&self, round: Round) -> Vec<VerifiedBlockHeader> {
        if round <= self.last_commit_round() {
            panic!("Round {} have committed blocks!", round);
        }

        let mut blocks = vec![];
        for (_block_ref, block) in self.recent_block_headers.range((
            Included(BlockRef::new(
                round,
                AuthorityIndex::ZERO,
                BlockHeaderDigest::MIN,
            )),
            Excluded(BlockRef::new(
                round + 1,
                AuthorityIndex::ZERO,
                BlockHeaderDigest::MIN,
            )),
        )) {
            blocks.push(block.clone())
        }
        blocks
    }

    /// Gets all ancestors in the history of a block at a certain round.
    pub(crate) fn ancestors_at_round(
        &self,
        later_block: &VerifiedBlockHeader,
        earlier_round: Round,
    ) -> Vec<VerifiedBlockHeader> {
        // Iterate through ancestors of later_block in round descending order.
        let mut linked: BTreeSet<BlockRef> = later_block.ancestors().iter().cloned().collect();
        while !linked.is_empty() {
            let round = linked.last().unwrap().round;
            // Stop after finishing traversal for ancestors above earlier_round.
            if round <= earlier_round {
                break;
            }
            let block_ref = linked.pop_last().unwrap();
            let Some(block) = self.get_block_header(&block_ref) else {
                panic!("Block Header {:?} should exist in DAG!", block_ref);
            };
            linked.extend(block.ancestors().iter().cloned());
        }
        linked
            .range((
                Included(BlockRef::new(
                    earlier_round,
                    AuthorityIndex::ZERO,
                    BlockHeaderDigest::MIN,
                )),
                Unbounded,
            ))
            .map(|r| {
                self.get_block_header(r)
                    .unwrap_or_else(|| panic!("Block {:?} should exist in DAG!", r))
                    .clone()
            })
            .collect()
    }

    /// Gets the last proposed block from this authority.
    /// If no block is proposed yet, returns Genesis block.
    pub(crate) fn get_last_proposed_block(&self) -> VerifiedBlock {
        if let Some(last) = self.recent_headers_refs_by_authority[self.context.own_index].last() {
            let header = self
                .recent_block_headers
                .get(last)
                .expect("Block header should exist for the most recent blocks");
            let transactions = self
                .recent_transactions
                .get(last)
                .expect("Transactions should exist for the most recent blocks");
            return VerifiedBlock::new(header.clone(), transactions.clone());
        }

        let (_, genesis_block) = self
            .genesis
            .iter()
            .find(|(block_ref, _)| block_ref.author == self.context.own_index)
            .expect("Genesis should be found for authority {authority_index}");
        genesis_block.clone()
    }

    /// Gets the last proposed block header from this authority.
    /// If no block is proposed yet, returns the genesis block header.
    pub(crate) fn get_last_proposed_block_header(&self) -> VerifiedBlockHeader {
        self.get_last_block_header_for_authority(self.context.own_index)
    }

    /// Retrieves the last accepted block from the specified `authority`. If no
    /// block is found in cache then the genesis block is returned as no other
    /// block has been received from that authority.
    pub(crate) fn get_last_block_header_for_authority(
        &self,
        authority: AuthorityIndex,
    ) -> VerifiedBlockHeader {
        if let Some(last) = self.recent_headers_refs_by_authority[authority].last() {
            return self
                .recent_block_headers
                .get(last)
                .expect("Block should be found in recent blocks")
                .clone();
        }

        // if none exists, then fallback to genesis
        let (_, genesis_block) = self
            .genesis
            .iter()
            .find(|(block_ref, _)| block_ref.author == authority)
            .expect("Genesis should be found for authority {authority_index}");
        genesis_block.verified_block_header.clone()
    }

    /// Returns cached recent blocks from the specified authority.
    /// Blocks returned are limited to round >= `start`, and cached.
    /// NOTE: caller should not assume returned blocks are always chained.
    /// "Disconnected" blocks can be returned when there are byzantine blocks,
    /// or when received blocks are not deduped.
    pub(crate) fn get_cached_blocks(
        &self,
        authority: AuthorityIndex,
        start: Round,
    ) -> Vec<VerifiedBlock> {
        let mut blocks = vec![];
        for block_ref in self.recent_headers_refs_by_authority[authority].range((
            Included(BlockRef::new(start, authority, BlockHeaderDigest::MIN)),
            Unbounded,
        )) {
            // TODO: panic if header is missing and return vector of tuples with header and
            //  option<transactions> as not all transactions must exist. Although this is
            //  only used to load own blocks to stream, this should not be problematic.
            if let Some(header) = self.recent_block_headers.get(block_ref) {
                if let Some(transactions) = self.recent_transactions.get(block_ref) {
                    blocks.push(VerifiedBlock::new(header.clone(), transactions.clone()));
                }
            }
        }
        blocks
    }

    /// Returns cached recent block headers from the specified authority.
    /// Block headers returned are limited to round >= `start`, and cached.
    /// NOTE: caller should not assume returned block headers are always
    /// chained. "Disconnected" block headers can be returned when there are
    /// byzantine block headers, or when received block headers are not
    /// deduped.
    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn get_cached_block_headers(
        &self,
        authority: AuthorityIndex,
        start: Round,
    ) -> Vec<VerifiedBlockHeader> {
        let mut block_headers = vec![];
        for block_ref in self.recent_headers_refs_by_authority[authority].range((
            Included(BlockRef::new(start, authority, BlockHeaderDigest::MIN)),
            Unbounded,
        )) {
            let block_header = self
                .recent_block_headers
                .get(block_ref)
                .expect("Block Header should exist in recent blocks headers");
            block_headers.push(block_header.clone());
        }
        block_headers
    }

    // Retrieves the cached block header within the range [start_round, end_round)
    // from a given authority. NOTE: end_round must be greater than
    // GENESIS_ROUND.
    #[cfg(test)]
    pub(crate) fn get_last_cached_block_header_in_range(
        &self,
        authority: AuthorityIndex,
        start_round: Round,
        end_round: Round,
    ) -> Option<VerifiedBlockHeader> {
        if end_round == GENESIS_ROUND {
            panic!(
                "Attempted to retrieve blocks earlier than the genesis round which is impossible"
            );
        }

        let block_ref = self.recent_headers_refs_by_authority[authority]
            .range((
                Included(BlockRef::new(
                    start_round,
                    authority,
                    BlockHeaderDigest::MIN,
                )),
                Excluded(BlockRef::new(
                    end_round,
                    AuthorityIndex::MIN,
                    BlockHeaderDigest::MIN,
                )),
            ))
            .last()?;

        self.recent_block_headers.get(block_ref).cloned()
    }

    /// Returns the last block proposed per authority with `evicted round <
    /// round < end_round`. The method is guaranteed to return results only
    /// when the `end_round` is not earlier of the available cached data for
    /// each authority (evicted round + 1), otherwise the method will panic.
    /// It's the caller's responsibility to ensure that is not requesting for
    /// earlier rounds. In case of equivocation for an authority's last
    /// slot, one block will be returned (the last in order) and the other
    /// equivocating blocks will be returned.
    pub(crate) fn get_last_cached_block_header_per_authority(
        &self,
        end_round: Round,
    ) -> Vec<(VerifiedBlockHeader, Vec<BlockRef>)> {
        // Initialize with the genesis blocks as fallback
        let mut block_headers = self
            .genesis
            .values()
            .map(|b| (**b).clone())
            .collect::<Vec<VerifiedBlockHeader>>();
        let mut equivocating_blocks = vec![vec![]; self.context.committee.size()];

        if end_round == GENESIS_ROUND {
            panic!(
                "Attempted to retrieve blocks earlier than the genesis round which is not possible"
            );
        }

        if end_round == GENESIS_ROUND + 1 {
            return block_headers.into_iter().map(|b| (b, vec![])).collect();
        }

        for (authority_index, block_refs) in
            self.recent_headers_refs_by_authority.iter().enumerate()
        {
            let authority_index = self
                .context
                .committee
                .to_authority_index(authority_index)
                .unwrap();

            let last_evicted_round = self.evicted_rounds[authority_index];
            if end_round.saturating_sub(1) <= last_evicted_round {
                panic!(
                    "Attempted to request for blocks of rounds < {end_round}, when the last evicted round is {last_evicted_round} for authority {authority_index}",
                );
            }

            let block_ref_iter = block_refs
                .range((
                    Included(BlockRef::new(
                        last_evicted_round + 1,
                        authority_index,
                        BlockHeaderDigest::MIN,
                    )),
                    Excluded(BlockRef::new(
                        end_round,
                        authority_index,
                        BlockHeaderDigest::MIN,
                    )),
                ))
                .rev();

            let mut last_round = 0;
            for block_ref in block_ref_iter {
                if last_round == 0 {
                    last_round = block_ref.round;
                    let block_header = self
                        .recent_block_headers
                        .get(block_ref)
                        .expect("Block should exist in recent blocks");
                    block_headers[authority_index] = block_header.clone();
                    continue;
                }
                if block_ref.round < last_round {
                    break;
                }
                equivocating_blocks[authority_index].push(*block_ref);
            }
        }

        block_headers.into_iter().zip(equivocating_blocks).collect()
    }

    /// Checks whether a block header exists in the slot. The method checks only
    /// against the cached data. If the user asks for a slot that is not
    /// within the cached data then a panic is thrown.
    pub(crate) fn contains_cached_block_header_at_slot(&self, slot: Slot) -> bool {
        // Always return true for genesis slots.
        if slot.round == GENESIS_ROUND {
            return true;
        }

        let eviction_round = self.evicted_rounds[slot.authority];
        if slot.round <= eviction_round {
            panic!(
                "{}",
                format!(
                    "Attempted to check for slot {slot} that is <= the last evicted round {eviction_round}"
                )
            );
        }

        let mut result = self.recent_headers_refs_by_authority[slot.authority].range((
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MIN,
            )),
            Included(BlockRef::new(
                slot.round,
                slot.authority,
                BlockHeaderDigest::MAX,
            )),
        ));
        result.next().is_some()
    }

    // TODO: implement for blocks as well
    /// Checks whether the required block headers are in cache, if exist, or
    /// otherwise will check in store. The method is not caching back the
    /// results, so its expensive if keep asking for cache missing block
    /// headers.
    pub(crate) fn contains_block_headers(&self, block_refs: Vec<BlockRef>) -> Vec<bool> {
        let mut exist = vec![false; block_refs.len()];
        let mut missing = Vec::new();

        for (index, block_ref) in block_refs.into_iter().enumerate() {
            let recent_refs = &self.recent_headers_refs_by_authority[block_ref.author];
            if recent_refs.contains(&block_ref) || self.genesis.contains_key(&block_ref) {
                exist[index] = true;
            } else if recent_refs.is_empty() || recent_refs.last().unwrap().round < block_ref.round
            {
                // Optimization: recent_refs contain the most recent blocks known to this
                // authority. If a block ref is not found there and has a higher
                // round, it definitely is missing from this authority and there
                // is no need to check disk.
                exist[index] = false;
            } else {
                missing.push((index, block_ref));
            }
        }

        if missing.is_empty() {
            return exist;
        }

        let missing_refs = missing
            .iter()
            .map(|(_, block_ref)| *block_ref)
            .collect::<Vec<_>>();
        let store_results = self
            .store
            .contains_block_headers(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));
        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["contains_block_headers"])
            .inc();

        for ((index, _), result) in missing.into_iter().zip(store_results.into_iter()) {
            exist[index] = result;
        }

        exist
    }

    pub(crate) fn contains_block_header(&self, block_ref: &BlockRef) -> bool {
        let blocks = self.contains_block_headers(vec![*block_ref]);
        blocks.first().cloned().unwrap()
    }

    /// Checks whether the required transactions are in cache, if exist, or
    /// otherwise will check in store. The method is not caching back the
    /// results, so its expensive if keep asking for cache missing transactions.
    pub(crate) fn contains_transactions(&self, block_refs: Vec<BlockRef>) -> Vec<bool> {
        let mut exist = vec![false; block_refs.len()];
        let mut missing = Vec::new();

        for (index, block_ref) in block_refs.into_iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Allow the caller to handle the invalid genesis ancestor error.
                if self.genesis.contains_key(&block_ref) {
                    exist[index] = true;
                }
                continue;
            }
            if self.recent_transactions.contains_key(&block_ref) {
                exist[index] = true;
            } else {
                missing.push((index, block_ref));
            }
        }

        if missing.is_empty() {
            return exist;
        }

        let missing_refs = missing
            .iter()
            .map(|(_, block_ref)| *block_ref)
            .collect::<Vec<_>>();
        let store_results = self
            .store
            .contains_transactions(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e));
        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["contains_transactions"])
            .inc();

        for ((index, _), result) in missing.into_iter().zip(store_results.into_iter()) {
            exist[index] = result;
        }

        exist
    }

    pub(crate) fn threshold_clock_round(&self) -> Round {
        self.threshold_clock.get_round()
    }

    pub(crate) fn threshold_clock_quorum_ts(&self) -> Instant {
        self.threshold_clock.get_quorum_ts()
    }

    pub(crate) fn highest_accepted_round(&self) -> Round {
        self.highest_accepted_round
    }

    // Buffers a new commit in memory and updates last committed rounds.
    // REQUIRED: must not skip over any commit index.
    pub(crate) fn add_commit(&mut self, commit: TrustedCommit) {
        if let Some(last_commit) = &self.last_commit {
            if commit.index() <= last_commit.index() {
                error!(
                    "New commit index {} <= last commit index {}!",
                    commit.index(),
                    last_commit.index()
                );
                return;
            }
            assert_eq!(commit.index(), last_commit.index() + 1);

            if commit.timestamp_ms() < last_commit.timestamp_ms() {
                panic!(
                    "Commit timestamps do not monotonically increment, prev commit {:?}, new commit {:?}",
                    last_commit, commit
                );
            }
        } else {
            assert_eq!(commit.index(), 1);
        }

        let commit_round_advanced = if let Some(previous_commit) = &self.last_commit {
            previous_commit.round() < commit.round()
        } else {
            true
        };

        self.last_commit = Some(commit.clone());

        if commit_round_advanced {
            let now = std::time::Instant::now();
            if let Some(previous_time) = self.last_commit_round_advancement_time {
                self.context
                    .metrics
                    .node_metrics
                    .commit_round_advancement_interval
                    .observe(now.duration_since(previous_time).as_secs_f64())
            }
            self.last_commit_round_advancement_time = Some(now);
        }

        for block_ref in commit.blocks().iter() {
            self.last_committed_rounds[block_ref.author] = max(
                self.last_committed_rounds[block_ref.author],
                block_ref.round,
            );
        }

        for (i, round) in self.last_committed_rounds.iter().enumerate() {
            let index = self.context.committee.to_authority_index(i).unwrap();
            let hostname = &self.context.committee.authority(index).hostname;
            self.context
                .metrics
                .node_metrics
                .last_committed_authority_round
                .with_label_values(&[hostname])
                .set((*round).into());
        }

        self.pending_commit_votes.push_back(commit.reference());
        self.commits_to_write.push(commit);
    }

    pub(crate) fn add_commit_info(&mut self, reputation_scores: ReputationScores) {
        // We create an empty scoring subdag once reputation scores are calculated.
        // Note: It is okay for this to not be gated by protocol config as the
        // scoring_subdag should be empty in either case at this point.
        assert!(self.scoring_subdag.is_empty());

        let commit_info = CommitInfo {
            committed_rounds: self.last_committed_rounds.clone(),
            reputation_scores,
        };
        let last_commit = self
            .last_commit
            .as_ref()
            .expect("Last commit should already be set.");
        self.commit_info_to_write
            .push((last_commit.reference(), commit_info));
    }

    /// Returns the set of block headers up to a given round that are not known
    /// by the given authority
    #[expect(unused)]
    pub(crate) fn get_past_block_headers_not_known_by_authority(
        &self,
        authority_index: AuthorityIndex,
        round: Round,
    ) -> Vec<BlockRef> {
        let set = &self.block_headers_not_known_by_authority[authority_index.value()];

        // Construct an upper bound (exclusive)
        let upper_bound = BlockRef::new(round, AuthorityIndex::MAX, BlockHeaderDigest::MAX);

        // Take all BlockRefs strictly less than the upper bound
        set.range(..upper_bound).cloned().collect()
    }

    /// Takes a batch of at most MAX_HEADERS_PER_BUNDLE unknown headers for the
    /// given authority, but only from round smaller than
    /// round_upper_bound_exclusive. Marks these headers as known to the
    /// authority.
    pub(crate) fn take_unknown_headers_for_authority(
        &mut self,
        authority_index: AuthorityIndex,
        round_upper_bound_exclusive: Round,
    ) -> Vec<VerifiedBlockHeader> {
        let mut set =
            mem::take(&mut self.block_headers_not_known_by_authority[authority_index.value()]);

        let split_point = {
            let round_bound = BlockRef::new(
                round_upper_bound_exclusive,
                AuthorityIndex::MIN,
                BlockHeaderDigest::MIN,
            );
            let nth_element = set
                .iter()
                .nth(MAX_HEADERS_PER_BUNDLE)
                .map_or(round_bound, |x| *x);
            min(nth_element, round_bound)
        };

        self.block_headers_not_known_by_authority[authority_index.value()] =
            set.split_off(&split_point);
        let mut block_refs: Vec<BlockRef> = vec![];
        for block_ref in set.into_iter() {
            block_refs.push(block_ref);
            let (_, who_knows_given_block) = self.recent_dag_cordial_knowledge
                [block_ref.author.value()]
            .get_mut(&(block_ref.round, block_ref.digest))
            .expect("We expect block ref to be in recent dag cordial knowledge");
            who_knows_given_block.insert(authority_index);
        }
        self.get_block_headers(&block_refs)
            .into_iter()
            .map(|opt| opt.expect("All headers should be in DagState or on disk"))
            .collect()
    }

    pub(crate) fn take_commit_votes(&mut self, limit: usize) -> Vec<CommitVote> {
        let mut votes = Vec::new();
        while !self.pending_commit_votes.is_empty() && votes.len() < limit {
            votes.push(self.pending_commit_votes.pop_front().unwrap());
        }
        votes
    }

    /// Function removes stalled transactions that are older than
    /// "last consume leader round minus MAX_TRANSACTIONS_ACK_DEPTH minus
    /// MAX_LINEARIZER_DEPTH"
    pub(crate) fn evict_transactions(&mut self) {
        let last_solid_leader_round = self.last_available_commit_leader_round;
        if let Some(round) = last_solid_leader_round {
            let min_round: Round =
                round.saturating_sub(MAX_TRANSACTIONS_ACK_DEPTH + MAX_LINEARIZER_DEPTH);

            // Construct a dummy BlockRef with the minimum round to split on.
            // All entries < dummy will be removed.
            let lower_bound =
                BlockRef::new(min_round + 1, AuthorityIndex::ZERO, BlockHeaderDigest::MIN);

            // Remove entries with round < min_round
            self.recent_transactions = self.recent_transactions.split_off(&lower_bound);
        }
    }

    /// Function removes stalled pending acknowledgments that are older than
    /// "current clock round minus MAX_TRANSACTIONS_ACK_DEPTH"
    pub(crate) fn evict_pending_acknowledgments(&mut self) {
        let clock_round = self.threshold_clock_round();
        let min_round: Round = clock_round.saturating_sub(MAX_TRANSACTIONS_ACK_DEPTH);

        // Construct a dummy BlockRef with the minimum round to split on.
        // All entries < dummy will be removed.
        let lower_bound = BlockRef::new(min_round, AuthorityIndex::ZERO, BlockHeaderDigest::MIN);

        // Remove entries with round < min_round
        self.pending_acknowledgments = self.pending_acknowledgments.split_off(&lower_bound);
    }

    /// Evicts old cordial knowledge and pending acknowledgments. It is aligned
    /// with the eviction method, thereby should be called every time the
    /// eviction happens.
    pub(crate) fn evict_cordial_knowledge(&mut self) {
        // === 1. Evict from recent_dag_cordial_knowledge ===
        for (authority_index, map) in self.recent_dag_cordial_knowledge.iter_mut().enumerate() {
            let evict_round = self.evicted_rounds[authority_index];

            // Only keep entries with round > evict_round
            let keep_from = (evict_round + 1, BlockHeaderDigest::MIN);
            *map = map.split_off(&keep_from);
        }
        // === 2. Evict from block_headers_not_known_by_authority ===
        for authority_index in 0..self.context.committee.size() {
            let old_set = &mut self.block_headers_not_known_by_authority[authority_index];
            old_set.retain(|block_ref| block_ref.round > self.evicted_rounds[block_ref.author]);
        }
    }

    /// Adds a block reference to pending acknowledgments.
    pub(crate) fn add_pending_acknowledgment(&mut self, block_ref: BlockRef) {
        self.pending_acknowledgments.insert(block_ref);
    }

    /// Takes at most `limit` acknowledgments from `pending_acknowledgments`,
    /// ensuring they are from rounds below `clock_round`.
    pub(crate) fn take_acknowledgments(&mut self, limit: usize) -> Vec<BlockRef> {
        self.evict_pending_acknowledgments();
        let clock_round = self.threshold_clock_round();
        let mut taken = Vec::with_capacity(limit);
        let mut last_ack = None;

        for ack in self.pending_acknowledgments.iter() {
            if taken.len() >= limit || ack.round >= clock_round {
                last_ack = Some(*ack);
                break;
            }
            taken.push(*ack);
        }

        if let Some(last_ack) = last_ack {
            self.pending_acknowledgments = self.pending_acknowledgments.split_off(&last_ack);
        }

        taken
    }

    /// Index of the last commit.
    pub(crate) fn last_commit_index(&self) -> CommitIndex {
        match &self.last_commit {
            Some(commit) => commit.index(),
            None => 0,
        }
    }

    /// Digest of the last commit.
    pub(crate) fn last_commit_digest(&self) -> CommitDigest {
        match &self.last_commit {
            Some(commit) => commit.digest(),
            None => CommitDigest::MIN,
        }
    }

    /// Timestamp of the last commit.
    pub(crate) fn last_commit_timestamp_ms(&self) -> BlockTimestampMs {
        match &self.last_commit {
            Some(commit) => commit.timestamp_ms(),
            None => 0,
        }
    }

    /// Leader slot of the last commit.
    pub(crate) fn last_commit_leader(&self) -> Slot {
        match &self.last_commit {
            Some(commit) => commit.leader().into(),
            None => self
                .genesis
                .iter()
                .next()
                .map(|(genesis_ref, _)| *genesis_ref)
                .expect("Genesis blocks should always be available.")
                .into(),
        }
    }

    /// Highest round where a block is committed, which is last commit's leader
    /// round.
    pub(crate) fn last_commit_round(&self) -> Round {
        match &self.last_commit {
            Some(commit) => commit.leader().round,
            None => 0,
        }
    }

    /// Last committed round per authority.
    pub(crate) fn last_committed_rounds(&self) -> Vec<Round> {
        self.last_committed_rounds.clone()
    }

    /// After each flush, DagState becomes persisted in storage and it expected
    /// to recover all internal states from storage after restarts.
    pub(crate) fn flush(&mut self) {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["DagState::flush"])
            .start_timer();

        // Take ownership of buffered data efficiently using mem::take
        let transactions = std::mem::take(&mut self.transactions_to_write);
        let block_headers = std::mem::take(&mut self.block_headers_to_write);
        let commits = std::mem::take(&mut self.commits_to_write);
        let commit_info = std::mem::take(&mut self.commit_info_to_write);

        // Early return if there's nothing to flush
        if transactions.is_empty()
            && block_headers.is_empty()
            && commits.is_empty()
            && commit_info.is_empty()
        {
            return;
        }

        debug!(
            "Flushing {} block headers ({}), {} transactions ({}), {} commits ({}) and {} commit info ({}) to storage.",
            block_headers.len(),
            block_headers
                .iter()
                .map(|b| b.reference().to_string())
                .join(","),
            transactions.len(),
            transactions
                .iter()
                .map(|b| b.transactions_commitment().to_string())
                .join(","),
            commits.len(),
            commits.iter().map(|c| c.reference().to_string()).join(","),
            commit_info.len(),
            commit_info
                .iter()
                .map(|(commit_ref, _)| commit_ref.to_string())
                .join(","),
        );

        // Write all buffered data to storage
        self.store
            .write(WriteBatch::new(
                transactions,
                block_headers,
                commits,
                commit_info,
            ))
            .unwrap_or_else(|e| panic!("Failed to write to storage: {:?}", e));

        self.context
            .metrics
            .node_metrics
            .dag_state_store_write_count
            .inc();

        // Clean up old cached data for each authority after flushing, all cached blocks
        // are guaranteed to be persisted.
        for (authority_index, _) in self.context.committee.authorities() {
            let eviction_round = self.calculate_authority_eviction_round(authority_index);
            let recent_refs = &mut self.recent_headers_refs_by_authority[authority_index];

            // Remove old entries from cached maps
            while let Some(block_ref) = recent_refs.first() {
                if block_ref.round <= eviction_round {
                    self.recent_block_headers.remove(block_ref);
                    recent_refs.pop_first();
                } else {
                    break;
                }
            }

            self.evicted_rounds[authority_index] = eviction_round;
        }

        // Clean up old transactions depending on the last solid leader round.
        self.evict_transactions();

        // Clean up old acknowledgments.
        self.evict_pending_acknowledgments();

        // Clean up old cordial knowledge.
        self.evict_cordial_knowledge();

        // Update metrics
        let metrics = &self.context.metrics.node_metrics;
        metrics
            .dag_state_recent_headers
            .set(self.recent_block_headers.len() as i64);
        metrics
            .dag_state_recent_transactions
            .set(self.recent_transactions.len() as i64);
        metrics.dag_state_recent_refs.set(
            self.recent_headers_refs_by_authority
                .iter()
                .map(BTreeSet::len)
                .sum::<usize>() as i64,
        );
    }

    pub(crate) fn recover_last_commit_info(&self) -> Option<(CommitRef, CommitInfo)> {
        self.store
            .read_last_commit_info()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {:?}", e))
    }

    pub(crate) fn add_scoring_subdags(&mut self, scoring_subdags: Vec<SubDagBase>) {
        self.scoring_subdag.add_subdags(scoring_subdags);
    }

    pub(crate) fn clear_scoring_subdag(&mut self) {
        self.scoring_subdag.clear();
    }

    pub(crate) fn scoring_subdags_count(&self) -> usize {
        self.scoring_subdag.scored_subdags_count()
    }

    pub(crate) fn is_scoring_subdag_empty(&self) -> bool {
        self.scoring_subdag.is_empty()
    }

    pub(crate) fn calculate_scoring_subdag_scores(&self) -> ReputationScores {
        self.scoring_subdag.calculate_distributed_vote_scores()
    }

    pub(crate) fn scoring_subdag_commit_range(&self) -> CommitIndex {
        self.scoring_subdag
            .commit_range
            .as_ref()
            .expect("commit range should exist for scoring subdag")
            .end()
    }

    /// The last round that should get evicted after a cache clean up operation.
    /// After this round we are guaranteed to have all the produced blocks
    /// from that authority. For any round that is <= `last_evicted_round`
    /// we don't have such guarantees as out of order blocks might exist.
    fn calculate_authority_eviction_round(&self, authority_index: AuthorityIndex) -> Round {
        let commit_round = self.last_committed_rounds[authority_index];
        Self::eviction_round(commit_round, self.cached_rounds)
    }

    /// Calculates the last eviction round based on the provided `commit_round`.
    /// Any blocks with round <= the evict round have been cleaned up.
    fn eviction_round(commit_round: Round, cached_rounds: Round) -> Round {
        commit_round.saturating_sub(cached_rounds)
    }

    /// Detects and returns the blocks of the round that forms the last quorum.
    /// The method will return the quorum even if that's genesis.
    #[cfg(test)]
    pub(crate) fn last_quorum(&self) -> Vec<VerifiedBlockHeader> {
        // the quorum should exist either on the highest accepted round or the one
        // before. If we fail to detect a quorum then it means that our DAG has
        // advanced with missing causal history.
        for round in
            (self.highest_accepted_round.saturating_sub(1)..=self.highest_accepted_round).rev()
        {
            if round == GENESIS_ROUND {
                return self.genesis_block_headers();
            }
            use crate::stake_aggregator::{QuorumThreshold, StakeAggregator};
            let mut quorum = StakeAggregator::<QuorumThreshold>::new();

            // Since the minimum wave length is 3 we expect to find a quorum in the
            // uncommitted rounds.
            let blocks = self.get_uncommitted_blocks_at_round(round);
            for block in &blocks {
                if quorum.add(block.author(), &self.context.committee) {
                    return blocks;
                }
            }
        }

        panic!("Fatal error, no quorum has been detected in our DAG on the last two rounds.");
    }

    #[expect(dead_code)]
    pub(crate) fn genesis_blocks(&self) -> Vec<VerifiedBlock> {
        self.genesis.values().cloned().collect()
    }

    #[cfg(test)]
    pub(crate) fn genesis_block_headers(&self) -> Vec<VerifiedBlockHeader> {
        self.genesis
            .values()
            .map(|b| (**b).clone())
            .collect::<Vec<VerifiedBlockHeader>>()
    }

    #[cfg(test)]
    pub(crate) fn set_last_commit(&mut self, commit: TrustedCommit) {
        self.last_commit = Some(commit);
    }

    #[cfg(test)]
    pub(crate) fn set_pending_acknowledgments(&mut self, acknowledgments: Vec<BlockRef>) {
        self.pending_acknowledgments = acknowledgments.into_iter().collect::<BTreeSet<_>>();
    }
}
#[cfg(test)]
mod test {
    use std::vec;

    use parking_lot::RwLock;
    use rstest::rstest;

    use super::*;
    use crate::{
        block_header::{
            BlockHeaderDigest, BlockRef, BlockTimestampMs, TestBlockHeader, VerifiedBlockHeader,
            genesis_block_headers,
        },
        storage::{WriteBatch, mem_store::MemStore},
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
    };

    // TODO: create similar test for get_block
    #[tokio::test]
    async fn test_get_block_header() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());
        let own_index = AuthorityIndex::new_for_test(0);

        // Populate test blocks for round 1 ~ 10, authorities 0 ~ 2.
        let num_rounds: u32 = 10;
        let non_existent_round: u32 = 100;
        let num_authorities: u32 = 3;
        let num_blocks_per_slot: usize = 3;
        let mut blocks = BTreeMap::new();
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                // Create 3 blocks per slot, with different timestamps and digests.
                let base_ts = round as BlockTimestampMs * 1000;
                for timestamp in base_ts..base_ts + num_blocks_per_slot as u64 {
                    let block = VerifiedBlockHeader::new_for_test(
                        TestBlockHeader::new(round, author)
                            .set_timestamp_ms(timestamp)
                            .build(),
                    );
                    dag_state.accept_block_header(block.clone());
                    blocks.insert(block.reference(), block);

                    // Only write one block per slot for own index
                    if AuthorityIndex::new_for_test(author) == own_index {
                        break;
                    }
                }
            }
        }

        // Check uncommitted blocks that exist.
        for (r, block) in &blocks {
            assert_eq!(&dag_state.get_block_header(r).unwrap(), block);
        }

        // Check uncommitted blocks that do not exist.
        let last_ref = blocks.keys().last().unwrap();
        assert!(
            dag_state
                .get_block(&BlockRef::new(
                    last_ref.round,
                    last_ref.author,
                    BlockHeaderDigest::MIN
                ))
                .is_none()
        );

        // Check slots with uncommitted blocks.
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let slot = Slot::new(
                    round,
                    context
                        .committee
                        .to_authority_index(author as usize)
                        .unwrap(),
                );
                let blocks = dag_state.get_uncommitted_blocks_at_slot(slot);

                // We only write one block per slot for own index
                if AuthorityIndex::new_for_test(author) == own_index {
                    assert_eq!(blocks.len(), 1);
                } else {
                    assert_eq!(blocks.len(), num_blocks_per_slot);
                }

                for b in blocks {
                    assert_eq!(b.round(), round);
                    assert_eq!(
                        b.author(),
                        context
                            .committee
                            .to_authority_index(author as usize)
                            .unwrap()
                    );
                }
            }
        }

        // Check slots without uncommitted blocks.
        let slot = Slot::new(non_existent_round, AuthorityIndex::ZERO);
        assert!(dag_state.get_uncommitted_blocks_at_slot(slot).is_empty());

        // Check rounds with uncommitted blocks.
        for round in 1..=num_rounds {
            let blocks = dag_state.get_uncommitted_blocks_at_round(round);
            // Expect 3 blocks per authority except for own authority which should
            // have 1 block.
            assert_eq!(
                blocks.len(),
                (num_authorities - 1) as usize * num_blocks_per_slot + 1
            );
            for b in blocks {
                assert_eq!(b.round(), round);
            }
        }

        // Check rounds without uncommitted blocks.
        assert!(
            dag_state
                .get_uncommitted_blocks_at_round(non_existent_round)
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_ancestors_at_uncommitted_round() {
        // Initialize DagState.
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Populate DagState.

        // Round 10 refs will not have their blocks in DagState.
        let round_10_refs: Vec<_> = (0..4)
            .map(|a| {
                VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(10, a).set_timestamp_ms(1000).build(),
                )
                .reference()
            })
            .collect();

        // Round 11 blocks.
        let round_11 = vec![
            // This will connect to round 12.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 0)
                    .set_timestamp_ms(1100)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // Slot(11, 1) has 3 blocks.
            // This will connect to round 12.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 1)
                    .set_timestamp_ms(1110)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // This will connect to round 13.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 1)
                    .set_timestamp_ms(1111)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // This will not connect to any block.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 1)
                    .set_timestamp_ms(1112)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // This will not connect to any block.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 2)
                    .set_timestamp_ms(1120)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // This will connect to round 12.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 3)
                    .set_timestamp_ms(1130)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
        ];

        // Round 12 blocks.
        let ancestors_for_round_12 = vec![
            round_11[0].reference(),
            round_11[1].reference(),
            round_11[5].reference(),
        ];
        let round_12 = vec![
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 0)
                    .set_timestamp_ms(1200)
                    .set_ancestors(ancestors_for_round_12.clone())
                    .build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 2)
                    .set_timestamp_ms(1220)
                    .set_ancestors(ancestors_for_round_12.clone())
                    .build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 3)
                    .set_timestamp_ms(1230)
                    .set_ancestors(ancestors_for_round_12.clone())
                    .build(),
            ),
        ];

        // Round 13 blocks.
        let ancestors_for_round_13 = vec![
            round_12[0].reference(),
            round_12[1].reference(),
            round_12[2].reference(),
            round_11[2].reference(),
        ];
        let round_13 = vec![
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 1)
                    .set_timestamp_ms(1300)
                    .set_ancestors(ancestors_for_round_13.clone())
                    .build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 2)
                    .set_timestamp_ms(1320)
                    .set_ancestors(ancestors_for_round_13.clone())
                    .build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(12, 3)
                    .set_timestamp_ms(1330)
                    .set_ancestors(ancestors_for_round_13.clone())
                    .build(),
            ),
        ];

        // Round 14 anchor block.
        let ancestors_for_round_14 = round_13.iter().map(|b| b.reference()).collect();
        let anchor = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(14, 1)
                .set_timestamp_ms(1410)
                .set_ancestors(ancestors_for_round_14)
                .build(),
        );

        // Add all blocks (at and above round 11) to DagState.
        for b in round_11
            .iter()
            .chain(round_12.iter())
            .chain(round_13.iter())
            .chain([anchor.clone()].iter())
        {
            dag_state.accept_block_header(b.clone());
        }

        // Check ancestors connected to anchor.
        let ancestors = dag_state.ancestors_at_round(&anchor, 11);
        let mut ancestors_refs: Vec<BlockRef> = ancestors.iter().map(|b| b.reference()).collect();
        ancestors_refs.sort();
        let mut expected_refs = vec![
            round_11[0].reference(),
            round_11[1].reference(),
            round_11[2].reference(),
            round_11[5].reference(),
        ];
        expected_refs.sort(); // we need to sort as blocks with same author and round of round 11 (position 1
        // & 2) might not be in right lexicographical order.
        assert_eq!(
            ancestors_refs, expected_refs,
            "Expected round 11 ancestors: {:?}. Got: {:?}",
            expected_refs, ancestors_refs
        );
    }

    // TODO: make similar test for blocks
    #[tokio::test]
    async fn test_contains_block_headers_in_cache_or_store() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u32 = 4;
        let mut block_headers = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block_header =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                block_headers.push(block_header);
            }
        }

        // Now write in store the block headers from first 4 rounds and the rest to the
        // dag state
        block_headers.clone().into_iter().for_each(|block_header| {
            if block_header.round() <= 4 {
                store
                    .write(WriteBatch::default().block_headers(vec![block_header]))
                    .unwrap();
            } else {
                dag_state.accept_block_headers(vec![block_header]);
            }
        });

        // Now when trying to query whether we have all the blocks, we should
        // successfully retrieve a positive answer where the blocks of first 4
        // round should be found in DagState and the rest in store.
        let mut block_refs = block_headers
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();
        let result = dag_state.contains_block_headers(block_refs.clone());

        // Ensure everything is found
        let mut expected = vec![true; (num_rounds * num_authorities) as usize];
        assert_eq!(result, expected);

        // Now try to ask also for one block ref that is neither in cache nor in store
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
        );
        let result = dag_state.contains_block_headers(block_refs.clone());

        // Then all should be found apart from the last one
        expected.insert(3, false);
        assert_eq!(result, expected.clone());
    }

    #[tokio::test]
    async fn test_contains_cached_block_header_at_slot() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let num_authorities: u32 = 4;
        let (mut context, _) = Context::new_for_test(num_authorities as usize);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let mut blocks = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                blocks.push(block.clone());
                dag_state.accept_block_header(block);
            }
        }

        // Query for genesis round 0, genesis blocks should be returned
        for (author, _) in context.committee.authorities() {
            assert!(
                dag_state.contains_cached_block_header_at_slot(Slot::new(GENESIS_ROUND, author)),
                "Genesis should always be found"
            );
        }

        // Now when trying to query whether we have all the blocks, we should
        // successfully retrieve a positive answer where the blocks of first 4
        // round should be found in DagState and the rest in store.
        let mut block_refs = blocks
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();

        for block_ref in block_refs.clone() {
            let slot = block_ref.into();
            let found = dag_state.contains_cached_block_header_at_slot(slot);
            assert!(found, "A block should be found at slot {}", slot);
        }

        // Now try to ask also for one block ref that is not in cache
        // Then all should be found apart from the last one
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
        );
        let mut expected = vec![true; (num_rounds * num_authorities) as usize];
        expected.insert(3, false);

        // Attempt to check the same for via the contains slot method
        for block_ref in block_refs {
            let slot = block_ref.into();
            let found = dag_state.contains_cached_block_header_at_slot(slot);

            assert_eq!(expected.remove(0), found);
        }
    }

    #[tokio::test]
    #[should_panic(
        expected = "Attempted to check for slot [0]8 that is <= the last evicted round 8"
    )]
    async fn test_contains_cached_block_at_slot_panics_when_ask_out_of_range() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10 for authority 0
        let mut blocks = Vec::new();
        for round in 1..=10 {
            let block = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build());
            blocks.push(block.clone());
            dag_state.accept_block_header(block);
        }

        // Now add a commit to trigger an eviction
        dag_state.add_commit(TrustedCommit::new_for_test(
            1 as CommitIndex,
            CommitDigest::MIN,
            0,
            blocks.last().unwrap().reference(),
            blocks
                .into_iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>(),
            vec![],
        ));

        dag_state.flush();

        // When trying to request for authority 0 at block slot 8 it should panic, as
        // anything that is <= commit_round - cached_rounds = 10 - 2 = 8 should
        // be evicted
        let _ = dag_state
            .contains_cached_block_header_at_slot(Slot::new(8, AuthorityIndex::new_for_test(0)));
    }

    #[tokio::test]
    async fn test_get_blocks_in_cache_or_store() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u32 = 4;
        let mut block_headers = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                block_headers.push(block);
            }
        }

        // Now write in store the blocks from first 4 rounds and the rest to the dag
        // state
        block_headers.clone().into_iter().for_each(|block_header| {
            if block_header.round() <= 4 {
                store
                    .write(WriteBatch::default().block_headers(vec![block_header]))
                    .unwrap();
            } else {
                dag_state.accept_block_headers(vec![block_header]);
            }
        });

        // Now when trying to query whether we have all the blocks, we should
        // successfully retrieve a positive answer where the blocks of first 4
        // round should be found in DagState and the rest in store.
        let mut block_refs = block_headers
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();
        let result = dag_state.get_block_headers(&block_refs);

        let mut expected = block_headers
            .into_iter()
            .map(Some)
            .collect::<Vec<Option<VerifiedBlockHeader>>>();

        // Ensure everything is found
        assert_eq!(result, expected.clone());

        // Now try to ask also for one block ref that is neither in cache nor in store
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
        );
        let result = dag_state.get_block_headers(&block_refs);

        // Then all should be found apart from the last one
        expected.insert(3, None);
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_flush_and_recovery() {
        telemetry_subscribers::init_for_testing();
        let num_authorities: u32 = 4;
        let (context, _) = Context::new_for_test(num_authorities as usize);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks and commits for round 1 ~ 10
        let num_rounds: u32 = 10;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=num_rounds).build();
        let mut commits = vec![];
        for (_subdag, commit) in dag_builder.get_sub_dag_and_commits(1..=num_rounds) {
            commits.push(commit);
        }

        // Add the blocks from first 5 rounds and first 5 commits to the dag state
        let temp_commits = commits.split_off(5);
        dag_state.accept_block_headers(dag_builder.block_headers(1..=5));
        for commit in commits.clone() {
            dag_state.add_commit(commit);
        }

        // Flush the dag state
        dag_state.flush();

        // Add the rest of the blocks and commits to the dag state
        dag_state.accept_block_headers(dag_builder.block_headers(6..=num_rounds));
        for commit in temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // All blocks should be found in DagState.
        let all_block_headers = dag_builder.block_headers(6..=num_rounds);
        let block_refs = all_block_headers
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();
        let result = dag_state
            .get_block_headers(&block_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, all_block_headers);

        // Last commit index should be 10.
        assert_eq!(dag_state.last_commit_index(), 10);
        assert_eq!(
            dag_state.last_committed_rounds(),
            dag_builder.last_committed_rounds.clone()
        );

        // Destroy the dag state.
        drop(dag_state);

        // Recover the state from the store
        let dag_state = DagState::new(context.clone(), store.clone());

        // Blocks of first 5 rounds should be found in DagState.
        let block_headers = dag_builder.block_headers(1..=5);
        let block_refs = block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();
        let result = dag_state
            .get_block_headers(&block_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, block_headers);

        // Blocks above round 5 should not be in DagState, because they are not flushed.
        let missing_blocks = dag_builder.block_headers(6..=num_rounds);
        let block_refs = missing_blocks
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();
        let retrieved_blocks = dag_state
            .get_block_headers(&block_refs)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(retrieved_blocks.is_empty());

        // Last commit index should be 5.
        assert_eq!(dag_state.last_commit_index(), 5);

        // This is the last_commit_rounds of the first 5 commits that were flushed
        let expected_last_committed_rounds = vec![4, 5, 4, 4];
        assert_eq!(
            dag_state.last_committed_rounds(),
            expected_last_committed_rounds
        );
        // Unscored subdags will be recovered based on the flushed commits and no commit
        // info
        assert_eq!(dag_state.scoring_subdags_count(), 5);
    }

    #[tokio::test]
    async fn test_get_cached_block_headers() {
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = 5;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create no blocks for authority 0
        // Create one block (round 10) for authority 1
        // Create two blocks (rounds 10,11) for authority 2
        // Create three blocks (rounds 10,11,12) for authority 3
        let mut all_blocks = Vec::new();
        for author in 1..=3 {
            for round in 10..(10 + author) {
                let block =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                all_blocks.push(block.clone());
                dag_state.accept_block_header(block);
            }
        }

        let cached_block_headers =
            dag_state.get_cached_block_headers(context.committee.to_authority_index(0).unwrap(), 0);
        assert!(cached_block_headers.is_empty());

        let cached_block_headers = dag_state
            .get_cached_block_headers(context.committee.to_authority_index(1).unwrap(), 10);
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 10);

        let cached_block_headers = dag_state
            .get_cached_block_headers(context.committee.to_authority_index(2).unwrap(), 10);
        assert_eq!(cached_block_headers.len(), 2);
        assert_eq!(cached_block_headers[0].round(), 10);
        assert_eq!(cached_block_headers[1].round(), 11);

        let cached_block_headers = dag_state
            .get_cached_block_headers(context.committee.to_authority_index(2).unwrap(), 11);
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 11);

        let cached_block_headers = dag_state
            .get_cached_block_headers(context.committee.to_authority_index(3).unwrap(), 10);
        assert_eq!(cached_block_headers.len(), 3);
        assert_eq!(cached_block_headers[0].round(), 10);
        assert_eq!(cached_block_headers[1].round(), 11);
        assert_eq!(cached_block_headers[2].round(), 12);

        let cached_block_headers = dag_state
            .get_cached_block_headers(context.committee.to_authority_index(3).unwrap(), 12);
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 12);
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_last_cached_block() {
        // GIVEN
        const CACHED_ROUNDS: Round = 2;
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create no blocks for authority 0
        // Create one block (round 1) for authority 1
        // Create two blocks (rounds 1,2) for authority 2
        // Create three blocks (rounds 1,2,3) for authority 3
        let dag_str = "DAG {
            Round 0 : { 4 },
            Round 1 : {
                B -> [*],
                C -> [*],
                D -> [*],
            },
            Round 2 : {
                C -> [*],
                D -> [*],
            },
            Round 3 : {
                D -> [*],
            },
        }";

        let dag_builder = parse_dag(dag_str).expect("Invalid dag");

        // Add equivocating block for round 2 authority 3
        let block = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(2, 2).build());

        // Accept all blocks
        for block_header in dag_builder
            .all_block_headers()
            .into_iter()
            .chain(std::iter::once(block))
        {
            dag_state.accept_block_header(block_header);
        }

        dag_state.add_commit(TrustedCommit::new_for_test(
            1 as CommitIndex,
            CommitDigest::MIN,
            context.clock.timestamp_utc_ms(),
            dag_builder.leader_block(3).unwrap().reference(),
            vec![],
            vec![],
        ));

        // WHEN search for the latest blocks
        let end_round = 4;
        let expected_rounds = vec![0, 1, 2, 3];
        let expected_excluded_and_equivocating_blocks = vec![0, 0, 1, 0];
        // THEN
        let last_block_headers = dag_state.get_last_cached_block_header_per_authority(end_round);
        assert_eq!(
            last_block_headers
                .iter()
                .map(|b| b.0.round())
                .collect::<Vec<_>>(),
            expected_rounds
        );
        assert_eq!(
            last_block_headers
                .iter()
                .map(|b| b.1.len())
                .collect::<Vec<_>>(),
            expected_excluded_and_equivocating_blocks
        );

        // THEN
        for (i, expected_round) in expected_rounds.iter().enumerate() {
            let round = dag_state
                .get_last_cached_block_header_in_range(
                    context.committee.to_authority_index(i).unwrap(),
                    0,
                    end_round,
                )
                .map(|b| b.round())
                .unwrap_or_default();
            assert_eq!(round, *expected_round, "Authority {i}");
        }

        // WHEN starting from round 2
        let start_round = 2;
        let expected_rounds = [0, 0, 2, 3];

        // THEN
        for (i, expected_round) in expected_rounds.iter().enumerate() {
            let round = dag_state
                .get_last_cached_block_header_in_range(
                    context.committee.to_authority_index(i).unwrap(),
                    start_round,
                    end_round,
                )
                .map(|b| b.round())
                .unwrap_or_default();
            assert_eq!(round, *expected_round, "Authority {i}");
        }

        // WHEN we flush the DagState - after adding a
        // commit with all the blocks, we expect this to trigger a clean up in
        // the internal cache. That will keep the all the blocks with rounds >=
        // authority_commit_round - CACHED_ROUND.
        dag_state.flush();

        // AND we request before round 3
        let end_round = 3;
        let expected_rounds = vec![0, 1, 2, 2];

        // THEN
        let last_block_headers = dag_state.get_last_cached_block_header_per_authority(end_round);
        assert_eq!(
            last_block_headers
                .iter()
                .map(|b| b.0.round())
                .collect::<Vec<_>>(),
            expected_rounds
        );

        // THEN
        for (i, expected_round) in expected_rounds.iter().enumerate() {
            let round = dag_state
                .get_last_cached_block_header_in_range(
                    context.committee.to_authority_index(i).unwrap(),
                    0,
                    end_round,
                )
                .map(|b| b.round())
                .unwrap_or_default();
            assert_eq!(round, *expected_round, "Authority {i}");
        }
    }

    #[tokio::test]
    #[should_panic(
        expected = "Attempted to request for blocks of rounds < 2, when the last evicted round is 1 for authority [2]"
    )]
    async fn test_get_cached_last_block_per_authority_requesting_out_of_round_range() {
        // GIVEN
        const CACHED_ROUNDS: Round = 1;
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create no blocks for authority 0
        // Create one block (round 1) for authority 1
        // Create two blocks (rounds 1,2) for authority 2
        // Create three blocks (rounds 1,2,3) for authority 3
        let mut all_blocks = Vec::new();
        for author in 1..=3 {
            for round in 1..=author {
                let block =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                all_blocks.push(block.clone());
                dag_state.accept_block_header(block);
            }
        }

        dag_state.add_commit(TrustedCommit::new_for_test(
            1 as CommitIndex,
            CommitDigest::MIN,
            0,
            all_blocks.last().unwrap().reference(),
            all_blocks
                .into_iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>(),
            vec![],
        ));

        // Flush the store so we keep in memory only the last 1 round from the last
        // commit for each authority.
        dag_state.flush();

        // THEN the method should panic, as some authorities have already evicted rounds
        // <= round 2
        let end_round = 2;
        dag_state.get_last_cached_block_header_per_authority(end_round);
    }

    #[tokio::test]
    async fn test_last_quorum() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        // WHEN no blocks exist then genesis should be returned
        {
            let genesis = genesis_block_headers(context.clone());

            assert_eq!(dag_state.read().last_quorum(), genesis);
        }

        // WHEN a fully connected DAG up to round 4 is created, then round 4 blocks
        // should be returned as quorum
        {
            let mut dag_builder = DagBuilder::new(context.clone());
            dag_builder
                .layers(1..=4)
                .build()
                .persist_layers(dag_state.clone());
            let round_4_block_headers: Vec<_> = dag_builder
                .block_headers(4..=4)
                .into_iter()
                .map(|block| block.reference())
                .collect();

            let last_quorum = dag_state.read().last_quorum();

            assert_eq!(
                last_quorum
                    .into_iter()
                    .map(|block_header| block_header.reference())
                    .collect::<Vec<_>>(),
                round_4_block_headers
            );
        }

        // WHEN adding one more block at round 5, still round 4 should be returned as
        // quorum
        {
            let block_header =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 0).build());
            dag_state.write().accept_block_header(block_header);

            let round_4_block_headers = dag_state.read().get_uncommitted_blocks_at_round(4);

            let last_quorum = dag_state.read().last_quorum();

            assert_eq!(last_quorum, round_4_block_headers);
        }
    }

    #[tokio::test]
    async fn test_last_block_for_authority() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        // WHEN no blocks exist then genesis should be returned
        {
            let genesis = genesis_block_headers(context.clone());
            let my_genesis = genesis
                .into_iter()
                .find(|block| block.author() == context.own_index)
                .unwrap();

            assert_eq!(
                dag_state.read().get_last_proposed_block_header(),
                my_genesis
            );
        }

        // WHEN adding some blocks for authorities, only the last ones should be
        // returned
        {
            // add blocks up to round 4
            let mut dag_builder = DagBuilder::new(context.clone());
            dag_builder
                .layers(1..=4)
                .build()
                .persist_layers(dag_state.clone());

            // add block 5 for authority 0
            let block = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 0).build());
            dag_state.write().accept_block_header(block);

            let block = dag_state
                .read()
                .get_last_block_header_for_authority(AuthorityIndex::new_for_test(0));
            assert_eq!(block.round(), 5);

            for (authority_index, _) in context.committee.authorities() {
                let block = dag_state
                    .read()
                    .get_last_block_header_for_authority(authority_index);

                if authority_index.value() == 0 {
                    assert_eq!(block.round(), 5);
                } else {
                    assert_eq!(block.round(), 4);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_contains_transactions() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u32 = 4;
        let mut blocks = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block =
                    VerifiedBlock::new_for_test(TestBlockHeader::new(round, author).build());
                blocks.push(block);
            }
        }

        // Now write in store the transactions from first 4 rounds and the rest to the
        // dag state
        blocks.clone().into_iter().for_each(|block| {
            if block.round() <= 4 {
                store
                    .write(
                        WriteBatch::default()
                            .transactions(vec![block.verified_transactions.clone()]),
                    )
                    .unwrap();
            } else {
                dag_state.add_transactions(block.verified_transactions.clone());
            }
        });

        // Now when trying to query whether we have all the transactions, we should
        // successfully retrieve a positive answer where the transactions of first 4
        // round should be found in store and the rest in DagState.
        let mut block_refs = blocks
            .iter()
            .map(|block| block.reference())
            .collect::<Vec<_>>();
        let result = dag_state.contains_transactions(block_refs.clone());

        // Ensure everything is found
        let mut expected = vec![true; (num_rounds * num_authorities) as usize];
        assert_eq!(result, expected);

        // Now try to ask also for one block ref that is neither in cache nor in store
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
        );
        let result = dag_state.contains_transactions(block_refs);

        // Ensure everything is found except the one we just added
        expected.insert(3, false);
        assert_eq!(result, expected);
    }
}
