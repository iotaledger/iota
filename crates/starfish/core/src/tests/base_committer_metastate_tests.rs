// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    base_committer::base_committer_builder::BaseCommitterBuilder,
    block_header::{
        BlockHeader, BlockHeaderV2, BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, Slot,
        TransactionsCommitment, VerifiedBlockHeader, genesis_block_headers,
    },
    commit::{CommitMetastate, DecidedLeader, LeaderStatus},
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    storage::mem_store::MemStore,
    universal_committer::universal_committer_builder::UniversalCommitterBuilder,
};

/// Verified V2 header for tests. `timestamp_ms` distinguishes equivocating
/// blocks at the same `(round, author)`.
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

/// Empty `AuthoritySet` — flags the carrier as a strong vote.
fn strong_vote() -> Option<AuthoritySet> {
    Some(AuthoritySet::new())
}

/// Non-empty `AuthoritySet` — flags the carrier as a strong blame. The
/// specific authority is immaterial; only emptiness matters.
fn strong_blame() -> Option<AuthoritySet> {
    let mut s = AuthoritySet::new();
    s.insert(AuthorityIndex::from(0u8));
    Some(s)
}

/// Assert that direct-committing `slot` yields `Commit(_, expected)`.
fn assert_direct_commit_metastate(
    committer: &crate::base_committer::BaseCommitter,
    slot: Slot,
    expected: Option<CommitMetastate>,
) {
    match committer.try_direct_decide(slot) {
        LeaderStatus::Commit(_, metastate) => assert_eq!(metastate, expected),
        status => panic!("expected Commit, got {status}"),
    }
}

/// Fully-connected V2 layers from `start` (or genesis) through `stop`. V2
/// mirror of `test_dag::build_dag`, matching the DAG shape used when
/// `consensus_starfish_speed` is enabled.
fn build_v2_layers(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    start: Option<Vec<BlockRef>>,
    stop: Round,
) -> Vec<BlockRef> {
    let mut ancestors: Vec<BlockRef> = match start {
        Some(refs) => refs,
        None => genesis_block_headers(context)
            .iter()
            .map(|b| b.reference())
            .collect(),
    };
    let starting_round = ancestors.first().map(|r| r.round).unwrap_or(0) + 1;
    for round in starting_round..=stop {
        let mut refs = Vec::new();
        for author in 0..context.committee.size() {
            let author = author as u8;
            let block = v2_block(
                round,
                author,
                ancestors.clone(),
                None,
                default_ts(round, author),
            );
            refs.push(block.reference());
            dag_state
                .write()
                .accept_block_header(block, DataSource::Test);
        }
        ancestors = refs;
    }
    ancestors
}

/// Test context with the starfish-speed flag set, plus an empty DAG state.
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

/// Populate `dag_state` through round 5; round-4 voters carry the given
/// `strong_vote` values. Returns the leader slot at round 3.
fn build_metastate_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> crate::block_header::Slot {
    let round_3_refs = build_v2_layers(&context, &dag_state, None, committer.leader_round(1));

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
        .expect("should elect a leader")
}

#[tokio::test]
async fn determine_metastate_disabled_returns_none() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(false);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    let leader = build_metastate_dag(context, dag_state, &committer, [strong_vote(); 4]);
    assert_direct_commit_metastate(&committer, leader, None);
}

#[tokio::test]
async fn determine_metastate_optimistic_when_strong_qc_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Four strong votes at r+1 → every r+2 certifier observes a StrongQC →
    // 2f+1 StrongQC quorum → Optimistic.
    let leader = build_metastate_dag(context, dag_state, &committer, [strong_vote(); 4]);
    assert_direct_commit_metastate(&committer, leader, Some(CommitMetastate::Optimistic));
}

#[tokio::test]
async fn determine_metastate_standard_when_strong_blame_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // 3 strong blames + 1 strong vote → no StrongQC quorum, 2f+1 strong-blame
    // quorum → Standard.
    let blame = strong_blame();
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [strong_vote(), blame, blame, blame],
    );
    assert_direct_commit_metastate(&committer, leader, Some(CommitMetastate::Standard));
}

