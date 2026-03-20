// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    iter,
    sync::Arc,
    time::Duration,
    vec,
};

use iota_macros::fail_point;
#[cfg(test)]
use iota_metrics::monitored_mpsc::{UnboundedReceiver, unbounded_channel};
use iota_metrics::monitored_scope;
use itertools::Itertools as _;
use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, ProtocolKeyPair};
#[cfg(test)]
use starfish_config::{Stake, local_committee_and_keys};
use tokio::{
    sync::{broadcast, watch},
    time::Instant,
};
use tracing::{debug, info, instrument, trace, warn};

#[cfg(test)]
use crate::storage::Store;
#[cfg(test)]
use crate::storage::rocksdb_store::RocksDBStore;
#[cfg(test)]
use crate::{CommitConsumer, CommittedSubDag, TransactionClient, storage::mem_store::MemStore};
use crate::{
    Transaction,
    block_header::{
        BlockHeader, BlockHeaderAPI, BlockHeaderV1, BlockRef, BlockTimestampMs, GENESIS_ROUND,
        Round, SignedBlockHeader, Slot, TransactionsCommitment, VerifiedBlock, VerifiedBlockHeader,
        VerifiedOwnShard, VerifiedTransactions,
    },
    block_manager::BlockManager,
    commit::{CertifiedCommits, CommitAPI, PendingSubDag},
    commit_observer::{CommitObserver, CommittedSubDagSource},
    commit_syncer::fast::FastSyncOutput,
    context::Context,
    dag_state::{DagState, DataSource},
    encoder::{ShardEncoder, create_encoder},
    error::{ConsensusError, ConsensusResult},
    leader_schedule::LeaderSchedule,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    transaction::TransactionConsumer,
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _},
    universal_committer::{
        UniversalCommitter, universal_committer_builder::UniversalCommitterBuilder,
    },
};

// Maximum number of commit votes to include in a block.
// TODO: Move to protocol config, and verify in BlockVerifier.
const MAX_COMMIT_VOTES_PER_BLOCK: usize = 100;

pub(crate) struct Core {
    context: Arc<Context>,
    /// The consumer to use in order to pull transactions to be included for the
    /// next proposals
    transaction_consumer: TransactionConsumer,
    /// The block manager which is responsible for keeping track of the DAG
    /// dependencies when processing new blocks and accept them or suspend
    /// if we are missing their causal history
    block_manager: BlockManager,
    /// Whether there is a quorum of 2f+1 subscribers waiting for new blocks
    /// proposed by this authority. Core stops proposing new blocks when
    /// there is not enough subscribers, because new proposed blocks will
    /// not be sufficiently propagated to the network.
    quorum_subscribers_exists: bool,

    /// Used to make commit decisions for leader blocks in the dag.
    committer: UniversalCommitter,
    /// The last new round for which core has sent out a signal.
    last_signaled_round: Round,
    /// The blocks of the last included ancestors per authority. This vector is
    /// basically used as a watermark in order to include in the next block
    /// proposal only ancestors of higher rounds. By default, is initialised
    /// with `None` values.
    last_included_ancestors: Vec<Option<BlockRef>>,
    /// The last decided leader returned from the universal committer. Important
    /// to note that this does not signify that the leader has been
    /// persisted yet as it still has to go through CommitObserver and
    /// persist the commit in store. On recovery/restart
    /// the last_decided_leader will be set to the last_commit leader in dag
    /// state.
    last_decided_leader: Slot,
    /// The consensus leader schedule to be used to resolve the leader for a
    /// given round.
    leader_schedule: Arc<LeaderSchedule>,
    /// The commit observer is responsible for observing the commits and
    /// collecting
    /// + sending subdags over the consensus output channel.
    commit_observer: CommitObserver,
    /// Sender of outgoing signals from Core.
    signals: CoreSignals,
    /// The keypair to be used for block signing
    block_signer: ProtocolKeyPair,
    /// Keeping track of state of the DAG, including blocks, commits and last
    /// committed rounds.
    dag_state: Arc<RwLock<DagState>>,
    /// The last known round for which the node has proposed. Any proposal
    /// should be for a round > of this. This is currently being used to
    /// avoid equivocations during a node recovering from amnesia. When value is
    /// None it means that the last block sync mechanism is enabled, but it
    /// hasn't been initialised yet.
    last_known_proposed_round: Option<Round>,
    /// Encoder is used to encode transactions into a longer vector of shards
    encoder: Box<dyn ShardEncoder + Send + Sync>,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub(crate) enum ReasonToCreateBlock {
    MinBlockDelayTimeout,
    AddBlock,
    AddBlockHeader,
    MaxLeaderTimeout,
    Recover,
    QuorumSubscribersExist,
    KnownLastBlock,
    #[allow(dead_code)]
    FastSyncComplete,
}

impl ReasonToCreateBlock {
    fn label(&self) -> &'static str {
        match self {
            ReasonToCreateBlock::MinBlockDelayTimeout => "MinBlockDelayTimeout",
            ReasonToCreateBlock::AddBlock => "AddBlock",
            ReasonToCreateBlock::MaxLeaderTimeout => "MaxLeaderTimeout",
            ReasonToCreateBlock::AddBlockHeader => "AddBlockHeader",
            ReasonToCreateBlock::Recover => "Recover",
            ReasonToCreateBlock::QuorumSubscribersExist => "QuorumSubscribersExist",
            ReasonToCreateBlock::KnownLastBlock => "KnownLastBlock",
            ReasonToCreateBlock::FastSyncComplete => "FastSyncComplete",
        }
    }

    // Some reason are forcing block creation, bypassing several checks such as
    // existence of a leader in quorum round and min timeout
    fn is_forced(&self) -> bool {
        match self {
            ReasonToCreateBlock::MinBlockDelayTimeout => false,
            ReasonToCreateBlock::AddBlock => false,
            ReasonToCreateBlock::MaxLeaderTimeout => true,
            ReasonToCreateBlock::AddBlockHeader => false,
            ReasonToCreateBlock::Recover => true,
            ReasonToCreateBlock::QuorumSubscribersExist => true,
            ReasonToCreateBlock::KnownLastBlock => true,
            ReasonToCreateBlock::FastSyncComplete => true,
        }
    }
}

impl Core {
    pub(crate) fn new(
        context: Arc<Context>,
        leader_schedule: Arc<LeaderSchedule>,
        transaction_consumer: TransactionConsumer,
        block_manager: BlockManager,
        quorum_subscribers_exists: bool,
        commit_observer: CommitObserver,
        signals: CoreSignals,
        block_signer: ProtocolKeyPair,
        dag_state: Arc<RwLock<DagState>>,
        sync_last_known_own_block: bool,
    ) -> Self {
        let last_decided_leader = dag_state.read().last_commit_leader();
        let committer = UniversalCommitterBuilder::new(
            context.clone(),
            leader_schedule.clone(),
            dag_state.clone(),
        )
        .build();

        // Recover the last proposed block
        let last_proposed_block_header = dag_state.read().get_last_proposed_block_header();

        let last_signaled_round = last_proposed_block_header.round();

        // Recover the last included ancestor rounds based on the last proposed block.
        // That will allow to perform the next block proposal by using ancestor
        // blocks of higher rounds and avoid re-including blocks that have been
        // already included in the last (or earlier) block proposal.
        // This is only strongly guaranteed for a quorum of ancestors. It is still
        // possible to re-include a block from an authority which hadn't been
        // added as part of the last proposal hence its latest included ancestor
        // is not accurately captured here. This is considered a small deficiency,
        // and it mostly matters just for this next proposal without any actual
        // penalties in performance or block proposal.
        let mut last_included_ancestors = vec![None; context.committee.size()];
        for ancestor in last_proposed_block_header.ancestors() {
            last_included_ancestors[ancestor.author] = Some(*ancestor);
        }

        let min_propose_round = if sync_last_known_own_block {
            None
        } else {
            // if the sync is disabled then we practically don't want to impose any
            // restriction.
            Some(0)
        };

        let encoder = create_encoder(&context);

        Self {
            context,
            last_signaled_round,
            last_included_ancestors,
            last_decided_leader,
            leader_schedule,
            transaction_consumer,
            block_manager,
            quorum_subscribers_exists,
            committer,
            commit_observer,
            signals,
            block_signer,
            dag_state,
            last_known_proposed_round: min_propose_round,
            encoder,
        }
        .recover()
    }

    fn recover(mut self) -> Self {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::recover"])
            .start_timer();
        // Check ancestor timestamps
        let ancestor_block_headers = self
            .dag_state
            .read()
            .get_last_cached_block_header_per_authority(Round::MAX);
        let max_ancestor_timestamp = ancestor_block_headers
            .iter()
            .fold(0, |ts, (b, _)| ts.max(b.timestamp_ms()));
        let wait_ms = max_ancestor_timestamp.saturating_sub(self.context.clock.timestamp_utc_ms());

        // NEW mode: no waiting on timestamp drift
        if wait_ms > 0 {
            info!(
                "Median based timestamp is enabled. Will not wait for {} ms while recovering ancestors from storage",
                wait_ms
            );
        }

        // Try to commit and propose, since they may not have run after the last write
        // to storage. The returned committed subdags and missing transaction
        // refs can be ignored as the missing transactions will be fetched by the
        // periodic transactions' synchronizer.
        self.try_commit(CommittedSubDagSource::Recover).unwrap();
        let last_own_non_genesis_block =
            match self.try_propose(ReasonToCreateBlock::Recover).unwrap() {
                (Some(block), _) => Some(block),
                (None, _) => {
                    if let Some(last_proposed_block) =
                        self.dag_state.read().get_last_own_non_genesis_block()
                    {
                        // if no new block proposed then just re-broadcast the last proposed one to
                        // ensure liveness.
                        self.signals.new_block(last_proposed_block.clone()).unwrap();
                        Some(last_proposed_block)
                    } else {
                        None
                    }
                }
            };

        // Try to set up leader timeout if needed.
        // This needs to be called after try_commit() and try_propose(), which may
        // have advanced the threshold clock round.
        self.try_signal_new_round();

        info!(
            "Core recovery completed with last proposed (non-genesis) block {:?}",
            last_own_non_genesis_block.map(|b| b.verified_block_header)
        );

        self
    }

    /// Processes the provided blocks and accepts them if possible when their
    /// causal history exists. The method also uses the input bool variable if
    /// this call is known to be about not old blocks. The method returns:
    /// - The references of ancestors missing their block
    /// - The references of committed transactions that are missing
    #[tracing::instrument("consensus_add_blocks", skip_all)]
    pub(crate) fn add_blocks(
        &mut self,
        blocks: Vec<VerifiedBlock>,
        source: DataSource,
    ) -> ConsensusResult<(
        BTreeSet<BlockRef>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _scope = monitored_scope("Core::add_blocks");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::add_blocks"])
            .start_timer();
        self.context
            .metrics
            .node_metrics
            .core_add_blocks_batch_size
            .observe(blocks.len() as f64);
        let (accepted_blocks_headers, missing_block_refs) =
            self.block_manager.try_accept_blocks(blocks, source);

        let missing_committed_txns = if !accepted_blocks_headers.is_empty() {
            debug!(
                "Accepted block headers: {}",
                accepted_blocks_headers
                    .iter()
                    .map(|b| b.reference().to_string())
                    .join(",")
            );

            // Try to commit the new blocks if possible.
            let (_subdags, new_missing_committed_txns) =
                self.try_commit(CommittedSubDagSource::Consensus)?;

            // Try to propose now since there are new blocks accepted.
            self.try_propose(ReasonToCreateBlock::AddBlock)?;

            // Now set up leader timeout if needed.
            // This needs to be called after try_commit() and try_propose(), which may
            // have advanced the threshold clock round.
            self.try_signal_new_round();

            new_missing_committed_txns
        } else {
            BTreeMap::new()
        };

        if !missing_block_refs.is_empty() {
            trace!(
                "Missing block refs: {}",
                missing_block_refs.iter().map(|b| b.to_string()).join(", ")
            );
        }
        Ok((missing_block_refs, missing_committed_txns))
    }

