// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, env, sync::Arc};

use parking_lot::RwLock;
use rand::{RngExt, SeedableRng, prelude::SliceRandom, rngs::StdRng};
use starfish_config::AuthorityIndex;

use crate::{
    block_header::{BlockHeaderAPI, BlockHeaderDigest, BlockRef, GENESIS_ROUND, Round, Slot},
    block_manager::{BlockManager, block_suspender::tests::evaluate_block_headers},
    commit::{CommitMetastate, DecidedLeader},
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    storage::mem_store::MemStore,
    test_dag::create_random_dag,
    test_dag_builder::DagBuilder,
    universal_committer::{
        UniversalCommitter, universal_committer_builder::UniversalCommitterBuilder,
    },
};

const NUM_RUNS: u32 = 100;
const NUM_ROUNDS: u32 = 200;
const NUM_EQUIVOCATION_RUNS: u32 = 20;
/// Honest authorities that each receive the blocks of a dag with
/// equivocations in their own order.
const NUM_EQUIVOCATION_OBSERVERS: usize = 3;
/// (percent of rounds whose blocks all link the leader, percent of blocks
/// acknowledged) for the runs of the equivocation test.
const EQUIVOCATION_SETTINGS: [(u64, u64); 6] = [
    (100, 100),
    (100, 90),
    (100, 70),
    (50, 100),
    (50, 90),
    (50, 70),
];

/// Test builds a randomized dag with the following conditions:
/// - Links to 2f+1 minimum ancestors
/// - Links to leader of previous round.
///
/// Should result in a direct commit for every round.
#[tokio::test]
async fn test_randomized_dag_all_direct_commit() {
    let mut random_test_setup = random_test_setup();

    for _ in 0..NUM_RUNS {
        let seed = random_test_setup.seeded_rng.random_range(0..10000);
        let num_authorities = random_test_setup.seeded_rng.random_range(4..10);
        let mut authority = authority_setup(num_authorities, 0);

        let include_leader_percentage = 100;
        let dag_builder = create_random_dag(
            seed,
            include_leader_percentage,
            100,
            NUM_ROUNDS,
            &[],
            0,
            authority.context.clone(),
        );
        assert_parents_follow_verifier(&dag_builder);

        dag_builder.persist_all_blocks(authority.dag_state.clone());

        tracing::info!(
            "Running test with committee size {num_authorities} & {NUM_ROUNDS} rounds in the DAG..."
        );

        let last_finalized = Slot::new(0, 0);
        let sequence = authority.committer.try_decide(last_finalized);
        tracing::debug!("Commit sequence: {sequence:#?}");

        assert_eq!(sequence.len(), (NUM_ROUNDS - 2) as usize);
        for (i, leader_block) in sequence.iter().enumerate() {
            // First sequenced leader should be in round 1.
            let leader_round = i as u32 + 1;
            assert_decision_is_valid(
                &authority.committer,
                &dag_builder,
                leader_block,
                leader_round,
            );
            // Every block links and acknowledges the whole previous round, so
            // every leader gets a quorum of strong votes.
            assert!(
                matches!(
                    leader_block,
                    DecidedLeader::Commit(_, CommitMetastate::Optimistic, _)
                ),
                "expected an optimistic commit at round {leader_round}, got {leader_block:?}"
            );
        }
    }
}
/// Test builds a randomized dag with the following conditions:
/// - Links to 2f+1 minimum ancestors
/// - Links to leader of previous round 50% of the time.
///
/// Blocks will randomly be fed through BlockManager and after each accepted
/// block we will try_decide() and if there is a committed sequence we will
/// update last_finalized and continue. We do this from the perspective of two
/// different authorities who receive the blocks in different orders and ensure
/// the resulting sequence is the same for both authorities. The resulting
/// sequence will include Commit & Skip decisions and potentially will stop
/// before coming to a decision on all waves as we may have an Undecided leader
/// somewhere early in the sequence.
#[tokio::test]
async fn test_randomized_dag_and_decision_sequence() {
    let mut random_test_setup = random_test_setup();

    for _ in 0..NUM_RUNS {
        let seed = random_test_setup.seeded_rng.random_range(0..10000);
        let num_authorities = random_test_setup.seeded_rng.random_range(4..10);

        let include_leader_percentage = 50;
        let dag_builder = create_random_dag(
            seed,
            include_leader_percentage,
            100,
            NUM_ROUNDS,
            &[],
            0,
            Arc::new(Context::new_for_test(num_authorities).0),
        );
        assert_parents_follow_verifier(&dag_builder);

        tracing::info!(
            "Running test with committee size {num_authorities} & {NUM_ROUNDS} rounds in the DAG..."
        );

        let sequenced_leaders_1 = deliver_in_random_order(
            num_authorities,
            AuthorityIndex::new_for_test(1),
            &dag_builder,
            &mut random_test_setup.seeded_rng,
        );
        let sequenced_leaders_2 = deliver_in_random_order(
            num_authorities,
            AuthorityIndex::new_for_test(2),
            &dag_builder,
            &mut random_test_setup.seeded_rng,
        );

        // Ensure despite the difference in when blocks were received eventually after
        // receiving all blocks both authorities should return the same sequence of
        // blocks.
        assert_eq!(sequenced_leaders_1, sequenced_leaders_2);
    }
}

