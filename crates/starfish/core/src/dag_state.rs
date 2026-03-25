// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::{max, min},
    collections::{BTreeMap, BTreeSet, VecDeque},
    mem,
    ops::Bound::{Excluded, Included, Unbounded},
    panic,
    sync::Arc,
    vec,
};

use bytes::Bytes;
use iota_metrics::monitored_mpsc::Sender;
use itertools::Itertools as _;
use starfish_config::AuthorityIndex;
use tokio::{
    sync::{mpsc::error::TrySendError, watch},
    time::Instant,
};
use tracing::{debug, error, info, trace, warn};

use crate::{
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, Slot,
        TransactionsCommitment, VerifiedBlock, VerifiedBlockHeader, VerifiedOwnShard,
        VerifiedTransactions, genesis_blocks,
    },
    commit::{
        CommitAPI as _, CommitDigest, CommitIndex, CommitInfo, CommitRef, CommitVote,
        GENESIS_COMMIT_INDEX, SubDagBase, TrustedCommit, load_pending_subdag_from_store,
    },
    context::Context,
    cordial_knowledge::CordialKnowledgeMessage,
    leader_scoring::{ReputationScores, ScoringSubdag},
    storage::{Store, WriteBatch},
    threshold_clock::ThresholdClock,
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _, TransactionRef},
};

/// Represents the source from which data (block headers or transactions) was
/// received and added to the DAG state. Used for metrics tracking and
/// debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DataSource {
    // Transaction-specific sources
    /// Transactions received via the transaction synchronizer component.
    /// This synchronizer periodically fetches missing transactions to ensure
    /// nodes stay up-to-date.
    TransactionSynchronizer,

    /// Transactions reconstructed from erasure-coded shards.
    /// Used when full transaction data isn't available, but enough shards
    /// have been collected to reconstruct it.
    ShardReconstructor,

    // Block header-specific sources
    /// Block headers received in bundles via block bundle streaming.
    BlockBundleStream,

    /// Block headers fetched by the live/periodic header synchronizer
    /// component.
    HeaderSynchronizer,

    /// Block headers loaded from persistent storage during node recovery.
    Recover,

    // Shared sources (used for both block headers and transactions)
    /// Block created by this node itself. Used when accepting our own
    /// newly-created block into the DAG before broadcasting it.
    OwnBlock,

    /// Data received via block streaming from peers in the network.
    /// This is the primary method for receiving real-time blocks and
    /// transactions as they're created.
    BlockStreaming,

    /// Data received via commit synchronization. Block headers and transactions
    /// are fetched for all the committed blocks in synced commits.
    CommitSyncer,

    /// Transactions received via fast commit synchronization.
    FastCommitSyncer,

    /// Data added during testing.
    /// Only used in test code.
    #[cfg(test)]
    Test,
}

impl DataSource {
    /// Returns the string label used for metrics reporting.
    /// This ensures consistency with existing metrics that may be monitored.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            DataSource::TransactionSynchronizer => "Transactions synchronizer",
            DataSource::ShardReconstructor => "Shard reconstructor",
            DataSource::BlockBundleStream => "Block headers in streaming",
            DataSource::HeaderSynchronizer => "Header synchronizer",
            DataSource::Recover => "Recover",
            DataSource::OwnBlock => "Own block",
            DataSource::BlockStreaming => "Block streaming",
            DataSource::CommitSyncer => "Commit syncer",
            DataSource::FastCommitSyncer => "Fast commit syncer",
            #[cfg(test)]
            DataSource::Test => "Test",
        }
    }
}

impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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

    /// The genesis blocks
    genesis: BTreeMap<BlockRef, VerifiedBlock>,

    /// Contains recent block headers within CACHED_ROUNDS from the last
    /// traversed round per authority. Note: all uncommitted block headers
    /// are kept in memory.
    recent_block_headers: BTreeMap<BlockRef, VerifiedBlockHeader>,

    /// Contains recent verified transactions per authority. To access a
    /// transaction with a given transaction_ref, one needs to read first the
    /// entry with index transaction_ref.author. Evicted using the minimum
    /// between GC round for the last solid leader round evicted rounds by
    /// authority.
    recent_transactions_by_authority: Vec<BTreeMap<GenericTransactionRef, VerifiedTransactions>>,
    /// Contains recent own serialized shards with their Merkle proofs per
    /// authority. To access own shard for a given transaction_ref, one
    /// needs to read first the entry with index transaction_ref.author.
    /// Eviction is aligned with headers
    recent_shards_by_authority: Vec<BTreeMap<GenericTransactionRef, Bytes>>,
    /// Indexes recent block headers refs by their authorities.
    /// Vec position corresponds to the authority index.
    recent_headers_refs_by_authority: Vec<BTreeSet<BlockRef>>,

    /// Maps (round, transactions_commitment) -> BlockHeaderDigest per
    /// authority. Used to look up block digests from TransactionRef
    /// components. Evicted based on solid commits, same as transactions.
    tx_ref_to_block_digest_by_authority:
        Vec<BTreeMap<(Round, TransactionsCommitment), BlockHeaderDigest>>,

    /// Keeps track of the threshold clock for proposing blocks.
    threshold_clock: ThresholdClock,

    /// Keeps track of the highest round that has been evicted for each
    /// authority. Any block header that are of round <= evict_round should
    /// be considered evicted, and if any exist we should not consider the
    /// causally complete in the order they appear. The `evicted_rounds`
    /// size should be the same as the committee size.
    evicted_rounds: Vec<Round>,

    /// Highest round of blocks accepted.
    highest_accepted_round: Round,

    /// Last pending consensus commit of the dag.
    last_commit: Option<TrustedCommit>,

    /// Last wall time when commit round advanced. Does not persist across
    /// restarts.
    last_commit_round_advancement_time: Option<std::time::Instant>,

    /// The last solid SubDagBase - a commit where all transactions are locally
    /// available. Does not persist across restarts and after recovery.
    /// All transactions below this round minus MAX_TRANSACTIONS_ACK_DEPTH
    /// (protocol_config.gc_depth) minus MAX_LINEARIZER_DEPTH
    /// (protocol_config.gc_depth) are evicted from memory.
    /// Used for GC calculations and as a starting point for fast sync.
    last_solid_subdag_base: Option<SubDagBase>,

    /// Rounds for latest blocks traversed by linearizer per authority.
    last_committed_rounds: Vec<Round>,

    /// The committed subdags that have been scored but scores have not been
    /// used for leader schedule yet.
    scoring_subdag: ScoringSubdag,

    /// Commit votes pending to be included in new blocks.
    pending_commit_votes: VecDeque<CommitVote>,

    /// Acknowledgments pending to be included in new blocks. These represent
    /// votes indicating the availability of transaction data from the
    /// corresponding blocks
    pending_acknowledgments: BTreeSet<BlockRef>,

    /// Transactions to be flushed to storage.
    transactions_to_write: Vec<VerifiedTransactions>,
    block_headers_to_write: Vec<VerifiedBlockHeader>,
    commits_to_write: Vec<TrustedCommit>,

    /// Voting block headers to be flushed to storage. These are block headers
    /// received during fast sync that contain commit votes used to certify
    /// commits.
    voting_block_headers_to_write: Vec<VerifiedBlockHeader>,

    /// Fast sync ongoing flag to be flushed to storage.
    fast_sync_ongoing_flag_to_write: Option<bool>,

    /// Buffer the reputation scores & last_committed_rounds to be flushed with
    /// the next dag state flush. This is okay because we can recover
    /// reputation scores & last_committed_rounds from the commits as
    /// needed.
    /// The index in CommitRef correspond to the first index of the next
    /// scheduler window, while the reputation scores in CommitInfoare for
    /// the previous window.
    commit_info_to_write: Vec<(CommitRef, CommitInfo)>,

    /// Persistent storage for blocks, commits and other consensus data.
    store: Arc<dyn Store>,

    /// The number of cached rounds
    cached_rounds: Round,

    /// Cordial Knowledge senders (main updates, eviction rounds).
    cordial_knowledge_senders: Option<(Sender<CordialKnowledgeMessage>, watch::Sender<Vec<Round>>)>,
}

