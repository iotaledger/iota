// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use parking_lot::RwLock;
use rstest::rstest;
use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    block_header::{
        BlockHeader, BlockHeaderV2, BlockRef, BlockTimestampMs, GENESIS_ROUND, Round, Slot,
        StrongVote, TransactionsCommitment, VerifiedBlockHeader, genesis_block_headers,
    },
    commit::{CommitMetastate, DecidedLeader, LeaderStatus, WAVE_LENGTH},
    committer::Committer,
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    storage::mem_store::MemStore,
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
    committer: &mut Committer,
    slot: Slot,
    expected: CommitMetastate,
) {
    match committer.try_direct_decide(slot) {
        LeaderStatus::Commit(_, metastate, _) => assert_eq!(metastate, expected),
        status => panic!("expected Commit, got {status}"),
    }
}

/// Fully linked V2 layers without strong votes from `start` (or genesis)
/// through `stop`.
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

/// Test context for a committee of `committee_size` plus an empty DAG state.
fn test_context(committee_size: usize) -> (Arc<Context>, Arc<RwLock<DagState>>) {
    let ctx = Arc::new(Context::new_for_test(committee_size).0);
    let dag_state = Arc::new(RwLock::new(DagState::new(
        ctx.clone(),
        Arc::new(MemStore::new()),
    )));
    (ctx, dag_state)
}

/// Populate `dag_state` through round 4 (voters link to all round-3 blocks).
/// Returns the leader slot and voter refs indexed by author.
fn build_through_voting_round(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &Committer,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> (Slot, Vec<BlockRef>) {
    let leader_round = 3;
    let voting_round = leader_round + 1;
    let round_3_refs = build_v2_layers(context, dag_state, None, leader_round);
    let leader_slot = committer.elect_leader(leader_round);

    let mut voter_refs = Vec::with_capacity(4);
    for (author, missing) in voter_strong_votes.into_iter().enumerate() {
        let author = author as u8;
        let block = v2_block(
            voting_round,
            author,
            round_3_refs.clone(),
            pin_strong_vote(leader_slot.authority, missing),
            default_ts(voting_round, author),
        );
        voter_refs.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    (leader_slot, voter_refs)
}

/// Build one round-5 certifier from the specified round-4 voter refs.
fn certifier(author: u8, voter_refs: Vec<BlockRef>) -> VerifiedBlockHeader {
    let round = 5;
    v2_block(round, author, voter_refs, None, default_ts(round, author))
}

/// Populate `dag_state` through round 5; round-4 voters carry strong-vote
/// payloads pinned to the canonical round-3 leader, and every round-5
/// certifier links all of them. Returns the leader slot at round 3.
fn build_metastate_dag(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &Committer,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> Slot {
    let (leader_slot, voters) =
        build_through_voting_round(context, dag_state, committer, voter_strong_votes);
    for author in 0..context.committee.size() as u8 {
        dag_state
            .write()
            .accept_block_header(certifier(author, voters.clone()), DataSource::Test);
    }
    leader_slot
}

/// Same as `build_through_voting_round`, plus a round-5 certifier per author
/// that links a different three of the four voters. With the votes
/// `[vote, vote, vote, None]`, only `c0` links three strong votes, so the
/// direct rule classifies the round-3 leader as Pending. Returns
/// `(leader_slot, [c0, c1, c2, c3])`.
fn build_through_certifying_round(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &Committer,
    voter_strong_votes: [Option<AuthoritySet>; 4],
) -> (Slot, [BlockRef; 4]) {
    let (leader_slot, voters) =
        build_through_voting_round(context, dag_state, committer, voter_strong_votes);
    let c0 = certifier(0, vec![voters[0], voters[1], voters[2]]);
    let c1 = certifier(1, vec![voters[0], voters[1], voters[3]]);
    let c2 = certifier(2, vec![voters[0], voters[2], voters[3]]);
    let c3 = certifier(3, vec![voters[1], voters[2], voters[3]]);
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

/// The direct rule's metastate follows the strong votes and blames of the
/// voting round: a StrongQC quorum gives Optimistic with the strong voters, a
/// strong-blame quorum gives Standard, and neither gives Pending.
#[rstest]
#[case::strong_votes(
    [strong_vote(), strong_vote(), strong_vote(), strong_blame()],
    CommitMetastate::Optimistic,
    vec![0, 1, 2]
)]
#[case::strong_blames(
    [strong_vote(), strong_blame(), strong_blame(), strong_blame()],
    CommitMetastate::Standard,
    vec![]
)]
#[case::neither(
    [strong_vote(), strong_vote(), strong_blame(), strong_blame()],
    CommitMetastate::Pending,
    vec![]
)]
#[tokio::test]
async fn direct_decide_metastate(
    #[case] votes: [Option<AuthoritySet>; 4],
    #[case] expected: CommitMetastate,
    #[case] expected_strong_voters: Vec<u8>,
) {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());
    let leader = build_metastate_dag(&context, &dag_state, &committer, votes);

    let LeaderStatus::Commit(_, metastate, strong_voters) = committer.try_direct_decide(leader)
    else {
        panic!("expected Commit");
    };
    assert_eq!(metastate, expected);
    assert_eq!(
        strong_voters.into_iter().collect::<BTreeSet<_>>(),
        expected_strong_voters
            .into_iter()
            .map(AuthorityIndex::from)
            .collect()
    );
}