#[tokio::test]
async fn determine_metastate_pending_when_neither_quorum() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // 2 strong votes + 2 strong blames → neither side reaches 2f+1 = 3 → Pending.
    let blame = strong_blame();
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [strong_vote(), strong_vote(), blame, blame],
    );
    assert_direct_commit_metastate(&committer, leader, Some(CommitMetastate::Pending));
}

/// Which equivocating leader a voter supports.
#[derive(Clone, Copy)]
enum LeaderChoice {
    A,
    B,
}

/// Same as `build_metastate_dag`, but the leader author produces two
/// equivocating blocks `L_A`/`L_B` at round 3 and voters split per
/// `voter_config`. Returns `(leader_slot, L_A, L_B)`.
fn build_equivocating_metastate_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_config: [(LeaderChoice, Option<AuthoritySet>); 4],
) -> (crate::block_header::Slot, BlockRef, BlockRef) {
    let leader_round = committer.leader_round(1);
    let voting_round = leader_round + 1;
    let certifying_round = committer.certifying_round(1);

    let prev_refs = build_v2_layers(&context, &dag_state, None, leader_round - 1);

    let leader_slot = committer
        .elect_leader(leader_round)
        .expect("should elect a leader");
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
async fn determine_metastate_pending_when_equivocating_strong_vote_is_filtered() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Discriminator for the `is_vote()` filter in `has_strong_qc_quorum`:
    // - 2 voters are strong votes for `L_A`
    // - 1 voter votes for `L_A` with no strong_vote
    // - 1 voter is a strong vote for the equivocating `L_B`
    //
    // `L_A` has 3 regular votes → committable. Strong votes *attributable*
    // to `L_A` are only 2 → below the 2f+1 = 3 threshold → not `Optimistic`
    // and not `Standard` (no blames) → `Pending`. If the `is_vote()` filter
    // were missing, the `L_B`-directed strong vote would be miscounted for
    // `L_A`, reaching the threshold and yielding a wrong `Optimistic`.
    let strong = strong_vote();
    let (leader_slot, leader_a_ref, _leader_b_ref) = build_equivocating_metastate_dag(
        context,
        dag_state,
        &committer,
        [
            (LeaderChoice::A, strong),
            (LeaderChoice::A, strong),
            (LeaderChoice::A, None),
            (LeaderChoice::B, strong),
        ],
    );

    match committer.try_direct_decide(leader_slot) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference(), leader_a_ref);
            assert_eq!(metastate, Some(CommitMetastate::Pending));
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn determine_metastate_pending_when_equivocating_strong_blame_is_filtered() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Discriminator for the `is_vote()` filter in `has_strong_blame_quorum`:
    // - 2 voters blame `L_A`
    // - 1 voter votes for `L_A` with no strong_vote
    // - 1 voter blames the equivocating `L_B`
    //
    // `L_A` has 3 regular votes → committable. Strong blames *attributable*
    // to `L_A` are only 2 → below the 2f+1 = 3 threshold → `Pending`. If
    // the `is_vote()` filter were missing, the `L_B`-directed strong blame
    // would be miscounted for `L_A`, reaching the threshold and yielding a
    // wrong `Standard`.
    let blame = strong_blame();
    let (leader_slot, leader_a_ref, _leader_b_ref) = build_equivocating_metastate_dag(
        context,
        dag_state,
        &committer,
        [
            (LeaderChoice::A, blame),
            (LeaderChoice::A, blame),
            (LeaderChoice::A, None),
            (LeaderChoice::B, blame),
        ],
    );

    match committer.try_direct_decide(leader_slot) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference(), leader_a_ref);
            assert_eq!(metastate, Some(CommitMetastate::Pending));
        }
        status => panic!("expected Commit, got {status}"),
    }
}

