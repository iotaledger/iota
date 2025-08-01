// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::Arc,
};

use itertools::Itertools;
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    Round,
    block_header::{BlockHeaderAPI, BlockHeaderDigest, BlockRef, VerifiedBlockHeader},
    commit::{Commit, CommitAPI, PendingSubDag, TrustedCommit, sort_sub_dag_blocks},
    context::Context,
    dag_state::{DagState, MAX_TRANSACTIONS_ACK_DEPTH},
    leader_schedule::LeaderSchedule,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
};

/// The maximum depth of the linearizer, i.e. how many rounds back it will
/// traverse the DAG from a committed leader block
// TODO: make it derivable from the protocol parameters
pub(crate) const MAX_LINEARIZER_DEPTH: Round = 60;

/// The `StorageAPI` trait provides an interface for the block store and has
/// been mostly introduced for allowing to inject the test store in
/// `DagBuilder`.
pub(crate) trait BlockStoreAPI {
    fn get_block_headers(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>>;
}

impl BlockStoreAPI
    for parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, DagState>
{
    fn get_block_headers(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>> {
        DagState::get_block_headers(self, refs)
    }
}

/// Expand a committed sequence of leader into a sequence of sub-dags.
pub(crate) struct Linearizer {
    /// In-memory block store representing the dag state
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    leader_schedule: Arc<LeaderSchedule>,

    // TODO: prune this map - any entries older than latest_leader.round() - max_ack_depth -
    //  max_linearizer_depth should be removed
    // TODO: should this be part of the Linearizer or
    //  its own component?
    transactions_ack_tracker: BTreeMap<BlockRef, StakeAggregator<QuorumThreshold>>,
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
        }
    }

    /// Collect the sub-dag and the corresponding commit from a specific leader
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
        let mut dag_state_guard = self.dag_state.write();
        let last_commit_index = dag_state_guard.last_commit_index();
        let last_commit_digest = dag_state_guard.last_commit_digest();
        let last_commit_timestamp_ms = dag_state_guard.last_commit_timestamp_ms();
        let last_committed_rounds = dag_state_guard.last_committed_rounds();
        let timestamp_ms = leader_block.timestamp_ms().max(last_commit_timestamp_ms);

        // Now linearize the sub-dag starting from the leader block
        let to_commit = Self::linearize_sub_dag(
            leader_block.clone(),
            last_committed_rounds,
            &mut dag_state_guard,
        );

        drop(dag_state_guard);

        // Collect all block references for transactions that reached quorum after
        // adding acknowledgments
        let committed_transactions = to_commit
            .iter()
            // Add the acknowledgments to the tracker and collect the ones that reached quorum.
            // This will return a vector of block references that reached the quorum threshold, so
            // using flat_map here to avoid nested vectors.
            .flat_map(|block| {
                self.add_committed_transaction_acks(
                    block.round(),
                    block.author(),
                    block.acknowledgments().to_vec(),
                )
            })
            // Remove duplicate block references
            .unique()
            .collect::<Vec<BlockRef>>();

        // Create the Commit.
        let commit = Commit::new(
            last_commit_index + 1,
            last_commit_digest,
            timestamp_ms,
            leader_block.reference(),
            to_commit
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<BlockRef>>(),
            committed_transactions,
        );
        let serialized = commit
            .serialize()
            .unwrap_or_else(|e| panic!("Failed to serialize commit: {}", e));
        let commit = TrustedCommit::new_trusted(commit, serialized);