/// Which equivocating leader a voter supports.
#[derive(Clone, Copy)]
enum LeaderChoice {
    A,
    B,
}

/// Same as `build_metastate_dag`, but the leader author produces two
/// equivocating blocks `L_A`/`L_B` at round 3 and voters split per
/// `voter_config`. Returns `(leader_slot, L_A)`.
fn build_equivocating_metastate_dag(
    context: &Context,
    dag_state: &Arc<RwLock<DagState>>,
    committer: &Committer,
    voter_config: [(LeaderChoice, Option<AuthoritySet>); 4],
) -> (Slot, BlockRef) {
    let leader_round = 3;
    let voting_round = leader_round + 1;
    let certifying_round = 5;

    let prev_refs = build_v2_layers(context, dag_state, None, leader_round - 1);

    let leader_slot = committer.elect_leader(leader_round);
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

    (leader_slot, leader_a_ref)
}

/// Two voters for `L_A` and one for the equivocating `L_B` carry `vote`, and
/// one more votes for `L_A` without one. `L_A` has 3 votes and commits, but
/// only 2 strong votes or blames that are votes for it, below the 2f+1 = 3
/// threshold, so it is Pending. Counting the `L_B` voter for `L_A` would give
/// Optimistic or Standard.
#[rstest]
#[case::strong_vote(strong_vote())]
#[case::strong_blame(strong_blame())]
#[tokio::test]
async fn equivocating_leader_vote_is_not_counted(#[case] vote: Option<AuthoritySet>) {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());
    let (leader_slot, leader_a_ref) = build_equivocating_metastate_dag(
        &context,
        &dag_state,
        &committer,
        [
            (LeaderChoice::A, vote),
            (LeaderChoice::A, vote),
            (LeaderChoice::A, None),
            (LeaderChoice::B, vote),
        ],
    );

    match committer.try_direct_decide(leader_slot) {
        LeaderStatus::Commit(block, metastate, _) => {
            assert_eq!(block.reference(), leader_a_ref);
            assert_eq!(metastate, CommitMetastate::Pending);
        }
        status => panic!("expected Commit, got {status}"),
    }
}

#[tokio::test]
async fn indirect_metastate_standard_when_strong_qc_outside_anchor_path() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());

    let (leader_slot, [_c0, c1, c2, c3]) = build_through_certifying_round(
        &context,
        &dag_state,
        &committer,
        [strong_vote(), strong_vote(), strong_vote(), None],
    );

    // Anchor excludes the StrongQC c0. Indirect must still resolve from the
    // restricted r+2 path and pick Standard, not Optimistic.
    let anchor_round = 6;
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

    let anchor_status = LeaderStatus::Commit(anchor, CommitMetastate::Standard, vec![]);
    let current = LeaderStatus::Undecided(leader_slot);
    match committer.try_indirect_decide(current, std::iter::once(&anchor_status)) {
        LeaderStatus::Commit(block, metastate, _) => {
            assert_eq!(block.reference().round, leader_slot.round);
            assert_eq!(block.reference().author, leader_slot.authority);
            assert_eq!(metastate, CommitMetastate::Standard);
        }
        status => panic!("expected Commit, got {status}"),
    }
}

