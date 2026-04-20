// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    Round,
    block_header::{BlockHeaderAPI, Slot, TestBlockHeader, VerifiedBlockHeader},
    commit::{DecidedLeader, WaveNumber},
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    storage::mem_store::MemStore,
    test_dag::build_dag,
    test_dag_builder::DagBuilder,
    test_dag_parser::parse_dag,
    universal_committer::universal_committer_builder::UniversalCommitterBuilder,
};
// TODO: merge all tests here with pipelined_committer_tests
// TODO: use one same mechanism for constructing a DAG

/// Directly commit 5 leader blocks.
#[tokio::test]
async fn direct_commit() {
    let mut test_setup = basic_dag_builder_test_setup();

    // Build fully connected dag with empty blocks adding up to round 7
    // such that we can commit all leader block up to round 5
    // note: waves & rounds are zero-indexed.
    let first_non_genesis_leader_round = 1;
    let last_round_of_dag = 7;
    test_setup
        .dag_builder
        .layers(1..=last_round_of_dag)
        .build()
        .persist_layers(test_setup.dag_state);

    test_setup.dag_builder.print();

    // Genesis cert will not be included in commit sequence, marking it as last
    // decided
    let last_decided = Slot::new(0, 0);

    // The universal committer should mark the potential leaders in leader round 6
    // as undecided because there is no way to get enough certificates for
    // leaders of leader round 6 without completing wave (6-7-8).
    let sequence = test_setup.committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");
    // The decided leaders should be all from round 1 to round 5
    assert_eq!(sequence.len(), 5);
    if let DecidedLeader::Commit(ref block) = sequence[0] {
        assert_eq!(
            block.author(),
            test_setup
                .committer
                .get_leaders(first_non_genesis_leader_round)[0]
        )
    } else {
        panic!("Expected a committed leader")
    };
}

