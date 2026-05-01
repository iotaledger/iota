// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    base_committer::{
        BaseCommitter, BaseCommitterOptions, base_committer_builder::BaseCommitterBuilder,
    },
    block_header::{
        BlockHeader, BlockHeaderV2, BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, Slot,
        StrongVote, TransactionsCommitment, VerifiedBlockHeader, VerifiedTransactions,
        genesis_block_headers,
    },
    commit::{CommitMetastate, DecidedLeader, LeaderStatus},
    context::Context,
    core::Core,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    linearizer::Linearizer,
    storage::mem_store::MemStore,
    transaction_ref::GenericTransactionRef,
    universal_committer::universal_committer_builder::UniversalCommitterBuilder,
};

/// Verified V2 header for tests. `timestamp_ms` distinguishes equivocating
/// blocks at the same `(round, author)`.
fn v2_block(
    round: Round,
    author: u8,
    ancestors: Vec<BlockRef>,
    strong_vote: Option<StrongVote>,
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

/// `v2_block` variant that takes a non-empty `acks` list.
fn v2_block_with_acks(
    round: Round,
    author: u8,
    ancestors: Vec<BlockRef>,
    acks: Vec<BlockRef>,
    strong_vote: Option<StrongVote>,
    timestamp_ms: BlockTimestampMs,
) -> VerifiedBlockHeader {
    let header = BlockHeaderV2::new(
        0,
        round,
        AuthorityIndex::from(author),
        timestamp_ms,
        ancestors,
        acks,
        vec![],
        TransactionsCommitment::DEFAULT_FOR_TEST,
        strong_vote,
    );
    VerifiedBlockHeader::new_for_test(BlockHeader::V2(header))
}

/// Wraps a "missing" mask into a `StrongVote` pinned to `leader_authority`.
/// `None` passes through (no vote).
fn pin_strong_vote(
    leader_authority: AuthorityIndex,
    missing: Option<AuthoritySet>,
) -> Option<StrongVote> {
    missing.map(|missing| StrongVote {
        leader_authority,
        missing,
    })
}

/// Marks `header`'s transaction data as locally available, so
/// `DagState::is_data_available` returns true for that ref.
fn add_transactions_for(dag_state: &Arc<RwLock<DagState>>, header: &VerifiedBlockHeader) {
    let verified = VerifiedTransactions::new_for_test(header, vec![]);
    dag_state
        .write()
        .add_transactions(verified, DataSource::Test);
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
        LeaderStatus::Commit(_, metastate, _) => assert_eq!(metastate, expected),
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

/// Populate `dag_state` through round 5; round-4 voters carry strong-vote
/// payloads pinned to the canonical round-3 leader. Returns the leader slot at
/// round 3.
fn build_metastate_dag(
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: &crate::base_committer::BaseCommitter,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> crate::block_header::Slot {
    let leader_round = committer.leader_round(1);
    let round_3_refs = build_v2_layers(&context, &dag_state, None, leader_round);
    let leader_authority = committer
        .elect_leader(leader_round)
        .expect("should elect a leader")
        .authority;

    // Round 4 (voting): each voter links to all round-3 blocks and carries the
    // configured missing-mask, pinned to the canonical leader.
    let mut round_4_refs = Vec::new();
    let voting_round = leader_round + 1;
    for (author, missing) in voter_strong_votes.into_iter().enumerate() {
        let author = author as u8;
        let block = v2_block(
            voting_round,
            author,
            round_3_refs.clone(),
            pin_strong_vote(leader_authority, missing),
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
async fn strong_qc_quorum_collects_strong_voter_authorities_from_dag() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let committer = BaseCommitterBuilder::new(context.clone(), dag_state.clone()).build();

    // 3 strong votes + 1 strong blame: every r+2 certifier still observes a
    // 2f+1 strong-vote quorum at r+1 (Optimistic), but the strong-voter list
    // returned by `strong_qc_quorum` must contain exactly authors {0, 1, 2}
    // and exclude the strong-blamer at author 3.
    let blame = strong_blame();
    let leader = build_metastate_dag(
        context,
        dag_state,
        &committer,
        [strong_vote(), strong_vote(), strong_vote(), blame],
    );

    let strong_voters = match committer.try_direct_decide(leader) {
        LeaderStatus::Commit(_, Some(CommitMetastate::Optimistic), strong_voters) => strong_voters,
        other => panic!("expected Commit(Optimistic), got {other}"),
    };
    let collected: BTreeSet<AuthorityIndex> = strong_voters.into_iter().collect();
    let expected: BTreeSet<AuthorityIndex> = (0..3u8).map(AuthorityIndex::from).collect();
    assert_eq!(collected, expected);
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
    // plus the chosen leader block, and carries the configured missing-mask
    // pinned to the (shared) leader authority — the equivocating L_A and L_B
    // share the same author.
    let mut round_4_refs = Vec::new();
    for (author, (leader_choice, missing)) in voter_config.into_iter().enumerate() {
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
            pin_strong_vote(leader_slot.authority, missing),
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
        LeaderStatus::Commit(block, metastate, _) => {
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
        LeaderStatus::Commit(block, metastate, _) => {
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
    let leader_authority = committer
        .elect_leader(leader_round)
        .expect("should elect a leader")
        .authority;

    let mut voter_refs = Vec::with_capacity(4);
    for (author, missing) in voter_strong_votes.into_iter().enumerate() {
        let author = author as u8;
        let block = v2_block(
            voting_round,
            author,
            round_3_refs.clone(),
            pin_strong_vote(leader_authority, missing),
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

    let anchor_status = LeaderStatus::Commit(anchor, None, vec![]);
    let current = LeaderStatus::Undecided(leader_slot);
    match committer.try_indirect_decide(current, std::iter::once(&anchor_status)) {
        LeaderStatus::Commit(block, metastate, _) => {
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

    let anchor_status = LeaderStatus::Commit(anchor, None, vec![]);
    let current = LeaderStatus::Undecided(leader_slot);
    match committer.try_indirect_decide(current, std::iter::once(&anchor_status)) {
        LeaderStatus::Commit(block, metastate, _) => {
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
    assert!(
        !LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Pending), vec![]).is_final()
    );
    assert!(
        LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Optimistic), vec![]).is_final()
    );
    assert!(
        LeaderStatus::Commit(block.clone(), Some(CommitMetastate::Standard), vec![]).is_final()
    );
    assert!(LeaderStatus::Commit(block, None, vec![]).is_final());
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
        DecidedLeader::Commit(_, m, _) => *m,
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

/// A strong vote computed against one leader must not be counted as evidence
/// for a different canonical leader (e.g. across a leader-schedule swap).
/// Asserts the StarfishSpeed safety invariant: every ref in
/// `committed_transaction_refs` has locally-available transaction data.
///
/// Setup: round-3 leaders have asymmetric ack lists — A (auth 0) acks
/// nothing; B (auth 1) acks `r2[3]`. Local data state covers everything
/// except `r2[3]`. Round-4 voters cast strong votes computed against A.
/// The committer's swap table designates B as canonical leader of round 3.
#[tokio::test]
async fn optimistic_commits_ref_without_actual_data_backing() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context_with_flag(true);
    let auth_1 = AuthorityIndex::from(1u8);
    let auth_3 = AuthorityIndex::from(3u8);

    // Round 1.
    let mut r1_blocks = Vec::new();
    let mut r1_refs = Vec::new();
    for author in 0..4u8 {
        let block = v2_block(
            1,
            author,
            genesis_block_headers(&context)
                .iter()
                .map(|b| b.reference())
                .collect(),
            None,
            default_ts(1, author),
        );
        r1_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block.clone(), DataSource::Test);
        r1_blocks.push(block);
    }

    // Round 2.
    let mut r2_blocks = Vec::new();
    let mut r2_refs = Vec::new();
    for author in 0..4u8 {
        let block = v2_block(2, author, r1_refs.clone(), None, default_ts(2, author));
        r2_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block.clone(), DataSource::Test);
        r2_blocks.push(block);
    }

    // Round 3: A (auth 0) has no acks. B (auth 1) has acks=[round-2 block from
    // auth 3]. Other authorities have no acks.
    let mut r3_blocks = Vec::new();
    let mut r3_refs = Vec::new();
    for author in 0..4u8 {
        let acks = if author == 1 {
            vec![r2_refs[3]]
        } else {
            vec![]
        };
        let block = v2_block_with_acks(
            3,
            author,
            r2_refs.clone(),
            acks,
            None,
            default_ts(3, author),
        );
        r3_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block.clone(), DataSource::Test);
        r3_blocks.push(block);
    }

    // Mark transaction data available for everything except `r2[3]` and
    // round-3 blocks other than A.
    for block in &r1_blocks {
        add_transactions_for(&dag_state, block);
    }
    for (i, block) in r2_blocks.iter().enumerate() {
        if i != 3 {
            add_transactions_for(&dag_state, block);
        }
    }
    add_transactions_for(&dag_state, &r3_blocks[0]);

    let a_block = &r3_blocks[0];
    let b_block = &r3_blocks[1];
    {
        let dag = dag_state.read();
        assert!(Core::compute_strong_vote(&dag, a_block).is_strong_vote());
        assert!(!Core::compute_strong_vote(&dag, b_block).is_strong_vote());
    }

    let voter_strong_vote = {
        let dag = dag_state.read();
        Core::compute_strong_vote(&dag, a_block)
    };
    let r4_refs: Vec<BlockRef> = (0..4u8)
        .map(|author| {
            let block = v2_block(
                4,
                author,
                r3_refs.clone(),
                Some(voter_strong_vote),
                default_ts(4, author),
            );
            let r = block.reference();
            dag_state
                .write()
                .accept_block_header(block, DataSource::Test);
            r
        })
        .collect();

    // Round 5 certifiers.
    for author in 0..4u8 {
        let block = v2_block(5, author, r4_refs.clone(), None, default_ts(5, author));
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    // Swap table forcing canonical leader of round 3 to authority 1.
    let alt_table = LeaderSwapTable {
        good_nodes: vec![(
            auth_1,
            context.committee.authority(auth_1).hostname.clone(),
            context.committee.authority(auth_1).stake,
        )],
        bad_nodes: std::iter::once((
            auth_3,
            (
                context.committee.authority(auth_3).hostname.clone(),
                context.committee.authority(auth_3).stake,
            ),
        ))
        .collect(),
        ..LeaderSwapTable::default()
    };
    let alt_schedule = Arc::new(LeaderSchedule::new(context.clone(), alt_table));
    let committer = BaseCommitter::new(
        context.clone(),
        alt_schedule.clone(),
        dag_state.clone(),
        BaseCommitterOptions::default(),
    );

    let leader_b_slot = committer.elect_leader(3).expect("leader at round 3");
    assert_eq!(leader_b_slot.authority, auth_1);

    let (block_b, metastate, strong_voters) = match committer.try_direct_decide(leader_b_slot) {
        LeaderStatus::Commit(b, m, sv) => (b, m, sv),
        other => panic!("expected Commit at any metastate, got {other}"),
    };

    let mut linearizer = Linearizer::new(context, dag_state.clone(), alt_schedule);
    let pending = linearizer.get_pending_sub_dags(vec![(block_b, metastate, strong_voters)]);
    assert_eq!(pending.len(), 1);

    let p_committed = pending[0]
        .committed_transaction_refs
        .iter()
        .any(|r| match r {
            GenericTransactionRef::BlockRef(br) => br.round == 2 && br.author == auth_3,
            GenericTransactionRef::TransactionRef(tr) => tr.round == 2 && tr.author == auth_3,
        });
    let p_data_available = dag_state.read().is_data_available(&r2_refs[3]);

    assert!(
        !p_committed || p_data_available,
        "ref {:?} committed without locally-available transaction data",
        r2_refs[3]
    );
}
