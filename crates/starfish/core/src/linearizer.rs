// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    block_header::{BlockHeaderAPI, BlockRef, GENESIS_ROUND, VerifiedBlockHeader},
    commit::{Commit, CommittedSubDag, TrustedCommit, sort_sub_dag_blocks},
    context::Context,
    dag_state::DagState,
    leader_schedule::LeaderSchedule,
};

/// The `StorageAPI` trait provides an interface for the block store and has
/// been mostly introduced for allowing to inject the test store in
/// `DagBuilder`.
pub(crate) trait BlockStoreAPI {
    fn get_blocks(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>>;

    fn set_committed(&mut self, block_ref: &BlockRef) -> bool;

    fn is_committed(&self, block_ref: &BlockRef) -> bool;
}

impl BlockStoreAPI
    for parking_lot::lock_api::RwLockWriteGuard<'_, parking_lot::RawRwLock, DagState>
{
    fn get_blocks(&self, refs: &[BlockRef]) -> Vec<Option<VerifiedBlockHeader>> {
        DagState::get_blocks(self, refs)
    }

    fn set_committed(&mut self, block_ref: &BlockRef) -> bool {
        DagState::set_committed(self, block_ref)
    }

    fn is_committed(&self, block_ref: &BlockRef) -> bool {
        DagState::is_committed(self, block_ref)
    }
}

/// Expand a committed sequence of leader into a sequence of sub-dags.
#[derive(Clone)]
pub(crate) struct Linearizer {
    /// In memory block store representing the dag state
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    leader_schedule: Arc<LeaderSchedule>,
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
        }
    }

    /// Collect the sub-dag and the corresponding commit from a specific leader
    /// excluding any duplicates or blocks that have already been committed
    /// (within previous sub-dags).
    fn collect_sub_dag_and_commit(
        &mut self,
        leader_block: VerifiedBlockHeader,
        reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
    ) -> (CommittedSubDag, TrustedCommit) {
        let _s = self
            .context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Linearizer::collect_sub_dag_and_commit"])
            .start_timer();
        // Grab latest commit state from dag state
        let mut dag_state = self.dag_state.write();
        let last_commit_index = dag_state.last_commit_index();
        let last_commit_digest = dag_state.last_commit_digest();
        let last_commit_timestamp_ms = dag_state.last_commit_timestamp_ms();
        let last_committed_rounds = dag_state.last_committed_rounds();
        let timestamp_ms = leader_block.timestamp_ms().max(last_commit_timestamp_ms);

        // Now linearize the sub-dag starting from the leader block
        let to_commit = Self::linearize_sub_dag(
            &self.context,
            leader_block.clone(),
            last_committed_rounds,
            &mut dag_state,
        );

        drop(dag_state);

        // Create the Commit.
        let commit = Commit::new(
            last_commit_index + 1,
            last_commit_digest,
            timestamp_ms,
            leader_block.reference(),
            to_commit
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>(),
        );
        let serialized = commit
            .serialize()
            .unwrap_or_else(|e| panic!("Failed to serialize commit: {}", e));
        let commit = TrustedCommit::new_trusted(commit, serialized);

        // Create the corresponding committed sub dag
        let sub_dag = CommittedSubDag::new(
            leader_block.reference(),
            to_commit,
            timestamp_ms,
            commit.reference(),
            reputation_scores_desc,
        );

        (sub_dag, commit)
    }

    pub(crate) fn linearize_sub_dag(
        context: &Context,
        leader_block: VerifiedBlockHeader,
        last_committed_rounds: Vec<u32>,
        dag_state: &mut impl BlockStoreAPI,
    ) -> Vec<VerifiedBlockHeader> {
        let leader_block_ref = leader_block.reference();
        let mut buffer = vec![leader_block];

        let mut to_commit = Vec::new();

        // The new logic will perform the recursion without stopping at the highest
        // round that has been committed per authority. Instead it will
        // allow to commit blocks that are lower than the highest committed round for an
        // authority.
        if context.protocol_config.consensus_linearize_subdag_v2() {
            assert!(
                dag_state.set_committed(&leader_block_ref),
                "Leader block with reference {:?} attempted to be committed twice",
                leader_block_ref
            );

            while let Some(x) = buffer.pop() {
                to_commit.push(x.clone());

                let ancestors: Vec<VerifiedBlockHeader> = dag_state
                    .get_blocks(
                        &x.ancestors()
                            .iter()
                            .copied()
                            .filter(|ancestor| {
                                ancestor.round > GENESIS_ROUND && !dag_state.is_committed(ancestor)
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
                    assert!(
                        dag_state.set_committed(&ancestor.reference()),
                        "Block with reference {:?} attempted to be committed twice",
                        ancestor.reference()
                    );
                }
            }
        } else {
            let mut committed = HashSet::new();
            assert!(committed.insert(leader_block_ref));

            while let Some(x) = buffer.pop() {
                to_commit.push(x.clone());

                let ancestors: Vec<VerifiedBlockHeader> = dag_state
                    .get_blocks(
                        &x.ancestors()
                            .iter()
                            .copied()
                            .filter(|ancestor| {
                                // We skip the block if we already committed it or we reached a
                                // round that we already committed.
                                !committed.contains(ancestor)
                                    && last_committed_rounds[ancestor.author] < ancestor.round
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
    ) -> Vec<CommittedSubDag> {
        if committed_leaders.is_empty() {
            return vec![];
        }

        // We check whether the leader schedule has been updated. If yes, then we'll
        // send the scores as part of the first sub dag.
        let schedule_updated = self
            .leader_schedule
            .leader_schedule_updated(&self.dag_state);

        let mut committed_sub_dags = vec![];
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

            // Collect the sub-dag generated using each of these leaders and the
            // corresponding commit.
            let (sub_dag, commit) =
                self.collect_sub_dag_and_commit(leader_block, reputation_scores_desc);

            // Buffer commit in dag state for persistence later.
            // This also updates the last committed rounds.
            self.dag_state.write().add_commit(commit.clone());

            committed_sub_dags.push(sub_dag);
        }

        // Committed blocks must be persisted to storage before sending them to IOTA and
        // executing their transactions.
        // Commit metadata can be persisted more lazily because they are recoverable.
        // Uncommitted blocks can wait to persist too.
        // But for simplicity, all unpersisted blocks and commits are flushed to
        // storage.
        self.dag_state.write().flush();

        committed_sub_dags
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        CommitIndex,
        commit::{CommitAPI as _, CommitDigest, WAVE_LENGTH},
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
                // First subdag includes the leader block only
                assert_eq!(subdag.blocks.len(), 1);
            } else {
                // Every subdag after will be missing the leader block from the previous
                // committed subdag
                assert_eq!(subdag.blocks.len(), num_authorities);
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
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
        dag_state.write().add_scoring_subdags(commits);
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
        let (mut context, _) = Context::new_for_test(num_authorities);
        context
            .protocol_config
            .set_consensus_linearize_subdag_v2_for_testing(false);

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
        let mut blocks = dag_builder.blocks(0..=leader_round_wave_1 - 1);
        blocks.push(
            dag_builder
                .leader_block(leader_round_wave_1)
                .expect("Leader block should have been found"),
        );
        dag_state.write().accept_blocks(blocks.clone());

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
        );
        dag_state.write().add_commit(first_commit_data);

        // Now take all the blocks from round `leader_round_wave_1` up to round
        // `leader_round_wave_2-1`
        let mut blocks = dag_builder.blocks(leader_round_wave_1..=leader_round_wave_2 - 1);
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
        dag_state.write().accept_blocks(blocks.clone());

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
        let (mut context, _keys) = Context::new_for_test(num_authorities);
        context.protocol_config.set_gc_depth_for_testing(0);
        context
            .protocol_config
            .set_consensus_linearize_subdag_v2_for_testing(false);

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
                    A -> [-D1],
                    B -> [-D1],
                    C -> [-D1],
                    D -> [*],
                },
                Round 3 : {
                    A -> [-D2],
                    B -> [-D2],
                    C -> [-D2],
                },
                Round 4 : {
                    A -> [-D3],
                    B -> [-D3],
                    C -> [-D3],
                    D -> [A3, B3, C3, D2],
                },
                Round 5 : { * },
            }";

        let (_, dag_builder) = parse_dag(dag_str).expect("Invalid dag");
        dag_builder.print();
        dag_builder.persist_all_blocks(dag_state.clone());

        let leaders = dag_builder
            .leader_blocks(1..=6)
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
            } else if idx == 1 {
                assert_eq!(subdag.blocks.len(), 3);
            } else if idx == 2 {
                // We commit:
                // * 1 block on round 4, the leader block
                // * 3 blocks on round 3, as no commit happened on round 3 since the leader was
                //   missing
                // * 2 blocks on round 2, again as no commit happened on round 3, we commit the
                //   "sub dag" of leader of round 3, which will be another 2 blocks
                assert_eq!(subdag.blocks.len(), 6);
            } else {
                // we expect to see all blocks of round >= 1
                assert_eq!(subdag.blocks.len(), 6);
                assert!(
                    subdag.blocks.iter().all(|block| block.round() >= 1),
                    "Found blocks that are of round < 1."
                );
            }
            for block in subdag.blocks.iter() {
                assert!(block.round() <= leaders[idx].round());
            }
            assert_eq!(subdag.commit_ref.index, idx as CommitIndex + 1);
        }
    }
}
