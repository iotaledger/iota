// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use parking_lot::RwLock;
use rstest::rstest;
use starfish_config::AuthorityIndex;

use crate::{
    Round,
    block_header::{
        BlockHeaderAPI, Slot, TestBlockHeader, TestBlockHeaderVersion, VerifiedBlockHeader,
    },
    commit::{CommitMetastate, DecidedLeader, LeaderStatus},
    context::Context,
    dag_state::{DagState, DataSource},
    storage::mem_store::MemStore,
    test_dag::build_dag,
    test_dag_parser::parse_dag,
};

#[tokio::test]
async fn indirect_commit_with_missing_old_own_ancestor() {
    use crate::{
        authority_set::AuthoritySet,
        block_header::{
            BlockHeaderDigest, BlockRef, SignedBlockHeader, StrongVote, genesis_block_headers,
        },
        block_manager::BlockManager,
        block_verifier::{BlockVerifier, SignedBlockVerifier},
        commit::{CommitDigest, LeaderStatus, TrustedCommit},
        transaction::NoopTransactionVerifier,
    };

    let (mut context, keypairs) = Context::new_for_test(4);
    context.protocol_config.set_gc_depth_for_testing(5);
    let context = Arc::new(context);
    let dag_state = Arc::new(RwLock::new(DagState::new(
        context.clone(),
        Arc::new(MemStore::new()),
    )));
    let mut committer = super::Committer::new_for_test(context.clone(), dag_state.clone());
    let verifier = SignedBlockVerifier::new(context.clone(), Arc::new(NoopTransactionVerifier));
    let mut block_manager = BlockManager::new(context.clone(), dag_state.clone());
    let missing_ancestor =
        BlockRef::new(1, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN);
    let mut previous: Vec<_> = genesis_block_headers(&context)
        .iter()
        .map(|header| header.reference())
        .collect();

    for round in 1..=20 {
        let mut headers = Vec::new();
        for author in 0..4u8 {
            let mut ancestors = previous.clone();
            match (round, author) {
                // Keep leader 15 undecided directly: three votes, two certificates.
                (16, 0) | (17, 0 | 1) => {
                    ancestors.retain(|ancestor| ancestor.author.value() != 3);
                }
                (18, 2) => ancestors[2] = missing_ancestor,
                _ => {}
            }
            ancestors.sort_by_key(|ancestor| ancestor.author.value() != author as usize);
            let leader_authority = committer.elect_leader(round - 1).authority;
            let strong_vote = ancestors
                .iter()
                .any(|ancestor| ancestor.round == round - 1 && ancestor.author == leader_authority)
                .then(|| {
                    let mut missing = AuthoritySet::new();
                    if round > 1 {
                        missing.insert(leader_authority);
                    }
                    StrongVote {
                        leader_authority,
                        missing,
                    }
                });
            let header = TestBlockHeader::new(round, author)
                .set_version(TestBlockHeaderVersion::V2)
                .set_ancestors(ancestors)
                .set_strong_vote(strong_vote)
                .build();
            let signed = SignedBlockHeader::new(header, &keypairs[author as usize].1)
                .expect("header signing should succeed");
            verifier.verify(&signed).expect("header should be valid");
            let serialized = signed.serialize().expect("header should serialize");
            headers.push(VerifiedBlockHeader::new_verified(signed, serialized));
        }
        previous = headers.iter().map(|header| header.reference()).collect();
        let (accepted, missing) =
            block_manager.try_accept_block_headers(headers, DataSource::BlockBundleStream);
        assert_eq!(accepted.len(), 4);
        assert!(missing.is_empty());

        if round == 14 {
            let commit = TrustedCommit::new_for_test(
                1,
                CommitDigest::MIN,
                0,
                previous[2],
                previous.clone(),
                vec![],
            );
            dag_state.write().set_last_commit(commit);
            assert_eq!(dag_state.read().gc_round_for_last_commit(), 4);
        }
    }

    assert!(
        dag_state
            .read()
            .get_verified_block_header(&missing_ancestor)
            .is_none()
    );
    assert!(block_manager.blocks_to_fetch().is_empty());
    assert!(matches!(
        committer.try_direct_decide(Slot::new(15, 3)),
        LeaderStatus::Undecided(_)
    ));
    assert!(matches!(
        committer.try_direct_decide(Slot::new(18, 2)),
        LeaderStatus::Commit(_, _, _)
    ));

    let sequence = committer.try_decide(Slot::new(14, 2));
    assert!(matches!(
        sequence.first(),
        Some(DecidedLeader::Commit(header, _, _)) if header.round() == 15
    ));
}

