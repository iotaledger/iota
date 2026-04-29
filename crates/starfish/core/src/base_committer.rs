// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, fmt::Display, sync::Arc};

use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, Stake};
use tracing::warn;

use crate::{
    block_header::{BlockHeaderAPI, BlockRef, Round, Slot, VerifiedBlockHeader},
    commit::{CommitMetastate, LeaderStatus, WAVE_LENGTH, WaveNumber},
    context::Context,
    dag_state::DagState,
    leader_schedule::LeaderSchedule,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
};

#[cfg(test)]
#[path = "tests/base_committer_tests.rs"]
mod base_committer_tests;

#[cfg(test)]
#[path = "tests/base_committer_declarative_tests.rs"]
mod base_committer_declarative_tests;

#[cfg(test)]
#[path = "tests/base_committer_metastate_tests.rs"]
mod base_committer_metastate_tests;

#[derive(Default)]
pub(crate) struct BaseCommitterOptions {
    /// The offset used in the leader-election protocol. This is used by the
    /// multi-committer to ensure that each [`BaseCommitter`] instance elects
    /// a different leader.
    pub leader_offset: u32,
    /// The offset of the first wave. This is used by the pipelined committer to
    /// ensure that each[`BaseCommitter`] instances operates on a different
    /// view of the dag.
    pub round_offset: u32,
}

/// The [`BaseCommitter`] contains the bare bone commit logic. Once
/// instantiated, the method `try_direct_decide` and `try_indirect_decide` can
/// be called at any time and any number of times (it is idempotent) to
/// determine whether a leader can be committed or skipped.
pub(crate) struct BaseCommitter {
    /// The per-epoch configuration of this authority.
    context: Arc<Context>,
    /// The consensus leader schedule to be used to resolve the leader for a
    /// given round.
    leader_schedule: Arc<LeaderSchedule>,
    /// In memory block store representing the dag state
    dag_state: Arc<RwLock<DagState>>,
    /// The options used by this committer
    options: BaseCommitterOptions,
}

impl BaseCommitter {
    pub fn new(
        context: Arc<Context>,
        leader_schedule: Arc<LeaderSchedule>,
        dag_state: Arc<RwLock<DagState>>,
        options: BaseCommitterOptions,
    ) -> Self {
        Self {
            context,
            leader_schedule,
            dag_state,
            options,
        }
    }

    /// Apply the direct decision rule to the specified leader to see whether we
    /// can direct-commit or direct-skip it.
    #[tracing::instrument(skip_all, fields(leader = %leader))]
    pub fn try_direct_decide(&self, leader: Slot) -> LeaderStatus {
        // Check whether the leader has enough blame. That is, whether there are 2f+1
        // non-votes for that leader (which ensure there will never be a
        // certificate for that leader).
        let voting_round = leader.round + 1;
        if self.enough_leader_blame(voting_round, leader.authority) {
            return LeaderStatus::Skip(leader);
        }

        // Check whether the leader(s) has enough support. That is, whether there are
        // 2f+1 certificates over the leader. Note that there could be more than
        // one leader block (created by Byzantine leaders).
        let wave = self.wave_number(leader.round);
        let certifying_round = self.certifying_round(wave);
        let leader_blocks = self
            .dag_state
            .read()
            .get_block_headers_at_slot_above_last_commit(leader);
        let mut leaders_with_enough_support: Vec<_> = leader_blocks
            .into_iter()
            .filter(|l| self.enough_leader_support(certifying_round, l))
            .map(|leader_block| {
                let (metastate, strong_voters) = self.determine_metastate_direct(&leader_block);
                LeaderStatus::Commit(leader_block, metastate, strong_voters)
            })
            .collect();

        // There can be at most one leader with enough support for each round, otherwise
        // it means the BFT assumption is broken.
        if leaders_with_enough_support.len() > 1 {
            panic!("[{self}] More than one certified block for {leader}")
        }

        leaders_with_enough_support
            .pop()
            .unwrap_or(LeaderStatus::Undecided(leader))
    }