/// Ensure idempotent replies.
#[tokio::test]
async fn idempotence() {
    let (context, dag_state, committer) = basic_test_setup();

    // note: waves & rounds are zero-indexed.
    let first_non_genesis_leader_round = 1;
    let certifying_round_for_first_leader = 3;
    let references_certifying_round_for_round_1 = build_dag(
        context.clone(),
        dag_state.clone(),
        None,
        certifying_round_for_first_leader,
    );

    // Commit one leader.
    let last_decided = Slot::new(0, 0);
    let first_sequence = committer.try_decide(last_decided);
    assert_eq!(first_sequence.len(), 1);

    if let DecidedLeader::Commit(ref block) = first_sequence[0] {
        assert_eq!(first_sequence[0].round(), first_non_genesis_leader_round);
        assert_eq!(
            block.author(),
            committer.get_leaders(first_non_genesis_leader_round)[0]
        )
    } else {
        panic!("Expected a committed leader")
    };

    // Ensure that if try_commit is called again with the same last decided leader
    // input the commit decision will be the same.
    let first_sequence = committer.try_decide(last_decided);

    assert_eq!(first_sequence.len(), 1);
    if let DecidedLeader::Commit(ref block) = first_sequence[0] {
        assert_eq!(first_sequence[0].round(), first_non_genesis_leader_round);
        assert_eq!(
            block.author(),
            committer.get_leaders(first_non_genesis_leader_round)[0]
        )
    } else {
        panic!("Expected a committed leader")
    };

    // Add more rounds so we have something to commit after the leader of wave 1
    let certifying_round_for_round_5 = 7;
    build_dag(
        context,
        dag_state,
        Some(references_certifying_round_for_round_1),
        certifying_round_for_round_5,
    );

    // Ensure we don't commit the same leader of round 1 again if we mark it as the
    // last decided.
    let leader_status_first_leader = first_sequence.last().unwrap();
    let last_decided = Slot::new(
        leader_status_first_leader.round(),
        leader_status_first_leader.authority(),
    );
    let round_5 = 5;
    let second_sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {second_sequence:#?}");

    // Expect that all leaders between round 2 and round 5 are committed.
    // The last one is a block of leader from round 5
    assert_eq!(second_sequence.len(), 4);
    if let DecidedLeader::Commit(ref block) = second_sequence[3] {
        assert_eq!(block.round(), round_5);
        assert_eq!(block.author(), committer.get_leaders(round_5)[0]);
    } else {
        panic!("Expected a committed leader")
    };
}

/// Commit one by one each leader as the dag progresses in ideal conditions.
#[tokio::test]
async fn multiple_direct_commit() {
    let (context, dag_state, committer) = basic_test_setup();

    let mut ancestors = None;
    let mut last_decided = Slot::new(0, 0);
    for n in 1..=10 {
        // Build the DAG up to the certifying round for leader blocks of authority 1,
        // i.e. full DAG is built with chunks of 3 rounds
        // note: waves & rounds are zero-indexed.
        let certifying_round = committer.committers[0].certifying_round(n);
        ancestors = Some(build_dag(
            context.clone(),
            dag_state.clone(),
            ancestors,
            certifying_round,
        ));

        // After every 3 rounds, try commit all leaders in between
        let leader_round = committer.committers[0].leader_round(n);
        let sequence = committer.try_decide(last_decided);
        tracing::info!("Commit sequence: {sequence:#?}");
        assert_eq!(sequence.len(), 3);
        if let DecidedLeader::Commit(ref block) = sequence[2] {
            assert_eq!(block.round(), leader_round);
            assert_eq!(block.author(), committer.get_leaders(leader_round)[0]);
        } else {
            panic!("Expected a committed leader")
        }

        // Update the last decided leader so only one new leader is committed as
        // each new wave is completed.
        let leader_status = sequence.last().unwrap();
        last_decided = Slot::new(leader_status.round(), leader_status.authority());
    }
}

/// Commit leaders from 10 waves in a row (calling the committer after adding
/// them).
#[tokio::test]
async fn direct_commit_late_call() {
    let (context, dag_state, committer) = basic_test_setup();

    // note: waves & rounds are zero-indexed.
    let num_waves = 11;
    let certifying_round_wave_10 = committer.committers[0].certifying_round(10);
    build_dag(context, dag_state, None, certifying_round_wave_10);

    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");

    // With 11 full non-intersecting waves completed, excluding genesis in wave 0 as
    // its leader round, ensure we have 30 leaders are committed.
    assert_eq!(sequence.len(), 3 * (num_waves - 1_usize));
    for (i, leader_block) in sequence.iter().enumerate() {
        let leader_round = committer.committers[(i + 1) % 3].leader_round((i as u32 + 1) / 3);
        if let DecidedLeader::Commit(ref block) = leader_block {
            assert_eq!(block.round(), leader_round);
            assert_eq!(block.author(), committer.get_leaders(leader_round)[0]);
        } else {
            panic!("Expected a committed leader")
        };
    }
}

/// Do not commit anything if we are still in the first wave.
#[tokio::test]
async fn no_genesis_commit() {
    let (context, dag_state, committer) = basic_test_setup();

    // note: waves & rounds are zero-indexed.
    let certifying_round = 3;
    let mut ancestors = None;
    for r in 0..certifying_round {
        ancestors = Some(build_dag(context.clone(), dag_state.clone(), ancestors, r));

        let last_committed = Slot::new(0, 0);
        let sequence = committer.try_decide(last_committed);
        tracing::info!("Commit sequence: {sequence:#?}");
        assert!(sequence.is_empty());
    }
}

/// We directly skip the leader if there are enough non-votes (blames).
#[tokio::test]
async fn direct_skip_no_leader_votes() {
    telemetry_subscribers::init_for_testing();
    // Dag Notes:
    // Pipeline is enabled
    // Leader of Round 1, i.e. B1, should be skipped due to lack of votes
    let dag_str = "DAG {
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

    let first_leader = AuthorityIndex::new_for_test(1);
    let first_round = 1 as Round;

    let dag_builder = parse_dag(dag_str).expect("Invalid dag");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new(dag_builder.context.clone())),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        dag_builder.context.clone(),
        LeaderSwapTable::default(),
    ));

    dag_builder.print();
    dag_builder.persist_all_blocks(dag_state.clone());

    // Create committer with pipelining and 1 leader per round
    let committer =
        UniversalCommitterBuilder::new(dag_builder.context, leader_schedule, dag_state).build();
    // note: without pipelining or multi-leader enabled there should only be one
    // committer.
    assert_eq!(committer.committers.len(), 3);

    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    // Only leader for slot B1 should be decided, specifically, skipped
    assert_eq!(sequence.len(), 1);
    if let DecidedLeader::Skip(leader) = sequence[0] {
        assert_eq!(leader.authority, first_leader);
        assert_eq!(leader.round, first_round);
    } else {
        panic!("Expected to directly skip the leader");
    }
}