impl DagState {
    /// Initializes DagState from storage.
    pub(crate) fn new(context: Arc<Context>, store: Arc<dyn Store>) -> Self {
        let cached_rounds = context.parameters.dag_state_cached_rounds as Round;
        let num_authorities = context.committee.size();

        let genesis = genesis_blocks(&context)
            .into_iter()
            .map(|block| (block.reference(), block))
            .collect();

        let threshold_clock = ThresholdClock::new(1, context.clone());

        let last_commit = store
            .read_last_commit()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));

        let commit_info = store
            .read_last_commit_info()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));
        let (mut last_committed_rounds, commit_recovery_start_index) =
            if let Some((commit_ref, commit_info)) = commit_info {
                tracing::info!("Recovering committed state from {commit_ref} {commit_info:?}");
                let range_end = commit_info.reputation_scores.commit_range.end();
                let recovery_start = if range_end == GENESIS_COMMIT_INDEX {
                    commit_ref.index.saturating_add(1)
                } else {
                    range_end.saturating_add(1)
                };
                (commit_info.committed_rounds, recovery_start)
            } else {
                tracing::info!("Found no stored CommitInfo to recover from");
                (vec![0; num_authorities], GENESIS_COMMIT_INDEX + 1)
            };

        // Read fast sync flag from storage
        let fast_sync_ongoing = store.read_fast_sync_ongoing();

        let mut unscored_committed_subdags = Vec::new();
        let mut scoring_subdag = ScoringSubdag::new(context.clone());

        // Skip subdag recovery if fast sync is ongoing - block data may not be
        // available and this will be reinitialized by fast commit syncer anyway
        if !fast_sync_ongoing {
            if let Some(last_commit) = last_commit.as_ref() {
                store
                    .scan_commits((commit_recovery_start_index..=last_commit.index()).into())
                    .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"))
                    .iter()
                    .for_each(|commit| {
                        for block_ref in commit.block_headers() {
                            last_committed_rounds[block_ref.author] =
                                max(last_committed_rounds[block_ref.author], block_ref.round);
                        }

                        let committed_subdag =
                            load_pending_subdag_from_store(store.as_ref(), commit.clone(), vec![]);
                        unscored_committed_subdags.push(committed_subdag.base);
                    });
            }

            scoring_subdag.add_subdags(mem::take(&mut unscored_committed_subdags));
        }

        info!(
            "DagState initialized: {last_commit:?}; {last_committed_rounds:?}; \
            {} unscored committed subdags; fast_sync_ongoing={fast_sync_ongoing}",
            unscored_committed_subdags.len()
        );

        let mut state = Self {
            context,
            genesis,
            recent_block_headers: BTreeMap::new(),
            recent_transactions_by_authority: vec![BTreeMap::new(); num_authorities],
            recent_shards_by_authority: vec![BTreeMap::new(); num_authorities],
            recent_headers_refs_by_authority: vec![BTreeSet::new(); num_authorities],
            tx_ref_to_block_digest_by_authority: vec![BTreeMap::new(); num_authorities],
            threshold_clock,
            highest_accepted_round: 0,
            last_commit,
            last_commit_round_advancement_time: None,
            last_committed_rounds: last_committed_rounds.clone(),
            last_solid_subdag_base: None, /* Later the commit observer might update
                                           * this value during recovery process. */
            pending_commit_votes: VecDeque::new(),
            transactions_to_write: vec![],
            block_headers_to_write: vec![],
            commits_to_write: vec![],
            voting_block_headers_to_write: vec![],
            fast_sync_ongoing_flag_to_write: None,
            commit_info_to_write: vec![],
            pending_acknowledgments: BTreeSet::new(),
            scoring_subdag,
            store: store.clone(),
            cached_rounds,
            evicted_rounds: vec![0; num_authorities],
            cordial_knowledge_senders: None,
        };

        // Load cached data for each authority from storage
        for (i, round) in last_committed_rounds.into_iter().enumerate() {
            let authority_index = state.context.committee.to_authority_index(i).unwrap();
            state.load_cached_data_for_authority(authority_index, round, DataSource::Recover);
        }
        state
    }

    pub fn set_cordial_knowledge_senders(
        &mut self,
        sender: Sender<CordialKnowledgeMessage>,
        eviction_sender: watch::Sender<Vec<Round>>,
    ) {
        self.cordial_knowledge_senders = Some((sender, eviction_sender));
    }

    /// Loads cached data (block headers and transactions) for a single
    /// authority from storage. Updates eviction round and populates
    /// in-memory caches.
    fn load_cached_data_for_authority(
        &mut self,
        authority_index: AuthorityIndex,
        committed_round: Round,
        data_source: DataSource,
    ) {
        let eviction_round = Self::eviction_round(committed_round, self.cached_rounds);
        self.evicted_rounds[authority_index] = eviction_round;

        // Reload block headers from storage
        let block_headers = self
            .store
            .scan_block_headers_by_author(authority_index, eviction_round + 1)
            .expect("Database error");
        for block_header in &block_headers {
            self.update_block_header_metadata(block_header, data_source);
        }

        // Reload transactions from storage
        let transactions = self
            .store
            .scan_transactions_by_author(authority_index, eviction_round + 1, self.context.clone())
            .expect("Database error");
        for txn in &transactions {
            self.update_transaction_metadata(txn, data_source);
        }

        info!(
            "Loaded cached data for authority {}: {} block headers, {} transactions",
            authority_index,
            block_headers.len(),
            transactions.len()
        );
    }

    /// Reinitialize DagState after fast sync completes.
    /// This clears in-memory caches and reloads from storage for the
    /// cached_rounds window. Should be called after block headers have been
    /// stored via accept_block_headers() and flush().
    pub(crate) fn reinitialize(&mut self) {
        let num_authorities = self.context.committee.size();

        info!(
            "Reinitializing DagState with cached_rounds={}, last_committed_rounds={:?}",
            self.cached_rounds, self.last_committed_rounds
        );

        // 1. Clear all in-memory caches
        // Note: scoring_subdag IS cleared because during fast sync,
        // CommittedSubDag.headers is empty, so scoring_subdag cannot be
        // properly populated (it would have leaders but no votes). After
        // reinitialize, regular operation will rebuild it correctly.
        self.scoring_subdag.clear();
        self.recent_block_headers.clear();
        self.recent_transactions_by_authority = vec![BTreeMap::new(); num_authorities];
        self.recent_shards_by_authority = vec![BTreeMap::new(); num_authorities];
        self.recent_headers_refs_by_authority = vec![BTreeSet::new(); num_authorities];
        self.tx_ref_to_block_digest_by_authority = vec![BTreeMap::new(); num_authorities];
        self.pending_commit_votes.clear();
        self.pending_acknowledgments.clear();

        // 2. Reinitialize threshold_clock with current round
        let current_round = self.threshold_clock.get_round();
        self.threshold_clock = ThresholdClock::new(current_round, self.context.clone());

        // 3. Reload cached data for each authority
        for (i, &committed_round) in self.last_committed_rounds.clone().iter().enumerate() {
            let authority_index = self.context.committee.to_authority_index(i).unwrap();
            self.load_cached_data_for_authority(
                authority_index,
                committed_round,
                DataSource::FastCommitSyncer,
            );
        }

        // Rebuild scoring_subdag from stored commits so leader schedule state
        // matches peers after fast sync reinitialization.
        self.rebuild_scoring_subdag_from_store();

        info!("DagState reinitialized successfully");
    }

    fn rebuild_scoring_subdag_from_store(&mut self) {
        let Some(last_commit) = self.last_commit.as_ref() else {
            return;
        };

        let commit_recovery_start_index = self.last_commit_info_index().saturating_add(1);

        if commit_recovery_start_index > last_commit.index() {
            return;
        }

        let commits = self
            .store
            .scan_commits((commit_recovery_start_index..=last_commit.index()).into())
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));

        let mut unscored_subdags = Vec::with_capacity(commits.len());
        for commit in commits {
            let pending_subdag =
                load_pending_subdag_from_store(self.store.as_ref(), commit, vec![]);
            unscored_subdags.push(pending_subdag.base);
        }

        if !unscored_subdags.is_empty() {
            self.scoring_subdag.add_subdags(unscored_subdags);
        }
    }

    /// Accepts a block header into DagState and keeps it in memory.
    pub(crate) fn accept_block_header(
        &mut self,
        block_header: VerifiedBlockHeader,
        source: DataSource,
    ) {
        assert_ne!(
            block_header.round(),
            GENESIS_ROUND,
            "Genesis header should not be accepted into DAG."
        );

        let block_ref = block_header.reference();
        if self.contains_block_header(&block_ref) {
            self.context
                .metrics
                .node_metrics
                .core_skipped_headers
                .with_label_values(&[
                    self.context.authority_hostname(block_ref.author),
                    source.as_str(),
                ])
                .inc();
            return;
        }

        let now = self.context.clock.timestamp_utc_ms();
        if block_header.timestamp_ms() > now {
            // blocks can have timestamps in the future, just log it
            trace!(
                "Block header {block_header:?} with timestamp {} is greater than local timestamp {now}.",
                block_header.timestamp_ms(),
            );
        }
        // Record the time drift metric
        let hostname = &self.context.committee.authority(block_ref.author).hostname;
        self.context
            .metrics
            .node_metrics
            .accepted_block_header_time_drift_ms
            .with_label_values(&[hostname])
            .inc_by(block_header.timestamp_ms().saturating_sub(now));

        // TODO: Move this check to core
        // Ensure we don't write multiple blocks per slot for our own index
        if block_ref.author == self.context.own_index {
            let existing_blocks = self.get_uncommitted_block_headers_at_slot(block_ref.into());
            assert!(
                existing_blocks.is_empty(),
                "Block header Rejected! Attempted to add block header {block_header:#?} to own slot where \
                block header(s) {existing_blocks:#?} already exists."
            );
        }
        self.update_block_header_metadata(&block_header, source);
        debug!(
            "block header {} pushed to write to store batch by {}",
            block_header, self.context.own_index
        );
        self.block_headers_to_write.push(block_header);
        let author_label = if self.context.own_index == block_ref.author {
            "own"
        } else {
            "others"
        };

        self.context
            .metrics
            .node_metrics
            .accepted_block_headers
            .with_label_values(&[author_label])
            .inc();
    }

    pub(crate) fn add_transactions(
        &mut self,
        transactions: VerifiedTransactions,
        source: DataSource,
    ) {
        let transaction_ref = transactions.transaction_ref();
        let generic_ref = if self.context.protocol_config.consensus_fast_commit_sync() {
            GenericTransactionRef::from(transaction_ref)
        } else {
            let Some(block_ref) = transactions.block_ref() else {
                error!("block_ref unavailable for transactions in non-transaction-ref path");
                return;
            };
            GenericTransactionRef::from(block_ref)
        };
        if self.recent_transactions_by_authority[transaction_ref.author].contains_key(&generic_ref)
        {
            if transactions.has_transactions() {
                self.context
                    .metrics
                    .node_metrics
                    .core_skipped_transactions
                    .with_label_values(&[
                        self.context.authority_hostname(transaction_ref.author),
                        source.as_str(),
                    ])
                    .inc();
            }
            return;
        }
        self.update_transaction_metadata(&transactions, source);
        self.transactions_to_write.push(transactions);
    }

    pub(crate) fn add_shard(&mut self, shard: VerifiedOwnShard) {
        let gen_transaction_ref = shard.gen_transaction_ref;
        if self.recent_shards_by_authority[gen_transaction_ref.author()]
            .insert(gen_transaction_ref, shard.serialized_shard)
            .is_none()
        {
            debug!("Adding shard for transaction ref: {}", gen_transaction_ref);
            if let Some((sender, _)) = &self.cordial_knowledge_senders {
                let cordial_message = CordialKnowledgeMessage::NewShard(gen_transaction_ref);
                if let Err(TrySendError::Closed(_)) = sender.try_send(cordial_message) {
                    warn!("Failed to send cordial knowledge update: channel closed");
                }
            }
        }
    }

    /// Adds voting block headers to be flushed to storage. These are block
    /// headers received during fast sync that contain commit votes used to
    /// certify commits.
    pub(crate) fn add_voting_block_headers(&mut self, headers: Vec<VerifiedBlockHeader>) {
        self.voting_block_headers_to_write.extend(headers);
    }

    pub(crate) fn set_fast_sync_ongoing_flag(&mut self, flag: bool) {
        self.fast_sync_ongoing_flag_to_write = Some(flag);
    }

    pub(crate) fn fast_sync_ongoing(&self) -> bool {
        self.store.read_fast_sync_ongoing()
    }

    /// Returns the leader round of the last solid commit (backward
    /// compatibility).
    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn last_solid_commit_leader_round(&self) -> Option<Round> {
        self.last_solid_subdag_base.as_ref().map(|s| s.leader.round)
    }

    /// Returns the commit index of the last solid commit.
    /// Used by fast sync to determine the starting point for fetching.
    pub(crate) fn last_solid_commit_index(&self) -> CommitIndex {
        self.last_solid_subdag_base
            .as_ref()
            .map(|s| s.commit_ref.index)
            .unwrap_or(0)
    }

    /// Updates the last solid SubDagBase - the most recent commit where all
    /// transactions are locally available.
    pub(crate) fn update_last_solid_subdag_base(&mut self, subdag_base: SubDagBase) {
        let last_solid_commit_leader_round = subdag_base.leader.round;
        let max_commit_round = self
            .last_committed_rounds
            .iter()
            .max()
            .expect("There should be at least one last committed round");
        debug!(
            "Last solid commit has leader at round {last_solid_commit_leader_round}; last commit has leader at round {max_commit_round}",
        );
        let gap = (*max_commit_round).saturating_sub(last_solid_commit_leader_round);
        self.context
            .metrics
            .node_metrics
            .gap_to_available_commit
            .set(gap as i64);
        self.last_solid_subdag_base = Some(subdag_base);
    }
    pub(crate) fn update_pending_commit_votes(&mut self, solid_commit_refs: Vec<CommitRef>) {
        self.pending_commit_votes.extend(solid_commit_refs);
    }

    /// Updates internal metadata for accepted block header.
    fn update_block_header_metadata(
        &mut self,
        block_header: &VerifiedBlockHeader,
        source: DataSource,
    ) {
        let block_ref = block_header.reference();
        self.recent_block_headers
            .insert(block_ref, block_header.clone());
        self.recent_headers_refs_by_authority[block_ref.author].insert(block_ref);
        self.tx_ref_to_block_digest_by_authority[block_ref.author].insert(
            (block_ref.round, block_header.transactions_commitment()),
            block_ref.digest,
        );
        self.threshold_clock.add_block_header(block_ref);
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
        self.context
            .metrics
            .node_metrics
            .accepted_block_headers_source
            .with_label_values(&[source.as_str(), hostname])
            .inc();
        let clock_round = self.threshold_clock_round();
        let clock_round_gap = clock_round.saturating_sub(block_ref.round);
        self.context
            .metrics
            .node_metrics
            .accepted_block_headers_round_gap
            .with_label_values(&[source.as_str()])
            .observe(clock_round_gap as f64);
        if source != DataSource::CommitSyncer && source != DataSource::Recover {
            if let Some((sender, _)) = &self.cordial_knowledge_senders {
                // Fetch transaction commitments for all acknowledged blocks in batch
                let acknowledgments = block_header.acknowledgments();
                let ack_transactions_commitments =
                    if self.context.protocol_config.consensus_fast_commit_sync() {
                        self.get_transactions_commitments_batch(acknowledgments)
                    } else {
                        vec![None; acknowledgments.len()]
                    };

                let cordial_message = CordialKnowledgeMessage::NewHeader {
                    header: block_header.clone(),
                    ack_transactions_commitments,
                };
                if let Err(TrySendError::Closed(_)) = sender.try_send(cordial_message) {
                    warn!("Failed to send cordial knowledge update: channel closed");
                }
            }
        }
    }

    fn update_transaction_metadata(
        &mut self,
        transactions: &VerifiedTransactions,
        source: DataSource,
    ) {
        let transaction_ref = transactions.transaction_ref();
        let generic_ref = if self.context.protocol_config.consensus_fast_commit_sync() {
            GenericTransactionRef::from(transaction_ref)
        } else {
            let Some(block_ref) = transactions.block_ref() else {
                error!("block_ref unavailable for transactions in non-transaction-ref path");
                return;
            };
            GenericTransactionRef::from(block_ref)
        };
        self.recent_transactions_by_authority[transaction_ref.author]
            .insert(generic_ref, transactions.clone());
        tracing::debug!("Adding transactions for {generic_ref}");

        // Handle pending acknowledgments for recent blocks
        let has_transactions = transactions.has_transactions();
        let clock_round = self.threshold_clock_round();
        let min_round: Round = clock_round.saturating_sub(self.context.protocol_config.gc_depth());
        let hostname = self
            .context
            .committee
            .authority(transaction_ref.author)
            .hostname
            .as_str();
        let clock_round_gap = clock_round.saturating_sub(transaction_ref.round);

        if has_transactions {
            // Record metrics
            self.context
                .metrics
                .node_metrics
                .accepted_transactions_source
                .with_label_values(&[source.as_str(), hostname])
                .inc();
            self.context
                .metrics
                .node_metrics
                .accepted_transactions_round_gap
                .with_label_values(&[source.as_str()])
                .observe(clock_round_gap as f64);
            if transaction_ref.round >= min_round
                && source != DataSource::FastCommitSyncer
                && source != DataSource::CommitSyncer
                && source != DataSource::Recover
            {
                self.add_pending_acknowledgment(
                    transaction_ref,
                    transactions.block_ref().map(|br| br.digest),
                    source,
                );
            }
        } else {
            self.context
                .metrics
                .node_metrics
                .skipped_empty_transaction_acknowledgments
                .with_label_values(&[hostname])
                .inc()
        }
    }

    /// Finds a genesis block matching the given TransactionRef. Uses a range
    /// query on (round, author) with transactions_commitment filtering.
    fn find_genesis_by_transaction_ref(&self, tx_ref: &TransactionRef) -> Option<&VerifiedBlock> {
        let author = tx_ref.author();
        let round = tx_ref.round;
        let min_ref = BlockRef {
            round,
            author,
            digest: BlockHeaderDigest::MIN,
        };
        let max_ref = BlockRef {
            round,
            author,
            digest: BlockHeaderDigest::MAX,
        };

        let matching: Vec<_> = self
            .genesis
            .range(min_ref..=max_ref)
            .filter(|(_, block)| block.transactions_commitment() == tx_ref.transactions_commitment)
            .collect();

        match matching.len() {
            1 => Some(matching[0].1),
            0 => None,
            n => {
                error!(
                    "Found {} genesis items matching slot ({}, {}) and transactions_commitment, expected 1 or 0",
                    n, round, author
                );
                None
            }
        }
    }

    /// Finds genesis block matching the generic transaction reference.
    /// For BlockRef: direct lookup. For TransactionRef: range query with
    /// transactions_commitment verification.
    fn get_genesis_block(&self, tx_ref: GenericTransactionRef) -> Option<&VerifiedBlock> {
        match tx_ref {
            GenericTransactionRef::BlockRef(block_ref) => self.genesis.get(&block_ref),
            GenericTransactionRef::TransactionRef(tx_ref) => {
                self.find_genesis_by_transaction_ref(&tx_ref)
            }
        }
    }

    /// Accepts block headers into DagState and keeps it in memory.
    pub(crate) fn accept_block_headers(
        &mut self,
        block_headers: Vec<VerifiedBlockHeader>,
        source: DataSource,
    ) {
        debug!(
            "Accepting block headers: {}",
            block_headers
                .iter()
                .map(|b| b.reference().to_string())
                .join(",")
        );
        for block_header in block_headers {
            self.accept_block_header(block_header, source);
        }
    }

    /// Gets transactions by checking cached recent transactions in memory, then
    /// storage. An element is None when the corresponding transaction is not
    /// found.
    pub(crate) fn get_verified_transactions(
        &self,
        transactions_refs: &[GenericTransactionRef],
    ) -> Vec<Option<VerifiedTransactions>> {
        let mut transactions = vec![None; transactions_refs.len()];
        let mut missing = Vec::new();

        for (index, transactions_ref) in transactions_refs.iter().enumerate() {
            if transactions_ref.round() == GENESIS_ROUND {
                if let Some(genesis_block) = self.get_genesis_block(*transactions_ref) {
                    transactions[index] = Some(genesis_block.verified_transactions.clone());
                }
                continue;
            }
            if let Some(transaction) = self.recent_transactions_by_authority
                [transactions_ref.author()]
            .get(transactions_ref)
            {
                transactions[index] = Some(transaction.clone());
                continue;
            }
            missing.push((index, transactions_ref));
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
            .read_verified_transactions(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));
        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["get_verified_transactions"])
            .inc();

        for ((index, _), result) in missing.into_iter().zip(store_results.into_iter()) {
            transactions[index] = result;
        }

        transactions
    }

    /// Returns all verified transactions or the list of missing transaction
    /// refs. This is the canonical way to load transactions for
    /// CommittedSubDag construction.
    pub(crate) fn try_get_all_verified_transactions(
        &self,
        tx_refs: &[GenericTransactionRef],
    ) -> Result<Vec<VerifiedTransactions>, Vec<GenericTransactionRef>> {
        let results = self.get_verified_transactions(tx_refs);
        let mut missing = Vec::new();
        for (i, tx_opt) in results.iter().enumerate() {
            if tx_opt.is_none() {
                missing.push(tx_refs[i]);
            }
        }
        if missing.is_empty() {
            Ok(results.into_iter().map(|tx| tx.unwrap()).collect())
        } else {
            Err(missing)
        }
    }

    /// Gets serialized transactions by checking cached recent transactions in
    /// memory, then storage. An element is None when the corresponding
    /// transaction is not found.
    pub(crate) fn get_serialized_transactions(
        &self,
        transactions_refs: &[GenericTransactionRef],
    ) -> Vec<Option<Bytes>> {
        let mut transactions = vec![None; transactions_refs.len()];
        let mut missing = Vec::new();

        for (index, transactions_ref) in transactions_refs.iter().enumerate() {
            if transactions_ref.round() == GENESIS_ROUND {
                if let Some(transaction) = self
                    .get_genesis_block(*transactions_ref)
                    .map(|block| block.verified_transactions.clone())
                {
                    transactions[index] = Some(transaction.serialized().clone());
                }
                continue;
            }
            if let Some(transaction) = self.recent_transactions_by_authority
                [transactions_ref.author()]
            .get(transactions_ref)
            {
                transactions[index] = Some(transaction.serialized().clone());
                continue;
            }
            missing.push((index, transactions_ref));
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
            .read_serialized_transactions(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));
        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["get_serialized_transactions"])
            .inc();

        for ((index, _), result) in missing.into_iter().zip(store_results.into_iter()) {
            transactions[index] = result;
        }

        transactions
    }

    /// Gets a block header by checking cached recent blocks then storage.
    /// Returns None when the block is not found.
    pub(crate) fn get_verified_block_header(
        &self,
        reference: &BlockRef,
    ) -> Option<VerifiedBlockHeader> {
        self.get_verified_block_headers(&[*reference])
            .pop()
            .expect("Exactly one element should be returned")
    }

    /// Checks if verified block headers exist for the given transaction refs.
    /// Checks in-memory data (genesis and recent_block_headers) first, then
    /// falls back to storage for blocks not found in memory.
    #[cfg_attr(test, expect(dead_code))]
    pub(crate) fn contains_verified_block_headers_for_transaction_refs(
        &self,
        tx_refs: &[TransactionRef],
    ) -> Vec<bool> {
        let mut results = vec![false; tx_refs.len()];

        for (index, tx_ref) in tx_refs.iter().enumerate() {
            let round = tx_ref.round;

            // Check genesis blocks
            if round == GENESIS_ROUND {
                results[index] = self.find_genesis_by_transaction_ref(tx_ref).is_some();
                continue;
            }

            // Check recent block headers. We are guaranteed to have the block
            // digest in tx_ref_to_block_digest_by_authority if the header is
            // still in recent_block_headers, because both are inserted together
            // and headers are evicted first.
            results[index] = self
                .resolve_block_ref(tx_ref)
                .and_then(|block_ref| self.recent_block_headers.get(&block_ref))
                .is_some();
        }

        // Collect refs that weren't found in memory
        let missing: Vec<(usize, TransactionRef)> = tx_refs
            .iter()
            .enumerate()
            .filter(|(i, _)| !results[*i])
            .map(|(i, tx)| (i, *tx))
            .collect();

        if !missing.is_empty() {
            // Look up block digests from in-memory map
            let refs_with_indices: Vec<_> = missing
                .iter()
                .filter_map(|(idx, tx)| self.resolve_block_ref(tx).map(|br| (*idx, br)))
                .collect();

            if !refs_with_indices.is_empty() {
                // Batch: Check block headers exist in storage
                let block_refs: Vec<_> = refs_with_indices.iter().map(|(_, br)| *br).collect();
                let headers_exist = self
                    .store
                    .contains_block_headers(&block_refs)
                    .unwrap_or_else(|e| {
                        warn!("Failed to check block headers: {e:?}");
                        vec![false; block_refs.len()]
                    });

                // Update results
                for ((idx, _), exists) in refs_with_indices.iter().zip(headers_exist.iter()) {
                    if *exists {
                        results[*idx] = true;
                    }
                }
            }
        }

        results
    }

    /// Gets verified block headers by checking genesis, cached recent block
    /// headers in memory, then storage. An element is None when the
    /// corresponding block header is not found.
    pub(crate) fn get_verified_block_headers(
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
            if let Some(block_header) = self.recent_block_headers.get(block_ref) {
                block_headers[index] = Some(block_header.clone());
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
            .read_verified_block_headers(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));

        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["get_verified_block_headers"])
            .inc();

        for ((index, _), result) in missing_headers.into_iter().zip(store_results.into_iter()) {
            block_headers[index] = result;
        }

        block_headers
    }

    /// Gets transaction commitments for a batch of block references by checking
    /// genesis, cached recent block headers in memory, then storage.
    /// Returns a vector of tuples (BlockRef, TransactionsCommitment) for blocks
    /// that were found. Skips blocks that are not found.
    pub(crate) fn get_transactions_commitments_batch(
        &self,
        block_refs: &[BlockRef],
    ) -> Vec<Option<TransactionsCommitment>> {
        let mut commitments: Vec<Option<TransactionsCommitment>> = vec![None; block_refs.len()];
        let mut missing_headers = Vec::new();

        for (index, block_ref) in block_refs.iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Genesis blocks don't have meaningful transaction commitments, skip them
                continue;
            }
            if let Some(block_header) = self.recent_block_headers.get(block_ref) {
                commitments[index] = Some(block_header.transactions_commitment());
                continue;
            }
            missing_headers.push((index, block_ref));
        }

        if missing_headers.is_empty() {
            return commitments;
        }

        let missing_refs = missing_headers
            .iter()
            .map(|(_, block_ref)| **block_ref)
            .collect::<Vec<_>>();
        let store_results = self
            .store
            .read_verified_block_headers(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));

        for ((index, _), result) in missing_headers.into_iter().zip(store_results.into_iter()) {
            if let Some(header) = result {
                commitments[index] = Some(header.transactions_commitment());
            }
        }
        commitments
    }

    /// Gets serialized block headers by checking genesis, cached recent block
    /// headers in memory, then storage. An element is None when the
    /// corresponding block header is not found.
    pub(crate) fn get_serialized_block_headers(
        &self,
        block_refs: &[BlockRef],
    ) -> Vec<Option<Bytes>> {
        let mut block_headers: Vec<Option<Bytes>> = vec![None; block_refs.len()];
        let mut missing_headers = Vec::new();
        for (index, block_ref) in block_refs.iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Allow the caller to handle the invalid genesis ancestor error.
                if let Some(block) = self.genesis.get(block_ref) {
                    block_headers[index] = Some(block.verified_block_header.serialized().clone());
                }
                continue;
            }
            if let Some(block) = self.recent_block_headers.get(block_ref) {
                block_headers[index] = Some(block.serialized().clone());
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
            .read_serialized_block_headers(&missing_refs)
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));

        self.context
            .metrics
            .node_metrics
            .dag_state_store_read_count
            .with_label_values(&["get_serialized_block_headers"])
            .inc();

        for ((index, _), result) in missing_headers.into_iter().zip(store_results.into_iter()) {
            block_headers[index] = result;
        }

        block_headers
    }

    /// Gets block headers by checking genesis and then cached recent block
    /// headers in memory. Storage is not checked in this method. An element
    /// is None when the corresponding block header is not found.
    pub(crate) fn get_cached_block_headers(
        &self,
        block_refs: &[BlockRef],
    ) -> Vec<Option<VerifiedBlockHeader>> {
        let mut block_headers: Vec<Option<VerifiedBlockHeader>> = vec![None; block_refs.len()];
        for (index, block_ref) in block_refs.iter().enumerate() {
            if block_ref.round == GENESIS_ROUND {
                // Allow the caller to handle the invalid genesis ancestor error.
                if let Some(block) = self.genesis.get(block_ref) {
                    block_headers[index] = Some(block.verified_block_header.clone());
                }
                continue;
            }
            if let Some(block) = self.recent_block_headers.get(block_ref) {
                block_headers[index] = Some(block.clone());
                continue;
            }
        }

        block_headers
    }

    /// Resolves a `TransactionRef` to a `BlockRef` using the in-memory
    /// `tx_ref_to_block_digest_by_authority` lookup table.
    pub(crate) fn resolve_block_ref(&self, tx_ref: &TransactionRef) -> Option<BlockRef> {
        self.tx_ref_to_block_digest_by_authority[tx_ref.author]
            .get(&(tx_ref.round, tx_ref.transactions_commitment))
            .map(|&digest| BlockRef::new(tx_ref.round, tx_ref.author, digest))
    }

    /// Gets cached block headers for a list of TransactionRefs by first looking
    /// up the block digest from the in-memory tx_ref_to_block_digest map, then
    /// fetching the cached block header.
    pub(crate) fn get_cached_block_headers_for_transaction_refs(
        &self,
        tx_refs: &[TransactionRef],
    ) -> Vec<Option<VerifiedBlockHeader>> {
        let mut block_headers: Vec<Option<VerifiedBlockHeader>> = vec![None; tx_refs.len()];
        for (index, tx_ref) in tx_refs.iter().enumerate() {
            if tx_ref.round == GENESIS_ROUND {
                if let Some(block) = self.find_genesis_by_transaction_ref(tx_ref) {
                    block_headers[index] = Some(block.verified_block_header.clone());
                }
                continue;
            }
            let Some(block_ref) = self.resolve_block_ref(tx_ref) else {
                continue;
            };
            if let Some(block) = self.recent_block_headers.get(&block_ref) {
                block_headers[index] = Some(block.clone());
            }
        }
        block_headers
    }

    /// Gets shards by checking cached recent shards in memory.
    pub(crate) fn get_cached_shards(
        &self,
        gen_tran_refs: &[GenericTransactionRef],
    ) -> Vec<Option<Bytes>> {
        let mut shards: Vec<Option<Bytes>> = vec![None; gen_tran_refs.len()];
        for (index, gen_tran_ref) in gen_tran_refs.iter().enumerate() {
            if let Some(shard) =
                self.recent_shards_by_authority[gen_tran_ref.author()].get(gen_tran_ref)
            {
                shards[index] = Some(shard.clone());
            }
        }
        shards
    }

    /// Gets all uncommitted block headers in a slot.
    /// Uncommitted block headers must exist in memory, so only in-memory block
    /// headers are checked.
    pub(crate) fn get_uncommitted_block_headers_at_slot(
        &self,
        slot: Slot,
    ) -> Vec<VerifiedBlockHeader> {
        // TODO: either panic below when the slot is at or below the last committed
        // round, or support reading from storage while limiting storage reads
        // to edge cases.

        let mut block_headers = vec![];
        for (_block_ref, block_header) in self.recent_block_headers.range((
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
            block_headers.push(block_header.clone())
        }
        block_headers
    }

    /// Gets all uncommitted block headers in a round.
    /// Uncommitted block headers must exist in memory, so only in-memory block
    /// headers are checked.
    pub(crate) fn get_uncommitted_block_headers_at_round(
        &self,
        round: Round,
    ) -> Vec<VerifiedBlockHeader> {
        if round <= self.last_commit_round() {
            panic!("Round {round} have committed block headers!");
        }

        let mut block_headers = vec![];
        for (_block_ref, block_header) in self.recent_block_headers.range((
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
            block_headers.push(block_header.clone())
        }
        block_headers
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
            let Some(block) = self.get_verified_block_header(&block_ref) else {
                panic!("Block Header {block_ref:?} should exist in DAG!");
            };
            linked.extend(
                block
                    .ancestors()
                    .iter()
                    .filter(|ancestor| ancestor.round >= earlier_round)
                    .cloned(),
            );
        }
        let block_headers =
            self.get_verified_block_headers(&linked.iter().cloned().collect::<Vec<_>>());
        block_headers
            .into_iter()
            .map(|opt| opt.unwrap_or_else(|| panic!("Block should exist in DAG!")))
            .collect()
    }

    /// Gets the last proposed (non-genesis) block from this authority.
    /// NOTE: the method will not panic if transactions or headers are not found
    /// in DAG State for the most recent header, as that could happen for
    /// instance when own header is synced and the node is restarted.
    pub(crate) fn get_last_own_non_genesis_block(&self) -> Option<VerifiedBlock> {
        if let Some(last) = self.recent_headers_refs_by_authority[self.context.own_index].last() {
            if last.round > GENESIS_ROUND {
                let last_header_opt = self.recent_block_headers.get(last);
                if let Some(last_header) = last_header_opt {
                    let transaction_ref =
                        if self.context.protocol_config.consensus_fast_commit_sync() {
                            GenericTransactionRef::from(TransactionRef {
                                round: last.round,
                                author: last.author,
                                transactions_commitment: last_header.transactions_commitment(),
                            })
                        } else {
                            GenericTransactionRef::from(*last)
                        };

                    if let Some(last_transactions) =
                        self.recent_transactions_by_authority[last.author].get(&transaction_ref)
                    {
                        return Some(VerifiedBlock::new(
                            last_header.clone(),
                            last_transactions.clone(),
                        ));
                    }
                }
            }
        }
        None
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
                .expect("Block header should be found in recent block headers")
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

    /// Returns own cached recent blocks.
    /// Blocks returned are limited to round >= `start`, and cached.
    /// NOTE: the method is soft in the sense that the if transactions are not
    /// found for a given block header, that block is not included in the return
    /// result
    pub(crate) fn get_own_cached_blocks(&self, start: Round) -> Vec<VerifiedBlock> {
        let authority = self.context.own_index;
        let mut blocks = vec![];
        for block_ref in self.recent_headers_refs_by_authority[authority].range((
            Included(BlockRef::new(start, authority, BlockHeaderDigest::MIN)),
            Unbounded,
        )) {
            let header_opt = self.recent_block_headers.get(block_ref);
            let mut block_constructed = false;
            if let Some(header) = header_opt {
                let transaction_ref = if self.context.protocol_config.consensus_fast_commit_sync() {
                    GenericTransactionRef::from(TransactionRef {
                        round: block_ref.round,
                        author: block_ref.author,
                        transactions_commitment: header.transactions_commitment(),
                    })
                } else {
                    GenericTransactionRef::from(*block_ref)
                };
                let transactions_opt =
                    self.recent_transactions_by_authority[block_ref.author].get(&transaction_ref);
                if let Some(transactions) = transactions_opt {
                    blocks.push(VerifiedBlock::new(header.clone(), transactions.clone()));
                    block_constructed = true;
                }
            }
            if !block_constructed {
                warn!("Block header or transactions missing for block ref: {block_ref}");
            }
        }
        blocks
    }

    /// Returns cached recent block headers from the specified authority.
    /// Block headers returned are limited to round >= `start`, and cached.
    /// NOTE: caller should not assume returned block headers are always
    /// chained.
    #[cfg_attr(not(test), expect(dead_code))]
    pub(crate) fn get_cached_block_headers_since_round(
        &self,
        authority: AuthorityIndex,
        start: Round,
    ) -> Vec<VerifiedBlockHeader> {
        self.get_cached_block_headers_in_range(authority, start, Round::MAX, usize::MAX)
    }

    /// Returns cached block headers from the specified authority within a given
    /// round range. Block headers returned are limited to `start_round` <=
    /// round < `end_round`, up to `limit` entries. NOTE: Only cached block
    /// headers are returned; storage is not checked.
    pub(crate) fn get_cached_block_headers_in_range(
        &self,
        authority: AuthorityIndex,
        start_round: Round,
        end_round: Round,
        limit: usize,
    ) -> Vec<VerifiedBlockHeader> {
        if start_round >= end_round || limit == 0 {
            return vec![];
        }

        let mut block_headers = vec![];
        for block_ref in self.recent_headers_refs_by_authority[authority].range((
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
        )) {
            let block_header = self
                .recent_block_headers
                .get(block_ref)
                .expect("Block header should exist in recent block headers");
            block_headers.push(block_header.clone());
            if block_headers.len() >= limit {
                break;
            }
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
        if start_round >= end_round {
            return None;
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
    /// slot, one block will be returned (the last in order) and for other
    /// equivocating blocks block references will be returned.
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
                        .expect("Block header should exist in recent block headers");
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

    /// Checks whether the required block headers are in cache; if not, then
    /// check in store. The method is not caching back the
    /// results, so it's expensive to keep asking for cache missing block
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
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));
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

    /// Checks whether the required transactions are in cache; if not, then
    /// check in store. The method is not caching back the
    /// results, so it's expensive to keep asking for cache missing
    /// transactions.
    pub(crate) fn contains_transactions(
        &self,
        transaction_refs: Vec<GenericTransactionRef>,
    ) -> Vec<bool> {
        let mut exist = vec![false; transaction_refs.len()];
        let mut missing = Vec::new();

        for (index, tx_ref) in transaction_refs.into_iter().enumerate() {
            if tx_ref.round() == GENESIS_ROUND {
                exist[index] = self.get_genesis_block(tx_ref).is_some();
                continue;
            }
            if self.recent_transactions_by_authority[tx_ref.author()].contains_key(&tx_ref) {
                exist[index] = true;
            } else {
                missing.push((index, tx_ref));
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
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"));
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

    /// Highest round where a block is committed, which is last commit's leader
    /// round.
    pub(crate) fn last_commit_round(&self) -> Round {
        match &self.last_commit {
            Some(commit) => commit.leader().round,
            None => 0,
        }
    }

    // Buffers a new commit in memory and updates last committed rounds.
    // REQUIRED: must not skip over any commit index.
    pub(crate) fn add_commit(&mut self, commit: TrustedCommit) {
        let time_diff = if let Some(last_commit) = &self.last_commit {
            if commit.index() <= last_commit.index() {
                debug!(
                    "New commit index {} <= last commit index {}!",
                    commit.index(),
                    last_commit.index()
                );
                return;
            }
            assert_eq!(commit.index(), last_commit.index() + 1);

            if commit.timestamp_ms() < last_commit.timestamp_ms() {
                panic!(
                    "Commit timestamps do not monotonically increment, prev commit {last_commit:?}, new commit {commit:?}"
                );
            }
            commit
                .timestamp_ms()
                .saturating_sub(last_commit.timestamp_ms())
        } else {
            assert_eq!(commit.index(), 1);
            0
        };

        self.context
            .metrics
            .node_metrics
            .last_commit_time_diff
            .observe(time_diff as f64);

        // Ensure that commit rounds are strictly increasing
        assert!(
            self.last_commit
                .as_ref()
                .is_none_or(|prev| prev.round() < commit.round()),
            "Commit round ordering violated: prev = {:?}, new = {:?}",
            self.last_commit.as_ref().map(|prev| prev.round()),
            commit.round()
        );

        self.last_commit = Some(commit.clone());

        if let Some(last_solid_subdag_base) = &self.last_solid_subdag_base {
            let gap = (commit.leader().round).saturating_sub(last_solid_subdag_base.leader.round);
            self.context
                .metrics
                .node_metrics
                .gap_to_available_commit
                .set(gap as i64);
        }

        let now = std::time::Instant::now();
        if let Some(previous_time) = self.last_commit_round_advancement_time {
            self.context
                .metrics
                .node_metrics
                .commit_round_advancement_interval
                .observe(now.duration_since(previous_time).as_secs_f64())
        }
        self.last_commit_round_advancement_time = Some(now);

        for block_ref in commit.block_headers().iter() {
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

        self.commits_to_write.push(commit);
    }

    /// Add commit info is called before the first commit in a new leader
    /// scheduler window
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

    pub(crate) fn take_commit_votes(&mut self, limit: usize) -> Vec<CommitVote> {
        let mut votes = Vec::new();
        while !self.pending_commit_votes.is_empty() && votes.len() < limit {
            votes.push(self.pending_commit_votes.pop_front().unwrap());
        }
        votes
    }

    /// Clean up old cached data for each authority, all cached blocks
    /// are guaranteed to be persisted. Used after flushing.
    pub(crate) fn evict_headers(&mut self) {
        for (authority_index, _) in self.context.committee.authorities() {
            let eviction_round = self.calculate_authority_eviction_round(authority_index);
            let recent_refs = &mut self.recent_headers_refs_by_authority[authority_index];

            // Evict everything below split_key
            let split_key =
                BlockRef::new(eviction_round + 1, authority_index, BlockHeaderDigest::MIN);

            let to_keep = recent_refs.split_off(&split_key);
            let evicted = std::mem::replace(recent_refs, to_keep);

            // Remove evicted headers from recent_block_headers
            for block_ref in &evicted {
                self.recent_block_headers.remove(block_ref);
            }
            self.evicted_rounds[authority_index] = eviction_round;
        }
    }

    /// Clean up old shards. Used after flushing.
    pub(crate) fn evict_shards(&mut self) {
        for (authority_index, _) in self.context.committee.authorities() {
            let eviction_round = self.calculate_authority_eviction_round(authority_index);

            // Evict everything below split_key
            let split_key = if self.context.protocol_config.consensus_fast_commit_sync() {
                GenericTransactionRef::from(TransactionRef {
                    round: eviction_round + 1,
                    author: authority_index,
                    transactions_commitment: TransactionsCommitment::MIN,
                })
            } else {
                GenericTransactionRef::from(BlockRef::new(
                    eviction_round + 1,
                    authority_index,
                    BlockHeaderDigest::MIN,
                ))
            };
            self.recent_shards_by_authority[authority_index] =
                self.recent_shards_by_authority[authority_index].split_off(&split_key);
        }
    }

    /// Function removes stalled transactions that are older than the minimum
    /// between "last solid leader round minus MAX_TRANSACTIONS_ACK_DEPTH
    /// (protocol_config.gc_depth) minus MAX_LINEARIZER_DEPTH
    /// (protocol_config.gc_depth)" and the eviction round of corresponding
    /// authority
    pub(crate) fn evict_transactions(&mut self) {
        let transaction_gc_round = self.gc_round_for_last_solid_commit();
        for (authority_index, _) in self.context.committee.authorities() {
            let eviction_round = self.calculate_authority_eviction_round(authority_index);
            // Take minimum between transaction_gc_round and eviction_round
            let transaction_eviction_round = min(transaction_gc_round, eviction_round + 1);

            // Evict everything below split_key
            let split_key = if self.context.protocol_config.consensus_fast_commit_sync() {
                GenericTransactionRef::from(TransactionRef {
                    round: transaction_eviction_round,
                    author: authority_index,
                    transactions_commitment: TransactionsCommitment::MIN,
                })
            } else {
                GenericTransactionRef::from(BlockRef::new(
                    transaction_eviction_round,
                    authority_index,
                    BlockHeaderDigest::MIN,
                ))
            };
            self.recent_transactions_by_authority[authority_index] =
                self.recent_transactions_by_authority[authority_index].split_off(&split_key);
        }
    }

    pub(crate) fn evict_tx_ref_to_block_digests(&mut self) {
        let transaction_gc_round = self.gc_round_for_last_solid_commit();
        for (authority_index, _) in self.context.committee.authorities() {
            let eviction_round = self.calculate_authority_eviction_round(authority_index);
            let eviction_round = min(transaction_gc_round, eviction_round + 1);
            let split_key = (eviction_round, TransactionsCommitment::MIN);
            self.tx_ref_to_block_digest_by_authority[authority_index] =
                self.tx_ref_to_block_digest_by_authority[authority_index].split_off(&split_key);
        }
    }

    /// Return the garbage collection round with respect to the last solid
    /// commit's leader round. Transactions of blocks at or below this round
    /// can be evicted from memory
    pub(crate) fn gc_round_for_last_solid_commit(&self) -> Round {
        if let Some(subdag_base) = &self.last_solid_subdag_base {
            self.gc_round(subdag_base.leader.round)
        } else {
            GENESIS_ROUND
        }
    }

    /// Function removes stalled pending acknowledgments that are older than
    /// "current clock round minus protocol_config.gc_depth() aka
    /// (MAX_TRANSACTIONS_ACK_DEPTH)"
    pub(crate) fn evict_pending_acknowledgments(&mut self) {
        let clock_round = self.threshold_clock_round();
        let min_round: Round = clock_round.saturating_sub(self.context.protocol_config.gc_depth());

        // Construct a dummy BlockRef with the minimum round to split on.
        // All entries < dummy will be removed.
        let lower_bound = BlockRef::new(min_round, AuthorityIndex::ZERO, BlockHeaderDigest::MIN);

        // Remove entries with round < min_round
        self.pending_acknowledgments = self.pending_acknowledgments.split_off(&lower_bound);
    }

    /// Evicts old cordial knowledge from CordialKnowledge. It is aligned
    /// with the eviction method, thereby should be called every time the
    /// eviction happens.
    pub(crate) fn evict_cordial_knowledge(&mut self) {
        if let Some((_, eviction_sender)) = &self.cordial_knowledge_senders {
            let mut eviction_rounds = vec![];
            for (authority_index, _) in self.context.committee.authorities() {
                let eviction_round = self.calculate_authority_eviction_round(authority_index);
                eviction_rounds.push(eviction_round);
            }
            if eviction_sender.send(eviction_rounds).is_err() {
                warn!("Failed to send cordial knowledge eviction message: channel closed");
            }
        }
    }

    /// Adds a block reference to pending acknowledgments by looking up the
    /// block digest from the transaction ref.
    pub(crate) fn add_pending_acknowledgment(
        &mut self,
        transaction_ref: TransactionRef,
        block_digest: Option<BlockHeaderDigest>,
        source: DataSource,
    ) {
        let block_ref = if let Some(digest) = block_digest {
            BlockRef::new(transaction_ref.round, transaction_ref.author, digest)
        } else {
            let Some(br) = self.resolve_block_ref(&transaction_ref) else {
                error!(
                    "block_digest not found for {transaction_ref:?} when adding pending acknowledgment, source: {source:?}"
                );
                return;
            };
            br
        };
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
                break;
            }
            taken.push(*ack);
        }

        if let Some(last) = taken.last() {
            last_ack = Some(*last);
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

    /// Return the garbage collection round. Transactions of blocks at or below
    /// this round which are not yet sequenced will never be sequenced.
    pub(crate) fn gc_round_for_last_commit(&self) -> Round {
        let last_commit_round = self.last_commit_round();
        self.gc_round(last_commit_round)
    }

    /// Return the garbage collection round with respect a given round.
    pub(crate) fn gc_round(&self, round: Round) -> Round {
        round.saturating_sub(self.context.protocol_config.gc_depth() * 2)
    }

    /// Last committed round per authority.
    pub(crate) fn last_committed_rounds(&self) -> Vec<Round> {
        self.last_committed_rounds.clone()
    }

    /// Returns block refs from the last `num_commits` commits stored in the
    /// database. This is used by the fast commit syncer to determine which
    /// block headers to fetch for the cached_rounds window before
    /// reinitializing components.
    pub(crate) fn get_block_refs_for_recent_commits(&self, num_commits: u32) -> Vec<BlockRef> {
        let last_commit_index = self.last_commit_index();
        if last_commit_index == 0 {
            return vec![];
        }

        let start_index = last_commit_index.saturating_sub(num_commits).max(1);
        let commits = self
            .store
            .scan_commits((start_index..=last_commit_index).into())
            .unwrap_or_else(|e| {
                warn!("Failed to scan commits for block refs: {:?}", e);
                vec![]
            });

        // Collect block refs from all commits, deduplicating them
        let mut block_refs: BTreeSet<BlockRef> = BTreeSet::new();
        for commit in &commits {
            for block_ref in commit.block_headers() {
                block_refs.insert(*block_ref);
            }
        }

        block_refs.into_iter().collect()
    }

    /// After each flush, DagState becomes persisted in storage and is expected
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
        let voting_block_headers = std::mem::take(&mut self.voting_block_headers_to_write);
        let fast_commit_sync_flag = self.fast_sync_ongoing_flag_to_write.take();

        // Early return if there's nothing to flush
        if transactions.is_empty()
            && block_headers.is_empty()
            && commits.is_empty()
            && commit_info.is_empty()
            && voting_block_headers.is_empty()
            && fast_commit_sync_flag.is_none()
        {
            return;
        }

        debug!(
            "Flushing {} block headers ({}), {} transactions ({}), {} commits ({}) and {} commit info ({}) and fast commit sync flag ({}) to storage.",
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
            fast_commit_sync_flag
                .map(|f| f.to_string())
                .unwrap_or_else(|| "unchanged".to_string())
        );

        // Write all buffered data to storage
        self.store
            .write(
                WriteBatch::new(
                    transactions,
                    block_headers,
                    commits,
                    commit_info,
                    voting_block_headers,
                    fast_commit_sync_flag,
                ),
                self.context.clone(),
            )
            .unwrap_or_else(|e| panic!("Failed to write to storage: {e:?}"));

        self.context
            .metrics
            .node_metrics
            .dag_state_store_write_count
            .inc();

        // Clean up old headers
        self.evict_headers();

        // Evict old shards
        self.evict_shards();

        // Clean up old transactions depending on the last solid leader round.
        self.evict_transactions();

        // Evict tx_ref_to_block_digest entries aligned with transaction eviction.
        self.evict_tx_ref_to_block_digests();

        // Clean up old acknowledgments.
        self.evict_pending_acknowledgments();

        // Clean up old cordial knowledge.
        self.evict_cordial_knowledge();

        // Update metrics
        let metrics = &self.context.metrics.node_metrics;
        metrics
            .dag_state_recent_headers
            .set(self.recent_block_headers.len() as i64);
        metrics.dag_state_recent_shards.set(
            self.recent_shards_by_authority
                .iter()
                .map(BTreeMap::len)
                .sum::<usize>() as i64,
        );
        metrics.dag_state_recent_transactions.set(
            self.recent_transactions_by_authority
                .iter()
                .map(BTreeMap::len)
                .sum::<usize>() as i64,
        );
        metrics.dag_state_recent_refs.set(
            self.recent_headers_refs_by_authority
                .iter()
                .map(BTreeSet::len)
                .sum::<usize>() as i64,
        );
        metrics
            .dag_state_pending_commit_votes
            .set(self.pending_commit_votes.len() as i64);
        metrics
            .dag_state_pending_acknowledgments
            .set(self.pending_acknowledgments.len() as i64);
    }

    pub(crate) fn recover_last_commit_info(&self) -> Option<(CommitRef, CommitInfo)> {
        self.store
            .read_last_commit_info()
            .unwrap_or_else(|e| panic!("Failed to read from storage: {e:?}"))
    }

    /// Returns the commit index of the last stored commit info, or
    /// GENESIS_COMMIT_INDEX if none. This is the end of the reputation
    /// score commit range (or the commit ref index for genesis).
    pub(crate) fn last_commit_info_index(&self) -> CommitIndex {
        self.recover_last_commit_info()
            .map(|(commit_ref, commit_info)| {
                let range_end = commit_info.reputation_scores.commit_range.end();
                if range_end == GENESIS_COMMIT_INDEX {
                    commit_ref.index
                } else {
                    range_end
                }
            })
            .unwrap_or(GENESIS_COMMIT_INDEX)
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

    /// The last round that should get evicted after a cache-clean-up operation.
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

            // Since the wave length is 3 we expect to find a quorum in the
            // uncommitted rounds.
            let blocks = self.get_uncommitted_block_headers_at_round(round);
            for block in &blocks {
                if quorum.add(block.author(), &self.context.committee) {
                    return blocks;
                }
            }
        }

        panic!("Fatal error, no quorum has been detected in our DAG on the last two rounds.");
    }

    #[cfg(test)]
    pub(crate) fn genesis_blocks(&self) -> Vec<VerifiedBlock> {
        self.genesis.values().cloned().collect()
    }

    #[cfg(test)]
    pub(crate) fn genesis_block_headers(&self) -> Vec<VerifiedBlockHeader> {
        self.genesis
            .values()
            .map(|b| b.verified_block_header.clone())
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
        Transaction,
        block_header::{
            BlockHeaderDigest, BlockRef, BlockTimestampMs, TestBlockHeader, TransactionsCommitment,
            VerifiedBlockHeader, genesis_block_headers,
        },
        encoder::create_encoder,
        storage::{WriteBatch, mem_store::MemStore},
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
    };

    #[tokio::test]
    async fn test_get_block_header() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);
        let own_index = AuthorityIndex::new_for_test(0);

        // Populate test blocks for round 1 ~ 10, authorities 0 ~ 2.
        let num_rounds: u32 = 10;
        let non_existent_round: u32 = 100;
        let num_authorities: u8 = 3;
        let num_blocks_per_slot: usize = 3;
        let mut block_headers = BTreeMap::new();
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                // Create 3 block headers per slot, with different timestamps and digests.
                let base_ts = round as BlockTimestampMs * 1000;
                for timestamp in base_ts..base_ts + num_blocks_per_slot as u64 {
                    let block_header = VerifiedBlockHeader::new_for_test(
                        TestBlockHeader::new(round, author)
                            .set_timestamp_ms(timestamp)
                            .build(),
                    );
                    dag_state.accept_block_header(block_header.clone(), DataSource::Test);
                    block_headers.insert(block_header.reference(), block_header);

                    // Only write one block header per slot for own index
                    if AuthorityIndex::new_for_test(author) == own_index {
                        break;
                    }
                }
            }
        }

        // Check uncommitted block headers that exist.
        for (block_ref, block_header) in &block_headers {
            assert_eq!(
                &dag_state.get_verified_block_header(block_ref).unwrap(),
                block_header
            );
        }

        // Check uncommitted block headers that do not exist.
        let last_ref = block_headers.keys().last().unwrap();
        assert!(
            dag_state
                .get_verified_block_header(&BlockRef::new(
                    last_ref.round,
                    last_ref.author,
                    BlockHeaderDigest::MIN
                ))
                .is_none()
        );

        // Check slots with uncommitted block headers.
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let slot = Slot::new(
                    round,
                    context
                        .committee
                        .to_authority_index(author as usize)
                        .unwrap(),
                );
                let block_headers = dag_state.get_uncommitted_block_headers_at_slot(slot);

                // We only write one block per slot for own index
                if AuthorityIndex::new_for_test(author) == own_index {
                    assert_eq!(block_headers.len(), 1);
                } else {
                    assert_eq!(block_headers.len(), num_blocks_per_slot);
                }

                for bh in block_headers {
                    assert_eq!(bh.round(), round);
                    assert_eq!(
                        bh.author(),
                        context
                            .committee
                            .to_authority_index(author as usize)
                            .unwrap()
                    );
                }
            }
        }

        // Check slots without uncommitted block headers.
        let slot = Slot::new(non_existent_round, AuthorityIndex::ZERO);
        assert!(
            dag_state
                .get_uncommitted_block_headers_at_slot(slot)
                .is_empty()
        );

        // Check rounds with uncommitted blocks.
        for round in 1..=num_rounds {
            let block_headers = dag_state.get_uncommitted_block_headers_at_round(round);
            // Expect 3 blocks per authority except for own authority which should
            // have 1 block.
            assert_eq!(
                block_headers.len(),
                (num_authorities - 1) as usize * num_blocks_per_slot + 1
            );
            for bh in block_headers {
                assert_eq!(bh.round(), round);
            }
        }

        // Check rounds without uncommitted block headers.
        assert!(
            dag_state
                .get_uncommitted_block_headers_at_round(non_existent_round)
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_ancestors_at_uncommitted_round() {
        // Initialize DagState.
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context, store);

        // Populate DagState.

        // Round 10 refs will not have their block headers in DagState.
        let round_10_refs: Vec<_> = (0..4)
            .map(|a| {
                VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(10, a).set_timestamp_ms(1000).build(),
                )
                .reference()
            })
            .collect();

        // Round 11 block headers.
        let round_11_headers = [
            // This will connect to round 12.
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(11, 0)
                    .set_timestamp_ms(1100)
                    .set_ancestors(round_10_refs.clone())
                    .build(),
            ),
            // Slot(11, 1) has 3 block headers.
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
                    .set_ancestors(round_10_refs)
                    .build(),
            ),
        ];

        // Round 12 block headers.
        let ancestors_for_round_12 = vec![
            round_11_headers[0].reference(),
            round_11_headers[1].reference(),
            round_11_headers[5].reference(),
        ];
        let round_12_headers = [
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
                    .set_ancestors(ancestors_for_round_12)
                    .build(),
            ),
        ];

        // Round 13 block headers.
        let ancestors_for_round_13 = vec![
            round_12_headers[0].reference(),
            round_12_headers[1].reference(),
            round_12_headers[2].reference(),
            round_11_headers[2].reference(),
        ];
        let round_13_headers = [
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
                    .set_ancestors(ancestors_for_round_13)
                    .build(),
            ),
        ];

        // Round 14 anchor block header.
        let ancestors_for_round_14 = round_13_headers.iter().map(|b| b.reference()).collect();
        let anchor = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(14, 1)
                .set_timestamp_ms(1410)
                .set_ancestors(ancestors_for_round_14)
                .build(),
        );

        // Add all block headers (at and above round 11) to DagState.
        for bh in round_11_headers
            .iter()
            .chain(round_12_headers.iter())
            .chain(round_13_headers.iter())
            .chain([anchor.clone()].iter())
        {
            dag_state.accept_block_header(bh.clone(), DataSource::Test);
        }

        // Check ancestors connected to anchor.
        let ancestors = dag_state.ancestors_at_round(&anchor, 11);
        let mut ancestors_refs: Vec<BlockRef> = ancestors.iter().map(|b| b.reference()).collect();
        ancestors_refs.sort();
        let mut expected_refs = vec![
            round_11_headers[0].reference(),
            round_11_headers[1].reference(),
            round_11_headers[2].reference(),
            round_11_headers[5].reference(),
        ];
        expected_refs.sort(); // we need to sort as block headers with same author and round of round 11 (position 1
        // & 2) might not be in right lexicographical order.
        assert_eq!(
            ancestors_refs, expected_refs,
            "Expected round 11 ancestors: {expected_refs:?}. Got: {ancestors_refs:?}",
        );
    }

    #[tokio::test]
    async fn test_contains_block_headers_in_cache_or_store() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test block headers for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u8 = 4;
        let mut block_headers = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block_header =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                block_headers.push(block_header);
            }
        }

        // Now write in store the block headers from the first 4 rounds and the rest to
        // the dag state
        block_headers.clone().into_iter().for_each(|block_header| {
            if block_header.round() <= 4 {
                store
                    .write(
                        WriteBatch::default().block_headers(vec![block_header]),
                        context.clone(),
                    )
                    .unwrap();
            } else {
                dag_state.accept_block_headers(vec![block_header], DataSource::Test);
            }
        });

        // Now when trying to query whether we have all the block headers, we should
        // receive a positive answer for all headers. Headers from the first 4 rounds
        // should be found in store and the rest in DagState.
        let mut block_refs = block_headers
            .iter()
            .map(|header| header.reference())
            .collect::<Vec<_>>();
        let result = dag_state.contains_block_headers(block_refs.clone());

        // Ensure everything is found
        let mut expected = vec![true; (num_rounds * num_authorities as u32) as usize];
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

        let num_authorities: u8 = 4;
        let (mut context, _) = Context::new_for_test(num_authorities as usize);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create test block headers for round 1 ~ 10
        let num_rounds: u32 = 10;
        let mut block_headers = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block_header =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                block_headers.push(block_header.clone());
                dag_state.accept_block_header(block_header, DataSource::Test);
            }
        }

        // Query for genesis round 0, genesis block headers should be returned
        for (author, _) in context.committee.authorities() {
            assert!(
                dag_state.contains_cached_block_header_at_slot(Slot::new(GENESIS_ROUND, author)),
                "Genesis should always be found"
            );
        }

        // Now when trying to query whether we have all the block headers, we should
        // receive a positive answer for all headers
        let mut block_refs = block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();

        for block_ref in block_refs.clone() {
            let slot = block_ref.into();
            let found = dag_state.contains_cached_block_header_at_slot(slot);
            assert!(found, "A block should be found at slot {slot}");
        }

        // Now try to ask also for one block ref that is not in the cache
        // Then all should be found apart from the last one
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
        );
        let mut expected = vec![true; (num_rounds * num_authorities as u32) as usize];
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
        expected = "Attempted to check for slot S8[0] that is <= the last evicted round 8"
    )]
    async fn test_contains_cached_block_at_slot_panics_when_ask_out_of_range() {
        /// Only keep elements up to 2 rounds before the last committed round
        const CACHED_ROUNDS: Round = 2;

        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create test block headers for round 1 ~ 10 for authority 0
        let mut block_headers = Vec::new();
        for round in 1..=10 {
            let block_header =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build());
            block_headers.push(block_header.clone());
            dag_state.accept_block_header(block_header, DataSource::Test);
        }

        // Now add a commit and flush to trigger an eviction
        dag_state.add_commit(TrustedCommit::new_for_test(
            &context,
            1 as CommitIndex,
            CommitDigest::MIN,
            0,
            block_headers.last().unwrap().reference(),
            block_headers
                .into_iter()
                .map(|block_header| block_header.reference())
                .collect::<Vec<_>>(),
            vec![],
        ));

        dag_state.flush();

        // When trying to request a header from authority 0 at round 8, it should panic,
        // as anything that is <= commit_round - cached_rounds = 10 - 2 = 8 should be
        // evicted.
        let _ = dag_state
            .contains_cached_block_header_at_slot(Slot::new(8, AuthorityIndex::new_for_test(0)));
    }

    #[tokio::test]
    async fn test_get_block_headers_in_cache_or_store() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test block headers for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u8 = 4;
        let mut block_headers = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block_header =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
                block_headers.push(block_header);
            }
        }

        // Now write the block headers from the first 4 rounds to the store, and the
        // rest to the dag state
        block_headers.clone().into_iter().for_each(|block_header| {
            if block_header.round() <= 4 {
                store
                    .write(
                        WriteBatch::default().block_headers(vec![block_header]),
                        context.clone(),
                    )
                    .unwrap();
            } else {
                dag_state.accept_block_headers(vec![block_header], DataSource::Test);
            }
        });

        // Now when trying to query whether we have all the block headers, we should
        // receive all headers. Headers from the first 4 rounds
        // should be found in store and the rest in DagState.
        let mut block_refs = block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();
        // Collect genesis block headers
        let genesis_headers = dag_state.genesis_block_headers();

        // Prepend genesis block references to block_refs
        let mut genesis_refs = genesis_headers
            .iter()
            .map(|h| h.reference())
            .collect::<Vec<_>>();
        genesis_refs.extend(block_refs);
        block_refs = genesis_refs;

        let result = dag_state.get_verified_block_headers(&block_refs);

        let mut expected_headers = block_headers
            .clone()
            .into_iter()
            .map(Some)
            .collect::<Vec<Option<VerifiedBlockHeader>>>();
        // Prepend genesis headers to expected
        let mut genesis_expected = genesis_headers.into_iter().map(Some).collect::<Vec<_>>();
        genesis_expected.extend(expected_headers);
        expected_headers = genesis_expected;

        // Ensure everything is found
        assert_eq!(result, expected_headers.clone());

        // Now try to find only cached headers
        let result_cached = dag_state.get_cached_block_headers(&block_refs);
        // Ensure everything is found in the cache for rounds > 4
        let expected_cached = expected_headers
            .iter()
            .map(|oh| {
                oh.as_ref()
                    .filter(|h| h.round() > 4 || h.round() == 0)
                    .cloned()
            })
            .collect::<Vec<_>>();
        assert_eq!(result_cached, expected_cached);

        // Now try to ask also for one block ref that is neither in cache nor in store
        block_refs.insert(
            3,
            BlockRef::new(
                11,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
        );
        let result = dag_state.get_verified_block_headers(&block_refs);

        // Then all should be found apart from the last one
        expected_headers.insert(3, None);
        assert_eq!(result, expected_headers);
    }

    #[rstest]
    #[tokio::test]
    async fn test_flush_and_recovery(#[values(true, false)] consensus_fast_commit_sync: bool) {
        telemetry_subscribers::init_for_testing();
        let num_authorities: u32 = 4;
        let (mut context, _) = Context::new_for_test(num_authorities as usize);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks and commits for round 1 ~ 10
        let num_rounds: u32 = 10;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=num_rounds).build();
        let mut commits = vec![];
        let mut subdag_bases = vec![];
        for (subdag, commit) in dag_builder.get_sub_dag_and_commits(1..=num_rounds) {
            subdag_bases.push(subdag.base);
            commits.push(commit);
        }

        // Add the block headers and transactions from the first 5 rounds and the first
        // 5 commits to the dag state; also add commit info after the second
        // commit.
        let later_commits = commits.split_off(5);
        let _later_subdag_bases = subdag_bases.split_off(5);
        dag_state.accept_block_headers(dag_builder.block_headers(1..=5), DataSource::Test);
        for verified_transactions in dag_builder.transactions(1..=5).into_iter() {
            dag_state.add_transactions(verified_transactions, DataSource::Test);
        }

        for commit in commits.clone() {
            dag_state.add_commit(commit.clone());
            if commit.index() == 2 {
                dag_state.add_commit_info(ReputationScores::default());
            }
        }
        dag_state.update_last_solid_subdag_base(subdag_bases.last().unwrap().clone());

        // Flush the dag state
        dag_state.flush();

        let (commit_ref, commit_info) = dag_state.recover_last_commit_info().unwrap();
        assert_eq!(commit_ref, commits[1].reference());
        assert_eq!(commit_info.committed_rounds, [1, 1, 2, 1]);

        // Add the rest of the block headers, transaction, and commits to the dag state
        dag_state.accept_block_headers(dag_builder.block_headers(6..=num_rounds), DataSource::Test);
        for verified_transactions in dag_builder.transactions(6..=num_rounds).into_iter() {
            dag_state.add_transactions(verified_transactions, DataSource::Test);
        }
        for commit in later_commits {
            dag_state.add_commit(commit);
        }

        // All block headers should be found in DagState.
        let mut all_block_headers = dag_state.genesis_block_headers();
        all_block_headers.extend(dag_builder.block_headers(1..=num_rounds));

        let block_refs = all_block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();

        let result = dag_state
            .get_verified_block_headers(&block_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(result, all_block_headers);

        // Collect genesis transactions
        let mut all_transactions = dag_state
            .genesis_blocks()
            .into_iter()
            .map(|b| b.verified_transactions)
            .collect::<Vec<_>>();

        // Extend with the rest of the transactions
        all_transactions.extend(dag_builder.transactions(1..=num_rounds));

        // All transactions should be found in DagState.
        let transactions_refs = if consensus_fast_commit_sync {
            all_block_headers
                .iter()
                .map(|bh| GenericTransactionRef::TransactionRef(bh.transaction_ref()))
                .collect::<Vec<_>>()
        } else {
            block_refs
                .iter()
                .map(|&br| GenericTransactionRef::from(br))
                .collect::<Vec<_>>()
        };
        let result = dag_state
            .get_verified_transactions(transactions_refs.as_slice())
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, all_transactions);

        // The last commit index should be 10.
        assert_eq!(dag_state.last_commit_index(), 10);
        assert_eq!(
            dag_state.last_committed_rounds(),
            dag_builder.last_committed_rounds.clone()
        );

        // Check the last proposed block
        assert_eq!(
            dag_state.get_last_own_non_genesis_block(),
            Some(dag_builder.blocks(num_rounds..=num_rounds)[0].clone())
        );

        // Destroy the dag state.
        drop(dag_state);

        // Recover the state from the store
        let dag_state = DagState::new(context, store);

        // Block headers from the first 5 rounds should be found in DagState.
        let block_headers = dag_builder.block_headers(1..=5);
        let block_refs = block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();
        let result = dag_state
            .get_verified_block_headers(&block_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, block_headers);
        // Transactions from the first 5 rounds should be found in DagState.
        let vec_transactions = dag_builder.transactions(1..=5);
        let transactions_refs = if consensus_fast_commit_sync {
            block_headers
                .iter()
                .map(|bh| GenericTransactionRef::TransactionRef(bh.transaction_ref()))
                .collect::<Vec<_>>()
        } else {
            block_refs
                .iter()
                .map(|&br| GenericTransactionRef::from(br))
                .collect::<Vec<_>>()
        };
        let result = dag_state
            .get_verified_transactions(&transactions_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, vec_transactions);

        // The last proposed block should be from the round 5
        assert_eq!(
            dag_state.get_last_own_non_genesis_block(),
            Some(dag_builder.blocks(5..=5)[0].clone())
        );

        // Block headers and transactions above round 5 should not be in DagState,
        // because they are not flushed.
        let missing_block_headers = dag_builder.block_headers(6..=num_rounds);
        let block_refs = missing_block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();
        let retrieved_block_headers = dag_state
            .get_verified_block_headers(&block_refs)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(retrieved_block_headers.is_empty());
        let transactions_refs = if consensus_fast_commit_sync {
            missing_block_headers
                .iter()
                .map(|bh| GenericTransactionRef::TransactionRef(bh.transaction_ref()))
                .collect::<Vec<_>>()
        } else {
            block_refs
                .iter()
                .map(|&br| GenericTransactionRef::from(br))
                .collect::<Vec<_>>()
        };
        let retrieved_transactions = dag_state
            .get_verified_transactions(&transactions_refs)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert!(retrieved_transactions.is_empty());

        // The last commit index should be 5.
        assert_eq!(dag_state.last_commit_index(), 5);

        // This is the last_commit_rounds of the first 5 commits that were flushed
        let expected_last_committed_rounds = vec![4, 5, 4, 4];
        assert_eq!(
            dag_state.last_committed_rounds(),
            expected_last_committed_rounds
        );
        // Unscored subdags will be recovered based on the flushed commits and commit
        // info for 2 commits
        assert_eq!(dag_state.scoring_subdags_count(), 3);
    }

    #[tokio::test]
    async fn test_get_cached_block_headers() {
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = 5;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create no block headers for authority 0
        // Create one block header (round 10) for authority 1
        // Create two block headers (rounds 10,11) for authority 2
        // Create three block headers (rounds 10,11,12) for authority 3
        let mut all_block_headers = Vec::new();
        for author in 1..=3 {
            for round in 10..(10 + author) {
                let block_header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author as u8).build(),
                );
                all_block_headers.push(block_header.clone());
                dag_state.accept_block_header(block_header, DataSource::Test);
            }
        }

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(0).unwrap(),
            0,
        );
        assert!(cached_block_headers.is_empty());

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(1).unwrap(),
            10,
        );
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 10);

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(2).unwrap(),
            10,
        );
        assert_eq!(cached_block_headers.len(), 2);
        assert_eq!(cached_block_headers[0].round(), 10);
        assert_eq!(cached_block_headers[1].round(), 11);

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(2).unwrap(),
            11,
        );
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 11);

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(3).unwrap(),
            10,
        );
        assert_eq!(cached_block_headers.len(), 3);
        assert_eq!(cached_block_headers[0].round(), 10);
        assert_eq!(cached_block_headers[1].round(), 11);
        assert_eq!(cached_block_headers[2].round(), 12);

        let cached_block_headers = dag_state.get_cached_block_headers_since_round(
            context.committee.to_authority_index(3).unwrap(),
            12,
        );
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 12);

        // Test get_cached_block_headers_in_range()

        // Start == end
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(3).unwrap(),
            10,
            10,
            1,
        );
        assert!(cached_block_headers.is_empty());

        // Start > end
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(3).unwrap(),
            11,
            10,
            1,
        );
        assert!(cached_block_headers.is_empty());

        // Empty result.
        let cached_blocks = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(0).unwrap(),
            9,
            10,
            1,
        );
        assert!(cached_blocks.is_empty());

        // Single block, one round before the end.
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(1).unwrap(),
            9,
            11,
            1,
        );
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 10);

        // Respect end round.
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(2).unwrap(),
            9,
            12,
            5,
        );
        assert_eq!(cached_block_headers.len(), 2);
        assert_eq!(cached_block_headers[0].round(), 10);
        assert_eq!(cached_block_headers[1].round(), 11);

        // Respect start round.
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(3).unwrap(),
            11,
            20,
            5,
        );
        assert_eq!(cached_block_headers.len(), 2);
        assert_eq!(cached_block_headers[0].round(), 11);
        assert_eq!(cached_block_headers[1].round(), 12);

        // Respect limit
        let cached_block_headers = dag_state.get_cached_block_headers_in_range(
            context.committee.to_authority_index(3).unwrap(),
            10,
            20,
            1,
        );
        assert_eq!(cached_block_headers.len(), 1);
        assert_eq!(cached_block_headers[0].round(), 10);
    }

    #[tokio::test]
    async fn test_get_last_cached_block_header() {
        // GIVEN
        const CACHED_ROUNDS: Round = 2;
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create no block headers for authority 0
        // Create one block header (round 1) for authority 1
        // Create two block headers (rounds 1,2) for authority 2
        // Create three block headers (rounds 1,2,3) for authority 3
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
        let block_header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(2, 2).build());

        // Accept all block headers
        for block_header in dag_builder
            .all_block_headers()
            .into_iter()
            .chain(std::iter::once(block_header))
        {
            dag_state.accept_block_header(block_header, DataSource::Test);
        }

        dag_state.add_commit(TrustedCommit::new_for_test(
            &context,
            1 as CommitIndex,
            CommitDigest::MIN,
            context.clock.timestamp_utc_ms(),
            dag_builder.leader_block(3).unwrap().reference(),
            vec![],
            vec![],
        ));

        // WHEN search for the latest block headers
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
        // the internal cache. That will keep all the block headers with rounds >=
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
    async fn test_get_cached_last_block_header_per_authority_requesting_out_of_round_range() {
        // GIVEN
        const CACHED_ROUNDS: Round = 1;
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create no block headers for authority 0
        // Create one block header (round 1) for authority 1
        // Create two block headers (rounds 1,2) for authority 2
        // Create three block headers (rounds 1,2,3) for authority 3
        let mut all_blocks_headers = Vec::new();
        for author in 1..=3 {
            for round in 1..=author {
                let block_header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author as u8).build(),
                );
                all_blocks_headers.push(block_header.clone());
                dag_state.accept_block_header(block_header, DataSource::Test);
            }
        }

        dag_state.add_commit(TrustedCommit::new_for_test(
            &context,
            1 as CommitIndex,
            CommitDigest::MIN,
            0,
            all_blocks_headers.last().unwrap().reference(),
            all_blocks_headers
                .into_iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>(),
            vec![],
        ));

        // Flush to the store so we keep in memory only the last 1 round from the last
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
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // WHEN no block headers exist, then genesis should be returned
        {
            let genesis = genesis_block_headers(&context);

            assert_eq!(dag_state.read().last_quorum(), genesis);
        }

        // WHEN a fully connected DAG up to round 4 is created, then round 4 block
        // headers should be returned as quorum
        {
            let mut dag_builder = DagBuilder::new(context);
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

        // WHEN adding one more block header at round 5, still round 4 should be
        // returned as quorum
        {
            let block_header =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 0).build());
            dag_state
                .write()
                .accept_block_header(block_header, DataSource::Test);

            let round_4_block_headers = dag_state.read().get_uncommitted_block_headers_at_round(4);

            let last_quorum = dag_state.read().last_quorum();

            assert_eq!(last_quorum, round_4_block_headers);
        }
    }

    #[tokio::test]
    async fn test_last_block_header_for_authority() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // WHEN no block headers exist, then genesis should be returned
        {
            let genesis_headers = genesis_block_headers(&context);
            let my_genesis_header = genesis_headers
                .into_iter()
                .find(|block| block.author() == context.own_index)
                .unwrap();

            assert_eq!(
                dag_state.read().get_last_proposed_block_header(),
                my_genesis_header
            );
            let genesis_blocks = genesis_blocks(&context);
            let my_genesis_block = genesis_blocks
                .into_iter()
                .find(|block| block.author() == context.own_index)
                .unwrap();
            assert_eq!(
                dag_state.read().get_last_proposed_block_header(),
                my_genesis_block.verified_block_header
            );
        }

        // WHEN adding some block headers for authorities, only the last ones should be
        // returned
        {
            // add block headers up to round 4
            let mut dag_builder = DagBuilder::new(context.clone());
            dag_builder
                .layers(1..=4)
                .build()
                .persist_layers(dag_state.clone());

            // add block header 5 for authority 0
            let block_header =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 0).build());
            dag_state
                .write()
                .accept_block_header(block_header, DataSource::Test);

            for (authority_index, _) in context.committee.authorities() {
                let block_header = dag_state
                    .read()
                    .get_last_block_header_for_authority(authority_index);

                if authority_index.value() == 0 {
                    assert_eq!(block_header.round(), 5);
                } else {
                    assert_eq!(block_header.round(), 4);
                }
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_contains_transactions(#[values(true, false)] consensus_fast_commit_sync: bool) {
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u8 = 4;
        let mut blocks = Vec::new();

        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block =
                    VerifiedBlock::new_for_test(TestBlockHeader::new(round, author).build());
                blocks.push(block);
            }
        }

        // Now write the transactions from the first 4 rounds to the store and the rest
        // to the dag state
        blocks.clone().into_iter().for_each(|block| {
            if block.round() <= 4 {
                store
                    .write(
                        WriteBatch::default().transactions(vec![block.verified_transactions]),
                        context.clone(),
                    )
                    .unwrap();
            } else {
                dag_state.add_transactions(block.verified_transactions, DataSource::Test);
            }
        });

        // Now when trying to query whether we have all the transactions, we should
        // receive all transactions. The first 4 retrieved from the store and the rest
        // is from DagState.
        let mut transactions_refs = blocks
            .iter()
            .map(|block| {
                if consensus_fast_commit_sync {
                    GenericTransactionRef::from(block.transaction_ref())
                } else {
                    GenericTransactionRef::from(block.reference())
                }
            })
            .collect::<Vec<_>>();
        let result = dag_state.contains_transactions(transactions_refs.clone());

        // Ensure everything is found
        let mut expected = vec![true; (num_rounds * num_authorities as u32) as usize];
        assert_eq!(result, expected);

        // Now try to ask also for one block ref that is neither in cache nor in store
        let non_existent_ref = if consensus_fast_commit_sync {
            GenericTransactionRef::from(TransactionRef {
                round: 11,
                author: AuthorityIndex::new_for_test(0),
                transactions_commitment: TransactionsCommitment::default(),
            })
        } else {
            GenericTransactionRef::from(BlockRef::new(
                11,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ))
        };
        transactions_refs.insert(3, non_existent_ref);
        let result = dag_state.contains_transactions(transactions_refs);

        // Ensure everything is found except the one we just added
        expected.insert(3, false);
        assert_eq!(result, expected);

        // Destroy the dag state.
        drop(dag_state);

        // Recover the state from the store
        let dag_state = DagState::new(context, store);

        let transactions_refs = blocks
            .iter()
            .map(|block| {
                if consensus_fast_commit_sync {
                    GenericTransactionRef::from(block.transaction_ref())
                } else {
                    GenericTransactionRef::from(block.reference())
                }
            })
            .collect::<Vec<_>>();
        let result = dag_state.contains_transactions(transactions_refs);

        // Only transactions flushed to the store should be found
        let expected = (1..=num_rounds)
            .flat_map(|round| vec![round <= 4; num_authorities as usize])
            .collect::<Vec<_>>();
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_no_panic_on_future_timestamp() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);

        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        let future_timestamp = context.clock.timestamp_utc_ms() + 100_000;
        let block_header = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(1, 1)
                .set_timestamp_ms(future_timestamp)
                .build(),
        );
        dag_state.accept_block_header(block_header.clone(), DataSource::Test);

        let accepted_header = dag_state
            .recent_block_headers
            .get(&block_header.reference())
            .unwrap();

        assert_eq!(accepted_header, &block_header);
    }

    #[rstest]
    #[tokio::test]
    async fn test_eviction(#[values(true, false)] consensus_fast_commit_sync: bool) {
        telemetry_subscribers::init_for_testing();
        let num_authorities: u32 = 4;
        let (mut context, _) = Context::new_for_test(num_authorities as usize);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        const CACHED_ROUNDS: Round = 5;
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);

        // Create test blocks and commits for round 1 ~ 200
        let num_rounds: u32 = 200;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=num_rounds).build();
        let mut commits = vec![];
        let mut subdag_bases = vec![];
        for (subdag, commit) in dag_builder.get_sub_dag_and_commits(1..=num_rounds) {
            subdag_bases.push(subdag.base);
            commits.push(commit);
        }

        dag_state.accept_block_headers(dag_builder.block_headers(1..=num_rounds), DataSource::Test);
        for verified_transactions in dag_builder.transactions(1..=num_rounds).into_iter() {
            dag_state.add_transactions(verified_transactions, DataSource::Test);
        }

        for commit in commits.clone() {
            dag_state.add_commit(commit.clone());
        }
        dag_state.update_last_solid_subdag_base(subdag_bases.last().unwrap().clone());

        // Flush the dag state (eviction is happening inside the flush)
        dag_state.flush();

        let last_committed_round = dag_state.last_committed_rounds();
        let expected_committed_rounds = (0..num_authorities)
            .map(|x| {
                num_rounds - 1 + (x % (num_authorities) == num_rounds % (num_authorities)) as u32
            })
            .collect::<Vec<_>>();
        assert_eq!(last_committed_round, expected_committed_rounds);

        let mut all_block_headers = dag_state.genesis_block_headers();
        all_block_headers.extend(dag_builder.block_headers(1..=num_rounds));

        let block_refs = all_block_headers
            .iter()
            .map(|block_header| block_header.reference())
            .collect::<Vec<_>>();

        let result = dag_state
            .get_cached_block_headers(&block_refs)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let expected_block_headers = all_block_headers
            .iter()
            .filter(|x| {
                x.round() > last_committed_round[x.author().value()] - CACHED_ROUNDS
                    || x.round() == GENESIS_ROUND
            })
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(result, expected_block_headers);

        // Collect genesis transactions
        let mut all_transactions = dag_state
            .genesis_blocks()
            .into_iter()
            .map(|b| b.verified_transactions)
            .collect::<Vec<_>>();

        // Extend with the rest of the transactions
        all_transactions.extend(dag_builder.transactions(1..=num_rounds));
        let gc_round = dag_state.gc_round_for_last_commit();

        let block_refs_with_transactions_in_dag: Vec<BlockRef> = block_refs
            .iter()
            .filter(|x| x.round > gc_round)
            .cloned()
            .collect();

        // Get block headers above GC round
        let block_headers_above_gc = dag_builder.block_headers(gc_round + 1..=num_rounds);

        // Create appropriate transaction refs based on the flag
        let transaction_refs = if consensus_fast_commit_sync {
            block_headers_above_gc
                .iter()
                .map(|bh| GenericTransactionRef::TransactionRef(bh.transaction_ref()))
                .collect::<Vec<_>>()
        } else {
            block_refs_with_transactions_in_dag
                .iter()
                .map(|br| GenericTransactionRef::from(*br))
                .collect::<Vec<_>>()
        };

        let expected_transactions_in_dag = dag_builder.transactions(gc_round + 1..=num_rounds);
        // All transactions should be found in DagState or store.
        let result = dag_state
            .get_verified_transactions(&transaction_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, expected_transactions_in_dag);

        assert_eq!(
            dag_state
                .context
                .metrics
                .node_metrics
                .dag_state_store_read_count
                .with_label_values(&["get_transactions"])
                .get(),
            0,
            "dag_state_store_read_count for get_transactions should be zero"
        );

        // All transactions should be found in DagState or store.
        let transaction_refs = if consensus_fast_commit_sync {
            all_block_headers
                .iter()
                .map(|bh| GenericTransactionRef::TransactionRef(bh.transaction_ref()))
                .collect::<Vec<_>>()
        } else {
            block_refs
                .iter()
                .map(|br| GenericTransactionRef::from(*br))
                .collect::<Vec<_>>()
        };

        let result = dag_state
            .get_verified_transactions(&transaction_refs)
            .into_iter()
            .map(|b| b.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result, all_transactions);
        // But some of them are already evicted and can be found only in the store.
        assert_eq!(
            dag_state
                .context
                .metrics
                .node_metrics
                .dag_state_store_read_count
                .with_label_values(&["get_verified_transactions"])
                .get(),
            1,
            "dag_state_store_read_count for get_transactions should be one"
        );

        // Calculate the eviction round for acknowledgments
        let clock_round = dag_state.threshold_clock_round();
        let acknowledgements_eviction_round =
            clock_round.saturating_sub(context.protocol_config.gc_depth() + 1);

        // Verify that for all blocks with round > eviction round, we have an
        // acknowledgement
        for block_ref in block_refs
            .iter()
            .filter(|b| b.round > acknowledgements_eviction_round)
        {
            assert!(
                dag_state.pending_acknowledgments.contains(block_ref),
                "Missing acknowledgment for block {:?} (round {})",
                block_ref,
                block_ref.round
            );
        }

        // Verify that for all blocks with round <= eviction round, there is no
        // acknowledgement
        for block_ref in block_refs
            .iter()
            .filter(|b| b.round <= acknowledgements_eviction_round)
        {
            assert!(
                !dag_state.pending_acknowledgments.contains(block_ref),
                "Unexpected acknowledgment for block {:?} (round {})",
                block_ref,
                block_ref.round
            );
        }
    }

    #[tokio::test]
    async fn test_accept_block_not_panics_when_timestamp_is_ahead() {
        // GIVEN
        let context = Arc::new(Context::new_for_test(4).0);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store);
        // Set a timestamp for the block that is ahead of the current time
        let block_timestamp = context.clock.timestamp_utc_ms() + 5_000;
        let block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(10, 0)
                .set_timestamp_ms(block_timestamp)
                .build(),
        );
        // Try to accept the block - it should not panic
        dag_state.accept_block_header(block, DataSource::Test);
    }

    #[tokio::test]
    async fn test_skip_acknowledgments_all_empty_transactions() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context, store);

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u8 = 4;
        let mut blocks = Vec::new();
        // create blocks
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block =
                    VerifiedBlock::new_for_test(TestBlockHeader::new(round, author).build());
                blocks.push(block);
            }
        }

        // add transactions for all blocks
        blocks.into_iter().for_each(|block| {
            dag_state.add_transactions(block.verified_transactions, DataSource::Test);
        });

        assert!(dag_state.pending_acknowledgments.is_empty());
    }

    #[tokio::test]
    async fn test_skip_acknowledgments_some_contain_transactions() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut encoder = create_encoder(&context);
        let mut dag_state = DagState::new(context.clone(), store);

        // Create test blocks for round 1 ~ 10
        let num_rounds: u32 = 10;
        let num_authorities: u8 = 4;
        // create blocks
        let mut block_refs_with_transactions = Vec::new();
        for round in 1..=num_rounds {
            for author in 0..num_authorities {
                let block_ref = BlockRef::new(round, author.into(), BlockHeaderDigest::default());
                let transactions = if round > 5 {
                    block_refs_with_transactions.push(block_ref);
                    vec![Transaction::random_transaction(64)]
                } else {
                    vec![]
                };
                let serialized = Transaction::serialize(&transactions).unwrap();
                let transaction_commitment =
                    TransactionsCommitment::compute_transactions_commitment(
                        &serialized,
                        &context,
                        &mut encoder,
                    )
                    .unwrap();
                let verified_transaction = VerifiedTransactions::new(
                    transactions,
                    TransactionRef::new(block_ref, transaction_commitment),
                    Some(block_ref.digest),
                    serialized,
                );
                dag_state.add_transactions(verified_transaction, DataSource::Test);
            }
        }
        assert_eq!(
            dag_state.pending_acknowledgments.len(),
            block_refs_with_transactions.len()
        );
        for block_ref in block_refs_with_transactions.iter() {
            assert!(dag_state.pending_acknowledgments.contains(block_ref));
        }
    }
}
