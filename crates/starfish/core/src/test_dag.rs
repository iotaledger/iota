// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::{
    authority_set::AuthoritySet,
    block_header::{
        BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, StrongVote, TestBlockHeader,
        TestBlockHeaderVersion, VerifiedBlockHeader, genesis_block_headers,
    },
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    test_dag_builder::DagBuilder,
};

/// Build a fully interconnected dag up to the specified round. This function
/// starts building the dag from the specified [`start`] parameter or from
/// genesis if none are specified up to and including the specified round
/// [`stop`] parameter.
pub(crate) fn build_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    start: Option<Vec<BlockRef>>,
    stop: Round,
) -> Vec<BlockRef> {
    let mut ancestors = match start {
        Some(start) => {
            assert!(!start.is_empty());
            assert_eq!(
                start.iter().map(|x| x.round).max(),
                start.iter().map(|x| x.round).min()
            );
            start
        }
        None => genesis_block_headers(&context)
            .iter()
            .map(|x| x.reference())
            .collect::<Vec<_>>(),
    };

    let num_authorities = context.committee.size();
    let starting_round = ancestors.first().unwrap().round + 1;
    let leader_schedule = LeaderSchedule::new(context.clone(), LeaderSwapTable::default());
    let version = TestBlockHeaderVersion::V2;
    for round in starting_round..=stop {
        // Every block of the round links the same ancestors, so they all carry
        // the same vote.
        let strong_vote = strong_vote_for_leader(&leader_schedule, round, &ancestors);
        let (references, blocks): (Vec<_>, Vec<_>) = context
            .committee
            .authorities()
            .map(|authority| {
                let author_idx = authority.0.value() as u8;
                // Test the case where a block from round R+1 has smaller timestamp than a block
                // from round R.
                let ts = round as BlockTimestampMs / 2 * num_authorities as BlockTimestampMs
                    + author_idx as BlockTimestampMs;
                let block = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author_idx)
                        .set_version(version)
                        .set_timestamp_ms(ts)
                        .set_ancestors(ancestors.clone())
                        .set_strong_vote(strong_vote)
                        .build(),
                );

                (block.reference(), block)
            })
            .unzip();
        dag_state
            .write()
            .accept_block_headers(blocks, DataSource::Test);
        ancestors = references;
    }

    ancestors
}

/// The strong vote for a block at `round` linking `ancestors`. Blocks built
/// here acknowledge nothing, so the vote on the leader at `round - 1` is a
/// blame, except on the genesis leader, which carries no transactions to be
/// missing. `None` when the block does not link the leader.
fn strong_vote_for_leader(
    leader_schedule: &LeaderSchedule,
    round: Round,
    ancestors: &[BlockRef],
) -> Option<StrongVote> {
    let leader_round = round - 1;
    let leader_authority = leader_schedule.elect_leader(leader_round, 0);
    let mut missing = AuthoritySet::new();
    if leader_round != GENESIS_ROUND {
        if !ancestors
            .iter()
            .any(|r| r.round == leader_round && r.author == leader_authority)
        {
            return None;
        }
        missing.insert(leader_authority);
    }
    Some(StrongVote {
        leader_authority,
        missing,
    })
}

/// Builds `num_rounds` rounds in which every block links its own previous
/// block and a random quorum of the previous round. In
/// `include_leader_percentage` percent of the rounds every block also links
/// the leader of the previous round; in the other rounds it may still link it
/// by chance. Each authority acknowledges each block of the previous round
/// with `acknowledgment_percentage` percent chance, so lower values make
/// strong votes blame leaders. In every round each authority in
/// `equivocators` proposes `1 + equivocations` blocks with the same parents
/// and acknowledgments.
pub(crate) fn create_random_dag(
    seed: u64,
    include_leader_percentage: u64,
    acknowledgment_percentage: u64,
    num_rounds: Round,
    equivocators: &[AuthorityIndex],
    equivocations: usize,
    context: Arc<Context>,
) -> DagBuilder {
    assert!(
        (0..=100).contains(&include_leader_percentage),
        "include_leader_percentage must be in the range 0..100"
    );
    assert!(
        acknowledgment_percentage <= 100,
        "acknowledgment_percentage must be in the range 0..=100"
    );
    let max_faulty = (context.committee.size() - 1) / 3;
    assert!(
        equivocators.len() <= max_faulty,
        "{} equivocators exceed the {max_faulty} faulty authorities the committee tolerates",
        equivocators.len()
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut dag_builder = DagBuilder::new(context);

    for r in 1..=num_rounds {
        let include_leader = rng.random_range(0..100) < include_leader_percentage;
        let parents_seed = rng.random();
        let mut layer = dag_builder.layer(r);
        if acknowledgment_percentage < 100 {
            layer = layer.random_acknowledgments(acknowledgment_percentage);
        }
        let layer = if equivocators.is_empty() {
            layer
        } else {
            layer
                .authorities(equivocators.to_vec())
                .equivocate(equivocations)
        };
        layer.min_ancestor_links(include_leader, Some(parents_seed));
    }

    dag_builder
}