    /// Processes the provided block headers and accepts them if possible when
    /// their causal history exists. The method returns:
    /// - The references of ancestors missing their block header
    /// - The references of committed transactions that are missing
    #[tracing::instrument(skip_all)]
    pub(crate) fn add_block_headers(
        &mut self,
        block_headers: Vec<VerifiedBlockHeader>,
        source: DataSource,
    ) -> ConsensusResult<(
        BTreeSet<BlockRef>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _scope = monitored_scope("Core::add_block_headers");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::add_block_headers"])
            .start_timer();
        self.context
            .metrics
            .node_metrics
            .core_add_block_headers_batch_size
            .observe(block_headers.len() as f64);
        let (accepted_block_headers, missing_block_refs) = self
            .block_manager
            .try_accept_block_headers(block_headers, source);

        let missing_committed_txns = if !accepted_block_headers.is_empty() {
            debug!(
                "Accepted block headers: {}",
                accepted_block_headers
                    .iter()
                    .map(|b| b.reference().to_string())
                    .join(",")
            );

            // Try to commit the new blocks if possible.
            let (_subdags, new_missing_committed_txns) =
                self.try_commit(CommittedSubDagSource::Consensus)?;

            // Try to propose now since there are new blocks accepted.
            self.try_propose(ReasonToCreateBlock::AddBlockHeader)?;

            // Now set up leader timeout if needed.
            // This needs to be called after try_commit() and try_propose(), which may
            // have advanced the threshold clock round.
            self.try_signal_new_round();

            new_missing_committed_txns
        } else {
            BTreeMap::new()
        };

        if !missing_block_refs.is_empty() {
            trace!(
                "Missing block refs: {}",
                missing_block_refs.iter().map(|b| b.to_string()).join(", ")
            );
        }
        Ok((missing_block_refs, missing_committed_txns))
    }

    /// Adds transactions to the DAG state. This is called when transactions are
    /// fetched from peers.
    pub(crate) fn add_transactions(
        &mut self,
        transactions: Vec<VerifiedTransactions>,
        source: DataSource,
    ) -> ConsensusResult<()> {
        let _scope = monitored_scope("Core::add_transactions");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::add_transactions"])
            .start_timer();

        // Add transactions to the dag state.
        let mut dag_state_guard = self.dag_state.write();
        for transaction in transactions {
            dag_state_guard.add_transactions(transaction, source);
        }
        // Safe to drop the guard here as the write/read locks will be acquired in
        // commit_observer
        drop(dag_state_guard);

        // After adding transactions, some pending subdags might be committable.
        // Commit observer is called with an empty vector of new leaders to check if all
        // transactions are available for any currently pending subdags, without
        // creating any new commits.
        self.commit_observer
            .handle_committed_leaders(Vec::new(), CommittedSubDagSource::Consensus)?;

        Ok(())
    }

    /// Adds shards to the DAG state. The proof is assumed to be already checked
    pub(crate) fn add_shards(
        &mut self,
        serialized_shards: Vec<VerifiedOwnShard>,
    ) -> ConsensusResult<()> {
        let _scope = monitored_scope("Core::add_shards");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::add_shards"])
            .start_timer();

        // Add shards to the dag state.
        let mut dag_state_guard = self.dag_state.write();
        for serialized_shard in serialized_shards {
            dag_state_guard.add_shard(serialized_shard);
        }
        // Safe to drop the guard here as the write/read locks will be acquired in
        // commit_observer
        drop(dag_state_guard);
        Ok(())
    }

    // Adds the certified commits that have been synced via the commit syncer. We
    // are using the commit info to skip running the decision
    // rule and immediately commit the corresponding leaders and sub dags.
    #[tracing::instrument(skip_all)]
    pub(crate) fn add_certified_commits(
        &mut self,
        certified_commits: CertifiedCommits,
    ) -> ConsensusResult<(
        BTreeSet<BlockRef>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _scope = monitored_scope("Core::add_certified_commits");

        // First, collect and add all transactions from certified commits.
        // Transactions must be added before processing commits to ensure they are
        // available when creating new commits.
        let all_transactions: Vec<VerifiedTransactions> = certified_commits
            .commits()
            .iter()
            .flat_map(|commit| commit.transactions())
            .cloned()
            .collect();

        if !all_transactions.is_empty() {
            self.add_transactions(all_transactions, DataSource::CommitSyncer)?;
        }

        // Then collect and add block headers.
        let block_headers = certified_commits
            .commits()
            .iter()
            .flat_map(|commit| commit.block_headers())
            .cloned()
            .collect::<Vec<_>>();

        // Add block headers in certified commits to the block manager.
        self.add_block_headers(block_headers, DataSource::CommitSyncer)
    }

    /// Handle committed subdags from fast sync.
    /// First stores the commits, transactions in DagState, then processes the
    /// subdags. Also updates the leader schedule from commits that contain
    /// reputation scores.
    ///
    /// This method follows a similar flow to `try_commit`:
    /// 1. Store commits and transactions in DagState
    /// 2. For commits with reputation scores, update leader schedule and store
    ///    CommitInfo
    /// 3. Flush to storage
    /// 4. Process subdags via commit_observer
    ///
    /// Commits must be stored so that recovery/reinitialization can rebuild
    /// the Linearizer's transaction acknowledgment tracker.
    /// Transactions must be stored so they are available for recovery and
    /// cache.
    pub(crate) fn handle_committed_sub_dags_from_fast_sync(
        &mut self,
        output: FastSyncOutput,
    ) -> ConsensusResult<()> {
        let FastSyncOutput {
            commits,
            committed_subdags,
            voting_block_headers,
        } = output;
        let _scope = monitored_scope("Core::handle_committed_sub_dags_from_fast_sync");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::handle_committed_sub_dags_from_fast_sync"])
            .start_timer();
        // First, store commits and transactions in DagState
        {
            let mut dag_state = self.dag_state.write();
            // Store commits for recovery and track those with reputation scores
            for commit in &commits {
                // Update leader schedule for commits with reputation scores.
                // This mirrors the flow in try_commit where update_leader_schedule is called
                // when commits_until_update reaches 0.
                let reputation_scores = commit.reputation_scores();
                if !reputation_scores.is_empty() {
                    // update_from_commit_scores will:
                    // 1. Clear scoring_subdag
                    // 2. Add commit_info for previous commit index to DagState
                    // 3. Update leader swap table
                    // 4. Update metrics
                    self.leader_schedule.update_from_commit_scores(
                        &mut dag_state,
                        commit.index(),
                        reputation_scores,
                    );
                }

                dag_state.add_commit(commit.clone());
            }

            // Store transactions for each subdag
            for subdag in &committed_subdags {
                for transactions in &subdag.transactions {
                    dag_state.add_transactions(transactions.clone(), DataSource::FastCommitSyncer);
                }
            }

            // Store voting block headers for later use when serving fetch_commits requests
            dag_state.add_voting_block_headers(voting_block_headers);
            dag_state.set_fast_sync_ongoing_flag(true);
        }

        // Flush commits to storage so they're available for
        // get_block_refs_for_recent_commits when close-to-quorum mode
        // triggers header fetching.
        self.dag_state.write().flush();

        // Then process subdags as usual
        self.commit_observer.finalize_and_send_solid_subdags(
            &[],
            &committed_subdags,
            CommittedSubDagSource::FastCommitSyncer,
        )
    }

    /// Reinitialize consensus components after fast sync completes.
    /// This stores block headers on disk and reinitializes DagState,
    /// BlockManager, and CommitObserver so that regular syncer can take
    /// over.
    ///
    /// Block headers should cover the cached_rounds window (~500 rounds).
    pub(crate) fn reinitialize_components(
        &mut self,
        block_headers: Vec<VerifiedBlockHeader>,
    ) -> ConsensusResult<()> {
        info!(
            "Reinitializing components with {} block headers",
            block_headers.len(),
        );

        // Hold the dag_state lock for the entire flow to ensure consistency
        let (last_commit_index, threshold_round, last_commit_leader) = {
            let mut dag_state = self.dag_state.write();

            // 1. Store block headers on disk
            dag_state.accept_block_headers(block_headers, DataSource::FastCommitSyncer);

            // 1.5. Clear fast sync flag (will be persisted with the flush)
            dag_state.set_fast_sync_ongoing_flag(false);

            // 2. Flush everything to storage
            dag_state.flush();

            // 3. Get current state before reinitializing
            let last_commit_index = dag_state.last_commit_index();

            // 4. Reinitialize DagState
            dag_state.reinitialize();

            let threshold_round = dag_state.threshold_clock_round();
            let last_commit_leader = dag_state.last_commit_leader();
            (last_commit_index, threshold_round, last_commit_leader)
        };

        // 5. Reinitialize LeaderSchedule from stored commit info
        self.leader_schedule.reinitialize(&self.dag_state);

        // 6. Reinitialize BlockManager
        self.block_manager.reinitialize();

        // 7. Update last_decided_leader to match the new DAG state
        self.last_decided_leader = last_commit_leader;

        // 8. Reinitialize CommitObserver with recovery (uses recover_and_send_commits)
        self.commit_observer.reinitialize(last_commit_index);

        // 9. Reset signaling state
        self.last_signaled_round = threshold_round.saturating_sub(1);

        info!("Components reinitialized successfully");
        Ok(())
    }

    /// If needed, signals a new clock round and sets up leader timeout.
    fn try_signal_new_round(&mut self) {
        // Signal only when the threshold clock round is more advanced than the last
        // signaled round.
        //
        // NOTE: a signal is still sent even when a block has been proposed at the new
        // round. We can consider changing this in the future.
        let new_clock_round = self.dag_state.read().threshold_clock_round();
        if new_clock_round <= self.last_signaled_round {
            return;
        }
        // Then send a signal to set up leader timeout.
        tracing::trace!(round = ?new_clock_round, "new_consensus_round_sent");
        self.signals.new_round(new_clock_round);
        self.last_signaled_round = new_clock_round;

        // Report the threshold clock round
        self.context
            .metrics
            .node_metrics
            .threshold_clock_round
            .set(new_clock_round as i64);
    }

    /// Creating a new block for the dictated round. This is used when either
    /// the min block delay timeout expires or max leader timeout expires. In
    /// the latter case, any checks like previous round leader existence
    /// will get skipped.
    pub(crate) fn new_block(
        &mut self,
        round: Round,
        reason: ReasonToCreateBlock,
    ) -> ConsensusResult<(
        Option<VerifiedBlock>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _scope = monitored_scope("Core::new_block");
        if self.last_proposed_round() < round {
            let result = self.try_propose(reason);
            // The threshold clock round may have advanced, so a signal needs to be sent.
            self.try_signal_new_round();
            return result;
        }
        Ok((None, BTreeMap::new()))
    }

    // Attempts to create a new block, persist and propose it to all peers.
    // When force is true, ignore if leader from the last round exists among
    // ancestors and if the minimum block delay has passed.
    fn try_propose(
        &mut self,
        reason: ReasonToCreateBlock,
    ) -> ConsensusResult<(
        Option<VerifiedBlock>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        if !self.should_propose() {
            return Ok((None, BTreeMap::new()));
        }
        if let Some(verified_block) = self.try_new_block(reason) {
            self.signals.new_block(verified_block.clone())?;

            fail_point!("consensus-after-propose");

            // The new block may help commit.
            let (_, missing_committed_txns) = self.try_commit(CommittedSubDagSource::Consensus)?;
            return Ok((Some(verified_block), missing_committed_txns));
        }
        Ok((None, BTreeMap::new()))
    }