/// Deciding again from the same last finalized leader returns the same leader,
/// and a finalized leader is not returned again.
#[tokio::test]
async fn idempotence() {
    let (context, dag_state, mut committer) = basic_test_setup();
    // Rounds 1 to 3 let the round-1 leader commit.
    build_dag(context, dag_state, None, 3);
    let round_1_leader = committer.elect_leader(1);

    for _ in 0..2 {
        let decided = committer.try_decide(Slot::new(0, 0));
        assert!(
            matches!(decided.as_slice(), [DecidedLeader::Commit(..)]),
            "got {decided:?}"
        );
        assert_eq!(decided[0].slot(), round_1_leader);
    }
    assert!(committer.try_decide(round_1_leader).is_empty());
}

/// Commit one by one each leader as the dag progresses in ideal conditions.
#[tokio::test]
async fn multiple_direct_commit() {
    let (context, dag_state, mut committer) = basic_test_setup();

    let mut last_finalized = Slot::new(0, 0);
    let mut ancestors = None;
    for leader_round in 1..=10 {
        // Build the dag up to the certifying round of each leader starting with
        // round 1.
        // note: rounds are zero-indexed.
        let certifying_round = leader_round + 2;

        ancestors = Some(build_dag(
            context.clone(),
            dag_state.clone(),
            ancestors,
            certifying_round,
        ));

        // There is a leader in every round, so we commit one leader per round.
        let sequence = committer.try_decide(last_finalized);
        tracing::info!("Commit sequence: {sequence:#?}");

        assert_eq!(sequence.len(), 1);
        if let DecidedLeader::Commit(ref block, _, _) = sequence[0] {
            assert_eq!(block.round(), leader_round);
            assert_eq!(
                block.author(),
                committer.elect_leader(leader_round).authority
            );
        } else {
            panic!("Expected a committed leader")
        }

        // Update the last decided leader so only one new leader is committed as
        // each new round is added.
        let last = sequence.into_iter().next_back().unwrap();
        last_finalized = Slot::new(last.round(), last.authority());
    }
}

/// Leader B1: three of the four round-2 blocks don't link it, so it is
/// skipped directly.
const DIRECT_SKIP_NO_LEADER_VOTES: &str = "DAG {
    Round 0 : { 4 },
    Round 1 : { * },
    Round 2 : {
        A -> [-B1],
        B -> [-B1],
        C -> [*],
        D -> [-B1],
    },
    Round 3 : { * },
}";

/// Leader D3 has no block, so it is skipped directly.
const DIRECT_SKIP_MISSING_LEADER_BLOCK: &str = "DAG {
    Round 0 : { 4 },
    Round 1 : { * },
    Round 2 : { * },
    Round 3 : {
        A -> [*],
        B -> [*],
        C -> [*],
    },
    Round 4 : { * },
    Round 5 : { * },
}";

/// Leader D3 has only f+1 certificates (A5, B5), so it is undecided directly
/// and committed through the leader of round 6. Blocks with restricted
/// ancestors keep their acknowledgments, so the missing links alone shape the
/// decisions.
const INDIRECT_COMMIT: &str = "DAG {
    Round 0 : { 4 },
    Round 1 : { * },
    Round 2 : { * },
    Round 3 : { * },
    Round 4 : {
        A -> ([-D3],[*]),
        B -> [*],
        C -> [*],
        D -> [*],
    },
    Round 5 : {
        A -> [*],
        B -> [*],
        C -> ([A4],[*]),
        D -> ([A4],[*]),
    },
    Round 6 : { * },
    Round 7 : { * },
    Round 8 : { * },
}";

/// Leader A4 has a quorum of votes, so it is not skipped directly, but the
/// leader of round 7 reaches none of its certificates, so it is skipped
/// indirectly. Blocks with restricted ancestors keep their acknowledgments, so
/// the missing links alone shape the decisions.
const INDIRECT_SKIP: &str = "DAG {
    Round 0 : { 4 },
    Round 1 : { * },
    Round 2 : { * },
    Round 3 : { * },
    Round 4 : { * },
    Round 5 : {
        A -> [*],
        B -> [*],
        C -> [*],
        D -> ([-A4],[*]),
    },
    Round 6 : {
        A -> [*],
        B -> ([-A5],[*]),
        C -> ([-B5],[*]),
        D -> [*],
    },
    Round 7 : {
        A -> [*],
        B -> [*],
        C -> [*],
        D -> ([B6],[*]),
    },
    Round 8 : { * },
    Round 9 : { * },
}";