/// Round-3 leader's metastate in `decided`. Panics if not a Commit.
fn round_3_metastate(committer: &Committer, decided: &[DecidedLeader]) -> CommitMetastate {
    let slot = committer.elect_leader(3);
    let decided = decided
        .iter()
        .find(|d| d.slot() == slot)
        .expect("round-3 leader should be decided");
    match decided {
        DecidedLeader::Commit(_, m, _) => *m,
        other => panic!("expected round-3 Commit, got {other:?}"),
    }
}

/// The round-3 leader is Pending by the direct rule, and `try_decide` resolves
/// it through the round-6 anchor: Optimistic when the StrongQC `c0` is on the
/// anchor's path, Standard when there are no strong votes at all.
#[rstest]
#[case::strong_qc_on_anchor_path(
    [strong_vote(), strong_vote(), strong_vote(), None],
    CommitMetastate::Optimistic
)]
#[case::no_strong_votes([None; 4], CommitMetastate::Standard)]
#[tokio::test]
async fn pending_leader_resolves_through_anchor(
    #[case] votes: [Option<AuthoritySet>; 4],
    #[case] expected: CommitMetastate,
) {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());

    let (round_3_slot, round_5_refs) =
        build_through_certifying_round(&context, &dag_state, &committer, votes);
    assert_direct_commit_metastate(&mut committer, round_3_slot, CommitMetastate::Pending);

    build_v2_layers(&context, &dag_state, Some(round_5_refs.to_vec()), 8);

    let decided = committer.try_decide(Slot::new(GENESIS_ROUND, 0u8));
    assert_eq!(round_3_metastate(&committer, &decided), expected);
}

/// Strong votes and blames pinned to a leader that a schedule swap replaces
/// don't count for the leader that replaces it, so it stays Pending.
#[rstest]
#[case::strong_vote(strong_vote())]
#[case::strong_blame(strong_blame())]
#[tokio::test]
async fn strong_votes_pinned_to_another_leader_are_not_counted(#[case] vote: Option<AuthoritySet>) {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let committer = Committer::new_for_test(context.clone(), dag_state.clone());
    let pinned = build_metastate_dag(&context, &dag_state, &committer, [vote; 4]).authority;

    let swapped_in = AuthorityIndex::from(1u8);
    let authority = |index| context.committee.authority(index);
    let table = LeaderSwapTable {
        good_nodes: vec![(
            swapped_in,
            authority(swapped_in).hostname.clone(),
            authority(swapped_in).stake,
        )],
        bad_nodes: [(
            pinned,
            (authority(pinned).hostname.clone(), authority(pinned).stake),
        )]
        .into_iter()
        .collect(),
        ..LeaderSwapTable::default()
    };
    let schedule = Arc::new(LeaderSchedule::new(context.clone(), table));
    let mut committer = Committer::new(context.clone(), schedule, dag_state);

    let leader = committer.elect_leader(3);
    assert_eq!(leader.authority, swapped_in);
    assert_direct_commit_metastate(&mut committer, leader, CommitMetastate::Pending);
}

#[tokio::test]
async fn decision_cache_answers_repeated_checks_until_evicted() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());
    let metrics = &context.metrics.node_metrics;

    let (round_3_slot, round_5_refs) = build_through_certifying_round(
        &context,
        &dag_state,
        &committer,
        [strong_vote(), strong_vote(), strong_vote(), None],
    );
    let first = committer.try_direct_decide(round_3_slot);

    // Blocks accepted later leave the entries about round 5 valid.
    build_v2_layers(&context, &dag_state, Some(round_5_refs.to_vec()), 8);
    let misses = metrics.decision_cache_misses_total.get();
    let hits = metrics.decision_cache_hits_total.get();
    assert_eq!(committer.try_direct_decide(round_3_slot), first);
    assert_eq!(metrics.decision_cache_misses_total.get(), misses);
    assert!(metrics.decision_cache_hits_total.get() > hits);

    // Evicting the slot's certifying round forces the same answers to be
    // recomputed.
    committer.evict_cache_below(round_3_slot.round + WAVE_LENGTH);
    assert_eq!(committer.try_direct_decide(round_3_slot), first);
    assert!(metrics.decision_cache_misses_total.get() > misses);
}