/// Test builds randomized dags in which up to f authorities propose several
/// blocks with the same parents in every round, and blocks acknowledge all or
/// only part of the previous round, so leaders are committed as optimistic or
/// standard. Blocks are fed in different random orders to honest authorities,
/// which must decide the same sequence, commit kinds included. When every
/// block links the leader of the previous round, every honest leader must be
/// committed.
#[tokio::test]
async fn test_randomized_dag_with_equivocations() {
    let mut random_test_setup = random_test_setup();
    let rng = &mut random_test_setup.seeded_rng;

    for run in 0..NUM_EQUIVOCATION_RUNS {
        let seed = rng.random_range(0..10000);
        let num_authorities = rng.random_range(4..10);
        let mut authorities = (0..num_authorities)
            .map(|index| AuthorityIndex::new_for_test(index as u8))
            .collect::<Vec<_>>();
        authorities.shuffle(rng);
        let (equivocators, honest) = authorities.split_at((num_authorities - 1) / 3);
        let equivocations = rng.random_range(1..=2);
        // Cycled rather than drawn, so every setting runs in every test run.
        let (include_leader_percentage, acknowledgment_percentage) =
            EQUIVOCATION_SETTINGS[run as usize % EQUIVOCATION_SETTINGS.len()];

        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let dag_builder = create_random_dag(
            seed,
            include_leader_percentage,
            acknowledgment_percentage,
            NUM_ROUNDS,
            equivocators,
            equivocations,
            context,
        );
        assert_parents_follow_verifier(&dag_builder);

        tracing::info!(
            "Running test with committee size {num_authorities}, equivocators {equivocators:?} \
             proposing {} blocks per round, leader links in {include_leader_percentage}% of \
             rounds, {acknowledgment_percentage}% of blocks acknowledged",
            1 + equivocations
        );

        let sequences = honest
            .iter()
            .take(NUM_EQUIVOCATION_OBSERVERS)
            .map(|observer| deliver_in_random_order(num_authorities, *observer, &dag_builder, rng))
            .collect::<Vec<_>>();
        for sequence in &sequences[1..] {
            assert_eq!(sequence, &sequences[0]);
        }
        let sequence = &sequences[0];

        if include_leader_percentage == 100 {
            for decision in sequence {
                let slot = Slot::new(decision.round(), decision.authority());
                if !equivocators.contains(&slot.authority) {
                    assert!(
                        matches!(decision, DecidedLeader::Commit(..)),
                        "honest leader {slot} linked by every block was not committed"
                    );
                }
            }
        }

        if include_leader_percentage == 100 && acknowledgment_percentage == 100 {
            // Every block links and fully acknowledges the previous leader, so
            // every honest leader up to round NUM_ROUNDS - 2 is committed
            // directly and optimistically. Leaders rotate, so every slot up to
            // round NUM_ROUNDS - num_authorities - 4 has such a leader three to
            // num_authorities + 2 rounds above it, which decides it.
            let decided_floor = NUM_ROUNDS as usize - num_authorities - 4;
            assert!(
                sequence.len() >= decided_floor,
                "{} leaders decided, expected at least {decided_floor}",
                sequence.len()
            );
            for decision in sequence {
                if !equivocators.contains(&decision.authority()) {
                    assert!(
                        matches!(
                            decision,
                            DecidedLeader::Commit(_, CommitMetastate::Optimistic, _)
                        ),
                        "honest leader at round {} was not committed optimistically",
                        decision.round()
                    );
                }
            }
        }
    }
}

