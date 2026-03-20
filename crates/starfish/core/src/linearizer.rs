// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, Stake};
use tracing::{error, instrument};

use crate::{
    Round,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, BlockTimestampMs, VerifiedBlockHeader,
    },
    commit::{Commit, CommitAPI, PendingSubDag, TrustedCommit, sort_sub_dag_blocks},
    context::Context,
    dag_state::DagState,
    leader_schedule::LeaderSchedule,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// The `StorageAPI` trait provides an interface for the block store and has
/// been mostly introduced for allowing to inject the test store in
/// `DagBuilder`.
pub(crate) trait BlockStoreAPI {
    fn get_block_headers(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>>;
}

impl BlockStoreAPI
    for parking_lot::lock_api::RwLockReadGuard<'_, parking_lot::RawRwLock, DagState>
{
    fn get_block_headers(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>> {
        DagState::get_verified_block_headers(self, refs)
    }
}

/// Expand a committed sequence of leaders into a sequence of sub-dags.
pub(crate) struct Linearizer {
    /// In-memory block store representing the dag state
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    leader_schedule: Arc<LeaderSchedule>,
    transactions_ack_tracker: BTreeMap<BlockRef, StakeAggregator<QuorumThreshold>>,
    traversed_headers_tracker: BTreeSet<BlockRef>,
}

impl Linearizer {
    pub(crate) fn new(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        leader_schedule: Arc<LeaderSchedule>,
    ) -> Self {
        Self {
            dag_state,
            leader_schedule,
            context,
            transactions_ack_tracker: BTreeMap::new(),
            traversed_headers_tracker: BTreeSet::new(),
        }
    }

    /// Reinitialize Linearizer after fast sync completes.
    /// Clears tracked state for a fresh start.
    pub(crate) fn clear_state(&mut self) {
        self.transactions_ack_tracker.clear();
        self.traversed_headers_tracker.clear();
    }

    /// Collect the sub-dag and the corresponding commit from a specific leader,
    /// excluding any duplicates or blocks that have already been committed
    /// (within previous sub-dags).
    fn collect_sub_dag_and_commit(
        &mut self,
        leader_block: VerifiedBlockHeader,
        reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
    ) -> (PendingSubDag, TrustedCommit) {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Linearizer::collect_sub_dag_and_commit"])
            .start_timer();
        // Grab latest commit state from dag state
        let dag_state_guard = self.dag_state.read();
        let last_commit_index = dag_state_guard.last_commit_index();
        let last_commit_digest = dag_state_guard.last_commit_digest();
        let last_commit_timestamp_ms = dag_state_guard.last_commit_timestamp_ms();
        let last_committed_rounds = dag_state_guard.last_committed_rounds();

        // Now linearize the sub-dag starting from the leader block
        let to_commit = Self::linearize_sub_dag(
            leader_block.clone(),
            last_committed_rounds,
            &dag_state_guard,
            self.context.protocol_config.gc_depth(),
        );

        // Calculate commit timestamp using median of leader's parents (NEW mode)
        let timestamp_ms = Self::calculate_commit_timestamp(
            &self.context,
            &dag_state_guard,
            &leader_block,
            last_commit_timestamp_ms,
        );

        drop(dag_state_guard);
        if self
            .context
            .protocol_config
            .consensus_commit_transactions_only_for_traversed_headers()
        {
            for block_header in &to_commit {
                self.traversed_headers_tracker
                    .insert(block_header.reference());
            }
        }

        // Collect all block references for transactions that reached quorum after
        // adding acknowledgments
        let committed_transactions = to_commit
            .iter()
            // Add the acknowledgments to the tracker and collect the ones that reached quorum.
            // This will return a vector of block references that reached the quorum threshold, so
            // using flat_map here to avoid nested vectors.
            .flat_map(|block_header| {
                self.add_committed_transaction_acks(
                    block_header.round(),
                    block_header.author(),
                    block_header.acknowledgments().to_vec(),
                )
            })
            .collect::<Vec<BlockRef>>();
        // Check that there are no duplicates in the committed transactions
        assert_eq!(
            committed_transactions.len(),
            committed_transactions.iter().collect::<HashSet<_>>().len(),
            "Duplicate BlockRef found"
        );

        // Convert BlockRef to GenericTransactionRef based on protocol flag
        let committed_transactions_refs: Vec<GenericTransactionRef> =
            if self.context.protocol_config.consensus_fast_commit_sync() {
                // Use batch function to get transaction commitments efficiently
                let dag_state_guard = self.dag_state.read();
                let transactions_commitments =
                    dag_state_guard.get_transactions_commitments_batch(&committed_transactions);

                // Zip block_refs with their corresponding transaction commitments
                committed_transactions
                    .into_iter()
                    .zip(transactions_commitments)
                    .map(|(block_ref, transactions_commitment_opt)| {
                        let transactions_commitment = transactions_commitment_opt
                            .expect("Block header must exist for committed transaction");
                        GenericTransactionRef::TransactionRef(TransactionRef {
                            round: block_ref.round,
                            author: block_ref.author,
                            transactions_commitment,
                        })
                    })
                    .collect()
            } else {
                committed_transactions
                    .into_iter()
                    .map(GenericTransactionRef::BlockRef)
                    .collect()
            };

        // Create the Commit.
        let commit = Commit::new(
            &self.context,
            last_commit_index + 1,
            last_commit_digest,
            timestamp_ms,
            leader_block.reference(),
            to_commit
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<BlockRef>>(),
            committed_transactions_refs,
            reputation_scores_desc.clone(),
        );
        let serialized = commit
            .serialize()
            .unwrap_or_else(|e| panic!("Failed to serialize commit: {e}"));
        let commit = TrustedCommit::new_trusted(commit, serialized);

        // Create the corresponding committed sub dag
        let sub_dag = PendingSubDag::new(
            leader_block.reference(),
            to_commit,
            commit.block_headers().to_vec(),
            commit.committed_transactions(),
            timestamp_ms,
            commit.reference(),
            reputation_scores_desc,
        );

        (sub_dag, commit)
    }

    pub(crate) fn linearize_sub_dag(
        leader_block: VerifiedBlockHeader,
        last_committed_rounds: Vec<u32>,
        dag_state: &impl BlockStoreAPI,
        max_linearizer_depth: u32,
    ) -> Vec<VerifiedBlockHeader> {
        let leader_block_ref = leader_block.reference();
        let leader_round = leader_block.round();
        let mut buffer = vec![leader_block];

        let mut to_commit = Vec::new();

        let mut traversed_headers = HashSet::new();
        assert!(traversed_headers.insert(leader_block_ref));

        while let Some(x) = buffer.pop() {
            to_commit.push(x.clone());

            let ancestors: Vec<VerifiedBlockHeader> = dag_state
                .get_block_headers(
                    &x.ancestors()
                        .iter()
                        .copied()
                        .filter(|ancestor| {
                            // We skip the block if we already committed it or
                            // we reached a round that we already committed or
                            // we traverse too far back in the past
                            !traversed_headers.contains(ancestor)
                                && last_committed_rounds[ancestor.author] < ancestor.round
                                && ancestor.round
                                    >= leader_round.saturating_sub(max_linearizer_depth)
                        })
                        .collect::<Vec<_>>(),
                )
                .into_iter()
                .map(|ancestor_opt| {
                    ancestor_opt.expect("We should have all uncommitted blocks in dag state.")
                })
                .collect();

            for ancestor in ancestors {
                buffer.push(ancestor.clone());
                assert!(traversed_headers.insert(ancestor.reference()));
            }
        }

        // Sort the blocks of the sub-dag blocks
        sort_sub_dag_blocks(&mut to_commit);

        to_commit
    }

    // This function should be called whenever a new commit is observed. This will
    // iterate over the sequence of committed leaders and produce a list of
    // committed sub-dags.
    // Leaders in `committed_leaders` are assumed to be ordered in increasing
    // rounds.
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn get_pending_sub_dags(
        &mut self,
        committed_leaders: Vec<VerifiedBlockHeader>,
    ) -> Vec<PendingSubDag> {
        if committed_leaders.is_empty() {
            return vec![];
        }

        // We check whether the leader schedule has been updated. If yes, then we'll
        // send the scores as part of the first sub dag.
        let schedule_updated = self
            .leader_schedule
            .leader_schedule_updated(&self.dag_state);

        let mut pending_sub_dags = vec![];

        for (i, leader_block) in committed_leaders.into_iter().enumerate() {
            let reputation_scores_desc = if schedule_updated && i == 0 {
                self.leader_schedule
                    .leader_swap_table
                    .read()
                    .reputation_scores_desc
                    .clone()
            } else {
                vec![]
            };

            let (sub_dag, commit) =
                self.collect_sub_dag_and_commit(leader_block, reputation_scores_desc);

            // Buffer commit in dag state for persistence later.
            // This also updates the last committed rounds.
            let mut dag_state_guard = self.dag_state.write();
            dag_state_guard.add_commit(commit.clone());
            drop(dag_state_guard);

            pending_sub_dags.push(sub_dag);
        }

        pending_sub_dags
    }

    /// This function evicts old acknowledgments and traversed headers from the
    /// tracker. Should be called for solid committed leader round since we
    /// rely on the ack tracker in transaction synchronizer.
    pub(crate) fn evict_linearizer(&mut self, solid_commit_leader_round: Round) {
        let lower_bound_round =
            solid_commit_leader_round.saturating_sub(self.context.protocol_config.gc_depth() * 2);
        let lower_header_bound = BlockRef::new(
            lower_bound_round + 1,
            AuthorityIndex::ZERO,
            BlockHeaderDigest::MIN,
        );

        self.transactions_ack_tracker =
            self.transactions_ack_tracker.split_off(&lower_header_bound);
        if self
            .context
            .protocol_config
            .consensus_commit_transactions_only_for_traversed_headers()
        {
            self.traversed_headers_tracker = self
                .traversed_headers_tracker
                .split_off(&lower_header_bound);
        }
    }

    /// This function is called to add the transaction acknowledgments to the
    /// tracker and returns the vector of block refs to transactions that
    /// reached the quorum threshold after adding the acknowledgments.
    pub(crate) fn add_committed_transaction_acks(
        &mut self,
        round: Round,
        authority: AuthorityIndex,
        acknowledgments: Vec<BlockRef>,
    ) -> Vec<BlockRef> {
        let mut transactions_to_commit = Vec::new();
        for block_ref in acknowledgments {
            if block_ref.round < round.saturating_sub(self.context.protocol_config.gc_depth()) {
                continue; // Ignore acknowledgments for blocks that are too old
            }
            let votes_collector = self
                .transactions_ack_tracker
                .entry(block_ref)
                .or_insert_with(StakeAggregator::<QuorumThreshold>::new);

            let was_below_threshold = !votes_collector.reached_threshold(&self.context.committee);

            if votes_collector.add(authority, &self.context.committee) && was_below_threshold {
                // We commit transactions only if at the moment of reaching the quorum the
                // corresponding header is traversed
                if !self
                    .context
                    .protocol_config
                    .consensus_commit_transactions_only_for_traversed_headers()
                    || self.traversed_headers_tracker.contains(&block_ref)
                {
                    transactions_to_commit.push(block_ref);
                }
            }
        }
        transactions_to_commit
    }

    /// This method accepts a vector of missing transaction references and
    /// returns a map of the passed transactions along with authorities that
    /// have acknowledged this reference.
    pub fn get_transaction_ack_authors(
        &self,
        missing_refs: Vec<GenericTransactionRef>,
    ) -> BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>> {
        let mut acknowledged_map = BTreeMap::new();

        for missing_ref in missing_refs {
            let block_ref = match missing_ref {
                GenericTransactionRef::BlockRef(br) => br,
                GenericTransactionRef::TransactionRef(tx_ref) => {
                    let dag = self.dag_state.read();
                    match dag.resolve_block_ref(&tx_ref) {
                        Some(br) => br,
                        None => {
                            error!(
                                "block_digest not found for {tx_ref:?} in transactions_ack_tracker lookup; \
                                 entry should exist since missing txns are above eviction boundary"
                            );
                            continue;
                        }
                    }
                }
            };
            if let Some(acknowledgments) = self.transactions_ack_tracker.get(&block_ref) {
                acknowledged_map.insert(missing_ref, acknowledgments.votes());
            }
        }

        acknowledged_map
    }

    /// Record headers as traversed when recovering state so transaction commit
    /// checks can succeed after a restart.
    pub(crate) fn record_traversed_headers<'a>(
        &mut self,
        headers: impl IntoIterator<Item = &'a VerifiedBlockHeader>,
    ) {
        if !self
            .context
            .protocol_config
            .consensus_commit_transactions_only_for_traversed_headers()
        {
            return;
        }

        for header in headers {
            self.traversed_headers_tracker.insert(header.reference());
        }
    }

    /// Calculates the commit's timestamp using the median of leader's parents
    /// (leader.round - 1) timestamps by stake. To ensure that commit timestamp
    /// monotonicity is respected, it is compared against the
    /// `last_commit_timestamp_ms` and the maximum of the two is returned.
    pub(crate) fn calculate_commit_timestamp(
        context: &Context,
        dag_state: &impl BlockStoreAPI,
        leader_block: &VerifiedBlockHeader,
        last_commit_timestamp_ms: BlockTimestampMs,
    ) -> BlockTimestampMs {
        // Select leaders' parent blocks (blocks at round - 1)
        let block_refs = leader_block
            .ancestors()
            .iter()
            .filter(|block_ref| block_ref.round == leader_block.round() - 1)
            .cloned()
            .collect::<Vec<_>>();

        // Get the blocks from dag state which should not fail
        let block_headers = dag_state
            .get_block_headers(&block_refs)
            .into_iter()
            .map(|block_opt| block_opt.expect("We should have all block headers in dag state."));

        let timestamp_ms = median_timestamp_by_stake(context, block_headers).unwrap_or_else(|e| {
            panic!(
                "Cannot compute median timestamp for leader block {:?} ancestors: {}",
                leader_block.reference(),
                e
            )
        });

        // Always make sure that commit timestamps are monotonic, so override if
        // necessary
        timestamp_ms.max(last_commit_timestamp_ms)
    }
}

/// Computes the median timestamp of the blocks weighted by the stake of their
/// authorities. This function assumes each block comes from a different
/// authority of the same round. Error is returned if no blocks are provided or
///  the total stake is less than a quorum threshold.
pub(crate) fn median_timestamp_by_stake(
    context: &Context,
    block_headers: impl Iterator<Item = VerifiedBlockHeader>,
) -> Result<BlockTimestampMs, String> {
    let mut total_stake = 0;
    let mut timestamps = vec![];
    for header in block_headers {
        let stake = context.committee.authority(header.author()).stake;
        timestamps.push((header.timestamp_ms(), stake));
        total_stake += stake;
    }

    if timestamps.is_empty() {
        return Err("No block headers provided".to_string());
    }
    if total_stake < context.committee.quorum_threshold() {
        return Err(format!(
            "Total stake {} < quorum threshold {}",
            total_stake,
            context.committee.quorum_threshold()
        ));
    }

    Ok(median_timestamps_by_stake_inner(timestamps, total_stake))
}

fn median_timestamps_by_stake_inner(
    mut timestamps: Vec<(BlockTimestampMs, Stake)>,
    total_stake: Stake,
) -> BlockTimestampMs {
    timestamps.sort_by_key(|(ts, _)| *ts);

    let mut cumulative_stake = 0;
    for (ts, stake) in &timestamps {
        cumulative_stake += stake;
        if cumulative_stake > total_stake / 2 {
            return *ts;
        }
    }

    timestamps.last().unwrap().0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CommitIndex, TestBlockHeader,
        commit::{CommitDigest, WAVE_LENGTH},
        context::Context,
        dag_state::DataSource,
        leader_schedule::{LeaderSchedule, LeaderSwapTable},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
        transaction_ref::GenericTransactionRefAPI,
    };