#[tokio::test]
async fn decision_cache_classifies_a_certifier_accepted_after_a_check_once() {
    telemetry_subscribers::init_for_testing();
    let (context, dag_state) = test_context(4);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());
    let metrics = &context.metrics.node_metrics;
    let (leader_slot, voters) =
        build_through_voting_round(&context, &dag_state, &committer, [strong_vote(); 4]);

    // Quorum stake at r+2 but only two certificates: the slot stays open and
    // the three classifications are cached.
    let c0 = certifier(0, voters.clone());
    let c1 = certifier(1, voters.clone());
    let c2 = certifier(2, vec![voters[0], voters[1]]);
    for c in [c0, c1, c2] {
        dag_state.write().accept_block_header(c, DataSource::Test);
    }
    assert!(matches!(
        committer.try_direct_decide(leader_slot),
        LeaderStatus::Undecided(_)
    ));
    assert_eq!(metrics.decision_cache_misses_total.get(), 3);

    // The third certificate arrives: it is the only new classification.
    let c3 = certifier(3, voters);
    dag_state.write().accept_block_header(c3, DataSource::Test);
    assert_direct_commit_metastate(&mut committer, leader_slot, CommitMetastate::Optimistic);
    assert_eq!(metrics.decision_cache_misses_total.get(), 4);
    assert_eq!(metrics.decision_cache_hits_total.get(), 3);
}

/// Replays the load shape of a 62-validator committee: one `try_decide` per
/// accepted block, and every fourth leader lacks a StrongQC quorum at r+2, so
/// it stays Pending until the anchor three rounds later resolves it.
#[tokio::test]
async fn decision_cache_serves_repeated_checks_of_pending_leaders() {
    const COMMITTEE: usize = 62;
    const ROUNDS: Round = 40;

    let (context, dag_state) = test_context(COMMITTEE);
    let mut committer = Committer::new_for_test(context.clone(), dag_state.clone());

    let mut ancestors: Vec<BlockRef> = genesis_block_headers(&context)
        .iter()
        .map(|b| b.reference())
        .collect();
    let mut last_finalized = Slot::new(GENESIS_ROUND, 0u8);
    let (mut optimistic, mut standard, mut other) = (0usize, 0usize, 0usize);
    for round in 1..=ROUNDS {
        let leader_authority = committer.elect_leader(round - 1).authority;
        let pending_leader = (round - 1) % 4 == 0;
        let mut refs = Vec::with_capacity(COMMITTEE);
        for author in 0..COMMITTEE as u8 {
            let strong_vote = (!pending_leader || author % 3 == 0).then(|| StrongVote {
                leader_authority,
                missing: AuthoritySet::new(),
            });
            let block = v2_block(
                round,
                author,
                ancestors.clone(),
                strong_vote,
                default_ts(round, author),
            );
            refs.push(block.reference());
            dag_state
                .write()
                .accept_block_header(block, DataSource::Test);
            loop {
                let decided = committer.try_decide(last_finalized);
                let Some(last) = decided.last() else { break };
                last_finalized = last.slot();
                for leader in &decided {
                    match leader {
                        DecidedLeader::Commit(_, CommitMetastate::Optimistic, _) => optimistic += 1,
                        DecidedLeader::Commit(_, CommitMetastate::Standard, _) => standard += 1,
                        _ => other += 1,
                    }
                }
            }
        }
        ancestors = refs;
    }

    assert!(last_finalized.round >= ROUNDS - 6);
    assert!(
        standard >= ROUNDS as usize / 4 - 3,
        "pending leaders resolve as Standard"
    );
    assert!(optimistic > 2 * standard);
    assert_eq!(other, 0);
    let metrics = &context.metrics.node_metrics;
    let (hits, misses) = (
        metrics.decision_cache_hits_total.get(),
        metrics.decision_cache_misses_total.get(),
    );
    assert!(hits > 10 * misses, "hits {hits}, misses {misses}");
    // `try_decide` drops the classifications it can no longer be asked about.
    assert!(
        committer
            .certificates
            .keys()
            .all(|round| *round >= last_finalized.round + WAVE_LENGTH)
    );
}