/// We directly skip the leader if it is missing.
#[tokio::test]
async fn direct_skip_missing_leader_block() {
    let mut test_setup = basic_dag_builder_test_setup();

    // Add enough blocks to reach the certifying round of genesis leader
    // note: waves & rounds are zero-indexed.
    let certifying_round_genesis = 2;
    test_setup
        .dag_builder
        .layers(1..=certifying_round_genesis)
        .build();

    // Create a leader round in the dag without the leader block.
    let leader_round_3 = 3;
    test_setup
        .dag_builder
        .layer(leader_round_3)
        .no_leader_block(vec![])
        .build();

    // Add enough blocks to reach the certifying round of leader of round 3.
    let voting_round_for_leader_round_3 = 4;
    let certifying_round_for_leader_round_3 = 5;
    test_setup
        .dag_builder
        .layers(voting_round_for_leader_round_3..=certifying_round_for_leader_round_3)
        .build();

    test_setup.dag_builder.print();
    test_setup
        .dag_builder
        .persist_all_blocks(test_setup.dag_state.clone());

    // Ensure that the leader of round 3 is skipped because the leader is missing.
    let last_committed = Slot::new(0, 0);
    let sequence = test_setup.committer.try_decide(last_committed);
    tracing::info!("Commit sequence: {sequence:#?}");

    assert_eq!(sequence.len(), 3);
    if let DecidedLeader::Skip(leader) = sequence[2] {
        assert_eq!(
            leader.authority,
            AuthorityIndex::new_for_test(leader_round_3 as u8),
        );
        assert_eq!(leader.round, leader_round_3);
    } else {
        panic!("Expected to directly skip the leader");
    }
}

/// Indirect-commit of the leader of round 3.
#[tokio::test]
async fn indirect_commit() {
    telemetry_subscribers::init_for_testing();
    // Dag Notes:
    // Pipeline is enabled
    // For the first 3 waves, the leaders are directly committed.
    // The leader of round 3 is not directly committed as there are f+1 certificates
    // only One needs to wait until the leader of Round 6 is directly decided
    // to indirectly decide the leader of round 3
    // - Fully connected blocks to decide the leader of wave 2.
    let dag_str = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : { * },
        Round 4 : {
            A -> [-D3],
            B -> [*],
            C -> [*],
            D -> [*],
        },
        Round 5 : {
            A -> [*],
            B -> [*],
            C -> [A4],
            D -> [A4],
        },
        Round 6 : { * },
        Round 7 : { * },
        Round 8 : { * },
     }";

    let dag_builder = parse_dag(dag_str).expect("Invalid dag");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new(dag_builder.context.clone())),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        dag_builder.context.clone(),
        LeaderSwapTable::default(),
    ));

    dag_builder.print();
    dag_builder.persist_all_blocks(dag_state.clone());

    // Create committer with pipelining and 1 leader per round
    let committer =
        UniversalCommitterBuilder::new(dag_builder.context, leader_schedule, dag_state).build();
    // note: with pipelining or multi-leader enabled there should be three
    // committer.
    assert_eq!(committer.committers.len(), 3);

    // Ensure we indirectly commit the leader of round 3 via the directly committed
    // leader of round 6.
    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");
    assert_eq!(sequence.len(), 6);

    for (idx, decided_leader) in sequence.iter().enumerate() {
        let leader_round =
            committer.committers[(idx + 1) % 3].leader_round(((idx + 1) / 3) as WaveNumber);
        let expected_leader = committer.get_leaders(leader_round)[0];
        if let DecidedLeader::Commit(ref block) = decided_leader {
            assert_eq!(block.round(), leader_round);
            assert_eq!(block.author(), expected_leader);
        } else {
            panic!("Expected a committed leader")
        };
    }
}

/// Skip indirectly the leader of round 4.
#[tokio::test]
async fn indirect_skip() {
    telemetry_subscribers::init_for_testing();
    // Dag Notes:
    // Pipeline is enabled
    // Leader of round 4 is not directly skipped due to a quorum of votes
    // But it is skipped indirectly since the leader of round 7 is directly
    // committed and is not linked with the leader of round 4 through a
    // certificate
    let dag_str = "DAG {
        Round 0 : { 4 },
        Round 1 : { * },
        Round 2 : { * },
        Round 3 : { * },
        Round 4 : { * },
        Round 5 : {
            A -> [*],
            B -> [*],
            C -> [*],
            D -> [-A4],
        },
        Round 6 : {
            A -> [*],
            B -> [-A5],
            C -> [-B5],
            D -> [*],
        },
        Round 7 : {
            A -> [*],
            B -> [*],
            C -> [*],
            D -> [B6],
        },
        Round 8 : { * },
        Round 9 : { * },
     }";

    let dag_builder = parse_dag(dag_str).expect("Invalid dag");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new(dag_builder.context.clone())),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        dag_builder.context.clone(),
        LeaderSwapTable::default(),
    ));

    dag_builder.print();
    dag_builder.persist_all_blocks(dag_state.clone());

    // Create committer with pipelining and 1 leader per round
    let committer =
        UniversalCommitterBuilder::new(dag_builder.context, leader_schedule, dag_state).build();
    // note: with pipelining or multi-leader enabled there should be three
    // committers.
    assert_eq!(committer.committers.len(), 3);

    // Ensure we indirectly skip the leader of round 4 via the directly committed
    // leader of round 7.
    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");
    assert_eq!(sequence.len(), 7);

    for (idx, decided_leader) in sequence.iter().enumerate() {
        if let DecidedLeader::Commit(ref block) = decided_leader {
            assert_eq!(block.round(), (idx + 1) as Round);
            assert_eq!(
                block.author(),
                AuthorityIndex::new_for_test((idx + 1) as u8 % 4)
            );
        } else if let DecidedLeader::Skip(ref slot) = decided_leader {
            assert_eq!(slot.round, 4 as Round);
        } else {
            panic!("Expected a decided leader");
        }
    }
}