    /// Attempts to propose a new block for the next round. If a block has
    /// already proposed for latest or earlier round, then no block is
    /// created and None is returned.
    #[instrument(level = "trace", skip_all)]
    fn try_new_block(&mut self, reason: ReasonToCreateBlock) -> Option<VerifiedBlock> {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::try_new_block"])
            .start_timer();

        // Ensure the new block has a higher round than the last proposed block.
        let clock_round = {
            let dag_state = self.dag_state.read();
            let clock_round = dag_state.threshold_clock_round();
            if clock_round <= dag_state.get_last_proposed_block_header().round() {
                return None;
            }
            clock_round
        };

        // There must be a quorum of blocks from the previous round.
        let quorum_round = clock_round.saturating_sub(1);

        // Create a new block either because we want to "forcefully" propose a block due
        // to a leader timeout, or because we are actually ready to produce the
        // block (leader exists and min delay has passed).
        if !reason.is_forced() {
            if !self.leaders_exist(quorum_round) {
                return None;
            }

            if Duration::from_millis(
                self.context
                    .clock
                    .timestamp_utc_ms()
                    .saturating_sub(self.last_proposed_timestamp_ms()),
            ) < self.context.parameters.min_block_delay
            {
                return None;
            }
        }

        // Determine the ancestors to be included in proposal. A quorum of ancestor must
        // exist due to a threshold clock
        let ancestors = self.ancestors_to_propose(clock_round);

        // Update the last included ancestor block refs
        for ancestor in &ancestors {
            self.last_included_ancestors[ancestor.author()] = Some(ancestor.reference());
        }

        let leader_authority = &self
            .context
            .committee
            .authority(self.first_leader(quorum_round))
            .hostname;
        self.context
            .metrics
            .node_metrics
            .block_proposal_leader_wait_ms
            .with_label_values(&[leader_authority])
            .inc_by(
                Instant::now()
                    .saturating_duration_since(self.dag_state.read().threshold_clock_quorum_ts())
                    .as_millis() as u64,
            );
        self.context
            .metrics
            .node_metrics
            .block_proposal_leader_wait_count
            .with_label_values(&[leader_authority])
            .inc();

        self.context
            .metrics
            .node_metrics
            .proposed_block_ancestors
            .observe(ancestors.len() as f64);
        for ancestor in &ancestors {
            let authority = &self.context.committee.authority(ancestor.author()).hostname;
            self.context
                .metrics
                .node_metrics
                .proposed_block_ancestors_depth
                .with_label_values(&[authority])
                .observe(clock_round.saturating_sub(ancestor.round()).into());
        }

        // Consume the next transactions to be included. Do not drop the guards yet as
        // this would acknowledge the inclusion of transactions. Just let this
        // be done in the end of the method.
        let (transactions, ack_transactions, _limit_reached) = self.transaction_consumer.next();
        // Serialize the transaction
        let serialized_transactions = Transaction::serialize(&transactions)
            .expect("We should expect correct serialization for transactions");
        // Compute transaction commitment that will be included in the block header
        let transactions_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized_transactions,
            &self.context,
            &mut self.encoder,
        )
        .expect("We should expect correct computation of the Merkle root for encoded transactions");

        self.context
            .metrics
            .node_metrics
            .proposed_block_transactions
            .observe(transactions.len() as f64);

        // Consume the acknowledgments about transaction data availability for past
        // blocks to be included.
        let acknowledgments = self.dag_state.write().take_acknowledgments(
            self.context
                .protocol_config
                .consensus_max_acknowledgments_per_block_or_default() as usize,
        );

        self.context
            .metrics
            .node_metrics
            .proposed_block_acknowledgments
            .observe(acknowledgments.len() as f64);
        for acknowledgment in &acknowledgments {
            let authority = &self
                .context
                .committee
                .authority(acknowledgment.author)
                .hostname;
            self.context
                .metrics
                .node_metrics
                .proposed_block_acknowledgments_depth
                .with_label_values(&[authority])
                .observe(clock_round.saturating_sub(acknowledgment.round).into());
        }

        // Consume the commit votes to be included.
        let commit_votes = self
            .dag_state
            .write()
            .take_commit_votes(MAX_COMMIT_VOTES_PER_BLOCK);

        // Get current timestamp and record drift but don't enforce ancestor timestamp
        // checks.
        let now = self.context.clock.timestamp_utc_ms();
        ancestors.iter().for_each(|block| {
            if block.timestamp_ms() > now {
                trace!("Ancestor block {block:?} has timestamp {}, greater than current timestamp {now}. Proposing for round {clock_round}.",  block.timestamp_ms());
                let authority = &self.context.committee.authority(block.author()).hostname;
                self.context
                    .metrics
                    .node_metrics
                    .proposed_block_ancestors_timestamp_drift_ms
                    .with_label_values(&[authority])
                    .inc_by(block.timestamp_ms().saturating_sub(now));
            }
        });

        // Create the block and insert to storage.
        let block_header = BlockHeader::V1(BlockHeaderV1::new(
            self.context.committee.epoch(),
            clock_round,
            self.context.own_index,
            now,
            ancestors.iter().map(|b| b.reference()).collect(),
            acknowledgments,
            commit_votes,
            transactions_commitment,
        ));

        let signed_block_header = SignedBlockHeader::new(block_header, &self.block_signer)
            .expect("Block signing failed.");

        // Make serialization over the whole signed block header even though we
        // serialized the block header when signing it.
        let serialized_signed_block_header = signed_block_header
            .serialize()
            .expect("Block serialization failed.");
        self.context
            .metrics
            .node_metrics
            .proposed_block_header_size
            .observe(serialized_signed_block_header.len() as f64);
        self.context
            .metrics
            .node_metrics
            .proposed_block_size
            .observe((serialized_signed_block_header.len() + serialized_transactions.len()) as f64);
        // Own blocks are assumed to be valid.
        let verified_block_header =
            VerifiedBlockHeader::new_verified(signed_block_header, serialized_signed_block_header);

        // Record the interval from last proposal, before accepting the proposed block.
        if self.last_proposed_round() > 0 {
            self.context
                .metrics
                .node_metrics
                .block_proposal_interval
                .observe(
                    Duration::from_millis(
                        verified_block_header
                            .timestamp_ms()
                            .saturating_sub(self.last_proposed_timestamp_ms()),
                    )
                    .as_secs_f64(),
                );
        }

        // Construct verified transactions to be used for storing and broadcasting
        let verified_transactions = VerifiedTransactions::new(
            transactions,
            verified_block_header.transaction_ref(),
            Some(verified_block_header.digest()),
            serialized_transactions,
        );
        let verified_block = VerifiedBlock {
            verified_block_header,
            verified_transactions,
        };
        // Accept the block into BlockManager and DagState.
        let (accepted_blocks, missing) = self
            .block_manager
            .try_accept_blocks(vec![verified_block.clone()], DataSource::OwnBlock);
        assert_eq!(accepted_blocks.len(), 1);
        assert!(missing.is_empty());
        // Ensure the new block and its ancestors are persisted, before broadcasting it.
        let mut dag_state_guard = self.dag_state.write();
        dag_state_guard.flush();
        drop(dag_state_guard);
        // Now acknowledge the transactions for their inclusion to block
        let block_ref = verified_block.reference();
        let gen_transaction_ref = if self.context.protocol_config.consensus_fast_commit_sync() {
            GenericTransactionRef::from(verified_block.transaction_ref())
        } else {
            GenericTransactionRef::from(block_ref)
        };
        ack_transactions(gen_transaction_ref);

        info!("Created block {block_ref} for round {clock_round}");

        self.context
            .metrics
            .node_metrics
            .proposed_blocks
            .with_label_values(&[&reason.label()])
            .inc();