    /// Apply the indirect rule; return `current` unchanged if no committed
    /// anchor is reachable.
    #[tracing::instrument(skip_all, fields(leader = %current))]
    pub fn try_indirect_decide<'a>(
        &self,
        current: LeaderStatus,
        leaders: impl Iterator<Item = &'a LeaderStatus>,
    ) -> LeaderStatus {
        let slot = current.slot();
        let anchors = leaders.filter(|x| slot.round + WAVE_LENGTH <= x.round());
        for anchor in anchors {
            tracing::trace!("[{self}] Trying to indirect-decide {slot} using anchor {anchor}",);
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

    pub fn elect_leader(&self, round: Round) -> Option<Slot> {
        let wave = self.wave_number(round);
        tracing::trace!(
            "elect_leader: round={}, wave={}, leader_round={}, leader_offset={}",
            round,
            wave,
            self.leader_round(wave),
            self.options.leader_offset
        );
        if self.leader_round(wave) != round {
            return None;
        }

        Some(Slot::new(
            round,
            self.leader_schedule
                .elect_leader(round, self.options.leader_offset),
        ))
    }

    /// Return the leader round of the specified wave. The leader round is
    /// always the first round of the wave. This takes into account round
    /// offset for when pipelining is enabled.
    pub(crate) fn leader_round(&self, wave_number: WaveNumber) -> Round {
        (wave_number * WAVE_LENGTH) + self.options.round_offset
    }

    /// Return the certifying round of the specified wave. The certifying round
    /// is always the last round of the wave. This takes into account round
    /// offset for when pipelining is enabled.
    pub(crate) fn certifying_round(&self, wave_number: WaveNumber) -> Round {
        (wave_number * WAVE_LENGTH) + WAVE_LENGTH - 1 + self.options.round_offset
    }

    /// Return the wave in which the specified round belongs. This takes into
    /// account the round offset for when pipelining is enabled.
    pub(crate) fn wave_number(&self, round: Round) -> WaveNumber {
        round.saturating_sub(self.options.round_offset) / WAVE_LENGTH
    }

    /// Check whether the specified block (`potential_vote`) is a vote for
    /// the specified leader (`leader_block`).
    fn is_vote(
        &self,
        potential_vote: &VerifiedBlockHeader,
        leader_block: &VerifiedBlockHeader,
    ) -> bool {
        potential_vote
            .ancestors()
            .contains(&leader_block.reference())
    }

    /// Return the set of refs of blocks at `leader_block.round() + 1` that
    /// directly include `leader_block` as an ancestor — i.e. the set of votes
    /// for this specific leader block.
    fn vote_refs_for_leader(&self, leader_block: &VerifiedBlockHeader) -> HashSet<BlockRef> {
        let voting_round = leader_block.round() + 1;
        self.dag_state
            .read()
            .get_block_headers_at_round_above_last_commit(voting_round)
            .into_iter()
            .filter(|voting_block| self.is_vote(voting_block, leader_block))
            .map(|voting_block| voting_block.reference())
            .collect()
    }

    /// Check whether `potential_certificate` is a certificate for the leader
    /// whose votes are captured in `vote_refs` — i.e. its ancestors include
    /// a quorum of vote refs.
    fn is_certificate(
        &self,
        potential_certificate: &VerifiedBlockHeader,
        vote_refs: &HashSet<BlockRef>,
    ) -> bool {
        let mut votes_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for reference in potential_certificate.ancestors() {
            if vote_refs.contains(reference)
                && votes_stake_aggregator.add(reference.author, &self.context.committee)
            {
                return true;
            }
        }
        false
    }

    /// Decide the status of a target leader from the specified anchor. We
    /// commit the target leader if it has a certified link to the anchor.
    /// Otherwise, we skip the target leader.
    fn decide_leader_from_anchor(
        &self,
        anchor: &VerifiedBlockHeader,
        leader_slot: Slot,
    ) -> LeaderStatus {
        // Get the block(s) proposed by the leader. There could be more than one leader
        // block in the slot from a Byzantine authority.
        let leader_blocks = self
            .dag_state
            .read()
            .get_block_headers_at_slot_above_last_commit(leader_slot);

        // TODO: Re-evaluate this check once we have a better way to handle/track
        // byzantine authorities.
        if leader_blocks.len() > 1 {
            tracing::warn!(
                "Multiple blocks found for leader slot {leader_slot}: {:?}",
                leader_blocks
            );
        }

        // Get all blocks that could be potential certificates for the target leader.
        // These blocks are in the certifying round of the target leader and are
        // linked to the anchor.
        let wave = self.wave_number(leader_slot.round);
        let certifying_round = self.certifying_round(wave);
        let potential_certificates = self
            .dag_state
            .read()
            .reachable_headers_at_round_above_last_commit(anchor, certifying_round);

        // Use those potential certificates to determine which (if any) of the target
        // leader blocks can be committed, and — when StarfishSpeed is enabled —
        // classify the metastate in the same walk. When the flag is off we
        // stick to the cheap regular-certificate-only check.
        let starfish_speed = self.context.protocol_config.consensus_starfish_speed();
        let mut certified_leader_blocks: Vec<(
            VerifiedBlockHeader,
            Option<CommitMetastate>,
            Vec<AuthorityIndex>,
        )> = Vec::new();
        for leader_block in leader_blocks {
            let (any_cert, metastate, strong_voters) = if starfish_speed {
                let (vote_refs, strong_vote_refs) =
                    self.vote_and_strong_vote_refs_for_leader(&leader_block);
                let mut any_cert = false;
                let mut any_strong = false;
                for potential_certificate in &potential_certificates {
                    let (is_cert, is_strong) = self.classify_certificate(
                        potential_certificate,
                        &vote_refs,
                        &strong_vote_refs,
                    );
                    any_cert |= is_cert;
                    any_strong |= is_strong;
                    if any_cert && any_strong {
                        break;
                    }
                }
                let metastate = if any_cert {
                    Some(if any_strong {
                        CommitMetastate::Optimistic
                    } else {
                        CommitMetastate::Standard
                    })
                } else {
                    None
                };
                let strong_voters = if any_strong {
                    strong_vote_refs.iter().map(|r| r.author).collect()
                } else {
                    Vec::new()
                };
                (any_cert, metastate, strong_voters)
            } else {
                let vote_refs = self.vote_refs_for_leader(&leader_block);
                let any_cert = potential_certificates.iter().any(|potential_certificate| {
                    self.is_certificate(potential_certificate, &vote_refs)
                });
                (any_cert, None, Vec::new())
            };
            if any_cert {
                certified_leader_blocks.push((leader_block, metastate, strong_voters));
            }
        }

        // There can be at most one certified leader, otherwise it means the BFT
        // assumption is broken.
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

    /// Check whether the specified leader has 2f+1 non-votes (blames) to be
    /// directly skipped.
    fn enough_leader_blame(&self, voting_round: Round, leader: AuthorityIndex) -> bool {
        let voting_blocks = self
            .dag_state
            .read()
            .get_block_headers_at_round_above_last_commit(voting_round);

        let mut blame_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for voting_block in &voting_blocks {
            let voter = voting_block.reference().author;
            if voting_block
                .ancestors()
                .iter()
                .all(|ancestor| ancestor.author != leader)
            {
                tracing::trace!(
                    "[{self}] {voting_block} is a blame for leader {}",
                    Slot::new(voting_round - 1, leader)
                );
                if blame_stake_aggregator.add(voter, &self.context.committee) {
                    return true;
                }
            } else {
                tracing::trace!(
                    "[{self}] {voting_block} is not a blame for leader {}",
                    Slot::new(voting_round - 1, leader)
                );
            }
        }
        false
    }

    /// Check whether the specified leader has 2f+1 certificates to be directly
    /// committed.
    fn enough_leader_support(
        &self,
        certifying_round: Round,
        leader_block: &VerifiedBlockHeader,
    ) -> bool {
        let decision_blocks = self
            .dag_state
            .read()
            .get_block_headers_at_round_above_last_commit(certifying_round);

        // Quickly reject if there isn't enough stake to support the leader from
        // the potential certificates.
        let total_stake: Stake = decision_blocks
            .iter()
            .map(|b| self.context.committee.stake(b.author()))
            .sum();
        if !self.context.committee.reached_quorum(total_stake) {
            tracing::debug!(
                "Not enough support for {leader_block}. Stake not enough: {total_stake} < {}",
                self.context.committee.quorum_threshold()
            );
            return false;
        }

        let mut certificate_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        let vote_refs = self.vote_refs_for_leader(leader_block);
        for decision_block in &decision_blocks {
            let authority = decision_block.reference().author;
            if self.is_certificate(decision_block, &vote_refs)
                && certificate_stake_aggregator.add(authority, &self.context.committee)
            {
                return true;
            }
        }
        false
    }

    /// Direct-commit metastate: StrongQC quorum → `Optimistic`, strong-blame
    /// quorum → `Standard`, else `Pending`. `None` when the flag is off.
    /// The returned `Vec<AuthorityIndex>` carries the leader's strong-voter
    /// authorities for the Optimistic case (used by the linearizer to seed
    /// the per-ref ack tracker); empty otherwise.
    fn determine_metastate_direct(
        &self,
        leader_block: &VerifiedBlockHeader,
    ) -> (Option<CommitMetastate>, Vec<AuthorityIndex>) {
        if !self.context.protocol_config.consensus_starfish_speed() {
            return (None, Vec::new());
        }

        if let Some(strong_voters) = self.strong_qc_quorum(leader_block) {
            return (Some(CommitMetastate::Optimistic), strong_voters);
        }

        if self.has_strong_blame_quorum(leader_block) {
            return (Some(CommitMetastate::Standard), Vec::new());
        }

        (Some(CommitMetastate::Pending), Vec::new())
    }

    /// `(vote_refs, strong_vote_refs)` at r+1 for `leader_block`, built in a
    /// single pass. Strong votes are a subset of votes; `is_strong_vote()`
    /// alone is insufficient (a voter may flag itself strong while supporting
    /// an equivocating leader).
    fn vote_and_strong_vote_refs_for_leader(
        &self,
        leader_block: &VerifiedBlockHeader,
    ) -> (HashSet<BlockRef>, HashSet<BlockRef>) {
        let voting_round = leader_block.round() + 1;
        let voters = self
            .dag_state
            .read()
            .get_uncommitted_block_headers_at_round(voting_round);
        let mut vote_refs = HashSet::new();
        let mut strong_vote_refs = HashSet::new();
        for voter in &voters {
            if self.is_vote(voter, leader_block) {
                let r = voter.reference();
                vote_refs.insert(r);
                if voter.is_strong_vote() {
                    strong_vote_refs.insert(r);
                }
            }
        }
        (vote_refs, strong_vote_refs)
    }

    /// `Some(strong-voter authorities)` when 2f+1 StrongQCs exist at `r+2`
    /// for `leader_block`; `None` otherwise.
    fn strong_qc_quorum(&self, leader_block: &VerifiedBlockHeader) -> Option<Vec<AuthorityIndex>> {
        let voting_round = leader_block.round() + 1;
        let certifying_round = leader_block.round() + 2;

        let (voters, decision_blocks) = {
            let dag_state = self.dag_state.read();
            (
                dag_state.get_uncommitted_block_headers_at_round(voting_round),
                dag_state.get_uncommitted_block_headers_at_round(certifying_round),
            )
        };

        let strong_vote_refs: HashSet<BlockRef> = voters
            .into_iter()
            .filter(|b| b.is_strong_vote() && self.is_vote(b, leader_block))
            .map(|b| b.reference())
            .collect();

        let mut strong_qc_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for decision_block in &decision_blocks {
            let authority = decision_block.reference().author;
            if self.is_strong_certificate(decision_block, &strong_vote_refs)
                && strong_qc_stake_aggregator.add(authority, &self.context.committee)
            {
                return Some(strong_vote_refs.iter().map(|r| r.author).collect());
            }
        }
        None
    }

    /// True if `potential_certificate` carries a StrongQC for the leader
    /// whose strong votes are pre-filtered in `strong_vote_refs`.
    fn is_strong_certificate(
        &self,
        potential_certificate: &VerifiedBlockHeader,
        strong_vote_refs: &HashSet<BlockRef>,
    ) -> bool {
        let mut strong_votes_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for reference in potential_certificate.ancestors() {
            if strong_vote_refs.contains(reference)
                && strong_votes_stake_aggregator.add(reference.author, &self.context.committee)
            {
                return true;
            }
        }
        false
    }

    /// Single-walk `(is_certificate, is_strong_certificate)` classifier.
    /// `vote_refs`/`strong_vote_refs` are the pre-computed voter sets.
    /// Invariant: `is_strong ⇒ is_certificate`.
    fn classify_certificate(
        &self,
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
                && vote_agg.add(reference.author, &self.context.committee)
            {
                is_cert = true;
            }
            if !is_strong
                && strong_vote_refs.contains(reference)
                && strong_agg.add(reference.author, &self.context.committee)
            {
                is_strong = true;
            }
            if is_cert && is_strong {
                break;
            }
        }
        (is_cert, is_strong)
    }

    /// Check whether 2f+1 blocks at `r+1` both vote for `leader_block` and
    /// carry `is_strong_blame() == true`.
    fn has_strong_blame_quorum(&self, leader_block: &VerifiedBlockHeader) -> bool {
        let voting_round = leader_block.round() + 1;
        let voting_blocks = self
            .dag_state
            .read()
            .get_uncommitted_block_headers_at_round(voting_round);

        let mut strong_blame_stake_aggregator = StakeAggregator::<QuorumThreshold>::new();
        for voting_block in &voting_blocks {
            if !voting_block.is_strong_blame() {
                continue;
            }
            if !self.is_vote(voting_block, leader_block) {
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

impl Display for BaseCommitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Committer-L{}-R{}",
            self.options.leader_offset, self.options.round_offset
        )
    }
}

/// A builder for the base committer. By default, the builder creates a base
/// committer that has no leader or round offset. Which indicates single leader
/// & pipelining disabled.
#[cfg(test)]
mod base_committer_builder {
    use super::*;
    use crate::leader_schedule::LeaderSwapTable;

    pub(crate) struct BaseCommitterBuilder {
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        leader_offset: u32,
        round_offset: u32,
    }

    impl BaseCommitterBuilder {
        pub(crate) fn new(context: Arc<Context>, dag_state: Arc<RwLock<DagState>>) -> Self {
            Self {
                context,
                dag_state,
                leader_offset: 0,
                round_offset: 0,
            }
        }

        #[expect(unused)]
        pub(crate) fn with_leader_offset(mut self, leader_offset: u32) -> Self {
            self.leader_offset = leader_offset;
            self
        }

        #[expect(unused)]
        pub(crate) fn with_round_offset(mut self, round_offset: u32) -> Self {
            self.round_offset = round_offset;
            self
        }

        pub(crate) fn build(self) -> BaseCommitter {
            let options = BaseCommitterOptions {
                leader_offset: self.leader_offset,
                round_offset: self.round_offset,
            };
            BaseCommitter::new(
                self.context.clone(),
                Arc::new(LeaderSchedule::new(
                    self.context,
                    LeaderSwapTable::default(),
                )),
                self.dag_state,
                options,
            )
        }
    }
}