/// If there is no leader with enough support nor blame, we commit nothing.
#[tokio::test]
async fn undecided() {
    telemetry_subscribers::init_for_testing();
    // Dag Notes:
    // Pipeline is enabled
    // Construct DAG such that no leader is directly committed
    // For instance, for round 1 leader, not enough certificates exist
    // For round 2 leader, not enough votes, etc.
    let dag_str = "DAG {
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
        Round 7 : {
            A -> [A6],
            B -> [B6],
            C -> [*],
            D -> [*],
        },
     }";

    let dag_builder = parse_dag(dag_str).expect("Invalid dag");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new(dag_builder.context.clone())),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        dag_builder.context.clone(),
        LeaderSwapTable::default(),
    ));

    dag_builder.print();
    dag_builder.persist_all_blocks(dag_state.clone());

    // Create committer with pipelining and 1 leader per round
    let committer =
        UniversalCommitterBuilder::new(dag_builder.context, leader_schedule, dag_state).build();
    // note: without pipelining or multi-leader enabled there should only be one
    // committer.
    assert_eq!(committer.committers.len(), 3);

    // Ensure we indirectly commit the leader of round3 via the directly committed
    // leader of round 6.
    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");
    assert!(sequence.is_empty());
}

// This test scenario has one authority that is acting in a byzantine manner. It
// will be sending multiple different blocks to different validators for a
// round. The commit rule should handle this and correctly commit the expected
// blocks.
#[tokio::test]
async fn test_byzantine_direct_commit() {
    let (context, dag_state, committer) = basic_test_setup();

    // Add enough blocks to reach first leader of wave 4
    // note: waves & rounds are zero-indexed.
    let round_12 = 12;
    let references_round_12 = build_dag(context.clone(), dag_state.clone(), None, round_12);

    // Add blocks to reach voting round of wave 4
    let voting_round_for_round_12_leader = 13;
    // This includes a "good vote" from validator C which is acting as a byzantine
    // validator
    let good_references_voting_round_for_round_12 = build_dag(
        context,
        dag_state.clone(),
        Some(references_round_12.clone()),
        voting_round_for_round_12_leader,
    );

    // DagState Update:
    // - 'A12' got a good vote from 'C' above
    // - 'A12' will then get a bad vote from 'C' indirectly through the ancenstors
    //   of the wave 4 certifying blocks of B C D

    // Add block layer for wave 4 certifying round with no votes for leader A12
    // from a byzantine validator C that sent different blocks to all validators.

    // Filter out leader from wave 4 { A12 }.
    let leader_round_12 = committer.get_leaders(round_12)[0];

    // References to blocks from leader round wave 4 { B12 C12 D12 }
    let references_without_leader_round_wave_4: Vec<_> = references_round_12
        .into_iter()
        .filter(|x| x.author != leader_round_12)
        .collect();

    // Accept these references/blocks as ancestors from certifying round blocks in
    // dag state
    let byzantine_block_c13_1 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(13, 2)
            .set_ancestors(references_without_leader_round_wave_4.clone())
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(byzantine_block_c13_1.clone(), DataSource::Test);

    let byzantine_block_c13_2 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(13, 2)
            .set_ancestors(references_without_leader_round_wave_4.clone())
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(byzantine_block_c13_2.clone(), DataSource::Test);

    let byzantine_block_c13_3 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(13, 2)
            .set_ancestors(references_without_leader_round_wave_4)
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(byzantine_block_c13_3.clone(), DataSource::Test);

    // Ancestors of certifying blocks in round 14 should include multiple byzantine
    // non-votes C13 but there are enough good votes to prevent a skip.
    // Additionally only one of the non-votes per authority should be counted so
    // we should not skip leader A12.
    let certifying_block_a14 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(14, 0)
            .set_ancestors(good_references_voting_round_for_round_12.clone())
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(certifying_block_a14, DataSource::Test);

    let good_references_voting_round_for_round_12_without_c13 =
        good_references_voting_round_for_round_12
            .into_iter()
            .filter(|r| r.author != AuthorityIndex::new_for_test(2))
            .collect::<Vec<_>>();

    let certifying_block_b14 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(14, 1)
            .set_ancestors(
                good_references_voting_round_for_round_12_without_c13
                    .iter()
                    .cloned()
                    .chain(std::iter::once(byzantine_block_c13_1.reference()))
                    .collect(),
            )
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(certifying_block_b14, DataSource::Test);

    let certifying_block_c14 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(14, 2)
            .set_ancestors(
                good_references_voting_round_for_round_12_without_c13
                    .iter()
                    .cloned()
                    .chain(std::iter::once(byzantine_block_c13_2.reference()))
                    .collect(),
            )
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(certifying_block_c14, DataSource::Test);

    let certifying_block_d14 = VerifiedBlockHeader::new_for_test(
        TestBlockHeader::new(14, 3)
            .set_ancestors(
                good_references_voting_round_for_round_12_without_c13
                    .iter()
                    .cloned()
                    .chain(std::iter::once(byzantine_block_c13_3.reference()))
                    .collect(),
            )
            .build(),
    );
    dag_state
        .write()
        .accept_block_header(certifying_block_d14, DataSource::Test);

    // DagState Update:
    // - We have A13, B13, D13 & C13 as good votes in the voting round for round-12
    //   leader block
    // - We have 3 byzantine C13 nonvotes that we received as ancestors from
    //   certifying round blocks from B, C, & D.
    // - We have B14, C14 & D14 that include this byzantine nonvote from C13 but
    // all of these blocks also have good votes for leader A12 through A, B, D.

    // Expect a successful direct commit of A12 and all leaders at previous rounds.
    let last_decided = Slot::new(0, 0);
    let sequence = committer.try_decide(last_decided);
    tracing::info!("Commit sequence: {sequence:#?}");

    assert_eq!(sequence.len(), 12);
    if let DecidedLeader::Commit(ref block) = sequence[11] {
        assert_eq!(block.author(), committer.get_leaders(round_12)[0])
    } else {
        panic!("Expected a committed leader")
    };
}

