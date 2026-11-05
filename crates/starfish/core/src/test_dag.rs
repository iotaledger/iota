// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use starfish_config::AuthorityIndex;

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

// todo: remove this once tests have been refactored to use DagBuilder/DagParser

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
    let version = TestBlockHeaderVersion::from_context(&context);
    for round in starting_round..=stop {
        // Every block of the round links the same ancestors, so they all carry
        // the same vote.
        let strong_vote = strong_vote_for_leader(&context, &leader_schedule, round, &ancestors);
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

// TODO: Add layer_round as input parameter so ancestors can be from any round.
pub(crate) fn build_dag_layer(
    context: &Arc<Context>,
    // A list of (authority, parents) pairs. For each authority, we add a block
    // linking to the specified parents.
    connections: Vec<(AuthorityIndex, Vec<BlockRef>)>,
    dag_state: Arc<RwLock<DagState>>,
) -> Vec<BlockRef> {
    let leader_schedule = LeaderSchedule::new(context.clone(), LeaderSwapTable::default());
    let version = TestBlockHeaderVersion::from_context(context);
    let mut references = Vec::new();
    for (authority, ancestors) in connections {
        let round = ancestors.first().unwrap().round + 1;
        let author = authority.value() as u8;
        let block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(round, author)
                .set_version(version)
                .set_strong_vote(strong_vote_for_leader(
                    context,
                    &leader_schedule,
                    round,
                    &ancestors,
                ))
                .set_ancestors(ancestors)
                .build(),
        );
        references.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }
    references
}

/// The strong vote for a block at `round` linking `ancestors`. Blocks built
/// here acknowledge nothing, so the vote on the leader at `round - 1` is a
/// blame, except on the genesis leader, which carries no transactions to be
/// missing. `None` when the block does not link the leader, or while
/// `consensus_starfish_speed` is off.
fn strong_vote_for_leader(
    context: &Context,
    leader_schedule: &LeaderSchedule,
    round: Round,
    ancestors: &[BlockRef],
) -> Option<StrongVote> {
    if !context.protocol_config.consensus_starfish_speed() {
        return None;
    }
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

pub(crate) fn create_random_dag(
    seed: u64,
    include_leader_percentage: u64,
    num_rounds: Round,
    context: Arc<Context>,
) -> DagBuilder {
    assert!(
        (0..=100).contains(&include_leader_percentage),
        "include_leader_percentage must be in the range 0..100"
    );

    let mut rng = StdRng::seed_from_u64(seed);
    let mut dag_builder = DagBuilder::new(context);

    for r in 1..=num_rounds {
        let random_num = rng.random_range(0..100);
        let include_leader = random_num <= include_leader_percentage;
        dag_builder
            .layer(r)
            .min_ancestor_links(include_leader, Some(random_num));
    }

    dag_builder
}