/// Leaders B1 to A4 have neither enough certificates nor enough blames. B5 is
/// committed directly, but the undecided A4 lies between it and the leaders
/// below, so nothing is decided.
const UNDECIDED: &str = "DAG {
    Round 0 : { 4 },
    Round 1 : { * },
    Round 2 : { * },
    Round 3 : {
        A -> [A2],
        B -> [*],
        C -> [*],
        D -> [D2],
    },
    Round 4 : {
        A -> [-D3],
        B -> [*],
        C -> [*],
        D -> [*],
    },
    Round 5 : {
        A -> [*],
        B -> [*],
        C -> [C4],
        D -> [C4],
    },
    Round 6 : { * },
    Round 7 : { * },
}";

/// Decides the leaders of a parsed DAG and checks the whole decided sequence
/// as `(leader round, committed)`; a leader that is not committed is skipped.
#[rstest]
#[case::direct_skip_no_leader_votes(DIRECT_SKIP_NO_LEADER_VOTES, &[(1, false)])]
#[case::direct_skip_missing_leader_block(
    DIRECT_SKIP_MISSING_LEADER_BLOCK,
    &[(1, true), (2, true), (3, false)]
)]
#[case::indirect_commit(
    INDIRECT_COMMIT,
    &[(1, true), (2, true), (3, true), (4, true), (5, true), (6, true)]
)]
#[case::indirect_skip(
    INDIRECT_SKIP,
    &[(1, true), (2, true), (3, true), (4, false), (5, true), (6, true), (7, true)]
)]
#[case::undecided(UNDECIDED, &[])]
#[tokio::test]
async fn try_decide_parsed_dag(#[case] dag: &str, #[case] expected: &[(Round, bool)]) {
    let mut committer = committer_for(dag);
    let decided: Vec<(Round, bool)> = committer
        .try_decide(Slot::new(0, 0))
        .iter()
        .map(|leader| (leader.round(), matches!(leader, DecidedLeader::Commit(..))))
        .collect();
    assert_eq!(decided, expected);
}

