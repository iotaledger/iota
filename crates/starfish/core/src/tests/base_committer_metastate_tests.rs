// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    base_committer::base_committer_builder::BaseCommitterBuilder,
    block_header::{
        BlockHeader, BlockHeaderV2, BlockRef, BlockTimestampMs, Round, TransactionsCommitment,
        VerifiedBlockHeader,
    },
    commit::{CommitMetastate, LeaderStatus},
    context::Context,
    dag_state::{DagState, DataSource},
    storage::mem_store::MemStore,
    test_dag::build_dag,
};

/// Build a `BlockHeaderV2` wrapped as a verified block header for test use.
/// `timestamp_ms` is exposed so callers can produce two equivocating blocks
/// with the same `(round, author)` but distinct `BlockRef`s.
fn v2_block(
    round: Round,
    author: u8,
    ancestors: Vec<BlockRef>,
    strong_vote: Option<AuthoritySet>,
    timestamp_ms: BlockTimestampMs,
) -> VerifiedBlockHeader {
    let header = BlockHeaderV2::new(
        0,
        round,
        AuthorityIndex::from(author),
        timestamp_ms,
        ancestors,
        vec![],
        vec![],
        TransactionsCommitment::DEFAULT_FOR_TEST,
        strong_vote,
    );
    VerifiedBlockHeader::new_for_test(BlockHeader::V2(header))
}

/// Default timestamp for single-block-per-slot callers.
fn default_ts(round: Round, author: u8) -> BlockTimestampMs {
    round as BlockTimestampMs * 1000 + author as BlockTimestampMs
}

/// Test context with starfish-speed enabled by default. Returns the context
/// and an empty in-memory DAG state.
fn test_context_with_flag(enable_starfish_speed: bool) -> (Arc<Context>, Arc<RwLock<DagState>>) {
    let (mut ctx, _) = Context::new_for_test(4);
    ctx.protocol_config
        .set_consensus_starfish_speed_for_testing(enable_starfish_speed);
    let ctx = Arc::new(ctx);
    let dag_state = Arc::new(RwLock::new(DagState::new(
        ctx.clone(),
        Arc::new(MemStore::new(ctx.clone())),
    )));
    (ctx, dag_state)
}

