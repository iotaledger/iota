// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use parking_lot::RwLock;
use rand::{RngExt, SeedableRng, prelude::SliceRandom, rngs::StdRng};
use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    block_header::{
        BlockHeaderAPI, BlockRef, GENESIS_ROUND, Round, Slot, StrongVote, TestBlockHeader,
        TestBlockHeaderVersion, VerifiedBlockHeader, genesis_block_headers,
    },
    block_manager::BlockManager,
    commit::DecidedLeader,
    context::Context,
    dag_state::{DagState, DataSource},
    leader_schedule::{LeaderSchedule, LeaderSwapTable},
    storage::mem_store::MemStore,
    universal_committer::universal_committer_builder::UniversalCommitterBuilder,
};

const NUM_RUNS: u64 = 300;
const NUM_ROUNDS: Round = 40;

/// Feeds the same random DAGs, in random chunks, to the committer and to the
/// three pipelined committers it replaces, and checks that both return the
/// same decisions after every chunk.
#[tokio::test]
async fn single_committer_decides_like_pipelined_committers() {
    telemetry_subscribers::init_for_testing();
    let mut decided_by_status: BTreeMap<String, u64> = BTreeMap::new();

    for seed in 0..NUM_RUNS {
        let mut rng = StdRng::seed_from_u64(seed);
        let num_authorities = rng.random_range(4..=10);
        let leader_link_percentage = [30, 60, 80, 95, 100][rng.random_range(0..5)];
        let context = Arc::new(Context::new_for_test(num_authorities).0);
        let leader_schedule = Arc::new(LeaderSchedule::new(
            context.clone(),
            LeaderSwapTable::default(),
        ));
        let dag_state = Arc::new(RwLock::new(DagState::new(
            context.clone(),
            Arc::new(MemStore::new()),
        )));
        let mut committer = UniversalCommitterBuilder::new(
            context.clone(),
            leader_schedule.clone(),
            dag_state.clone(),
        )
        .build();
        let mut pipelined = pipelined_committers::UniversalCommitter::new(
            context.clone(),
            leader_schedule.clone(),
            dag_state.clone(),
        );
        let mut block_manager = BlockManager::new(context.clone(), dag_state.clone());

        for round in GENESIS_ROUND..=NUM_ROUNDS {
            assert_eq!(
                committer.get_leaders(round),
                pipelined.get_leaders(round),
                "seed {seed}, round {round}"
            );
        }

        let mut blocks = random_dag(&context, &leader_schedule, &mut rng, leader_link_percentage);
        blocks.shuffle(&mut rng);

        let mut last_finalized = Slot::new(GENESIS_ROUND, 0u8);
        let mut next = 0;
        while next < blocks.len() {
            let chunk_size = rng.random_range(1..=(blocks.len() - next).min(2 * num_authorities));
            let chunk = blocks[next..next + chunk_size].to_vec();
            next += chunk_size;
            block_manager.try_accept_block_headers(chunk, DataSource::Test);

            let decided = with_sorted_strong_voters(committer.try_decide(last_finalized));
            let decided_by_pipelined =
                with_sorted_strong_voters(pipelined.try_decide(last_finalized));
            assert_eq!(
                decided, decided_by_pipelined,
                "seed {seed}, last finalized {last_finalized}"
            );
            if let Some(last) = decided.last() {
                last_finalized = last.slot();
            }
        }
        assert!(block_manager.is_empty(), "seed {seed}");

        let metric = &context.metrics.node_metrics.decided_leaders_total;
        for (_, authority) in context.committee.authorities() {
            for decision in ["direct", "indirect", "upgraded"] {
                for outcome in ["commit-optimistic", "commit-standard", "skip"] {
                    let status = format!("{decision}-{outcome}");
                    let count = metric
                        .with_label_values(&[&authority.hostname, &status])
                        .get();
                    *decided_by_status.entry(status).or_default() += count;
                }
            }
        }
    }

    tracing::info!("Decided leaders by status: {decided_by_status:?}");
    let decided = |status: &str| decided_by_status.get(status).copied().unwrap_or(0);
    for status in [
        "direct-commit-optimistic",
        "direct-commit-standard",
        "direct-skip",
        "indirect-skip",
    ] {
        assert!(decided(status) > 0, "no {status} in {decided_by_status:?}");
    }
    assert!(
        decided("indirect-commit-optimistic") + decided("indirect-commit-standard") > 0,
        "no indirect commit in {decided_by_status:?}"
    );
    assert!(
        decided("upgraded-commit-optimistic") + decided("upgraded-commit-standard") > 0,
        "no upgraded commit in {decided_by_status:?}"
    );
}

