// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;

use crate::{
    commit::{CommitMetastate, LeaderStatus},
    committer::Committer,
    dag_state::DagState,
    storage::mem_store::MemStore,
    test_dag_parser::parse_dag,
};

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

/// A committer over the blocks of the parsed `dag`.
fn committer_for(dag: &str) -> Committer {
    telemetry_subscribers::init_for_testing();
    let dag_builder = parse_dag(dag).expect("a DAG should be valid");
    let dag_state = Arc::new(RwLock::new(DagState::new(
        dag_builder.context.clone(),
        Arc::new(MemStore::new()),
    )));
    dag_builder.persist_all_blocks(dag_state.clone());
    Committer::new_for_test(dag_builder.context, dag_state)
}