        Some(verified_block)
    }

    /// Runs commit rule to attempt to commit additional blocks from the DAG. If
    /// any `certified_commits` are provided, then it will attempt to commit
    /// those first before trying to commit any further leaders.
    #[instrument(level = "trace", skip_all)]
    fn try_commit(
        &mut self,
        source: CommittedSubDagSource,
    ) -> ConsensusResult<(
        Vec<PendingSubDag>,
        BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    )> {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::try_commit"])
            .start_timer();

        let mut committed_sub_dags = Vec::new();
        let mut all_missing_committed_txns = BTreeMap::new();
        // TODO: Add optimization to abort early without quorum for a round.
        loop {
            // LeaderSchedule has a limit to how many sequenced leaders can be committed
            // before a change is triggered. Calling into leader schedule will get you
            // how many commits till next leader change. We will loop back and recalculate
            // any discarded leaders with the new schedule.
            let mut commits_until_update = self
                .leader_schedule
                .commits_until_leader_schedule_update(self.dag_state.clone());

            if commits_until_update == 0 {
                let last_commit_index = self.dag_state.read().last_commit_index();

                tracing::info!(
                    "Leader schedule change triggered at commit index {last_commit_index}"
                );

                self.leader_schedule
                    .update_leader_schedule(&mut self.dag_state.write());

                commits_until_update = self
                    .leader_schedule
                    .commits_until_leader_schedule_update(self.dag_state.clone());

                fail_point!("consensus-after-leader-schedule-change");
            }
            assert!(commits_until_update > 0);

            // Always try to process the synced commits first. If there are certified
            // commits to process then the decided leaders and the commits will be returned.

            let mut decided_leaders = self.committer.try_decide(self.last_decided_leader);

            // Truncate the decided leaders to fit the commit schedule limit.
            if decided_leaders.len() >= commits_until_update {
                let _ = decided_leaders.split_off(commits_until_update);
            }

            // If the decided leaders list is empty then just break the loop.
            let Some(last_decided) = decided_leaders.last().cloned() else {
                break;
            };

            self.last_decided_leader = last_decided.slot();

            let sequenced_leaders = decided_leaders
                .into_iter()
                .filter_map(|leader| leader.into_committed_block())
                .collect::<Vec<_>>();

            tracing::debug!(
                "Decided {} leaders and {commits_until_update} commits can be made before next leader schedule change",
                sequenced_leaders.len()
            );

            self.context
                .metrics
                .node_metrics
                .last_decided_leader_round
                .set(self.last_decided_leader.round as i64);

            // It's possible to reach this point as the decided leaders might all of them be
            // "Skip" decisions. In this case there is no leader to commit and
            // we should break the loop.
            if sequenced_leaders.is_empty() {
                break;
            }

            tracing::info!(
                "Committing {} leaders: {}",
                sequenced_leaders.len(),
                sequenced_leaders
                    .iter()
                    .map(|b| b.reference().to_string())
                    .join(",")
            );

            // TODO: refcount subdags
            let (subdags, missing_transactions_refs) = self
                .commit_observer
                .handle_committed_leaders(sequenced_leaders, source)?;

            // Check for duplicates before extending
            assert!(
                !missing_transactions_refs
                    .keys()
                    .any(|k| all_missing_committed_txns.contains_key(k)),
                "duplicate committed missing transactions reference found"
            );
            all_missing_committed_txns.extend(missing_transactions_refs);

            // Both pending and solid sub DAGs should be added to scoring subdags.
            self.dag_state
                .write()
                .add_scoring_subdags(subdags.iter().map(|s| s.base.clone()).collect());

            committed_sub_dags.extend(subdags);

            fail_point!("consensus-after-handle-commit");
        }

        // Notify about our own committed transactions
        let committed_transaction_refs = committed_sub_dags
            .iter()
            .flat_map(|sub_dag| sub_dag.committed_transaction_refs.iter())
            .filter_map(|block_ref| {
                (block_ref.author() == self.context.own_index).then_some(*block_ref)
            })
            .collect::<Vec<_>>();

        self.transaction_consumer.notify_own_transactions_status(
            committed_transaction_refs,
            self.dag_state.read().gc_round_for_last_commit(),
        );

        Ok((committed_sub_dags, all_missing_committed_txns))
    }

    pub(crate) fn get_missing_blocks(&self) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        let _scope = monitored_scope("Core::get_missing_blocks");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::get_missing_blocks"])
            .start_timer();
        self.block_manager.blocks_to_fetch()
    }
    pub(crate) fn get_missing_transaction_data(
        &self,
    ) -> BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>> {
        let _scope = monitored_scope("Core::get_missing_transaction_data");
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Core::get_missing_transaction_data"])
            .start_timer();

        // Use CommitObserver to get missing transaction data with authority
        // acknowledgments
        self.commit_observer.get_missing_transaction_data()
    }

    /// Sets if there is 2f+1 subscriptions to the block stream.
    pub(crate) fn set_quorum_subscribers_exists(&mut self, exists: bool) {
        info!("A quorum of block subscribers exists: {exists}");
        self.quorum_subscribers_exists = exists;
    }

    /// Sets the min propose round for the proposer allowing to propose blocks
    /// only for round numbers `> last_known_proposed_round`. At the moment
    /// is allowed to call the method only once leading to a panic
    /// if attempt to do multiple times.
    pub(crate) fn set_last_known_proposed_round(&mut self, round: Round) {
        if self.last_known_proposed_round.is_some() {
            panic!(
                "Should not attempt to set the last known proposed round if that has been already set"
            );
        }
        self.last_known_proposed_round = Some(round);
        info!("Last known proposed round set to {round}");
    }

    /// Whether the core should propose new blocks.
    pub(crate) fn should_propose(&self) -> bool {
        let clock_round = self.dag_state.read().threshold_clock_round();
        let core_skipped_proposals = &self.context.metrics.node_metrics.core_skipped_proposals;

        if !self.quorum_subscribers_exists {
            debug!("Skip proposing for round {clock_round}, don't have a quorum of subscribers.");
            core_skipped_proposals
                .with_label_values(&["no_quorum_subscriber"])
                .inc();
            return false;
        }

        let Some(last_known_proposed_round) = self.last_known_proposed_round else {
            debug!(
                "Skip proposing for round {clock_round}, last known proposed round has not been synced yet."
            );
            core_skipped_proposals
                .with_label_values(&["no_last_known_proposed_round"])
                .inc();
            return false;
        };
        if clock_round <= last_known_proposed_round {
            debug!(
                "Skip proposing for round {clock_round} as last known proposed round is {last_known_proposed_round}"
            );
            core_skipped_proposals
                .with_label_values(&["higher_last_known_proposed_round"])
                .inc();
            return false;
        }

        true
    }

    /// Retrieves the next ancestors to propose to form a block at `clock_round`
    /// round.
    fn ancestors_to_propose(&mut self, clock_round: Round) -> Vec<VerifiedBlockHeader> {
        // Take the ancestors before the clock_round (excluded) for each authority.
        let all_ancestors = self
            .dag_state
            .read()
            .get_last_cached_block_header_per_authority(clock_round);

        assert_eq!(
            all_ancestors.len(),
            self.context.committee.size(),
            "Fatal error, number of returned ancestors don't match committee size."
        );

        let quorum_round = clock_round.saturating_sub(1);

        // Propose only ancestors of higher rounds than what has already been proposed.
        // And always include own last proposed block first among ancestors.
        let included_ancestors = iter::once(self.last_proposed_block_header())
            .chain(all_ancestors.into_iter().flat_map(|(ancestor, _)| {
                if ancestor.author() == self.context.own_index {
                    return None;
                }
                if let Some(last_block_ref) = self.last_included_ancestors[ancestor.author()] {
                    if last_block_ref.round >= ancestor.round() {
                        return None;
                    }
                }
                Some(ancestor)
            }))
            .collect::<Vec<_>>();

        let mut parent_round_quorum = StakeAggregator::<QuorumThreshold>::new();

        // Make a sanity check that the total stake of quorum clock round ancestors is
        // above a quorum threshold. This must be guaranteed by a threshold
        // clock component that advanced the round.
        for ancestor in included_ancestors
            .iter()
            .filter(|a| a.round() == quorum_round)
        {
            parent_round_quorum.add(ancestor.author(), &self.context.committee);
        }

        assert!(
            parent_round_quorum.reached_threshold(&self.context.committee),
            "Fatal error, quorum not reached for parent round when proposing for round {clock_round}. Possible mismatch between DagState and Core."
        );

        included_ancestors
    }

    /// Checks whether the leaders of the round exist.
    fn leaders_exist(&self, round: Round) -> bool {
        let dag_state = self.dag_state.read();
        for leader in self.leaders(round) {
            // Search for all the leaders. If at least one is not found, then return false.
            // A linear search should be fine here as the set of elements is not expected to
            // be small enough and more sophisticated data structures might not
            // give us much here.
            if !dag_state.contains_cached_block_header_at_slot(leader) {
                return false;
            }
        }

        true
    }

    /// Returns the leaders of the provided round.
    fn leaders(&self, round: Round) -> Vec<Slot> {
        self.committer
            .get_leaders(round)
            .into_iter()
            .map(|authority_index| Slot::new(round, authority_index))
            .collect()
    }

    /// Returns the 1st leader of the round.
    fn first_leader(&self, round: Round) -> AuthorityIndex {
        self.leaders(round).first().unwrap().authority
    }

    fn last_proposed_timestamp_ms(&self) -> BlockTimestampMs {
        self.last_proposed_block_header().timestamp_ms()
    }

    fn last_proposed_round(&self) -> Round {
        self.last_proposed_block_header().round()
    }

    fn last_proposed_block_header(&self) -> VerifiedBlockHeader {
        self.dag_state.read().get_last_proposed_block_header()
    }
}

/// Senders of signals from Core, for outputs and events (ex new block
/// produced).
pub(crate) struct CoreSignals {
    tx_block_broadcast: broadcast::Sender<VerifiedBlock>,
    new_round_sender: watch::Sender<Round>,
    context: Arc<Context>,
}

impl CoreSignals {
    pub fn new(context: Arc<Context>) -> (Self, CoreSignalsReceivers) {
        // Blocks buffered in broadcast channel should be roughly equal to thosed cached
        // in dag state, since the underlying blocks are ref counted so a lower
        // buffer here will not reduce memory usage significantly.
        let (tx_block_broadcast, rx_block_broadcast) = broadcast::channel::<VerifiedBlock>(
            context.parameters.dag_state_cached_rounds as usize,
        );
        let (new_round_sender, new_round_receiver) = watch::channel(0);

        let me = Self {
            tx_block_broadcast,
            new_round_sender,
            context,
        };

        let receivers = CoreSignalsReceivers {
            rx_block_broadcast,
            new_round_receiver,
        };

        (me, receivers)
    }

    /// Sends a signal to all the waiters that a new block has been produced.
    /// The method will return true if block has reached even one
    /// subscriber, false otherwise.
    pub(crate) fn new_block(&self, verified_block: VerifiedBlock) -> ConsensusResult<()> {
        // When there is only one authority in committee, it is unnecessary to broadcast
        // the block which will fail anyway without subscribers to the signal.
        if self.context.committee.size() > 1 {
            if verified_block.round() == GENESIS_ROUND {
                debug!("Ignoring broadcasting genesis block to peers");
                return Ok(());
            }

            if let Err(err) = self.tx_block_broadcast.send(verified_block) {
                warn!("Couldn't broadcast the block to any receiver: {err}");
                return Err(ConsensusError::Shutdown);
            }
        } else {
            debug!(
                "Did not broadcast block {verified_block:?} to receivers as committee size is <= 1"
            );
        }
        Ok(())
    }

    /// Sends a signal that threshold clock has advanced to new round. The
    /// `round_number` is the round at which the threshold clock has
    /// advanced to.
    pub(crate) fn new_round(&mut self, round_number: Round) {
        let _ = self.new_round_sender.send_replace(round_number);
    }
}

/// Receivers of signals from Core.
/// Intentionally un-cloneable. Components should only subscribe to channels
/// they need.
pub(crate) struct CoreSignalsReceivers {
    rx_block_broadcast: broadcast::Receiver<VerifiedBlock>,
    new_round_receiver: watch::Receiver<Round>,
}

impl CoreSignalsReceivers {
    pub(crate) fn block_broadcast_receiver(&self) -> broadcast::Receiver<VerifiedBlock> {
        self.rx_block_broadcast.resubscribe()
    }

    pub(crate) fn new_round_receiver(&self) -> watch::Receiver<Round> {
        self.new_round_receiver.clone()
    }
}

/// Creates cores for the specified number of authorities for their
/// corresponding stakes. The method returns the cores and their respective
/// signal receivers are returned in `AuthorityIndex` order asc.
#[cfg(test)]
pub(crate) fn create_cores(context: Context, authorities: Vec<Stake>) -> Vec<CoreTextFixture> {
    let mut cores = Vec::new();

    for index in 0..authorities.len() {
        let own_index = AuthorityIndex::new_for_test(index as u8);
        let core = CoreTextFixture::new(
            context.clone(),
            authorities.clone(),
            own_index,
            false,
            false,
        );
        cores.push(core);
    }
    cores
}

#[cfg(test)]
pub(crate) struct CoreTextFixture {
    pub core: Core,
    pub signal_receivers: CoreSignalsReceivers,
    pub block_receiver: broadcast::Receiver<VerifiedBlock>,
    pub commit_receiver: UnboundedReceiver<CommittedSubDag>,
    pub store: Arc<dyn Store>,
}