/// Populate `dag_state` through round 4 (voters link to all round-3 blocks).
/// Returns the leader slot and voter refs indexed by author.
fn build_through_voting_round(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> (crate::block_header::Slot, Vec<BlockRef>) {
    let leader_round = committer.leader_round(1);
    let voting_round = leader_round + 1;
    let round_3_refs = build_v2_layers(context, dag_state, None, leader_round);

    let mut voter_refs = Vec::with_capacity(4);
    for (author, strong_vote) in voter_strong_votes.into_iter().enumerate() {
        let author = author as u8;
        let block = v2_block(
            voting_round,
            author,
            round_3_refs.clone(),
            strong_vote,
            default_ts(voting_round, author),
        );
        voter_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    let leader_slot = committer
        .elect_leader(leader_round)
        .expect("should elect a leader");
    (leader_slot, voter_refs)
}

/// Build one round-5 certifier from the specified round-4 voter refs.
fn certifier(
    committer: &crate::base_committer::BaseCommitter,
    author: u8,
    voter_refs: Vec<BlockRef>,
) -> VerifiedBlockHeader {
    let round = committer.certifying_round(1);
    v2_block(round, author, voter_refs, None, default_ts(round, author))
}

/// Wave-1 DAG with a single StrongQC at r+2 (the others are regular QCs).
/// The direct rule classifies the round-3 leader as Pending under this
/// setup. Returns `(leader_slot, [c0, c1, c2, c3])` where `c0` is the
/// StrongQC and `c1..c3` are regular QCs.
fn build_wave_one_with_single_strong_qc(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
) -> (Slot, [BlockRef; 4]) {
    let (leader_slot, voters) = build_through_voting_round(
        context,
        dag_state,
        committer,
        [strong_vote(), strong_vote(), strong_vote(), None],
    );
    let c0 = certifier(committer, 0, vec![voters[0], voters[1], voters[2]]);
    let c1 = certifier(committer, 1, vec![voters[0], voters[1], voters[3]]);
    let c2 = certifier(committer, 2, vec![voters[0], voters[2], voters[3]]);
    let c3 = certifier(committer, 3, vec![voters[1], voters[2], voters[3]]);
    let refs = [
        c0.reference(),
        c1.reference(),
        c2.reference(),
        c3.reference(),
    ];
    for b in [c0, c1, c2, c3] {
        dag_state.write().accept_block_header(b, DataSource::Test);
    }
    (leader_slot, refs)
}

#[tokio::test]
async fn indirect_metastate_optimistic_when_anchor_path_contains_strong_qc() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    let (leader_slot, [c0, c1, c2, _c3]) =
        build_wave_one_with_single_strong_qc(&context, &dag_state, &committer);

    // Anchor at round 6 includes the StrongQC c0 in its round-5 ancestors.
    let anchor_round = committer.certifying_round(1) + 1;
    let anchor = v2_block(
        anchor_round,
        0,
        vec![c0, c1, c2],
        None,
        default_ts(anchor_round, 0),
    );
    dag_state
        .write()
        .accept_block_header(anchor.clone(), DataSource::Test);

    let anchor_status = LeaderStatus::Commit(anchor, None);
    let current = LeaderStatus::Undecided(leader_slot);
    match committer.try_indirect_decide(current, std::iter::once(&anchor_status)) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference().round, leader_slot.round);
            assert_eq!(block.reference().author, leader_slot.authority);
            assert_eq!(metastate, Some(CommitMetastate::Optimistic));
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn indirect_metastate_standard_when_strong_qc_outside_anchor_path() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    let (leader_slot, [_c0, c1, c2, c3]) =
        build_wave_one_with_single_strong_qc(&context, &dag_state, &committer);

    // Anchor excludes the StrongQC c0. Indirect must still resolve from the
    // restricted r+2 path and pick Standard, not Optimistic.
    let anchor_round = committer.certifying_round(1) + 1;
    let anchor = v2_block(
        anchor_round,
        1,
        vec![c1, c2, c3],
        None,
        default_ts(anchor_round, 1),
    );
    dag_state
        .write()
        .accept_block_header(anchor.clone(), DataSource::Test);

    let anchor_status = LeaderStatus::Commit(anchor, None);
    let current = LeaderStatus::Undecided(leader_slot);
    match committer.try_indirect_decide(current, std::iter::once(&anchor_status)) {
        LeaderStatus::Commit(block, metastate) => {
            assert_eq!(block.reference().round, leader_slot.round);
            assert_eq!(block.reference().author, leader_slot.authority);
            assert_eq!(metastate, Some(CommitMetastate::Standard));
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn leader_status_is_final_classification() {
    let (context, dag_state) = test_context_with_flag(true);
    let refs = build_v2_layers(&context, &dag_state, None, 1);
    let block = dag_state
        .read()
        .get_verified_block_header(&refs[0])
        .expect("round-1 block exists");

    let slot = Slot::new(3, AuthorityIndex::from(0u8));
    assert!(!LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Pending)).is_final());
    assert!(LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Optimistic)).is_final());
    assert!(LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Standard)).is_final());
    assert!(LeaderStatus::Commit(block, None).is_final());
    assert!(LeaderStatus::Skip(slot).is_final());
    assert!(!LeaderStatus::Undecided(slot).is_final());
}