// This test scenario has one authority that is acting in a byzantine manner. It
// will be sending multiple different blocks to different validators for a
// round. The commit rule should handle this and correctly commit the expected
// blocks. However when extra dag layers are added and the byzantine node is
// meant to be a leader, its block is skipped as there is not enough votes to
// directly decide it and not any certified links to indirectly commit it.
#[tokio::test]
async fn test_byzantine_validator() {
    let (context, dag_state, mut committer) = basic_test_setup();
    let version = TestBlockHeaderVersion::V2;

    // Add enough blocks to reach leader A12
    // note: rounds are zero-indexed.
    let leader_round_12 = 12;
    let references_leader_round_12 =
        build_dag(context.clone(), dag_state.clone(), None, leader_round_12);

    // Add blocks to reach voting round for leader A12
    let voting_round_12 = leader_round_12 + 1;
    // This includes a "good vote" from validator B which is acting as a byzantine
    // validator
    let good_references_voting_round_12 = build_dag(
        context.clone(),
        dag_state.clone(),
        Some(references_leader_round_12.clone()),
        voting_round_12,
    );

    // DagState Update:
    // - A12 got a good vote from 'B' above
    // - A12 will then get a bad vote from 'B' indirectly through the ancestors of the certifying
    //   round blocks (B, C, & D) of leader A12

    // Add block layer for certifying round of leader A12 with no votes for leader
    // A12 from a byzantine validator B that sent different blocks to all
    // validators.

    // Filter out leader A12
    let leader_12 = committer.elect_leader(leader_round_12).authority;
    let references_without_leader_12: Vec<_> = references_leader_round_12
        .into_iter()
        .filter(|x| x.author != leader_12)
        .collect();

    // Three B13 blocks that differ only in timestamp; none links A12.
    let byzantine_b13: Vec<_> = (0..3)
        .map(|timestamp_ms| {
            let block = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(13, 1)
                    .set_version(version)
                    .set_ancestors(references_without_leader_12.clone())
                    .set_timestamp_ms(timestamp_ms)
                    .build(),
            );
            dag_state
                .write()
                .accept_block_header(block.clone(), DataSource::Test);
            block.reference()
        })
        .collect();

    // A14 links the good round-13 blocks. B14, C14 and D14 each replace the good
    // B13 with a different byzantine one, which is a non-vote for A12, but they
    // still link the good votes of A, C and D, so A12 is not skipped.
    let good_without_b13: Vec<_> = good_references_voting_round_12
        .iter()
        .filter(|r| r.author != AuthorityIndex::new_for_test(1))
        .copied()
        .collect();
    let ancestors_14 = std::iter::once(good_references_voting_round_12).chain(
        byzantine_b13
            .iter()
            .map(|b13| good_without_b13.iter().copied().chain([*b13]).collect()),
    );
    let mut references_round_14 = vec![];
    for (author, ancestors) in (0u8..).zip(ancestors_14) {
        let block = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(14, author)
                .set_version(version)
                .set_ancestors(ancestors)
                .build(),
        );
        references_round_14.push(block.reference());
        dag_state
            .write()
            .accept_block_header(block, DataSource::Test);
    }

    // Expect a successful direct commit of A12 and leaders at rounds 1 ~ 11 as
    // every round has a leader.
    let sequence = committer.try_decide(Slot::new(0, 0));
    assert_eq!(sequence.len(), 12);
    assert!(
        matches!(sequence[11], DecidedLeader::Commit(..)),
        "got {:?}",
        sequence[11]
    );
    assert_eq!(sequence[11].slot(), committer.elect_leader(leader_round_12));

    // Now build an additional dag layer on top of the existing dag so a commit
    // decision can be made about leader B13 which is the byzantine validator.
    let references_round_15 = build_dag(
        context.clone(),
        dag_state.clone(),
        Some(references_round_14),
        15,
    );

    // Ensure B13 is marked as undecided as there is <2f+1 blame and <2f+1 certs
    let last_finalized = sequence[11].slot();
    assert!(committer.try_decide(last_finalized).is_empty());

    // Now build an additional 3 dag layers on top of the existing dag so a commit
    // decision can be made about leader A16 and then an indirect decision can be
    // made about B13
    build_dag(context, dag_state, Some(references_round_15), 18);
    let sequence = committer.try_decide(last_finalized);
    assert_eq!(sequence.len(), 4);

    // Ensure we skip B13 as there is no way to have a certified link to any one
    // of the multiple blocks at slot B13.
    assert_eq!(sequence[0], DecidedLeader::Skip(committer.elect_leader(13)));
}

// TODO: Add byzantine variant of tests for indirect/direct
// commit/skip/undecided decisions

#[tokio::test]
async fn direct_commit() {
    // D3 is the leader of round 3, round 4 votes and round 5 certifies.
    //
    // The round 4 votes acknowledge D3 alone, but they already acknowledged the
    // data D3 acknowledges at round 3, so they are strong votes and the round 5
    // certificates are strong.
    let dag = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : { * },
        Round 4 : {
            A -> [D3],
            B -> [D3],
            C -> [D3],
            D -> [],
        },
        Round 5 : {
            A -> [A4, B4, C4],
            B -> [A4, B4, C4],
            C -> [A4, B4, C4],
            D -> [],
        },
    }";
    let mut committer = committer_for(dag);

    let status = committer.try_direct_decide(committer.elect_leader(3));
    assert!(
        matches!(
            status,
            LeaderStatus::Commit(_, CommitMetastate::Optimistic, _)
        ),
        "got {status}"
    );
}

/// A leader without a block and without a blame quorum stays undecided, and so
/// does a leader whose certifying round has no blocks. Neither has an anchor to
/// decide it indirectly.
#[tokio::test]
async fn direct_undecided_while_blocks_are_missing() {
    // D3 has no block, and only A4 and B4 blame it, which is not a quorum. A4
    // has no blocks at its certifying round 6.
    let dag = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : {
            A -> [*],
            B -> [*],
            C -> [*],
        },
        Round 4 : {
            A -> [*],
            B -> [*],
        },
    }";
    let mut committer = committer_for(dag);

    for round in [3, 4] {
        let leader = committer.elect_leader(round);
        let status = committer.try_direct_decide(leader);
        assert_eq!(status, LeaderStatus::Undecided(leader));
        assert_eq!(
            committer.try_indirect_decide(status, [].iter()),
            LeaderStatus::Undecided(leader)
        );
    }
}