        // Create the corresponding committed sub dag
        let sub_dag = PendingSubDag::new(
            leader_block.reference(),
            to_commit,
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
        dag_state: &mut impl BlockStoreAPI,
    ) -> Vec<VerifiedBlockHeader> {
        let leader_block_ref = leader_block.reference();
        let leader_round = leader_block.round();
        let mut buffer = vec![leader_block];

        let mut to_commit = Vec::new();

        let mut committed = HashSet::new();
        assert!(committed.insert(leader_block_ref));

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
                            !committed.contains(ancestor)
                                && last_committed_rounds[ancestor.author] < ancestor.round
                                && ancestor.round
                                    >= leader_round.saturating_sub(MAX_LINEARIZER_DEPTH)
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
                assert!(committed.insert(ancestor.reference()));
            }
        }

        // Sort the blocks of the sub-dag blocks
        sort_sub_dag_blocks(&mut to_commit);

        to_commit
    }

    // This function should be called whenever a new commit is observed. This will
    // iterate over the sequence of committed leaders and produce a list of
    // committed sub-dags.
    pub(crate) fn handle_commit(
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

        let max_round_committed_leader = committed_leaders
            .iter()
            .map(|block| block.round())
            .max()
            .expect("We should expect at least one leader block");

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

        // Committed blocks must be persisted to storage before sending them to IOTA and
        // executing their transactions.
        // Commit metadata can be persisted more lazily because they are recoverable.
        // Uncommitted blocks can wait to persist too.
        // But for simplicity, all unpersisted blocks and commits are flushed to
        // storage.
        let mut dag_state_guard = self.dag_state.write();
        dag_state_guard.flush();
        drop(dag_state_guard);

        // Evict old acknowledgments from the tracker of the linearizer.
        self.evict_old_acknowledgments(max_round_committed_leader);

        // TODO: we should resubmit transactions from own blocks that are not sequenced
        // and below certain round
        pending_sub_dags
    }

    /// This function evicts old acknowledgments from the tracker.
    fn evict_old_acknowledgments(&mut self, committed_leader_round: Round) {
        let lower_bound_round = committed_leader_round
            .saturating_sub(MAX_LINEARIZER_DEPTH + MAX_TRANSACTIONS_ACK_DEPTH);
        let lower_bound = BlockRef::new(
            lower_bound_round + 1,
            AuthorityIndex::ZERO,
            BlockHeaderDigest::MIN,
        );
        self.transactions_ack_tracker = self.transactions_ack_tracker.split_off(&lower_bound);
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
        let mut acknowledged_data = Vec::new();
        for block_ref in acknowledgments {
            if block_ref.round < round.saturating_sub(MAX_TRANSACTIONS_ACK_DEPTH) {
                continue; // Ignore acknowledgments for blocks that are too old
            }
            let votes_collector = self
                .transactions_ack_tracker
                .entry(block_ref)
                .or_insert_with(StakeAggregator::<QuorumThreshold>::new);

            if !votes_collector.reached_threshold(&self.context.committee)
                && votes_collector.add(authority, &self.context.committee)
            {
                acknowledged_data.push(block_ref);
            }
        }
        acknowledged_data
    }

    /// This method accepts a vector of missing transaction references and
    /// returns a map of the passed transactions along with authorities that
    /// have acknowledged this reference.
    pub fn get_transaction_ack_authors(
        &self,
        missing_refs: Vec<BlockRef>,
    ) -> BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> {
        let mut acknowledged_map = BTreeMap::new();

        for missing_ref in missing_refs {
            if let Some(acknowledgments) = self.transactions_ack_tracker.get(&missing_ref) {
                acknowledged_map.insert(missing_ref, acknowledgments.votes());
            }
        }

        acknowledged_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CommitIndex,
        commit::{CommitDigest, WAVE_LENGTH},
        context::Context,
        leader_schedule::{LeaderSchedule, LeaderSwapTable},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
        test_dag_parser::parse_dag,
    };

    #[tokio::test]
    async fn test_handle_commit() {
        telemetry_subscribers::init_for_testing();
        let num_authorities = 4;
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
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

        let commits = linearizer.handle_commit(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());
            assert_eq!(subdag.timestamp_ms, leaders[idx].timestamp_ms());
            if idx == 0 {
                // First subdag includes the leader block only and no committed data
                assert_eq!(subdag.blocks.len(), 1);
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 1 {
                // Genesis blocks are included in the first commit
                assert_eq!(subdag.blocks.len(), num_authorities);
                // Transactions from genesis are not committed
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.blocks.len(), num_authorities);
                // Every subdag after the first one will have all the committed transactions
                // from 2 rounds before the leader round
                assert_eq!(subdag.committed_transaction_refs.len(), num_authorities);
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }

            for committed_transactions_ref in subdag.committed_transaction_refs.iter() {
                assert!(committed_transactions_ref.round == leaders[idx].round() - 2);
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
            Arc::new(MemStore::new()),
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
        let mut dag_builder = DagBuilder::new(context.clone());
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
        let commits = linearizer.handle_commit(leaders.clone());

        // Write them in DagState
        dag_state
            .write()
            .add_scoring_subdags(commits.iter().map(|d| d.base.clone()).collect());
        // Now update the leader schedule
        leader_schedule.update_leader_schedule(&dag_state);
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
        let commits = linearizer.handle_commit(leaders.clone());
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
            Arc::new(MemStore::new()),
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
        let mut blocks = dag_builder.block_headers(0..=leader_round_wave_1 - 1);
        blocks.push(
            dag_builder
                .leader_block(leader_round_wave_1)
                .expect("Leader block should have been found"),
        );
        dag_state.write().accept_block_headers(blocks.clone());

        let first_leader = dag_builder
            .leader_block(leader_round_wave_1)
            .expect("Wave 1 leader round block should exist");
        let mut last_commit_index = 1;
        let first_commit_data = TrustedCommit::new_for_test(
            last_commit_index,
            CommitDigest::MIN,
            0,
            first_leader.reference(),
            blocks.into_iter().map(|block| block.reference()).collect(),
            vec![],
        );
        dag_state.write().add_commit(first_commit_data);

        // Now take all the blocks from round `leader_round_wave_1` up to round
        // `leader_round_wave_2-1`
        let mut blocks = dag_builder.block_headers(leader_round_wave_1..=leader_round_wave_2 - 1);
        // Filter out leader block of round `leader_round_wave_1`
        blocks.retain(|block| {
            !(block.round() == leader_round_wave_1
                && block.author() == leader_schedule.elect_leader(leader_round_wave_1, 0))
        });
        // Add the leader block of round `leader_round_wave_2`
        blocks.push(
            dag_builder
                .leader_block(leader_round_wave_2)
                .expect("Leader block should have been found"),
        );
        // Write them in dag state
        dag_state.write().accept_block_headers(blocks.clone());

        let mut blocks: Vec<_> = blocks.into_iter().map(|block| block.reference()).collect();

        // Now get the latest leader which is the leader round of wave 2
        let leader = dag_builder
            .leader_block(leader_round_wave_2)
            .expect("Leader block should exist");

        last_commit_index += 1;
        let expected_second_commit = TrustedCommit::new_for_test(
            last_commit_index,
            CommitDigest::MIN,
            0,
            leader.reference(),
            blocks.clone(),
            vec![],
        );

        let commit = linearizer.handle_commit(vec![leader.clone()]);
        assert_eq!(commit.len(), 1);

        let subdag = &commit[0];
        tracing::info!("{subdag:?}");
        assert_eq!(subdag.leader, leader.reference());
        assert_eq!(subdag.timestamp_ms, leader.timestamp_ms());
        assert_eq!(subdag.commit_ref.index, expected_second_commit.index());

        // Using the same sorting as used in CommittedSubDag::sort
        blocks.sort_by(|a, b| a.round.cmp(&b.round).then_with(|| a.author.cmp(&b.author)));
        assert_eq!(
            subdag
                .blocks
                .clone()
                .into_iter()
                .map(|b| b.reference())
                .collect::<Vec<_>>(),
            blocks
        );
        for block in subdag.blocks.iter() {
            assert!(block.round() <= expected_second_commit.leader().round);
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
            Arc::new(MemStore::new()),
        )));
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let mut linearizer = Linearizer::new(context.clone(), dag_state.clone(), leader_schedule);

        // Authorities of index 0->2 will always creates blocks that see each other, but
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

        let commits = linearizer.handle_commit(leaders.clone());
        for (idx, subdag) in commits.into_iter().enumerate() {
            tracing::info!("{subdag:?}");
            assert_eq!(subdag.leader, leaders[idx].reference());
            assert_eq!(subdag.timestamp_ms, leaders[idx].timestamp_ms());
            if idx == 0 {
                // First subdag includes the leader block only
                assert_eq!(subdag.blocks.len(), 1);
                // First subdag does not commit any transactions
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 1 {
                assert_eq!(subdag.blocks.len(), 3);
                // The second subdag does not commit any transactions either yet
                assert_eq!(subdag.committed_transaction_refs.len(), 0);
            } else if idx == 2 {
                // We commit:
                // * 1 block on round 4, the leader block
                // * 3 blocks on round 3, as no commit happened on round 3 since the leader was
                //   missing
                // * 2 blocks on round 2, again as no commit happened on round 3, we commit the
                //   "sub dag" of leader of round 3, which will be another 2 blocks
                assert_eq!(subdag.blocks.len(), 6);

                // We commit transactions from:
                // * 3 blocks on round 1, as no commit happened on round 3 since the leader was
                //   missing
                // * 3 blocks on round 2, committed without delay
                assert_eq!(subdag.committed_transaction_refs.len(), 6);
            } else {
                // we expect to see all blocks of round >= 1
                assert_eq!(subdag.blocks.len(), 6);
                assert!(
                    subdag.blocks.iter().all(|block| block.round() >= 1),
                    "Found blocks that are of round < 1."
                );

                // The following subdag commits all data from round 3 (leader block was missing,
                // so only 3 block refs)
                assert_eq!(subdag.committed_transaction_refs.len(), 3);
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }

            for committed_transactions_ref in subdag.committed_transaction_refs.iter() {
                assert!(committed_transactions_ref.round <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }
}