/// The strong voters come out of a hash set, so their order is arbitrary.
fn with_sorted_strong_voters(mut leaders: Vec<DecidedLeader>) -> Vec<DecidedLeader> {
    for leader in &mut leaders {
        if let DecidedLeader::Commit(_, _, strong_voters) = leader {
            strong_voters.sort();
        }
    }
    leaders
}

/// Builds `NUM_ROUNDS` rounds of random blocks. Each round has blocks from at
/// least a quorum. A block links its own previous block, at most one block per
/// authority, a random quorum or more of the previous round, and the previous
/// round's leader with `leader_link_percentage` probability. A block that
/// links the leader carries a random strong vote: a vote, a blame, one pinned
/// to another authority, or none. One authority may equivocate.
fn random_dag(
    context: &Context,
    leader_schedule: &LeaderSchedule,
    rng: &mut StdRng,
    leader_link_percentage: u32,
) -> Vec<VerifiedBlockHeader> {
    let committee = &context.committee;
    let authorities: Vec<AuthorityIndex> = committee.authorities().map(|(a, _)| a).collect();
    // DagState rejects equivocations by its own authority.
    let equivocator = (rng.random_range(0..100) < 30).then(|| {
        loop {
            let authority = authorities[rng.random_range(0..authorities.len())];
            if authority != context.own_index {
                break authority;
            }
        }
    });

    let mut all_blocks = Vec::new();
    let mut previous: BTreeMap<AuthorityIndex, Vec<BlockRef>> = genesis_block_headers(context)
        .iter()
        .map(|b| (b.author(), vec![b.reference()]))
        .collect();

    for round in 1..=NUM_ROUNDS {
        let leader = leader_schedule.elect_leader(round - 1, 0);
        let link_leader_wanted = rng.random_range(0..100) < leader_link_percentage;

        let mut producers = authorities.clone();
        producers.shuffle(rng);
        let mut stake = 0;
        let producers: Vec<AuthorityIndex> = producers
            .into_iter()
            .filter(|&authority| {
                let produce = !committee.reached_quorum(stake) || rng.random_range(0..100) < 80;
                if produce {
                    stake += committee.stake(authority);
                }
                produce
            })
            .collect();

        let mut current: BTreeMap<AuthorityIndex, Vec<BlockRef>> = BTreeMap::new();
        for author in producers {
            let num_blocks = if equivocator == Some(author) && rng.random_range(0..100) < 50 {
                2
            } else {
                1
            };
            for block_index in 0..num_blocks {
                let ancestors =
                    random_ancestors(context, rng, &previous, author, leader, link_leader_wanted);
                let links_leader = ancestors.iter().any(|a| a.author == leader);
                let strong_vote = links_leader
                    .then(|| random_strong_vote(rng, &authorities, round, leader))
                    .flatten();
                let block = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author.value() as u8)
                        .set_version(TestBlockHeaderVersion::V2)
                        .set_ancestors(ancestors)
                        .set_timestamp_ms(round as u64 * 1000 + block_index)
                        .set_strong_vote(strong_vote)
                        .build(),
                );
                current.entry(author).or_default().push(block.reference());
                all_blocks.push(block);
            }
        }
        previous = current;
    }
    all_blocks
}