#[cfg(test)]
impl CoreTextFixture {
    fn new(
        context: Context,
        authorities: Vec<Stake>,
        own_index: AuthorityIndex,
        sync_last_known_own_block: bool,
        with_rocksdb: bool,
    ) -> Self {
        let (committee, mut signers) = local_committee_and_keys(0, authorities);
        let mut context = context;
        context = context
            .with_committee(committee)
            .with_authority_index(own_index);
        context
            .protocol_config
            .set_consensus_bad_nodes_stake_threshold_for_testing(33);

        let context = Arc::new(context);
        let store: Arc<dyn Store> = if !with_rocksdb {
            Arc::new(MemStore::new(context.clone()))
        } else {
            let store_path = context.parameters.db_path.as_path().to_str().unwrap();
            Arc::new(RocksDBStore::new(store_path, context.clone()))
        };
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(
            LeaderSchedule::from_store(context.clone(), dag_state.clone())
                .with_num_commits_per_schedule(10),
        );
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let block_receiver = signal_receivers.block_broadcast_receiver();

        let (commit_sender, commit_receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(commit_sender, 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let block_signer = signers.remove(own_index.value()).1;

        let core = Core::new(
            context,
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            block_signer,
            dag_state,
            sync_last_known_own_block,
        );

        Self {
            core,
            signal_receivers,
            block_receiver,
            commit_receiver,
            store,
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{BTreeSet, HashSet},
        time::Duration,
    };

    use futures::{StreamExt, stream::FuturesUnordered};
    use iota_metrics::monitored_mpsc::unbounded_channel;
    use iota_protocol_config::ProtocolConfig;
    use rstest::rstest;
    use serial_test::serial;
    use starfish_config::{AuthorityIndex, Parameters};
    use tokio::time::sleep;

    use super::*;
    use crate::{
        CommitConsumer, CommitIndex, Transaction,
        block_header::{
            BlockHeaderDigest, TestBlockHeader, TransactionsCommitment, genesis_block_headers,
            genesis_blocks,
        },
        commit::CommitAPI,
        leader_scoring::ReputationScores,
        storage::{Store, WriteBatch, mem_store::MemStore},
        test_dag_builder::DagBuilder,
        transaction::{BlockStatus, TransactionClient},
        transaction_ref::TransactionRef,
    };

    /// Recover Core and continue proposing from the last round which forms a
    /// quorum.
    #[rstest]
    #[tokio::test]
    async fn test_core_recover_from_store_for_full_round(
        #[values(true, false)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        let (mut context, mut key_pairs) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let mut block_status_subscriptions = FuturesUnordered::new();

        // Create a fully connected DAG with 6 rounds.
        let num_rounds = 6;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=num_rounds).build();
        dag_builder.print();

        // Subscribe to all created "own" blocks. We know that for our node (A) we'll be
        // able to commit transactions up to round 2.
        for block in dag_builder.block_headers(1..=2) {
            if block.author() == context.own_index {
                let generic_ref = if consensus_fast_commit_sync {
                    // When consensus_fast_commit_sync is enabled, create TransactionRef variant
                    GenericTransactionRef::TransactionRef(TransactionRef {
                        round: block.round(),
                        author: block.author(),
                        transactions_commitment: block.transactions_commitment(),
                    })
                } else {
                    // When disabled, use BlockRef variant
                    GenericTransactionRef::from(block.reference())
                };
                let subscription =
                    transaction_consumer.subscribe_for_block_status_testing(generic_ref);
                block_status_subscriptions.push(subscription);
            }
        }

        // write headers and transactions in store
        store
            .write(
                WriteBatch::default()
                    .block_headers(dag_builder.block_headers(1..=num_rounds))
                    .transactions(dag_builder.transactions(1..=num_rounds)),
                context.clone(),
            )
            .expect("We should expect a successful storing of headers");

        // create dag state after all blocks have been written to store
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        // Check no commits have been persisted to dag_state or store.
        let last_commit = store.read_last_commit().unwrap();
        assert!(last_commit.is_none());
        assert_eq!(dag_state.read().last_commit_index(), 0);

        // Now spin up core
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let mut block_receiver = signal_receivers.block_broadcast_receiver();
        let _core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        // New round should be num_round + 1
        let mut new_round = signal_receivers.new_round_receiver();
        assert_eq!(*new_round.borrow_and_update(), num_rounds + 1);

        // Block for round 6 should have been proposed.
        let proposed_block = block_receiver
            .recv()
            .await
            .expect("A block should have been created");
        assert_eq!(proposed_block.round(), num_rounds + 1);
        let ancestors = proposed_block.ancestors();

        // Only ancestors of round 4 should be included.
        assert_eq!(ancestors.len(), 4);
        for ancestor in ancestors {
            assert_eq!(ancestor.round, num_rounds);
        }

        let last_commit = store
            .read_last_commit()
            .unwrap()
            .expect("We expect that the last commit is properly defined");

        // We should commit leaders in round 1 & 2 & 3 & 4 as the new block for round 7
        // is proposed.
        assert_eq!(last_commit.index(), num_rounds - 2);
        assert_eq!(dag_state.read().last_commit_index(), num_rounds - 2);
        let all_stored_commits = store.scan_commits((0..=CommitIndex::MAX).into()).unwrap();
        assert_eq!(all_stored_commits.len(), num_rounds as usize - 2);

        // And ensure that our "own" transaction data of blocks from rounds 1 & 2 sent
        // to TransactionConsumer as notification
        while let Some(result) = block_status_subscriptions.next().await {
            let status = result.unwrap();
            assert!(matches!(status, BlockStatus::Sequenced(_)));
        }
    }

    /// Recover Core and continue proposing when having a partial last round
    /// which doesn't form a quorum and we haven't proposed for that round
    /// yet.
    #[tokio::test]
    async fn test_core_recover_from_store_for_partial_round() {
        telemetry_subscribers::init_for_testing();

        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());

        // Create test blocks for all authorities except our's (index = 0).
        let mut last_round_blocks = genesis_blocks(&context);
        let mut all_blocks = last_round_blocks.clone();
        for round in 1..=4 {
            let mut this_round_blocks = Vec::new();

            // For round 4 only produce f+1 blocks. Skip our validator 0 and that of
            // position 1 from creating blocks.
            let authorities_to_skip = if round == 4 {
                context.committee.validity_threshold() as usize
            } else {
                // otherwise always skip creating a block for our authority
                1
            };

            for (index, _authority) in context.committee.authorities().skip(authorities_to_skip) {
                let block = TestBlockHeader::new(round, index.value() as u8)
                    .set_ancestors(last_round_blocks.iter().map(|b| b.reference()).collect())
                    .build();
                this_round_blocks.push(VerifiedBlock::new_for_test(block));
            }
            all_blocks.extend(this_round_blocks.clone());
            last_round_blocks = this_round_blocks;
        }

        // write them in store
        let (block_headers, block_transactions) = all_blocks
            .into_iter()
            .map(|b| (b.verified_block_header, b.verified_transactions))
            .unzip();
        store
            .write(
                WriteBatch::default()
                    .block_headers(block_headers)
                    .transactions(block_transactions),
                context.clone(),
            )
            .expect("Storage error");

        // create dag state after all blocks have been written to store
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        // Check no commits have been persisted to dag_state & store
        let last_commit = store.read_last_commit().unwrap();
        assert!(last_commit.is_none());
        assert_eq!(dag_state.read().last_commit_index(), 0);

        // Now spin up core
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let mut block_receiver = signal_receivers.block_broadcast_receiver();
        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        // Clock round should have advanced to 5 during recovery because
        // a quorum has formed in round 4.
        let mut new_round = signal_receivers.new_round_receiver();
        assert_eq!(*new_round.borrow_and_update(), 5);

        // During recovery, round 4 block should have been proposed.
        let proposed_block = block_receiver
            .recv()
            .await
            .expect("A block should have been created");
        assert_eq!(proposed_block.round(), 4);
        let ancestors = proposed_block.ancestors();

        assert_eq!(ancestors.len(), 4);
        for ancestor in ancestors {
            if ancestor.author == context.own_index {
                assert_eq!(ancestor.round, 0);
            } else {
                assert_eq!(ancestor.round, 3);
            }
        }

        // Run commit rule.
        core.try_commit(CommittedSubDagSource::Consensus).ok();
        let last_commit = store
            .read_last_commit()
            .unwrap()
            .expect("last commit should be set");

        // There were no commits prior to the core starting up but there was completed
        // rounds up to round 4. So we should commit leaders in round 1 & 2 as soon
        // as the new block for round 4 is proposed.
        assert_eq!(last_commit.index(), 2);
        assert_eq!(dag_state.read().last_commit_index(), 2);
        let all_stored_commits = store.scan_commits((0..=CommitIndex::MAX).into()).unwrap();
        assert_eq!(all_stored_commits.len(), 2);
    }

    #[tokio::test]
    async fn test_core_propose_after_genesis() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_consensus_max_transaction_size_bytes_for_testing(2_000);
            config.set_consensus_max_transactions_in_block_bytes_for_testing(2_000);
            config
        });

        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let mut block_receiver = signal_receivers.block_broadcast_receiver();
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );
        let mut encoder = create_encoder(&context);

        // First send some transactions, since the block will be created once we recover
        // core
        let mut total = 0;
        let mut index = 0;
        let mut transactions = vec![];
        loop {
            let transaction =
                bcs::to_bytes(&format!("Transaction {index}")).expect("Shouldn't fail");
            transactions.push(Transaction::new(transaction.clone()));
            total += transaction.len();
            index += 1;
            let _w = transaction_client
                .submit_no_wait(vec![transaction])
                .await
                .unwrap();

            // Create total size of transactions up to 1KB
            if total >= 1_000 {
                break;
            }
        }

        // Second set dummy acknowledgments in DagState. First 200 acknowledgments are
        // from eligible round; the rest are from the clock round, thereby they
        // will not be taken when creating a block
        let mut acknowledgments = vec![];
        let num_acks = 200;
        let mut num_pending_acks = 0;
        let mut rng = &mut rand::thread_rng();
        loop {
            acknowledgments.push(BlockRef::new(
                0,
                AuthorityIndex::new_for_test(2),
                BlockHeaderDigest::random(&mut rng),
            ));
            num_pending_acks += 1;
            if num_pending_acks >= num_acks {
                break;
            }
        }

        loop {
            acknowledgments.push(BlockRef::new(
                1,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::random(&mut rng),
            ));
            num_pending_acks += 1;
            if num_pending_acks >= 500 {
                break;
            }
        }

        dag_state
            .write()
            .set_pending_acknowledgments(acknowledgments.clone());

        // Recover core and immoderately create a new block
        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        // Manually check the transaction commitment that is expected to be computed in
        // next block
        let serialized_transactions = Transaction::serialize(&transactions)
            .expect("we should expect correct serialization for transactions");
        // Compute transaction commitment that will be included in the block header
        let transactions_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized_transactions,
            &context,
            &mut encoder,
        )
        .expect("we should expect correct computation of the transactions commitment");

        // a new block should have been created during recovery.
        let verified_block = block_receiver
            .recv()
            .await
            .expect("A new block should have been created");

        // A new block created - assert the details
        assert_eq!(verified_block.round(), 1);
        assert_eq!(verified_block.author().value(), 0);
        assert_eq!(verified_block.ancestors().len(), 4);
        assert_eq!(
            verified_block.transactions_commitment(),
            transactions_commitment
        );
        assert_eq!(verified_block.acknowledgments().len(), num_acks);

        // genesis blocks should be referenced
        let all_genesis = genesis_block_headers(&context);

        for ancestor in verified_block.ancestors() {
            all_genesis
                .iter()
                .find(|block| block.reference() == *ancestor)
                .expect("Block should be found amongst genesis blocks");
        }

        // Try to propose again - with or without ignore leaders check, it will not
        // return any block
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MinBlockDelayTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());

        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MaxLeaderTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());
        // Check no commits have been persisted to dag_state & store
        let last_commit = store.read_last_commit().unwrap();
        assert!(last_commit.is_none());
        assert_eq!(dag_state.read().last_commit_index(), 0);
    }

    #[tokio::test]
    async fn test_core_propose_once_receiving_a_quorum() {
        telemetry_subscribers::init_for_testing();
        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let _block_receiver = signal_receivers.block_broadcast_receiver();

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        let mut expected_ancestors = BTreeSet::new();

        // Adding one block now will trigger the creation of new block for round 1
        let verified_block = VerifiedBlock::new_for_test(TestBlockHeader::new(1, 1).build());
        expected_ancestors.insert(verified_block.reference());
        // Wait for min block delay to allow blocks to be proposed.
        sleep(context.parameters.min_block_delay).await;
        // add blocks to trigger proposal.
        _ = core.add_blocks(vec![verified_block], DataSource::Test);

        assert_eq!(core.last_proposed_round(), 1);
        expected_ancestors.insert(core.last_proposed_block_header().reference());
        // attempt to create a block - none will be produced.
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MinBlockDelayTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());

        // Adding another block now forms a quorum for round 1, so block at round 2 will
        // be proposed
        let block_3 = VerifiedBlock::new_for_test(TestBlockHeader::new(1, 2).build());
        expected_ancestors.insert(block_3.reference());
        // Wait for min block delay to allow blocks to be proposed.
        sleep(context.parameters.min_block_delay).await;
        // add blocks to trigger proposal.
        _ = core.add_blocks(vec![block_3], DataSource::Test);

        assert_eq!(core.last_proposed_round(), 2);

        let proposed_block = core.last_proposed_block_header();
        assert_eq!(proposed_block.round(), 2);
        assert_eq!(proposed_block.author(), context.own_index);
        assert_eq!(proposed_block.ancestors().len(), 3);
        let ancestors = proposed_block.ancestors();
        let ancestors = ancestors.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(ancestors, expected_ancestors);

        // Check no commits have been persisted to dag_state & store
        let last_commit = store.read_last_commit().unwrap();
        assert!(last_commit.is_none());
        assert_eq!(dag_state.read().last_commit_index(), 0);
    }

    #[tokio::test]
    async fn test_core_set_min_propose_round() {
        telemetry_subscribers::init_for_testing();
        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context.with_parameters(Parameters {
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        }));

        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let _block_receiver = signal_receivers.block_broadcast_receiver();

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender, 0),
            dag_state.clone(),
            store,
            leader_schedule.clone(),
        );

        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state,
            true,
        );

        // No new block should have been produced
        assert_eq!(
            core.last_proposed_round(),
            GENESIS_ROUND,
            "No block should have been created other than genesis"
        );

        // Trying to explicitly propose a block will not produce anything
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MaxLeaderTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());

        // Create blocks for the whole network - even "our" node in order to replicate
        // an "amnesia" recovery.
        let mut builder = DagBuilder::new(context.clone());
        builder.layers(1..=10).build();

        // Process all the blocks
        let (missing_ancestors, missing_committed_txns) = core
            .add_blocks(builder.blocks(1..=10), DataSource::Test)
            .unwrap();

        assert!(missing_ancestors.is_empty());
        assert!(missing_committed_txns.is_empty());

        // Try to propose - no block should be produced.
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MaxLeaderTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());

        // Now set the last known proposed round which is the highest round for which
        // the network informed us that we do have proposed a block about.
        core.set_last_known_proposed_round(10);

        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::KnownLastBlock)
            .expect("No error");
        assert!(missing_committed_txns.is_empty());

        let block = new_block_opt.unwrap();
        assert_eq!(block.round(), 11);
        assert_eq!(block.ancestors().len(), 4);

        let our_ancestor_included = block.ancestors()[0];
        assert_eq!(our_ancestor_included.author, context.own_index);
        assert_eq!(our_ancestor_included.round, 10);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_core_try_new_block_leader_timeout() {
        telemetry_subscribers::init_for_testing();

        // Since we run the test with started_paused = true, any time-dependent
        // operations using Tokio's time facilities, such as tokio::time::sleep
        // or tokio::time::Instant, will not advance. So practically each Core's
        // clock will have initialised potentially with different values but it never
        // advances. To ensure that blocks won't get rejected by cores we'll
        // need to manually wait for the time diff before processing them. By
        // calling the `tokio::time::sleep` we implicitly also advance the tokio
        // clock.
        async fn wait_blocks(blocks: &[VerifiedBlockHeader], context: &Context) {
            // Simulate the time wait before processing a block to ensure that
            // block.timestamp <= now
            let now = context.clock.timestamp_utc_ms();
            let max_timestamp = blocks
                .iter()
                .max_by_key(|block| block.timestamp_ms() as BlockTimestampMs)
                .map(|block| block.timestamp_ms())
                .unwrap_or(0);

            let wait_time = Duration::from_millis(max_timestamp.saturating_sub(now));
            sleep(wait_time).await;
        }

        let (context, _) = Context::new_for_test(4);
        // Create the cores for all authorities
        let mut all_cores = create_cores(context, vec![1, 1, 1, 1]);

        // Create blocks for rounds 1..=3 from all Cores except last Core of authority
        // 3, so we miss the block from it. As it will be the leader of round 3
        // then no-one will be able to progress to round 4 unless we explicitly trigger
        // the block creation.
        // create the cores and their signals for all the authorities
        let (_last_core, cores) = all_cores.split_last_mut().unwrap();

        // Now iterate over a few rounds and ensure the corresponding signals are
        // created while network advances
        let mut last_round_blocks = Vec::<VerifiedBlockHeader>::new();
        for round in 1..=3 {
            let mut this_round_blocks = Vec::new();

            for core_fixture in cores.iter_mut() {
                wait_blocks(&last_round_blocks, &core_fixture.core.context).await;

                core_fixture
                    .core
                    .add_block_headers(last_round_blocks.clone(), DataSource::Test)
                    .unwrap();

                // Only when round > 1 and using non-genesis parents.
                if let Some(r) = last_round_blocks.first().map(|b| b.round()) {
                    assert_eq!(round - 1, r);
                    if core_fixture.core.last_proposed_round() == r {
                        // Force propose new block regardless of min block delay.
                        let (new_block_opt, missing_committed_txns) = core_fixture
                            .core
                            .try_propose(ReasonToCreateBlock::MaxLeaderTimeout)
                            .unwrap();
                        assert!(missing_committed_txns.is_empty());
                        new_block_opt.unwrap_or_else(|| {
                            panic!("Block should have been proposed for round {round}")
                        });
                    }
                }

                assert_eq!(core_fixture.core.last_proposed_round(), round);

                this_round_blocks.push(core_fixture.core.last_proposed_block_header());
            }

            last_round_blocks = this_round_blocks;
        }

        // Try to create the blocks for round 4 by calling the try_propose() method. No
        // block should be created as the leader - authority 3 - hasn't proposed
        // any block.
        for core_fixture in cores.iter_mut() {
            wait_blocks(&last_round_blocks, &core_fixture.core.context).await;

            core_fixture
                .core
                .add_block_headers(last_round_blocks.clone(), DataSource::Test)
                .unwrap();
            let (new_block_opt, missing_committed_txns) = core_fixture
                .core
                .try_propose(ReasonToCreateBlock::AddBlockHeader)
                .unwrap();
            assert!(new_block_opt.is_none());
            assert!(missing_committed_txns.is_empty());
        }

        // Now try to create the blocks for round 4 via the leader timeout method which
        // should ignore any leader checks or min block delay.
        for core_fixture in cores.iter_mut() {
            let (new_block, missing_committed_txns) = core_fixture
                .core
                .new_block(4, ReasonToCreateBlock::MaxLeaderTimeout)
                .unwrap();
            assert!(missing_committed_txns.is_empty());
            assert!(new_block.is_some());

            assert_eq!(core_fixture.core.last_proposed_round(), 4);

            // Check commits have been persisted to store
            let last_commit = core_fixture
                .store
                .read_last_commit()
                .unwrap()
                .expect("last commit should be set");
            // There are 1 leader rounds with rounds completed up to and including
            // round 4
            assert_eq!(last_commit.index(), 1);
            let all_stored_commits = core_fixture
                .store
                .scan_commits((0..=CommitIndex::MAX).into())
                .unwrap();
            assert_eq!(all_stored_commits.len(), 1);
        }
    }

    #[tokio::test]
    async fn test_core_set_subscriber_exists() {
        telemetry_subscribers::init_for_testing();
        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let _block_receiver = signal_receivers.block_broadcast_receiver();

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender, 0),
            dag_state.clone(),
            store,
            leader_schedule.clone(),
        );

        let mut core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            // Set to no subscriber exists initially.
            false,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state,
            false,
        );

        // There is no proposal during recovery because there is no subscriber.
        assert_eq!(
            core.last_proposed_round(),
            GENESIS_ROUND,
            "No block should have been created other than genesis"
        );

        // There is no proposal even with forced proposing.
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::MaxLeaderTimeout)
            .unwrap();
        assert!(new_block_opt.is_none());
        assert!(missing_committed_txns.is_empty());

        // Let Core know subscriber exists.
        core.set_quorum_subscribers_exists(true);

        // Proposing now would succeed.
        let (new_block_opt, missing_committed_txns) = core
            .try_propose(ReasonToCreateBlock::QuorumSubscribersExist)
            .unwrap();
        assert!(new_block_opt.is_some());
        assert!(missing_committed_txns.is_empty());
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_leader_schedule_change() {
        telemetry_subscribers::init_for_testing();
        let default_params = Parameters::default();

        let (context, _) = Context::new_for_test(4);
        // create the cores and their signals for all the authorities
        let mut cores = create_cores(context, vec![1, 1, 1, 1]);

        // Now iterate over a few rounds and ensure the corresponding signals are
        // created while network advances
        let mut last_round_block_headers = Vec::new();
        for round in 1..=30 {
            let mut this_round_block_headers = Vec::new();

            // Wait for min block delay to allow blocks to be proposed.
            sleep(default_params.min_block_delay).await;

            for core_fixture in &mut cores {
                // add the blocks from last round
                // this will trigger a block creation for the round and a signal should be
                // emitted
                core_fixture
                    .core
                    .add_block_headers(last_round_block_headers.clone(), DataSource::Test)
                    .unwrap();

                // A "new round" signal should be received given that all the blocks of previous
                // round have been processed
                let new_round = receive(
                    Duration::from_secs(1),
                    core_fixture.signal_receivers.new_round_receiver(),
                )
                .await;
                assert_eq!(new_round, round);

                // Check that a new block has been proposed.
                let verified_block = tokio::time::timeout(
                    Duration::from_secs(1),
                    core_fixture.block_receiver.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(verified_block.round(), round);
                assert_eq!(verified_block.author(), core_fixture.core.context.own_index);

                // append the new block to this round blocks
                this_round_block_headers
                    .push(core_fixture.core.last_proposed_block_header().clone());

                let block_header = core_fixture.core.last_proposed_block_header();

                // ensure that produced block is referring to the blocks of last_round
                assert_eq!(
                    block_header.ancestors().len(),
                    core_fixture.core.context.committee.size()
                );
                for ancestor in block_header.ancestors() {
                    if block_header.round() > 1 {
                        // don't bother with round 1 block which just contains the genesis blocks.
                        assert!(
                            last_round_block_headers
                                .iter()
                                .any(|block_header| block_header.reference() == *ancestor),
                            "Reference from previous round should be added"
                        );
                    }
                }
            }

            last_round_block_headers = this_round_block_headers;
        }

        for core_fixture in cores {
            // Check commits have been persisted to store
            let last_commit = core_fixture
                .store
                .read_last_commit()
                .unwrap()
                .expect("last commit should be set");
            // There are 28 leader rounds with rounds completed up to and including
            // round 29. Round 30 blocks will only include their own blocks, so the
            // 28th leader will not be committed.
            assert_eq!(last_commit.index(), 27);
            let all_stored_commits = core_fixture
                .store
                .scan_commits((0..=CommitIndex::MAX).into())
                .unwrap();
            assert_eq!(all_stored_commits.len(), 27);
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .bad_nodes
                    .len(),
                1
            );
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .good_nodes
                    .len(),
                1
            );
            let expected_reputation_scores =
                ReputationScores::new((11..=20).into(), vec![29, 29, 29, 29]);
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .reputation_scores,
                expected_reputation_scores
            );
        }
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn test_sequenced_transactions_no_headers(
        #[values((true, true), (true, false), (false, false))] params: (bool, bool),
    ) {
        let (commit_only_for_traversed_headers, consensus_fast_commit_sync) = params;
        test_sequenced_transactions_no_headers_impl(
            commit_only_for_traversed_headers,
            consensus_fast_commit_sync,
        )
        .await;
    }

    #[tokio::test]
    #[serial]
    #[should_panic(
        expected = "consensus_fast_commit_sync requires consensus_commit_transactions_only_for_traversed_headers to be enabled"
    )]
    async fn test_sequenced_transactions_no_headers_invalid_config() {
        test_sequenced_transactions_no_headers_impl(false, true).await;
    }

    async fn test_sequenced_transactions_no_headers_impl(
        commit_only_for_traversed_headers: bool,
        consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        let committee_size = 10;
        let (mut context, _key_pairs) = Context::new_for_test(committee_size);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        context
            .protocol_config
            .set_consensus_commit_transactions_only_for_traversed_headers_for_testing(
                commit_only_for_traversed_headers,
            );
        let own_index = AuthorityIndex::new_for_test(0);
        let core_fixture_own = CoreTextFixture::new(
            context.clone(),
            vec![1; committee_size],
            own_index,
            true,
            false,
        );
        // create a DAG of 2*gc_depth rounds
        let mut dag_builder = DagBuilder::new(Arc::new(context.clone()));
        let gc_depth = context.protocol_config.gc_depth();
        let cached_rounds = core_fixture_own
            .core
            .context
            .parameters
            .dag_state_cached_rounds;
        // One authority will try to catch up, so it does not create any block
        let catch_up_index = AuthorityIndex::new_for_test((committee_size - 1) as u8);
        let core_fixture_catch_up = CoreTextFixture::new(
            context.clone(),
            vec![1; committee_size],
            catch_up_index,
            true,
            true,
        );
        let active_authorities = (0..(committee_size - 1) as u8)
            .map(AuthorityIndex::new_for_test)
            .collect::<Vec<_>>();
        // Blocks of one authority will not be referenced, but acknowledged
        let authority_to_skip = AuthorityIndex::new_for_test((committee_size - 2) as u8);
        let authorities_to_skip_ancestors = vec![authority_to_skip, catch_up_index];

        let num_rounds_with_skip_ancestors = cached_rounds + gc_depth;
        let total_rounds = 2 * num_rounds_with_skip_ancestors;
        dag_builder
            .layers(1..=num_rounds_with_skip_ancestors)
            .authorities(active_authorities.clone())
            .skip_ancestor_links(authorities_to_skip_ancestors)
            .build();
        let authorities_to_skip_ancestors = vec![catch_up_index];
        dag_builder
            .layers(num_rounds_with_skip_ancestors + 1..=total_rounds)
            .authorities(active_authorities)
            .skip_ancestor_links(authorities_to_skip_ancestors)
            .build();
        let sub_dags_and_commits = dag_builder.get_sub_dag_and_certified_commits(1..=total_rounds);

        let (mut core_own, mut commit_receiver_own) =
            (core_fixture_own.core, core_fixture_own.commit_receiver);
        let _ = core_own.add_blocks(dag_builder.blocks(1..=total_rounds), DataSource::Test);
        let mut existing_headers = HashSet::new();
        let mut all_sequenced_transactions = Vec::new();
        // Check the commits that are produced after processing blocks.
        // Record traversed headers and sequenced transactions
        while let Some(sub_dag) = commit_receiver_own.recv().await {
            let sub_dag_leader_round = sub_dag.leader.round;
            let CommittedSubDag { base, transactions } = sub_dag;

            for block_ref in &base.committed_header_refs {
                existing_headers.insert(*block_ref);
            }
            for transaction in &transactions {
                // Transactions from all authors except authority_to_skip should also have a
                // corresponding block_ref being traversed in the committed sub_dag
                // Same after num_rounds_with_skip_ancestors
                if transaction.author() != authority_to_skip
                    || transaction.round() >= num_rounds_with_skip_ancestors
                {
                    assert!(
                        existing_headers.contains(
                            &transaction
                                .block_ref()
                                .expect("block_ref should be set in test")
                        ),
                        "{}",
                        transaction
                            .block_ref()
                            .expect("block_ref should be set in test")
                    );
                } else {
                    assert!(
                        !existing_headers.contains(
                            &transaction
                                .block_ref()
                                .expect("block_ref should be set in test")
                        ) && !commit_only_for_traversed_headers,
                        "{}",
                        transaction
                            .block_ref()
                            .expect("block_ref should be set in test")
                    );
                }
            }
            all_sequenced_transactions.extend(transactions);
            if sub_dag_leader_round == total_rounds - 2 {
                break;
            }
        }
        // Now the node that tries to catch up will sync the certified commits
        // The commits contain only the headers
        let certified_commits = sub_dags_and_commits
            .iter()
            .map(|(_, c)| c.clone())
            .collect::<Vec<_>>();
        let mut core_catch_up = core_fixture_catch_up.core;
        let (missing_references, missing_transactions) = core_catch_up
            .add_certified_commits(CertifiedCommits::new(certified_commits))
            .expect("We should not fail with certified commits");
        assert!(missing_references.is_empty());
        let first_missing_transaction_from_skipped = *missing_transactions
            .iter()
            .find(|(a, _)| a.author() == authority_to_skip)
            .unwrap()
            .0;
        if commit_only_for_traversed_headers {
            assert_eq!(
                first_missing_transaction_from_skipped.round(),
                num_rounds_with_skip_ancestors
            );
        } else {
            assert_eq!(first_missing_transaction_from_skipped.round(), 1);
        }
        // Ensure that the block header corresponding to the
        // first_missing_transaction_from_skipped is not in dag_state
        // if commit_only_for_traversed_headers=false and in dag_state otherwise
        let is_in_dag_state = {
            let dag = core_catch_up.dag_state.read();
            match first_missing_transaction_from_skipped {
                GenericTransactionRef::BlockRef(ref b) => {
                    let block_ref = BlockRef::new(b.round, b.author, b.digest);
                    dag.get_verified_block_headers(&[block_ref])[0].is_some()
                }
                GenericTransactionRef::TransactionRef(ref t) => {
                    // resolve_block_ref returns None iff the block header is absent from
                    // dag_state, which is exactly the condition we want to check.
                    dag.resolve_block_ref(t).is_some_and(|block_ref| {
                        dag.get_verified_block_headers(&[block_ref])[0].is_some()
                    })
                }
            }
        };
        assert_eq!(is_in_dag_state, commit_only_for_traversed_headers);
        let last_solid_commit_round = core_catch_up
            .dag_state
            .read()
            .last_solid_commit_leader_round()
            .unwrap();
        // Latest solid (with all transactions being locally available) leader round is
        // 2 as only transactions from round 0 could be sequenced at this point.
        // All other transactions are not yet synced
        assert_eq!(last_solid_commit_round, 2u32);
        // Now assume that the missing transactions were synced by the transaction
        // synchronizer
        let missing_verified_transactions: Vec<_> = all_sequenced_transactions
            .into_iter()
            .filter(|tx| {
                let generic_ref = if consensus_fast_commit_sync {
                    GenericTransactionRef::TransactionRef(tx.transaction_ref())
                } else {
                    GenericTransactionRef::BlockRef(
                        tx.block_ref().expect("block_ref should be set in test"),
                    )
                };
                missing_transactions.contains_key(&generic_ref)
            })
            .collect();
        core_catch_up
            .add_transactions(
                missing_verified_transactions,
                DataSource::TransactionSynchronizer,
            )
            .unwrap();

        let last_commit_round = core_catch_up.dag_state.read().last_commit_round();
        let last_solid_commit_round = core_catch_up
            .dag_state
            .read()
            .last_solid_commit_leader_round()
            .unwrap();
        // Latest solid (with all transactions being locally available) leader round
        // coincides now with last_commit_round
        assert_eq!(last_solid_commit_round, last_commit_round);
        // Flush to evict verified transactions from first rounds;
        core_catch_up.dag_state.write().flush();
        // Try to get a verified transaction from skipped authority. It should be
        // impossible if commit_only_for_traversed_headers = false with
        // get_verified_transactions() method since the header is not available;
        // should be possible with commit_only_for_traversed_headers = true
        let opt_verified_transaction = core_catch_up
            .dag_state
            .read()
            .get_verified_transactions(&[first_missing_transaction_from_skipped]);
        assert_eq!(
            opt_verified_transaction[0].is_some(),
            commit_only_for_traversed_headers
        );
        // Try to get a serialized transaction from skipped authority. It should now be
        // possible with get_serialized_transactions() method since it just
        // reads bytes from storage
        let opt_serialized_transaction = core_catch_up
            .dag_state
            .read()
            .get_serialized_transactions(&[first_missing_transaction_from_skipped]);
        assert!(opt_serialized_transaction[0].is_some());
    }

    #[tokio::test]
    async fn test_add_certified_commits() {
        telemetry_subscribers::init_for_testing();

        let (context, _key_pairs) = Context::new_for_test(4);
        let context = context.with_parameters(Parameters {
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        });

        let authority_index = AuthorityIndex::new_for_test(0);
        let core = CoreTextFixture::new(context, vec![1, 1, 1, 1], authority_index, true, false);
        let store = core.store.clone();
        let mut core = core.core;

        // No new block should have been produced
        assert_eq!(
            core.last_proposed_round(),
            GENESIS_ROUND,
            "No block should have been created other than genesis"
        );

        // create a DAG of 12 rounds
        let mut dag_builder = DagBuilder::new(core.context.clone());
        dag_builder.layers(1..=12).build();

        // Store all blocks up to round 6 which should be enough to decide up to leader
        // 4
        dag_builder.print();
        let block_headers = dag_builder.block_headers(1..=6);

        for block_header in block_headers {
            core.dag_state
                .write()
                .accept_block_header(block_header, DataSource::Test);
        }

        // Get all the committed sub dags up to round 10
        let sub_dags_and_commits = dag_builder.get_sub_dag_and_certified_commits(1..=10);

        // Now try to commit up to the latest leader (round = 4). Do not provide any
        // certified commits.
        let (committed_sub_dags, _) = core.try_commit(CommittedSubDagSource::Consensus).unwrap();

        // We should have committed up to round 4
        assert_eq!(committed_sub_dags.len(), 4);

        let last_commit = store
            .read_last_commit()
            .unwrap()
            .expect("Last commit should be set");
        assert_eq!(last_commit.reference().index, 4);

        println!("Case 1. Provide no certified commits. No commit should happen.");

        let last_commit = store
            .read_last_commit()
            .unwrap()
            .expect("Last commit should be set");
        assert_eq!(last_commit.reference().index, 4);

        println!(
            "Case 2. Provide certified commits that before and after the last committed round and also there are additional blocks so can run the direct decide rule as well."
        );

        // The commits of leader rounds 5-8 should be committed via the certified
        // commits.
        let certified_commits = sub_dags_and_commits
            .iter()
            .skip(3)
            .take(5)
            .map(|(_, c)| c.clone())
            .collect::<Vec<_>>();

        // Now only add the blocks of rounds 8..=12. The blocks up to round 7 should be
        // accepted via the certified commits processing.
        let block_headers = dag_builder.block_headers(8..=12);
        for block_header in block_headers {
            core.dag_state
                .write()
                .accept_block_header(block_header, DataSource::Test);
        }

        // The corresponding blocks of the certified commits should be accepted and
        // stored before linearizing and committing the DAG.
        core.add_certified_commits(CertifiedCommits::new(certified_commits))
            .expect("Should not fail");

        let commits = store.scan_commits((6..=10).into()).unwrap();

        // We expect all the sub dags up to leader round 10 to be committed.
        assert_eq!(commits.len(), 5);

        for i in 6..=10 {
            let commit = &commits[i - 6];
            assert_eq!(commit.reference().index, i as u32);
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_commit_on_leader_schedule_change_boundary_without_multileader() {
        telemetry_subscribers::init_for_testing();
        let default_params = Parameters::default();

        let (context, _) = Context::new_for_test(6);

        // create the cores and their signals for all the authorities
        let mut cores = create_cores(context, vec![1, 1, 1, 1, 1, 1]);

        // Now iterate over a few rounds and ensure the corresponding signals are
        // created while network advances
        let mut last_round_block_headers = Vec::new();
        for round in 1..=33 {
            let mut this_round_block_headers = Vec::new();
            // Wait for min block delay to allow blocks to be proposed.
            sleep(default_params.min_block_delay).await;
            for core_fixture in &mut cores {
                // add the blocks from last round
                // this will trigger a block creation for the round and a signal should be
                // emitted
                core_fixture
                    .core
                    .add_block_headers(last_round_block_headers.clone(), DataSource::Test)
                    .unwrap();
                // A "new round" signal should be received given that all the blocks of previous
                // round have been processed
                let new_round = receive(
                    Duration::from_secs(1),
                    core_fixture.signal_receivers.new_round_receiver(),
                )
                .await;
                assert_eq!(new_round, round);
                // Check that a new block has been proposed.
                let verified_block = tokio::time::timeout(
                    Duration::from_secs(1),
                    core_fixture.block_receiver.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(verified_block.round(), round);
                assert_eq!(verified_block.author(), core_fixture.core.context.own_index);

                // append the new block to this round blocks
                this_round_block_headers
                    .push(core_fixture.core.last_proposed_block_header().clone());
                let block_header = core_fixture.core.last_proposed_block_header();
                // ensure that produced block is referring to the blocks of last_round
                assert_eq!(
                    block_header.ancestors().len(),
                    core_fixture.core.context.committee.size()
                );
                for ancestor in block_header.ancestors() {
                    if block_header.round() > 1 {
                        // don't bother with round 1 block which just contains the genesis blocks.
                        assert!(
                            last_round_block_headers
                                .iter()
                                .any(|block| block.reference() == *ancestor),
                            "Reference from previous round should be added"
                        );
                    }
                }
            }
            last_round_block_headers = this_round_block_headers;
        }
        for core_fixture in cores {
            // Check commits have been persisted to store
            let last_commit = core_fixture
                .store
                .read_last_commit()
                .unwrap()
                .expect("last commit should be set");
            // There are 31 leader rounds with rounds completed up to and including
            // round 33. Round 33 blocks will only include their own blocks, so there
            // should only be 30 commits.
            // However on a leader schedule change boundary its is possible for a
            // new leader to get selected for the same round if the leader elected
            // gets swapped allowing for multiple leaders to be committed at a round.
            // Meaning with multi leader per round explicitly set to 1 we will have 30,
            // otherwise 31.
            // NOTE: We used 31 leader rounds to specifically trigger the scenario
            // where the leader schedule boundary occurred AND we had a swap to a new
            // leader for the same round
            let expected_commit_count = 30;
            // Leave the code for re-use.
            // let expected_commit_count = match num_leaders_per_round {
            //    Some(1) => 30,
            //    _ => 31,
            //};
            assert_eq!(last_commit.index(), expected_commit_count);
            let all_stored_commits = core_fixture
                .store
                .scan_commits((0..=CommitIndex::MAX).into())
                .unwrap();
            assert_eq!(all_stored_commits.len(), expected_commit_count as usize);
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .bad_nodes
                    .len(),
                1
            );
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .good_nodes
                    .len(),
                1
            );
            let expected_reputation_scores =
                ReputationScores::new((21..=30).into(), vec![43, 43, 43, 43, 43, 43]);
            assert_eq!(
                core_fixture
                    .core
                    .leader_schedule
                    .leader_swap_table
                    .read()
                    .reputation_scores,
                expected_reputation_scores
            );
        }
    }

    #[tokio::test]
    async fn test_core_signals() {
        telemetry_subscribers::init_for_testing();
        let default_params = Parameters::default();

        let (context, _) = Context::new_for_test(4);
        // create the cores and their signals for all the authorities
        let mut cores = create_cores(context, vec![1, 1, 1, 1]);

        // Now iterate over a few rounds and ensure the corresponding signals are
        // created while network advances
        let mut last_round_block_headers = Vec::new();
        for round in 1..=10 {
            let mut this_round_block_headers = Vec::new();

            // Wait for min block delay to allow blocks to be proposed.
            sleep(default_params.min_block_delay).await;

            for core_fixture in &mut cores {
                // add the blocks from last round
                // this will trigger a block creation for the round and a signal should be
                // emitted
                core_fixture
                    .core
                    .add_block_headers(last_round_block_headers.clone(), DataSource::Test)
                    .unwrap();

                // A "new round" signal should be received given that all the blocks of previous
                // round have been processed
                let new_round = receive(
                    Duration::from_secs(1),
                    core_fixture.signal_receivers.new_round_receiver(),
                )
                .await;
                assert_eq!(new_round, round);

                // Check that a new block has been proposed.
                let verified_block = tokio::time::timeout(
                    Duration::from_secs(1),
                    core_fixture.block_receiver.recv(),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(verified_block.round(), round);
                assert_eq!(verified_block.author(), core_fixture.core.context.own_index);

                // append the new block to this round blocks
                this_round_block_headers
                    .push(core_fixture.core.last_proposed_block_header().clone());

                let block_header = core_fixture.core.last_proposed_block_header();

                // ensure that produced block is referring to the blocks of last_round
                assert_eq!(
                    block_header.ancestors().len(),
                    core_fixture.core.context.committee.size()
                );
                for ancestor in block_header.ancestors() {
                    if block_header.round() > 1 {
                        // don't bother with round 1 block which just contains the genesis blocks.
                        assert!(
                            last_round_block_headers
                                .iter()
                                .any(|block_header| block_header.reference() == *ancestor),
                            "Reference from previous round should be added"
                        );
                    }
                }
            }

            last_round_block_headers = this_round_block_headers;
        }

        for core_fixture in cores {
            // Check commits have been persisted to store
            let last_commit = core_fixture
                .store
                .read_last_commit()
                .unwrap()
                .expect("last commit should be set");
            // There are 8 leader rounds with rounds completed up to and including
            // round 9. Round 10 blocks will only include their own blocks, so the
            // 8th leader will not be committed.
            assert_eq!(last_commit.index(), 7);
            let all_stored_commits = core_fixture
                .store
                .scan_commits((0..=CommitIndex::MAX).into())
                .unwrap();
            assert_eq!(all_stored_commits.len(), 7);
        }
    }

    #[tokio::test]
    async fn test_core_compress_proposal_references() {
        telemetry_subscribers::init_for_testing();
        let default_params = Parameters::default();

        let (context, _) = Context::new_for_test(4);
        // create the cores and their signals for all the authorities
        let mut cores = create_cores(context, vec![1, 1, 1, 1]);

        let mut last_round_block_headers = Vec::new();
        let mut all_block_headers = Vec::new();

        let excluded_authority = AuthorityIndex::new_for_test(3);

        for round in 1..=10 {
            let mut this_round_block_headers = Vec::new();

            for core_fixture in &mut cores {
                // do not produce any block for authority 3
                if core_fixture.core.context.own_index == excluded_authority {
                    continue;
                }

                // try to propose to ensure that we are covering the case where we miss the
                // leader authority 3
                core_fixture
                    .core
                    .add_block_headers(last_round_block_headers.clone(), DataSource::Test)
                    .unwrap();
                core_fixture
                    .core
                    .new_block(round, ReasonToCreateBlock::MaxLeaderTimeout)
                    .unwrap();

                let block_header = core_fixture.core.last_proposed_block_header();
                assert_eq!(block_header.round(), round);

                // append the new block to this round blocks
                this_round_block_headers.push(block_header.clone());
            }

            last_round_block_headers = this_round_block_headers.clone();
            all_block_headers.extend(this_round_block_headers);
        }

        // Now send all the produced blocks to core of authority 3. It should produce a
        // new block. If no compression is applied then we should expect
        // all the previous blocks to be referenced from round 0..=10. However, since
        // compression is applied only the last round's (10) blocks should be
        // referenced + the authority's block of round 0.
        let core_fixture = &mut cores[excluded_authority];
        // Wait for min block delay to allow blocks to be proposed.
        sleep(default_params.min_block_delay).await;
        // add blocks to trigger proposal.
        core_fixture
            .core
            .add_block_headers(all_block_headers, DataSource::Test)
            .unwrap();

        // Assert that a block has been created for round 11 and it references to blocks
        // of round 10 for the other peers, and to round 1 for its own block
        // (created after recovery).
        let block_header = core_fixture.core.last_proposed_block_header();
        assert_eq!(block_header.round(), 11);
        assert_eq!(block_header.ancestors().len(), 4);
        for block_ref in block_header.ancestors() {
            if block_ref.author == excluded_authority {
                assert_eq!(block_ref.round, 1);
            } else {
                assert_eq!(block_ref.round, 10);
            }
        }

        // Check commits have been persisted to store
        let last_commit = core_fixture
            .store
            .read_last_commit()
            .unwrap()
            .expect("last commit should be set");
        // There are 8 leader rounds with rounds completed up to and including
        // round 10. However because there were no blocks produced for authority 3
        // 2 leader rounds will be skipped.
        assert_eq!(last_commit.index(), 6);
        let all_stored_commits = core_fixture
            .store
            .scan_commits((0..=CommitIndex::MAX).into())
            .unwrap();
        assert_eq!(all_stored_commits.len(), 6);
    }

    pub(crate) async fn receive<T: Copy>(timeout: Duration, mut receiver: watch::Receiver<T>) -> T {
        tokio::time::timeout(timeout, receiver.changed())
            .await
            .expect("Timeout while waiting to read from receiver")
            .expect("Signal receive channel shouldn't be closed");
        *receiver.borrow_and_update()
    }

    #[rstest]
    #[tokio::test]
    async fn test_commit_and_notify_for_block_status(
        #[values(true, false)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        let (mut context, mut key_pairs) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);

        let context = Arc::new(context);

        let store = Arc::new(MemStore::new(context.clone()));
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let mut block_status_subscriptions = FuturesUnordered::new();

        // Create a fully connected DAG with 8 rounds.
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=8).build();
        dag_builder.print();

        // Subscribe to all created "own" blocks. We know that for our node (A) we'll be
        // able to commit transactions up to round 4.
        for block in dag_builder.block_headers(1..=4) {
            if block.author() == context.own_index {
                let generic_ref = if consensus_fast_commit_sync {
                    // When consensus_fast_commit_sync is enabled, create TransactionRef variant
                    GenericTransactionRef::TransactionRef(TransactionRef {
                        round: block.round(),
                        author: block.author(),
                        transactions_commitment: block.transactions_commitment(),
                    })
                } else {
                    // When disabled, use BlockRef variant
                    GenericTransactionRef::from(block.reference())
                };
                let subscription =
                    transaction_consumer.subscribe_for_block_status_testing(generic_ref);
                block_status_subscriptions.push(subscription);
            }
        }

        // write headers in store
        store
            .write(
                WriteBatch::default().block_headers(dag_builder.block_headers(1..=8)),
                context.clone(),
            )
            .expect("We should expect a successful storing of headers");

        // write transactions in store
        store
            .write(
                WriteBatch::default().transactions(dag_builder.transactions(1..=8)),
                context.clone(),
            )
            .expect("We should expect a successful storing of transactions");

        // create dag state after all blocks have been written to store
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));

        let (sender, _receiver) = unbounded_channel("consensus_output");
        let commit_consumer = CommitConsumer::new(sender.clone(), 0);
        let commit_observer = CommitObserver::new(
            context.clone(),
            commit_consumer,
            dag_state.clone(),
            store.clone(),
            leader_schedule.clone(),
        );

        // Check no commits have been persisted to dag_state or store.
        let last_commit = store.read_last_commit().unwrap();
        assert!(last_commit.is_none());
        assert_eq!(dag_state.read().last_commit_index(), 0);

        // Now spin up core
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        // Need at least one subscriber to the block broadcast channel.
        let _block_receiver = signal_receivers.block_broadcast_receiver();
        let _core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        let last_commit = store
            .read_last_commit()
            .unwrap()
            .expect("last commit should be set");

        // The latest committed leader is from round 6 as the DAG is fully connected
        assert_eq!(last_commit.index(), 6);

        // Add timeout to prevent infinite waiting
        let timeout_duration = Duration::from_secs(10);
        let mut received_notifications = 0;
        let expected_notifications = block_status_subscriptions.len();

        loop {
            tokio::select! {
                result = block_status_subscriptions.next() => {
                    match result {
                        Some(status_result) => {
                            let status = status_result.unwrap();
                            assert!(matches!(status, BlockStatus::Sequenced(_)));
                            received_notifications += 1;

                            // If we received all expected notifications, break
                            if received_notifications >= expected_notifications {
                                break;
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(timeout_duration) => {
                    panic!("Test timed out after {:?}. Received {}/{} notifications. \
                           This suggests notifications are not being sent properly.",
                           timeout_duration, received_notifications, expected_notifications);
                }
            }
        }

        // Verify we got all expected notifications
        assert_eq!(
            received_notifications, expected_notifications,
            "Expected {} notifications but only received {}",
            expected_notifications, received_notifications
        );
    }
}