    #[tokio::test]
    async fn test_handle_commit() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new(context.clone())),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone(), leader_schedule);

        // Populate fully connected test blocks for round 0 ~ 10, authorities 0 ~ 3.
        let num_rounds: u32 = 10;
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        let leaders = dag_builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        let commits = linearizer.get_pending_sub_dags(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            let block_refs = leaders[idx]
                .ancestors()
                .iter()
                .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                .cloned()
                .collect::<Vec<_>>();
            let blocks = dag_state
                .read()
                .get_block_headers(&block_refs)
                .into_iter()
                .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));
            let expected_ts = median_timestamp_by_stake(&context, blocks).unwrap();

            assert_eq!(subdag.timestamp_ms, expected_ts);

            if idx == 0 {
                // First subdag includes the leader block only and no committed data
                assert_eq!(subdag.headers.len(), 1);
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 1 {
                // Genesis blocks are included in the first commit
                assert_eq!(subdag.headers.len(), num_authorities);
                // Transactions from genesis are not committed
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.headers.len(), num_authorities);
                // Every subdag after the first one will have all the committed transactions
                // from 2 rounds before the leader round
                assert_eq!(subdag.committed_transaction_refs.len(), num_authorities);
            }
            for block_ref in subdag.base.committed_header_refs.iter() {
                assert!(block_ref.round <= leaders[idx].round());
            }

            for committed_transactions_ref in subdag.committed_transaction_refs.iter() {
                assert!(committed_transactions_ref.round() == leaders[idx].round() - 2);
            }

            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }

    #[tokio::test]
    async fn test_handle_commit_with_schedule_update() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new(context.clone())),
        )));
        const NUM_OF_COMMITS_PER_SCHEDULE: u64 = 10;
        let leader_schedule = Arc::new(
            LeaderSchedule::new(context.clone(), LeaderSwapTable::default())
                .with_num_commits_per_schedule(NUM_OF_COMMITS_PER_SCHEDULE),
        );
        let mut linearizer =
            Linearizer::new(context.clone(), dag_state.clone(), leader_schedule.clone());

        // Populate fully connected test blocks for round 0 ~ 20, authorities 0 ~ 3.
        let num_rounds: u32 = 20;
        let mut dag_builder = DagBuilder::new(context);
        dag_builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state.clone());

        // Take the first 10 leaders
        let leaders = dag_builder
            .leader_blocks(1..=10)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Create some commits
        let commits = linearizer.get_pending_sub_dags(leaders);
        {
            // Write them in DagState
            let mut write = dag_state.write();
            write.add_scoring_subdags(commits.iter().map(|d| d.base.clone()).collect());
            // Now update the leader schedule
            leader_schedule.update_leader_schedule(&mut write);
        }
        assert!(
            leader_schedule.leader_schedule_updated(&dag_state),
            "Leader schedule should have been updated"
        );

        // Try to commit now the rest of the 10 leaders
        let leaders = dag_builder
            .leader_blocks(11..=20)
            .into_iter()
            .map(Option::unwrap)
            .collect::<Vec<_>>();

        // Now on the commits only the first one should contain the updated scores, the
        // other should be empty
        let commits = linearizer.get_pending_sub_dags(leaders);
        assert_eq!(commits.len(), 10);
        let scores = vec![
            (AuthorityIndex::new_for_test(1), 29),
            (AuthorityIndex::new_for_test(0), 29),
            (AuthorityIndex::new_for_test(3), 29),
            (AuthorityIndex::new_for_test(2), 29),
        ];
        assert_eq!(commits[0].reputation_scores_desc, scores);
        for commit in commits.into_iter().skip(1) {
            assert_eq!(commit.reputation_scores_desc, vec![]);
        }
    }

    #[tokio::test]
    async fn test_handle_already_committed() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let (context, _) = Context::new_for_test(num_authorities);

        let context = Arc::new(context);

        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new(context.clone())),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer =
            Linearizer::new(context.clone(), dag_state.clone(), leader_schedule.clone());
        let wave_length = WAVE_LENGTH;

        let leader_round_wave_1 = 3;
        let leader_round_wave_2 = leader_round_wave_1 + wave_length;

        // Build a Dag from round 1..=6
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=leader_round_wave_2).build();

        // Now retrieve all the blocks up to round leader_round_wave_1 - 1
        // And then only the leader of round leader_round_wave_1
        // Also store those to DagState
        let mut block_headers_wave_1 = dag_builder.block_headers(0..=leader_round_wave_1 - 1);
        block_headers_wave_1.push(
            dag_builder
                .leader_block(leader_round_wave_1)
                .expect("Leader block should have been found"),
        );
        dag_state
            .write()
            .accept_block_headers(block_headers_wave_1.clone(), DataSource::Test);

        let first_leader = dag_builder
            .leader_block(leader_round_wave_1)
            .expect("Wave 1 leader round block should exist");
        let mut last_commit_index = 1;
        let first_commit_data = TrustedCommit::new_for_test(
            &context,
            last_commit_index,
            CommitDigest::MIN,
            0,
            first_leader.reference(),
            block_headers_wave_1
                .iter()
                .map(|block_header| block_header.reference())
                .collect(),
            vec![],
        );
        dag_state.write().add_commit(first_commit_data);

        // Now take all the blocks from round `leader_round_wave_1` up to round
        // `leader_round_wave_2-1`
        let mut block_headers_wave_2 =
            dag_builder.block_headers(leader_round_wave_1..=leader_round_wave_2 - 1);
        // Filter out leader block of round `leader_round_wave_1`
        block_headers_wave_2.retain(|block| {
            !(block.round() == leader_round_wave_1
                && block.author() == leader_schedule.elect_leader(leader_round_wave_1, 0))
        });
        // Add the leader block of round `leader_round_wave_2`
        block_headers_wave_2.push(
            dag_builder
                .leader_block(leader_round_wave_2)
                .expect("Leader block should have been found"),
        );
        // Write them in dag state
        dag_state
            .write()
            .accept_block_headers(block_headers_wave_2.clone(), DataSource::Test);

        let mut block_refs_wave_2: Vec<_> = block_headers_wave_2
            .into_iter()
            .map(|block| block.reference())
            .collect();

        // Now get the latest leader which is the leader round of wave 2
        let leader = dag_builder
            .leader_block(leader_round_wave_2)
            .expect("Leader block should exist");

        last_commit_index += 1;
        let expected_second_commit = TrustedCommit::new_for_test(
            &context,
            last_commit_index,
            CommitDigest::MIN,
            0,
            leader.reference(),
            block_refs_wave_2.clone(),
            vec![],
        );

        let commit = linearizer.get_pending_sub_dags(vec![leader.clone()]);
        assert_eq!(commit.len(), 1);

        let subdag = &commit[0];
        tracing::info!("{subdag:?}");
        assert_eq!(subdag.leader, leader.reference());
        assert_eq!(subdag.commit_ref.index, expected_second_commit.index());

        let expected_ts = median_timestamp_by_stake(
            &context,
            subdag.headers.iter().filter_map(|header| {
                if header.round() == subdag.leader.round - 1 {
                    Some(header.clone())
                } else {
                    None
                }
            }),
        )
        .unwrap();

        assert_eq!(subdag.timestamp_ms, expected_ts);

        // Using the same sorting as used in CommittedSubDag::sort
        block_refs_wave_2
            .sort_by(|a, b| a.round.cmp(&b.round).then_with(|| a.author.cmp(&b.author)));
        assert_eq!(subdag.committed_header_refs, block_refs_wave_2);
        for block_ref in subdag.base.committed_header_refs.iter() {
            assert!(block_ref.round <= expected_second_commit.leader().round);
        }
    }

    /// This test will make sure that the linearizer will commit blocks
    /// according to the rules.
    #[tokio::test]
    async fn test_handle_commit_simple() {
        telemetry_subscribers::init_for_testing();

        let num_authorities = 4;
        let (context, _keys) = Context::new_for_test(num_authorities);

        let context = Arc::new(context);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new(context.clone())),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone(), leader_schedule);

        // Authorities of index 0-2 will always create blocks that see each other, but
        // until round 5 they won't see the blocks of authority 3. For authority
        // 3 we create blocks that connect to all the other authorities.
        // On round 5 we finally make the other authorities see the blocks of authority
        // 3. Practically we "simulate" here a long chain created by authority 3
        // that is visible in round 5. All blocks will be
        // committed for rounds >= 1.
        let dag_str = "DAG {
                Round 0 : { 4 },
                Round 1 : { * },
                Round 2 : {
                    A -> ([-D1],[-D1]),
                    B -> ([-D1],[-D1]),
                    C -> ([-D1],[-D1]),
                    D -> [*],
                },
                Round 3 : {
                    A -> ([-D2],[-D2]),
                    B -> ([-D2],[-D2]),
                    C -> ([-D2],[-D2]),
                },
                Round 4 : {
                    A -> ([-D3],[-D3]),
                    B -> ([-D3],[-D3]),
                    C -> ([-D3],[-D3]),
                    D -> [A3, B3, C3, D2],
                },
                Round 5 : { * },
            }";

        let dag_builder = parse_dag(dag_str).expect("Invalid dag");
        dag_builder.print();
        dag_builder.persist_all_blocks(dag_state.clone());

        // Blocks B1, C2, A4, B5 are the leaders of rounds 1-5 (e.g. D3 is not present
        // in DAG)
        let leaders = dag_builder
            .leader_blocks(1..=5)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        for (idx, leader) in leaders.iter().enumerate() {
            let subdags = linearizer.get_pending_sub_dags(vec![leader.clone()]);
            assert_eq!(subdags.len(), 1);
            let subdag = &subdags[0];

            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());

            let block_refs = leaders[idx]
                .ancestors()
                .iter()
                .filter(|block_ref| block_ref.round == leaders[idx].round() - 1)
                .cloned()
                .collect::<Vec<_>>();
            let blocks = dag_state
                .read()
                .get_block_headers(&block_refs)
                .into_iter()
                .map(|block_opt| block_opt.expect("We should have all blocks in dag state."));

            let expected_ts = median_timestamp_by_stake(&context, blocks).unwrap();

            assert_eq!(subdag.timestamp_ms, expected_ts);

            if idx == 0 {
                // First subdag includes the leader block only
                assert_eq!(subdag.headers.len(), 1);
                // First subdag does not commit any transactions
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 1 {
                assert_eq!(subdag.headers.len(), 3);
                // The second subdag does not commit any transactions either yet
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 2 {
                // We commit:
                // * 1 block on round 4, the leader block
                // * 3 blocks on round 3, as no commit happened on round 3 since the leader was
                //   missing
                // * 2 blocks on round 2, again as no commit happened on round 3, we commit the
                //   "sub dag" of leader of round 3, which will be another 2 blocks
                assert_eq!(subdag.headers.len(), 6);

                // We commit transactions from:
                // * 3 blocks on round 1, as no commit happened on round 3 since the leader was
                //   missing
                // * 3 blocks on round 2, committed without delay
                assert_eq!(subdag.committed_transaction_refs.len(), 6);
                // Check that transactions are acknowledged by all authorities except authority
                // 3.
                let ack_authors = linearizer
                    .get_transaction_ack_authors(subdag.committed_transaction_refs.clone());
                for (block_ref, authors) in ack_authors {
                    assert_eq!(
                        authors,
                        (0..3).map(AuthorityIndex::new_for_test).collect(),
                        "{block_ref}"
                    );
                }
            } else {
                // we expect to see all blocks of round >= 1
                assert_eq!(subdag.headers.len(), 6);
                assert!(
                    subdag.headers.iter().all(|block| block.round() >= 1),
                    "Found blocks that are of round < 1."
                );

                // The following subdag commits all data from round 3 (leader block was missing,
                // so only 3 block refs)
                assert_eq!(subdag.committed_transaction_refs.len(), 3);

                // Check that transactions are acknowledged by all authorities.
                let ack_authors = linearizer
                    .get_transaction_ack_authors(subdag.committed_transaction_refs.clone());
                for (block_ref, authors) in ack_authors {
                    tracing::info!("{block_ref:?}");
                    assert_eq!(authors, (0..=3).map(AuthorityIndex::new_for_test).collect());
                }
            }
            for block_ref in subdag.base.committed_header_refs.iter() {
                assert!(block_ref.round <= leaders[idx].round());
            }

            for committed_transactions_ref in subdag.committed_transaction_refs.iter() {
                assert!(committed_transactions_ref.round() < leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }

    #[tokio::test]
    async fn test_eviction() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new(context.clone())),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone(), leader_schedule);
        let num_rounds_to_evict = 20;
        // Populate fully connected test blocks for round 0 ~ protocol_config.gc_depth()
        // * 2
        // + num_rounds_to_evict, authorities 0 ~
        // 3.
        let num_rounds: u32 = context.protocol_config.gc_depth() * 2 + num_rounds_to_evict;
        let mut dag_builder = DagBuilder::new(context);
        dag_builder
            .layers(1..=num_rounds)
            .build()
            .persist_layers(dag_state);

        let leaders = dag_builder
            .leader_blocks(1..=num_rounds)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        linearizer.get_pending_sub_dags(leaders);
        // Check that before eviction acknowledgements for all rounds up to num_rounds-2
        // are stored
        for round in 1..=num_rounds - 2 {
            let round_references: Vec<_> = dag_builder
                .block_headers(round..=round)
                .into_iter()
                .map(|bh| GenericTransactionRef::from(bh.reference()))
                .collect();

            let ack_authors = linearizer.get_transaction_ack_authors(round_references.clone());
            assert_eq!(ack_authors.len(), 4);
        }

        linearizer.evict_linearizer(num_rounds);
        // Check that acknowledgements for the first num_rounds_to_evict rounds are
        // evicted and the rest are still stored
        for round in 1..=num_rounds - 2 {
            let round_references: Vec<_> = dag_builder
                .block_headers(round..=round)
                .into_iter()
                .map(|bh| GenericTransactionRef::from(bh.reference()))
                .collect();
            let ack_authors = linearizer.get_transaction_ack_authors(round_references.clone());
            if round <= num_rounds_to_evict {
                assert!(ack_authors.is_empty());
            } else {
                assert_eq!(ack_authors.len(), 4);
            }
        }
    }

    #[tokio::test]
    async fn test_calculate_commit_timestamp() {
        let timestamp_1 = 3_000;
        let timestamp_2 = 3_000;
        let timestamp_3 = 6_000;
        // GIVEN
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let ancestors = vec![
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(4, 0).set_timestamp_ms(1_000).build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(4, 1).set_timestamp_ms(2_000).build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(4, 2).set_timestamp_ms(3_000).build(),
            ),
            VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(4, 3).set_timestamp_ms(4_000).build(),
            ),
        ];
        let leader_block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );
        {
            let mut dag_state_guard = dag_state.write();
            for block in &ancestors {
                dag_state_guard.accept_block_header(block.clone(), DataSource::Test);
            }
        }
        let last_commit_timestamp_ms = 0;
        // WHEN
        let dag_state_guard = dag_state.read();

        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &dag_state_guard,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_1);
        // AND skip the block of authority 0 and round 4.
        let leader_block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .skip(1)
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &dag_state_guard,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_2);
        // AND set the `last_commit_timestamp_ms` to 6_000
        let last_commit_timestamp_ms = 6_000;
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &dag_state_guard,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, timestamp_3);
        // AND there is only one ancestor block to commit
        let (context, _) = Context::new_for_test(1);
        let leader_block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(5, 0)
                .set_timestamp_ms(5_000)
                .set_ancestors(
                    ancestors
                        .iter()
                        .take(1)
                        .map(|block| block.reference())
                        .collect::<Vec<_>>(),
                )
                .build(),
        );
        let last_commit_timestamp_ms = 0;
        let timestamp = Linearizer::calculate_commit_timestamp(
            &context,
            &dag_state_guard,
            &leader_block,
            last_commit_timestamp_ms,
        );
        assert_eq!(timestamp, 1_000);
    }
    #[test]
    fn test_median_timestamps_by_stake() {
        // One total stake.
        let timestamps = vec![(1_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 1), 1_000);
        // Odd number of total stakes.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 3), 2_000);
        // Even the number of total stakes.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1), (4_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 4), 3_000);
        // Even number of total stakes, different order.
        let timestamps = vec![(4_000, 1), (3_000, 1), (1_000, 1), (2_000, 1)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 4), 3_000);
        // Unequal stakes.
        let timestamps = vec![(2_000, 2), (4_000, 2), (1_000, 3), (3_000, 3)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 10), 3_000);
        // Unequal stakes.
        let timestamps = vec![
            (500, 2),
            (4_000, 2),
            (2_500, 3),
            (1_000, 5),
            (3_000, 3),
            (2_000, 4),
        ];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 19), 2_000);
        // One authority dominates.
        let timestamps = vec![(1_000, 1), (2_000, 1), (3_000, 1), (4_000, 1), (5_000, 10)];
        assert_eq!(median_timestamps_by_stake_inner(timestamps, 14), 5_000);
    }
    #[tokio::test]
    async fn test_median_timestamps_by_stake_errors() {
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        // No blocks provided
        let err = median_timestamp_by_stake(&context, vec![].into_iter()).unwrap_err();
        assert_eq!(err, "No block headers provided");
        // Blocks provided but total stake is less than a quorum threshold
        let block = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 0).build());
        let err = median_timestamp_by_stake(&context, vec![block].into_iter()).unwrap_err();
        assert_eq!(err, "Total stake 1 < quorum threshold 3");
    }
}