/// Picks the ancestors of a block by `author` from the previous round's
/// blocks, with the author's own block first.
fn random_ancestors(
    context: &Context,
    rng: &mut StdRng,
    previous: &BTreeMap<AuthorityIndex, Vec<BlockRef>>,
    author: AuthorityIndex,
    leader: AuthorityIndex,
    link_leader_wanted: bool,
) -> Vec<BlockRef> {
    let committee = &context.committee;
    let pick = |rng: &mut StdRng, refs: &Vec<BlockRef>| refs[rng.random_range(0..refs.len())];

    let mut ancestors = Vec::new();
    let mut stake = 0;
    if let Some(own) = previous.get(&author) {
        ancestors.push(pick(rng, own));
        stake += committee.stake(author);
    }
    if author != leader && link_leader_wanted {
        if let Some(leader_refs) = previous.get(&leader) {
            ancestors.push(pick(rng, leader_refs));
            stake += committee.stake(leader);
        }
    }

    let mut others: Vec<AuthorityIndex> = previous
        .keys()
        .copied()
        .filter(|&a| a != author && a != leader)
        .collect();
    others.shuffle(rng);
    // The leader is the last resort when the others cannot make a quorum.
    if author != leader && !link_leader_wanted && previous.contains_key(&leader) {
        others.push(leader);
    }
    for other in others {
        if committee.reached_quorum(stake) && rng.random_range(0..100) >= 30 {
            continue;
        }
        if other == leader && committee.reached_quorum(stake) {
            continue;
        }
        ancestors.push(pick(rng, &previous[&other]));
        stake += committee.stake(other);
    }
    ancestors
}

/// A random strong vote of a block at `round` that links `leader`.
fn random_strong_vote(
    rng: &mut StdRng,
    authorities: &[AuthorityIndex],
    round: Round,
    leader: AuthorityIndex,
) -> Option<StrongVote> {
    if round - 1 == GENESIS_ROUND {
        return Some(StrongVote {
            leader_authority: leader,
            missing: AuthoritySet::new(),
        });
    }
    let mut missing = AuthoritySet::new();
    let roll = rng.random_range(0..100);
    let leader_authority = if roll < 60 {
        leader
    } else if roll < 85 {
        missing.insert(authorities[rng.random_range(0..authorities.len())]);
        leader
    } else if roll < 92 {
        if rng.random_range(0..100) < 50 {
            missing.insert(leader);
        }
        authorities[rng.random_range(0..authorities.len())]
    } else {
        return None;
    };
    Some(StrongVote {
        leader_authority,
        missing,
    })
}

/// The committer before it was replaced by a single one: three
/// `BaseCommitter`s with round offsets 0, 1 and 2, each deciding every third
/// round. Copied without logging and metrics.
mod pipelined_committers {
    use std::{
        collections::{BTreeMap, HashSet, VecDeque, hash_map::Entry},
        sync::Arc,
    };

    use ahash::AHashMap;
    use parking_lot::RwLock;
    use starfish_config::{AuthorityIndex, Committee, Stake};

    use crate::{
        block_header::{BlockHeaderAPI, BlockRef, GENESIS_ROUND, Round, Slot, VerifiedBlockHeader},
        commit::{CommitMetastate, DecidedLeader, LeaderStatus, WAVE_LENGTH},
        context::Context,
        dag_state::DagState,
        leader_schedule::LeaderSchedule,
        stake_aggregator::{QuorumThreshold, StakeAggregator},
    };

    type WaveNumber = u32;

    pub(super) struct UniversalCommitter {
        dag_state: Arc<RwLock<DagState>>,
        committers: Vec<BaseCommitter>,
    }

    impl UniversalCommitter {
        pub(super) fn new(
            context: Arc<Context>,
            leader_schedule: Arc<LeaderSchedule>,
            dag_state: Arc<RwLock<DagState>>,
        ) -> Self {
            let committers = (0..WAVE_LENGTH)
                .map(|round_offset| BaseCommitter {
                    context: context.clone(),
                    leader_schedule: leader_schedule.clone(),
                    dag_state: dag_state.clone(),
                    leader_offset: 0,
                    round_offset,
                    certificates: BTreeMap::new(),
                })
                .collect();
            Self {
                dag_state,
                committers,
            }
        }