/// Populate `dag_state` with a wave-1 DAG in which every round-4 voter has
/// `strong_vote == voter_strong_votes[i]` and every round-5 certifier links
/// to all four round-4 voters. Returns the elected leader slot at round 3.
fn build_metastate_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> crate::block_header::Slot {
    // Rounds 1..=3 are built fully-connected with V1 blocks.
    let round_3_refs = build_dag(
        context.clone(),
        dag_state.clone(),
        None,
        committer.leader_round(1),
    );

    // Round 4 (voting): each voter links to all round-3 blocks and carries the
    // configured strong_vote.
    let mut round_4_refs = Vec::new();
    let voting_round = committer.leader_round(1) + 1;
    for (author, strong_vote) in voter_strong_votes.into_iter().enumerate() {
        let author = author as u8;
        let block = v2_block(
            voting_round,
            author,
            round_3_refs.clone(),
            strong_vote,
            default_ts(voting_round, author),
        );
        round_4_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    // Round 5 (certifying): each certifier links to all round-4 blocks.
    let certifying_round = committer.certifying_round(1);
    for author in 0..context.committee.size() {
        let author = author as u8;
        let block = v2_block(
            certifying_round,
            author,
            round_4_refs.clone(),
            None,
            default_ts(certifying_round, author),
        );
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    committer
        .elect_leader(committer.leader_round(1))
        .expect("wave 1 should elect a leader")
}

#[tokio::test]
async fn determine_metastate_disabled_returns_none() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(false);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [Some(AuthoritySet::new()); 4],
    );

    match committer.try_direct_decide(leader) {
        LeaderStatus::Commit(_, metastate) => assert_eq!(metastate, None),
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn determine_metastate_optimistic_when_strong_qc_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // All four voters carry strong_vote = Some(empty) → every certifier at r+2
    // will observe four strong votes in its ancestry, so it is a StrongQC. The
    // 2f+1 StrongQC quorum at r+2 is reached → Optimistic.
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [Some(AuthoritySet::new()); 4],
    );

    match committer.try_direct_decide(leader) {
        LeaderStatus::Commit(_, metastate) => {
            assert_eq!(metastate, Some(CommitMetastate::Optimistic))
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn determine_metastate_standard_when_strong_blame_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Three voters carry strong_vote = Some(nonempty) (strong blame). Only one
    // voter is a strong vote → no StrongQC quorum possible. 2f+1 strong blames
    // form a quorum → Standard.
    let blame = Some(AuthoritySet::new_with(
        AuthorityIndex::from(0u8),
        AuthorityIndex::from(0u8),
    ));
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [Some(AuthoritySet::new()), blame, blame, blame],
    );

    match committer.try_direct_decide(leader) {
        LeaderStatus::Commit(_, metastate) => {
            assert_eq!(metastate, Some(CommitMetastate::Standard))
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn determine_metastate_pending_when_neither_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Split: 2 strong votes, 2 strong blames. Neither side reaches the 2f+1 = 3
    // threshold → Pending.
    let blame = Some(AuthoritySet::new_with(
        AuthorityIndex::from(0u8),
        AuthorityIndex::from(0u8),
    ));
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [
            Some(AuthoritySet::new()),
            Some(AuthoritySet::new()),
            blame,
            blame,
        ],
    );

    match committer.try_direct_decide(leader) {
        LeaderStatus::Commit(_, metastate) => {
            assert_eq!(metastate, Some(CommitMetastate::Pending))
        }
        status => panic!("expected Commit, got {status}"),
    }
}

/// Selects which of the two equivocating leader blocks a round-4 voter supports
/// via its ancestor chain.
#[derive(Clone, Copy)]
enum LeaderChoice {
    A,
    B,
}

/// Populate `dag_state` with a wave-1 DAG where the leader author produces two
/// equivocating blocks `L_A` and `L_B` at the leader round. Each round-4 voter
/// is wired (per `voter_config`) to include **either** `L_A` or `L_B` in its
/// ancestors, plus all three non-leader round-3 blocks, and carries the
/// configured `strong_vote`. Round 5 certifiers link to all four voters.
/// Returns `(leader_slot, L_A ref, L_B ref)`.
fn build_equivocating_metastate_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_config: [(LeaderChoice, Option<AuthoritySet>); 4],
) -> (crate::block_header::Slot, BlockRef, BlockRef) {
    let leader_round = committer.leader_round(1);
    let voting_round = leader_round + 1;
    let certifying_round = committer.certifying_round(1);

    // Rounds 1..=leader_round-1 built uniformly.
    let prev_refs = build_dag(context.clone(), dag_state.clone(), None, leader_round - 1);

    let leader_slot = committer
        .elect_leader(leader_round)
        .expect("wave 1 should elect a leader");
    let leader_author: u8 = leader_slot.authority.value() as u8;

    // Round `leader_round`: one block per non-leader authority, plus TWO
    // equivocating leader blocks (different timestamps → distinct refs).
    let mut non_leader_refs = Vec::new();
    for author in 0..context.committee.size() {
        let author = author as u8;
        if author == leader_author {
            continue;
        }
        let block = v2_block(
            leader_round,
            author,
            prev_refs.clone(),
            None,
            default_ts(leader_round, author),
        );
        non_leader_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    let leader_a = v2_block(
        leader_round,
        leader_author,
        prev_refs.clone(),
        None,
        default_ts(leader_round, leader_author),
    );
    let leader_b = v2_block(
        leader_round,
        leader_author,
        prev_refs,
        None,
        default_ts(leader_round, leader_author) + 1,
    );
    let leader_a_ref = leader_a.reference();
    let leader_b_ref = leader_b.reference();
    dag_state
        .write()
        .accept_block_header(leader_a, DataSource::Test);
    dag_state
        .write()
        .accept_block_header(leader_b, DataSource::Test);

    // Round `voting_round`: each voter includes all non-leader round-3 blocks
    // plus the chosen leader block, and carries the configured strong_vote.
    let mut round_4_refs = Vec::new();
    for (author, (leader_choice, strong_vote)) in voter_config.into_iter().enumerate() {
        let author = author as u8;
        let chosen_leader = match leader_choice {
            LeaderChoice::A => leader_a_ref,
            LeaderChoice::B => leader_b_ref,
        };
        let mut ancestors = non_leader_refs.clone();
        ancestors.push(chosen_leader);
        let block = v2_block(
            voting_round,
            author,
            ancestors,
            strong_vote,
            default_ts(voting_round, author),
        );
        round_4_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    // Round `certifying_round`: each certifier links to all voters.
    for author in 0..context.committee.size() {
        let author = author as u8;
        let block = v2_block(
            certifying_round,
            author,
            round_4_refs.clone(),
            None,
            default_ts(certifying_round, author),
        );
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    (leader_slot, leader_a_ref, leader_b_ref)
}

#[tokio::test]
async fn determine_metastate_optimistic_under_equivocation() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // All four voters carry `is_strong_vote() == true`, but one supports the
    // equivocating `L_B` instead of `L_A`. Only the three voters whose
    // ancestor chain resolves to `L_A` count as strong votes for `L_A`, which
    // still meets the 2f+1 threshold → Optimistic. This exercises the
    // `is_vote()` conjunction inside `has_strong_qc_quorum`.
    let strong = || Some(AuthoritySet::new());
    let (leader_slot, leader_a_ref, _leader_b_ref) = build_equivocating_metastate_dag(
        context,
        dag_state,
        &committer,
        [
            (LeaderChoice::A, strong()),
            (LeaderChoice::A, strong()),
            (LeaderChoice::A, strong()),
            (LeaderChoice::B, strong()),
        ],
    );

    match committer.try_direct_decide(leader_slot) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference(), leader_a_ref);
            assert_eq!(metastate, Some(CommitMetastate::Optimistic));
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn determine_metastate_standard_under_equivocation() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Three voters supporting `L_A` carry strong_blame, one voter supporting
    // `L_B` carries strong_vote. For `L_A`: no strong-vote-for-A exists (the
    // lone strong vote points at L_B and must be filtered out), and the three
    // blamers of `L_A` meet the 2f+1 threshold → Standard. This exercises the
    // `is_vote()` filter inside both quorum checks.
    let blame = Some(AuthoritySet::new_with(
        AuthorityIndex::from(0u8),
        AuthorityIndex::from(0u8),
    ));
    let strong = Some(AuthoritySet::new());
    let (leader_slot, leader_a_ref, _leader_b_ref) = build_equivocating_metastate_dag(
        context,
        dag_state,
        &committer,
        [
            (LeaderChoice::A, blame),
            (LeaderChoice::A, blame),
            (LeaderChoice::A, blame),
            (LeaderChoice::B, strong),
        ],
    );

    match committer.try_direct_decide(leader_slot) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference(), leader_a_ref);
            assert_eq!(metastate, Some(CommitMetastate::Standard));
        }
        status => panic!("expected Commit, got {status}"),
    }
}