// TODO: Add byzantine variant of tests for indirect/direct
// commit/skip/undecided decisions

fn basic_test_setup() -> (
    Arc<Context>,
    Arc<RwLock<DagState>>,
    super::UniversalCommitter,
) {
    telemetry_subscribers::init_for_testing();
    // Committee of 4 with even stake
    let context = Arc::new(Context::new_for_test(4).0);
    let dag_state = Arc::new(RwLock::new(DagState::new(
        context.clone(),
        Arc::new(MemStore::new(context.clone())),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        context.clone(),
        LeaderSwapTable::default(),
    ));

    // Create committer with pipelining and only 1 leader per leader round
    let committer =
        UniversalCommitterBuilder::new(context.clone(), leader_schedule, dag_state.clone()).build();

    // note: with pipelining or multi-leader enabled there should be three pipelined
    // committers.
    assert!(committer.committers.len() == 3);

    (context, dag_state, committer)
}

struct TestSetup {
    dag_builder: DagBuilder,
    dag_state: Arc<RwLock<DagState>>,
    committer: super::UniversalCommitter,
}

// TODO: Make this the basic_test_setup()
fn basic_dag_builder_test_setup() -> TestSetup {
    telemetry_subscribers::init_for_testing();
    let context = Arc::new(Context::new_for_test(4).0);
    let dag_builder = DagBuilder::new(context.clone());

    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new(context)),
    )));
    let leader_schedule = Arc::new(LeaderSchedule::new(
        dag_builder.context.clone(),
        LeaderSwapTable::default(),
    ));

    // Create committer with pipelining and only 1 leader per leader round
    let committer = UniversalCommitterBuilder::new(
        dag_builder.context.clone(),
        leader_schedule,
        dag_state.clone(),
    )
    .build();
    // note: with pipelining or no multi-leader enabled there should be three
    // committers.
    assert_eq!(committer.committers.len(), 3);

    TestSetup {
        dag_builder,
        dag_state,
        committer,
    }
}