fn build_universal_committer(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
) -> crate::universal_committer::UniversalCommitter {
    let leader_schedule = Arc::new(LeaderSchedule::new(
        context.clone(),
        LeaderSwapTable::default(),
    ));
    UniversalCommitterBuilder::new(context, leader_schedule, dag_state).build()
}

/// Round-3 leader's metastate in `decided`. Panics if not a Commit.
fn round_3_metastate(
    universal: &crate::universal_committer::UniversalCommitter,
    decided: &[DecidedLeader],
) -> Option<CommitMetastate> {
    let leader_authority = universal
        .get_leaders(3)
        .into_iter()
        .next()
        .expect("round-3 has a leader");
    let slot = Slot::new(3, leader_authority);
    let decided = decided
        .iter()
        .find(|d| d.slot() == slot)
        .expect("round-3 leader should be decided");
    match decided {
        DecidedLeader::Commit(_, m) => *m,
        other => panic!("expected round-3 Commit, got {other:?}"),
    }
}

#[tokio::test]
async fn pending_leader_resolves_to_optimistic() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let universal = build_universal_committer(context.clone(), dag_state.clone());
    let base = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    let (round_3_slot, round_5_refs) =
        build_wave_one_with_single_strong_qc(&context, &dag_state, &base);

    // No wave-2 anchor yet — direct says Pending and can't upgrade.
    assert_direct_commit_metastate(&base, round_3_slot, Some(CommitMetastate::Pending));

    // Wave 2 pulls the StrongQC (c0) into the round-6 anchor's r+2 path.
    build_v2_layers(&context, &dag_state, Some(round_5_refs.to_vec()), 8);

    let decided = universal.try_decide(Slot::new(GENESIS_ROUND, 0u8));
    assert_eq!(
        round_3_metastate(&universal, &decided),
        Some(CommitMetastate::Optimistic),
    );
}

#[tokio::test]
async fn pending_leader_resolves_to_standard() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let universal = build_universal_committer(context.clone(), dag_state.clone());
    let base = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // Wave 1 only — strong_vote = None everywhere → direct says Pending.
    let round_5_refs = build_v2_layers(&context, &dag_state, None, 5);
    let round_3_slot = base.elect_leader(3).expect("round 3 has a leader");
    assert_direct_commit_metastate(&base, round_3_slot, Some(CommitMetastate::Pending));

    // Wave 2 arrives. The round-6 anchor's r+2 path has only regular QCs.
    build_v2_layers(&context, &dag_state, Some(round_5_refs), 8);

    let decided = universal.try_decide(Slot::new(GENESIS_ROUND, 0u8));
    assert_eq!(
        round_3_metastate(&universal, &decided),
        Some(CommitMetastate::Standard),
    );
}