/// Asserts that every block of `dag_builder` follows the block verifier's
/// parent rules: own previous block first, at most one block per author, all
/// parents from earlier rounds of the dag, and a quorum by stake from the
/// previous round.
fn assert_parents_follow_verifier(dag_builder: &DagBuilder) {
    let committee = &dag_builder.context.committee;
    let genesis = dag_builder.genesis_block_refs();
    for header in dag_builder.block_headers.values() {
        if header.round() == GENESIS_ROUND {
            continue;
        }
        let ancestors = header.ancestors();
        assert_eq!(
            ancestors.first().map(|parent| parent.author),
            Some(header.author()),
            "{} does not link its own previous block first",
            header.reference()
        );
        let mut authors = HashSet::new();
        let mut parent_stake = 0;
        for parent in ancestors {
            assert!(
                authors.insert(parent.author),
                "{} links two blocks of {}",
                header.reference(),
                parent.author
            );
            assert!(parent.round < header.round());
            assert!(
                dag_builder.block_headers.contains_key(parent) || genesis.contains(parent),
                "{} links {parent}, which is not in the dag",
                header.reference()
            );
            if parent.round + 1 == header.round() {
                parent_stake += committee.stake(parent.author);
            }
        }
        assert!(
            committee.reached_quorum(parent_stake),
            "{} links less than a quorum of the previous round",
            header.reference()
        );
    }
}

/// Asserts that `decision` is for the leader of `round`, that a committed
/// leader is a block of `dag_builder`, and that its strong voters match its
/// commit kind: for an optimistic commit, authors with a quorum of stake that
/// each have a block in the next round linking the leader; none otherwise. An
/// author that equivocated in the next round can be listed once per block.
fn assert_decision_is_valid(
    committer: &UniversalCommitter,
    dag_builder: &DagBuilder,
    decision: &DecidedLeader,
    round: Round,
) {
    let leader = committer.get_leaders(round)[0];
    assert_eq!(
        (decision.round(), decision.authority()),
        (round, leader),
        "decision {decision:?} is not for the leader of round {round}"
    );
    let DecidedLeader::Commit(block, metastate, strong_voters) = decision else {
        return;
    };
    assert!(
        dag_builder.block_headers.contains_key(&block.reference()),
        "committed leader {} is not in the dag",
        block.reference()
    );
    if *metastate != CommitMetastate::Optimistic {
        assert!(
            strong_voters.is_empty(),
            "{metastate:?} commit of {} lists strong voters {strong_voters:?}",
            block.reference()
        );
        return;
    }
    let committee = &dag_builder.context.committee;
    let mut voters = HashSet::new();
    let mut voter_stake = 0;
    for voter in strong_voters {
        if !voters.insert(*voter) {
            continue;
        }
        voter_stake += committee.stake(*voter);
        let voter_slot = BlockRef::new(round + 1, *voter, BlockHeaderDigest::MIN)
            ..=BlockRef::new(round + 1, *voter, BlockHeaderDigest::MAX);
        assert!(
            dag_builder
                .block_headers
                .range(voter_slot)
                .any(|(_, vote)| vote.ancestors().contains(&block.reference())),
            "strong voter {voter} has no block linking {}",
            block.reference()
        );
    }
    assert!(
        committee.reached_quorum(voter_stake),
        "optimistic commit of {} has strong voters {strong_voters:?} below a quorum",
        block.reference()
    );
}

