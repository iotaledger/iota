// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, fmt::Display, sync::Arc};

use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, Stake};
use tracing::warn;

use crate::{
    block_header::{BlockHeaderAPI, BlockRef, Round, Slot, VerifiedBlockHeader},
    commit::{LeaderStatus, WAVE_LENGTH, WaveNumber},
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
            .get_uncommitted_block_headers_at_slot(leader);
        let mut leaders_with_enough_support: Vec<_> = leader_blocks
            .into_iter()
            .filter(|l| self.enough_leader_support(certifying_round, l))
            .map(LeaderStatus::Commit)
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

    /// Apply the indirect decision rule to the specified leader to see whether
    /// we can indirect-commit or indirect-skip it.
    #[tracing::instrument(skip_all, fields(leader = %leader_slot))]
    pub fn try_indirect_decide<'a>(
        &self,
        leader_slot: Slot,
        leaders: impl Iterator<Item = &'a LeaderStatus>,
    ) -> LeaderStatus {
        // The anchor is the first committed leader with round higher than the decision
        // round of the target leader. We must stop the iteration upon
        // encountering an undecided leader.
        let anchors = leaders.filter(|x| leader_slot.round + WAVE_LENGTH <= x.round());

        for anchor in anchors {
            tracing::trace!(
                "[{self}] Trying to indirect-decide {leader_slot} using anchor {anchor}",
            );
            match anchor {
                LeaderStatus::Commit(anchor) => {
                    return self.decide_leader_from_anchor(anchor, leader_slot);
                }
                LeaderStatus::Skip(..) => (),
                LeaderStatus::Undecided(..) => break,
            }
        }

        LeaderStatus::Undecided(leader_slot)
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
            .get_uncommitted_block_headers_at_round(voting_round)
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
            .get_uncommitted_block_headers_at_slot(leader_slot);

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
            .ancestors_at_round(anchor, certifying_round);

        // Use those potential certificates to determine which (if any) of the target
        // leader blocks can be committed.
        let mut certified_leader_blocks: Vec<_> = leader_blocks
            .into_iter()
            .filter(|leader_block| {
                let vote_refs = self.vote_refs_for_leader(leader_block);
                potential_certificates.iter().any(|potential_certificate| {
                    self.is_certificate(potential_certificate, &vote_refs)
                })
            })
            .collect();

        // There can be at most one certified leader, otherwise it means the BFT
        // assumption is broken.
        if certified_leader_blocks.len() > 1 {
            panic!("More than one certified block at wave {wave} from leader {leader_slot}")
        }

        // We commit the target leader if it has a certificate that is an ancestor of
        // the anchor. Otherwise skip it.
        match certified_leader_blocks.pop() {
            Some(certified_leader_block) => LeaderStatus::Commit(certified_leader_block),
            None => LeaderStatus::Skip(leader_slot),
        }
    }

    /// Check whether the specified leader has 2f+1 non-votes (blames) to be
    /// directly skipped.
    fn enough_leader_blame(&self, voting_round: Round, leader: AuthorityIndex) -> bool {
        let voting_blocks = self
            .dag_state
            .read()
            .get_uncommitted_block_headers_at_round(voting_round);

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
            .get_uncommitted_block_headers_at_round(certifying_round);

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