        pub(super) fn try_decide(&mut self, last_finalized: Slot) -> Vec<DecidedLeader> {
            for committer in &mut self.committers {
                committer.evict_cache_below(last_finalized.round + WAVE_LENGTH);
            }
            let highest_accepted_round = self.dag_state.read().highest_accepted_round();

            let mut leaders: VecDeque<LeaderStatus> = VecDeque::new();
            let last_round = last_finalized.round + 1;
            'outer: for round in (last_round..=highest_accepted_round.saturating_sub(2)).rev() {
                for committer in self.committers.iter_mut().rev() {
                    let Some(slot) = committer.elect_leader(round) else {
                        continue;
                    };
                    if slot == last_finalized {
                        break 'outer;
                    }
                    let mut status = committer.try_direct_decide(slot);
                    if !status.is_resolved() {
                        status = committer.try_indirect_decide(status, leaders.iter());
                    }
                    leaders.push_front(status);
                }
            }

            let mut decided_leaders = Vec::new();
            for leader in leaders {
                if leader.round() == GENESIS_ROUND {
                    continue;
                }
                if !leader.is_resolved() {
                    break;
                }
                decided_leaders.push(
                    leader
                        .into_decided_leader()
                        .expect("is_resolved implies a DecidedLeader"),
                );
            }
            decided_leaders
        }