/// Feeds all blocks of `dag_builder` to a new authority `observer` in a random
/// order and in random batches, checks the block manager and every decision
/// after each batch, and returns the decided leaders with the strong voters
/// cleared.
fn deliver_in_random_order(
    num_authorities: usize,
    observer: AuthorityIndex,
    dag_builder: &DagBuilder,
    rng: &mut StdRng,
) -> Vec<DecidedLeader> {
    let mut authority = authority_setup(num_authorities, observer.value() as u8);
    let mut all_blocks = dag_builder
        .block_headers
        .values()
        .cloned()
        .collect::<Vec<_>>();
    all_blocks.shuffle(rng);

    let mut sequenced_leaders = vec![];
    let mut last_finalized = Slot::new(0, 0);
    let mut i = 0;
    let mut seen_so_far = vec![];
    while i < all_blocks.len() {
        let chunk_size = rng.random_range(1..=(all_blocks.len() - i));
        let chunk = &all_blocks[i..i + chunk_size];
        seen_so_far.extend(chunk.iter().cloned());
        let _ = authority
            .block_manager
            .try_accept_block_headers(chunk.to_vec(), DataSource::Test);
        let sequence = authority.committer.try_decide(last_finalized);

        for decision in &sequence {
            // One leader per round, decided in order starting at round 1.
            let round = sequenced_leaders.len() as Round + 1;
            assert_decision_is_valid(&authority.committer, dag_builder, decision, round);
            sequenced_leaders.push(decision.clone());
        }
        if let Some(leader_status) = sequence.last() {
            last_finalized = Slot::new(leader_status.round(), leader_status.authority());
        }

        i += chunk_size;
        let (suspended, missing) = evaluate_block_headers(&seen_so_far);
        assert_eq!(suspended, authority.block_manager.suspended_blocks_refs());
        assert_eq!(missing, authority.block_manager.blocks_to_fetch_refs());
    }
    assert!(authority.block_manager.is_empty());
    // An empty block manager does not rule out a header dropped on the way.
    let dag_state = authority.dag_state.read();
    for block_ref in dag_builder.block_headers.keys() {
        assert!(
            dag_state.contains_block_header(block_ref),
            "{block_ref} was never accepted"
        );
    }

    // The strong voters are the voters the deciding authority had accepted at
    // that point, so authorities that received blocks in different orders
    // list different supersets of the quorum behind the commit.
    decisions_without_strong_voters(&sequenced_leaders)
}

/// The decisions with the strong voters cleared, leaving what every authority
/// must agree on.
fn decisions_without_strong_voters(leaders: &[DecidedLeader]) -> Vec<DecidedLeader> {
    leaders
        .iter()
        .map(|leader| match leader {
            DecidedLeader::Commit(block, metastate, _) => {
                DecidedLeader::Commit(block.clone(), *metastate, vec![])
            }
            DecidedLeader::Skip(slot) => DecidedLeader::Skip(*slot),
        })
        .collect()
}

struct AuthorityTestFixture {
    context: Arc<Context>,
    dag_state: Arc<RwLock<DagState>>,
    committer: UniversalCommitter,
    block_manager: BlockManager,
}

fn authority_setup(num_authorities: usize, authority_index: u8) -> AuthorityTestFixture {
    let context = Context::new_for_test(num_authorities)
        .0
        .with_authority_index(AuthorityIndex::new_for_test(authority_index));
    let context = Arc::new(context);
    let leader_schedule = Arc::new(LeaderSchedule::new(
        context.clone(),
        LeaderSwapTable::default(),
    ));
    let dag_state = Arc::new(RwLock::new(DagState::new(
        context.clone(),
        Arc::new(MemStore::new()),
    )));

    // Create committer with pipelining and only 1 leader per leader round
    let committer =
        UniversalCommitterBuilder::new(context.clone(), leader_schedule, dag_state.clone()).build();

    let block_manager = BlockManager::new(context.clone(), dag_state.clone());

    AuthorityTestFixture {
        context,
        dag_state,
        committer,
        block_manager,
    }
}

struct RandomTestFixture {
    seeded_rng: StdRng,
}

fn random_test_setup() -> RandomTestFixture {
    telemetry_subscribers::init_for_testing();
    let mut rng = rand::make_rng::<StdRng>();
    let seed = match env::var("DAG_TEST_SEED") {
        Ok(seed_str) => {
            if let Ok(seed) = seed_str.parse::<u64>() {
                seed
            } else {
                tracing::warn!("Invalid DAG_TEST_SEED format. Using random seed.");
                rng.random_range(0..10000)
            }
        }
        Err(_) => rng.random_range(0..10000),
    };
    tracing::warn!("Using Random Seed: {seed}");

    let seeded_rng = StdRng::seed_from_u64(seed);
    RandomTestFixture { seeded_rng }
}