#[tokio::test]
async fn indirect_commit() {
    // D3 is the leader of round 3 and C6 the leader of round 6.
    //
    // Blocks with no ancestors acknowledge nothing either, so only A holds A5,
    // the block C6 acknowledges. B7 and D7 therefore vote for C6 while blaming
    // it, and C6 commits with neither a strong-vote nor a strong-blame quorum.
    let dag = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : { * },
        Round 4 : {
            A -> [D3],
            B -> [D3],
            C -> [D3],
            D -> [],
        },
        Round 5 : {
            A -> [A4, B4, C4],
            B -> [],
            C -> [],
            D -> [],
        },
        Round 6 : {
            A -> [],
            B -> [],
            C -> [A5],
            D -> [],
        },
        Round 7 : {
            A -> [C6],
            B -> [C6],
            C -> [],
            D -> [C6],
        },
        Round 8 : {
            A -> [A7, B7, D7],
            B -> [A7, B7, D7],
            C -> [],
            D -> [A7, B7, D7],
        },
    }";
    let mut committer = committer_for(dag);
    let leader_3 = committer.elect_leader(3);
    let leader_6 = committer.elect_leader(6);

    assert_eq!(
        committer.try_direct_decide(leader_3),
        LeaderStatus::Undecided(leader_3)
    );
    let leader_6_status = committer.try_direct_decide(leader_6);
    assert!(
        matches!(
            leader_6_status,
            LeaderStatus::Commit(_, CommitMetastate::Pending, _)
        ),
        "got {leader_6_status}"
    );

    // A5 is a strong certificate on the anchor's path, built from the strong
    // votes of A4, B4 and C4.
    match committer.try_indirect_decide(LeaderStatus::Undecided(leader_3), [leader_6_status].iter())
    {
        LeaderStatus::Commit(_, CommitMetastate::Optimistic, strong_voters) => assert_eq!(
            strong_voters.into_iter().collect::<BTreeSet<_>>(),
            (0..3u8).map(AuthorityIndex::from).collect()
        ),
        status => panic!("expected an Optimistic commit, got {status}"),
    }
}

/// Commit the first leader, indirectly skip the 2nd, and commit the 3rd leader.
#[tokio::test]
async fn indirect_skip() {
    // Every block links all blocks of the previous round, except that only A7
    // and B7 link C6, the leader of round 6.
    let dag = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : { * },
        Round 4 : { * },
        Round 5 : { * },
        Round 6 : { * },
        Round 7 : {
            A -> [*],
            B -> [*],
            C -> [-C6],
            D -> [-C6],
        },
        Round 8 : { * },
        Round 9 : { * },
        Round 10 : { * },
        Round 11 : { * },
    }";
    let mut committer = committer_for(dag);
    let [leader_3, leader_6, leader_9] = [3, 6, 9].map(|round| committer.elect_leader(round));

    let leader_9_status = committer.try_direct_decide(leader_9);
    for status in [
        committer.try_direct_decide(leader_3),
        leader_9_status.clone(),
    ] {
        assert!(
            matches!(
                status,
                LeaderStatus::Commit(_, CommitMetastate::Optimistic, _)
            ),
            "got {status}"
        );
    }
    assert_eq!(
        committer.try_direct_decide(leader_6),
        LeaderStatus::Undecided(leader_6)
    );
    assert_eq!(
        committer.try_indirect_decide(LeaderStatus::Undecided(leader_6), [leader_9_status].iter()),
        LeaderStatus::Skip(leader_6)
    );
}

fn basic_test_setup() -> (Arc<Context>, Arc<RwLock<DagState>>, super::Committer) {
    telemetry_subscribers::init_for_testing();
    // Committee of 4 with even stake
    let context = Arc::new(Context::new_for_test(4).0);
    let dag_state = Arc::new(RwLock::new(DagState::new(
        context.clone(),
        Arc::new(MemStore::new()),
    )));
    let committer = super::Committer::new_for_test(context.clone(), dag_state.clone());

    (context, dag_state, committer)
}

/// A committer over the blocks of the parsed `dag`.
fn committer_for(dag: &str) -> super::Committer {
    telemetry_subscribers::init_for_testing();
    let dag_builder = parse_dag(dag).expect("a DAG should be valid");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new()),
    )));
    dag_builder.persist_all_blocks(dag_state.clone());
    super::Committer::new_for_test(dag_builder.context, dag_state)
}