        pub(super) fn get_leaders(&self, round: Round) -> Vec<AuthorityIndex> {
            self.committers
                .iter()
                .filter_map(|committer| committer.elect_leader(round))
                .map(|l| l.authority)
                .collect()
        }
    }

    struct BaseCommitter {
        context: Arc<Context>,
        leader_schedule: Arc<LeaderSchedule>,
        dag_state: Arc<RwLock<DagState>>,
        leader_offset: u32,
        round_offset: u32,
        certificates: BTreeMap<Round, AHashMap<(BlockRef, BlockRef), (bool, bool)>>,
    }

    impl BaseCommitter {
        fn evict_cache_below(&mut self, round: Round) {
            self.certificates = self.certificates.split_off(&round);
        }

        fn try_direct_decide(&mut self, leader: Slot) -> LeaderStatus {
            let voting_round = leader.round + 1;
            let voting_blocks = self
                .dag_state
                .read()
                .get_block_headers_at_round_above_last_commit(voting_round);
            if self.enough_leader_blame(&voting_blocks, leader.authority) {
                return LeaderStatus::Skip(leader);
            }

            let leader_blocks = self
                .dag_state
                .read()
                .get_block_headers_at_slot_above_last_commit(leader);
            if leader_blocks.is_empty() {
                return LeaderStatus::Undecided(leader);
            }

            let wave = self.wave_number(leader.round);
            let certifying_round = self.certifying_round(wave);
            let decision_blocks = self
                .dag_state
                .read()
                .get_block_headers_at_round_above_last_commit(certifying_round);

            let total_stake: Stake = decision_blocks
                .iter()
                .map(|b| self.context.committee.stake(b.author()))
                .sum();
            if !self.context.committee.reached_quorum(total_stake) {
                return LeaderStatus::Undecided(leader);
            }

            let mut leaders_with_enough_support: Vec<_> = leader_blocks
                .into_iter()
                .filter_map(|leader_block| {
                    self.direct_decide_leader_block(&leader_block, &voting_blocks, &decision_blocks)
                        .map(|(metastate, strong_voters)| {
                            LeaderStatus::Commit(leader_block, metastate, strong_voters)
                        })
                })
                .collect();

            if leaders_with_enough_support.len() > 1 {
                panic!("More than one certified block for {leader}")
            }

            leaders_with_enough_support
                .pop()
                .unwrap_or(LeaderStatus::Undecided(leader))
        }

        fn try_indirect_decide<'a>(
            &mut self,
            current: LeaderStatus,
            leaders: impl Iterator<Item = &'a LeaderStatus>,
        ) -> LeaderStatus {
            let slot = current.slot();
            let anchors = leaders.filter(|x| slot.round + WAVE_LENGTH <= x.round());
            for anchor in anchors {
                match anchor {
                    LeaderStatus::Commit(anchor, _, _) => {
                        return self.decide_leader_from_anchor(anchor, slot);
                    }
                    LeaderStatus::Skip(..) => (),
                    LeaderStatus::Undecided(..) => return current,
                }
            }
            current
        }

        fn elect_leader(&self, round: Round) -> Option<Slot> {
            let wave = self.wave_number(round);
            if self.leader_round(wave) != round {
                return None;
            }
            Some(Slot::new(
                round,
                self.leader_schedule.elect_leader(round, self.leader_offset),
            ))
        }

        fn leader_round(&self, wave_number: WaveNumber) -> Round {
            (wave_number * WAVE_LENGTH) + self.round_offset
        }

        fn certifying_round(&self, wave_number: WaveNumber) -> Round {
            (wave_number * WAVE_LENGTH) + WAVE_LENGTH - 1 + self.round_offset
        }

        fn wave_number(&self, round: Round) -> WaveNumber {
            round.saturating_sub(self.round_offset) / WAVE_LENGTH
        }

        fn is_vote(
            potential_vote: &VerifiedBlockHeader,
            leader_block: &VerifiedBlockHeader,
        ) -> bool {
            potential_vote
                .ancestors()
                .contains(&leader_block.reference())
        }

        fn decide_leader_from_anchor(
            &mut self,
            anchor: &VerifiedBlockHeader,
            leader_slot: Slot,
        ) -> LeaderStatus {
            let leader_blocks = self
                .dag_state
                .read()
                .get_block_headers_at_slot_above_last_commit(leader_slot);
            if leader_blocks.is_empty() {
                return LeaderStatus::Skip(leader_slot);
            }

            let wave = self.wave_number(leader_slot.round);
            let certifying_round = self.certifying_round(wave);
            let potential_certificates = self
                .dag_state
                .read()
                .reachable_headers_at_round_above_last_commit(anchor, certifying_round);

            let voting_blocks = self
                .dag_state
                .read()
                .get_block_headers_at_round_above_last_commit(leader_slot.round + 1);
            let mut certified_leader_blocks: Vec<(
                VerifiedBlockHeader,
                CommitMetastate,
                Vec<AuthorityIndex>,
            )> = Vec::new();
            for leader_block in leader_blocks {
                let (vote_refs, strong_vote_refs) =
                    self.vote_and_strong_vote_refs_for_leader(&leader_block, &voting_blocks);
                let mut any_cert = false;
                let mut any_strong = false;
                for potential_certificate in &potential_certificates {
                    let (is_cert, is_strong) = self.classify_certificate_cached(
                        potential_certificate,
                        &leader_block,
                        &vote_refs,
                        &strong_vote_refs,
                    );
                    any_cert |= is_cert;
                    any_strong |= is_strong;
                    if any_cert && any_strong {
                        break;
                    }
                }
                if !any_cert {
                    continue;
                }
                let (metastate, strong_voters) = if any_strong {
                    (
                        CommitMetastate::Optimistic,
                        strong_vote_refs.iter().map(|r| r.author).collect(),
                    )
                } else {
                    (CommitMetastate::Standard, Vec::new())
                };
                certified_leader_blocks.push((leader_block, metastate, strong_voters));
            }

            if certified_leader_blocks.len() > 1 {
                panic!("More than one certified block at wave {wave} from leader {leader_slot}")
            }

            match certified_leader_blocks.pop() {
                Some((certified_leader_block, metastate, strong_voters)) => {
                    LeaderStatus::Commit(certified_leader_block, metastate, strong_voters)
                }
                None => LeaderStatus::Skip(leader_slot),
            }
        }

        fn enough_leader_blame(
            &self,
            voting_blocks: &[VerifiedBlockHeader],
            leader: AuthorityIndex,
        ) -> bool {
            let mut blame_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            for voting_block in voting_blocks {
                let voter = voting_block.reference().author;
                if voting_block
                    .ancestors()
                    .iter()
                    .all(|ancestor| ancestor.author != leader)
                    && blame_stake_aggregator.add(voter, &self.context.committee)
                {
                    return true;
                }
            }
            false
        }

        fn direct_decide_leader_block(
            &mut self,
            leader_block: &VerifiedBlockHeader,
            voting_blocks: &[VerifiedBlockHeader],
            decision_blocks: &[VerifiedBlockHeader],
        ) -> Option<(CommitMetastate, Vec<AuthorityIndex>)> {
            let (vote_refs, strong_vote_refs) =
                self.vote_and_strong_vote_refs_for_leader(leader_block, voting_blocks);

            let mut certificate_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            let mut strong_qc_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            let mut enough_support = false;
            let mut strong_qc_quorum = false;
            for decision_block in decision_blocks {
                let (is_cert, is_strong) = self.classify_certificate_cached(
                    decision_block,
                    leader_block,
                    &vote_refs,
                    &strong_vote_refs,
                );
                let authority = decision_block.reference().author;
                if is_cert && certificate_stake_aggregator.add(authority, &self.context.committee) {
                    enough_support = true;
                }
                if is_strong && strong_qc_stake_aggregator.add(authority, &self.context.committee) {
                    strong_qc_quorum = true;
                }
                if enough_support && strong_qc_quorum {
                    break;
                }
            }

            if !enough_support {
                return None;
            }
            if strong_qc_quorum {
                let strong_voters = strong_vote_refs.iter().map(|r| r.author).collect();
                return Some((CommitMetastate::Optimistic, strong_voters));
            }
            if self.has_strong_blame_quorum(leader_block, voting_blocks) {
                return Some((CommitMetastate::Standard, Vec::new()));
            }
            Some((CommitMetastate::Pending, Vec::new()))
        }

        fn vote_and_strong_vote_refs_for_leader(
            &self,
            leader_block: &VerifiedBlockHeader,
            voting_blocks: &[VerifiedBlockHeader],
        ) -> (HashSet<BlockRef>, HashSet<BlockRef>) {
            let mut vote_refs = HashSet::new();
            let mut strong_vote_refs = HashSet::new();
            for voter in voting_blocks {
                if Self::is_vote(voter, leader_block) {
                    let r = voter.reference();
                    vote_refs.insert(r);
                    if voter.is_strong_vote_for(leader_block.author()) {
                        strong_vote_refs.insert(r);
                    }
                }
            }
            (vote_refs, strong_vote_refs)
        }

        fn classify_certificate_cached(
            &mut self,
            potential_certificate: &VerifiedBlockHeader,
            leader_block: &VerifiedBlockHeader,
            vote_refs: &HashSet<BlockRef>,
            strong_vote_refs: &HashSet<BlockRef>,
        ) -> (bool, bool) {
            let key = (potential_certificate.reference(), leader_block.reference());
            match self.certificates.entry(key.0.round).or_default().entry(key) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => *entry.insert(Self::classify_certificate(
                    &self.context.committee,
                    potential_certificate,
                    vote_refs,
                    strong_vote_refs,
                )),
            }
        }

        fn classify_certificate(
            committee: &Committee,
            potential_certificate: &VerifiedBlockHeader,
            vote_refs: &HashSet<BlockRef>,
            strong_vote_refs: &HashSet<BlockRef>,
        ) -> (bool, bool) {
            let mut vote_agg = StakeAggregator::<QuorumThreshold>::new();
            let mut strong_agg = StakeAggregator::<QuorumThreshold>::new();
            let mut is_cert = false;
            let mut is_strong = false;
            for reference in potential_certificate.ancestors() {
                if !is_cert
                    && vote_refs.contains(reference)
                    && vote_agg.add(reference.author, committee)
                {
                    is_cert = true;
                }
                if !is_strong
                    && strong_vote_refs.contains(reference)
                    && strong_agg.add(reference.author, committee)
                {
                    is_strong = true;
                }
                if is_cert && is_strong {
                    break;
                }
            }
            (is_cert, is_strong)
        }

        fn has_strong_blame_quorum(
            &self,
            leader_block: &VerifiedBlockHeader,
            voting_blocks: &[VerifiedBlockHeader],
        ) -> bool {
            let mut strong_blame_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
            for voting_block in voting_blocks {
                if !voting_block.is_strong_blame_for(leader_block.author()) {
                    continue;
                }
                if !Self::is_vote(voting_block, leader_block) {
                    continue;
                }
                if strong_blame_stake_aggregator
                    .add(voting_block.reference().author, &self.context.committee)
                {
                    return true;
                }
            }
            false
        }
    }
}
