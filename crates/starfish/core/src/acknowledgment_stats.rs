// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Which pending acknowledgments our leader block carries.
//!
//! Our leader block at round `t` commits optimistically only if a quorum of
//! stake strong-votes for it at round `t + 1` and a quorum of round `t + 2`
//! blocks references those votes. A voter strong-votes only if it already
//! holds the transactions of our block and of every block we acknowledge;
//! otherwise it blames the authors it lacks. Its vote counts only if the next
//! round's blocks reference it, which they do when its block reaches them
//! within a round.
//!
//! So the leader block keeps an acknowledgment only while the voters expected
//! to hold everything kept still weigh a quorum. A voter weighs its stake
//! times how often our own blocks referenced its previous-round block, and is
//! expected to hold a block if it acknowledged that author's recent blocks
//! soon enough nearly every time. Both counts come from the headers this node
//! accepts. What is left out waits for our next block, which is not a leader
//! block and acknowledges everything.

use std::collections::BTreeSet;

use starfish_config::AuthorityIndex;

use crate::{
    authority_set::AuthoritySet,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, GENESIS_ROUND, Round, VerifiedBlockHeader,
    },
    context::Context,
};

/// Acknowledgment depths counted separately; deeper ones share the last
/// bucket.
const ACK_DEPTH_BUCKETS: usize = 12;
/// Clock rounds between halvings of the statistics.
const ACK_STATS_HALVING_ROUNDS: Round = 400;
/// Below this many samples a voter counts as on time.
const ACK_STATS_MIN_SAMPLES: u32 = 8;
/// Share of samples, in percent, that must be on time.
const ACK_STATS_ON_TIME_PERCENT: u32 = 95;

/// How soon each voter acknowledges each author's blocks, and how often our
/// blocks reference each voter's block of the previous round.
pub(crate) struct AcknowledgmentStats {
    /// Per voter and author: how many of the voter's acknowledgments of the
    /// author's blocks came `i + 1` rounds after the block, in bucket `i`.
    depth_counts: Vec<Vec<[u16; ACK_DEPTH_BUCKETS]>>,
    /// Per voter: our blocks that referenced its block of the previous round.
    referenced: Vec<u16>,
    /// Our blocks counted in `referenced`.
    own_blocks: u16,
    halved_at_round: Round,
}

impl AcknowledgmentStats {
    pub(crate) fn new(committee_size: usize) -> Self {
        Self {
            depth_counts: vec![vec![[0; ACK_DEPTH_BUCKETS]; committee_size]; committee_size],
            referenced: vec![0; committee_size],
            own_blocks: 0,
            halved_at_round: GENESIS_ROUND,
        }
    }

    /// Halves every count once per `ACK_STATS_HALVING_ROUNDS` clock rounds.
    pub(crate) fn halve_if_due(&mut self, clock_round: Round) {
        if clock_round < self.halved_at_round + ACK_STATS_HALVING_ROUNDS {
            return;
        }
        self.depth_counts
            .iter_mut()
            .flatten()
            .flatten()
            .chain(&mut self.referenced)
            .for_each(|count| *count /= 2);
        self.own_blocks /= 2;
        self.halved_at_round = clock_round;
    }

    /// Counts an accepted `block_header`: our own block for the voters it
    /// references, another voter's block for how soon it acknowledges each
    /// author. Other authors' leader blocks choose their acknowledgments, so
    /// they add no samples.
    pub(crate) fn record_header(
        &mut self,
        context: &Context,
        block_header: &VerifiedBlockHeader,
        leader_block: bool,
    ) {
        let voter = block_header.author();
        let round = block_header.round();
        if voter == context.own_index {
            self.own_blocks = self.own_blocks.saturating_add(1);
            for ancestor in block_header.ancestors() {
                if ancestor.round + 1 == round {
                    let count = &mut self.referenced[ancestor.author];
                    *count = count.saturating_add(1);
                }
            }
            return;
        }
        // A block after a skipped round, or at the cap, acknowledges some refs
        // later than their data arrived.
        let acknowledgments = block_header.acknowledgments();
        let max_acknowledgments = context
            .protocol_config
            .max_acknowledgments_per_block(context.committee.size());
        let follows_previous_round = block_header
            .ancestors()
            .first()
            .is_some_and(|own_previous| own_previous.round + 1 == round);
        if leader_block || !follows_previous_round || acknowledgments.len() >= max_acknowledgments {
            return;
        }
        for ack in acknowledgments.iter().filter(|ack| ack.author != voter) {
            let depth = round
                .saturating_sub(ack.round)
                .clamp(1, ACK_DEPTH_BUCKETS as Round);
            let count = &mut self.depth_counts[voter][ack.author][depth as usize - 1];
            *count = count.saturating_add(1);
        }
    }

    /// Whether at least `ACK_STATS_ON_TIME_PERCENT` of `voter`'s
    /// acknowledgments of `author`'s blocks came within `depth` rounds, or
    /// there are fewer than `ACK_STATS_MIN_SAMPLES` of them. Samples in the
    /// last bucket count against it at every depth.
    fn acknowledges_within(
        &self,
        voter: AuthorityIndex,
        author: AuthorityIndex,
        depth: Round,
    ) -> bool {
        let counts = &self.depth_counts[voter][author];
        let total: u32 = counts.iter().map(|&count| u32::from(count)).sum();
        // The last bucket has no upper depth, so age alone must not make its
        // samples count as on time.
        let within: u32 = counts
            .iter()
            .take((depth as usize).min(ACK_DEPTH_BUCKETS - 1))
            .map(|&count| u32::from(count))
            .sum();
        total < ACK_STATS_MIN_SAMPLES || within * 100 >= total * ACK_STATS_ON_TIME_PERCENT
    }

    /// Per voter, its stake times the number of our blocks that referenced its
    /// block of the previous round, and the weight of a quorum of stake in the
    /// same units. With too few samples every voter counts as referenced.
    fn voter_weights(&self, context: &Context) -> (Vec<u64>, u64) {
        let own_blocks = u64::from(self.own_blocks).max(1);
        let few_samples = u32::from(self.own_blocks) < ACK_STATS_MIN_SAMPLES;
        let weights = context
            .committee
            .authorities()
            .map(|(voter, authority)| {
                let referenced = if few_samples || voter == context.own_index {
                    own_blocks
                } else {
                    u64::from(self.referenced[voter])
                };
                authority.stake * referenced
            })
            .collect();
        (weights, context.committee.quorum_threshold() * own_blocks)
    }

    /// The `pending` acknowledgments our leader block at `clock_round` leaves
    /// out: as few as possible, while the voters expected to hold everything
    /// it keeps still weigh a quorum.
    pub(crate) fn acknowledgments_to_defer(
        &self,
        context: &Context,
        clock_round: Round,
        pending: &BTreeSet<BlockRef>,
    ) -> BTreeSet<BlockRef> {
        let (weights, quorum) = self.voter_weights(context);
        let weight_of =
            |voters: &AuthoritySet| -> u64 { voters.iter().map(|voter| weights[voter]).sum() };
        let holders = |holds: &dyn Fn(AuthorityIndex) -> bool| {
            let mut holders = AuthoritySet::new();
            for (voter, _) in context.committee.authorities() {
                if voter == context.own_index || holds(voter) {
                    holders.insert(voter);
                }
            }
            holders
        };
        let mut voters = holders(&|_| true);
        let total = weight_of(&voters);
        let mut deferred = BTreeSet::new();
        // Each of our blocks references a quorum of the previous round, so
        // this fails only just after the counts are halved.
        if total < quorum {
            return deferred;
        }
        let min_round = clock_round.saturating_sub(context.protocol_config.gc_depth());
        let mut candidates: Vec<(u64, BlockRef, AuthoritySet)> = pending
            .range(
                BlockRef::new(min_round, AuthorityIndex::ZERO, BlockHeaderDigest::MIN)
                    ..BlockRef::new(clock_round, AuthorityIndex::ZERO, BlockHeaderDigest::MIN),
            )
            .map(|ack| {
                let depth = clock_round + 1 - ack.round;
                let holders = holders(&|voter| {
                    voter == ack.author || self.acknowledges_within(voter, ack.author, depth)
                });
                (total - weight_of(&holders), *ack, holders)
            })
            .collect();
        // Refs that the least weight would lack are kept first. Ties go by
        // digest, so no author is always the one left out.
        candidates
            .sort_unstable_by_key(|(lost, ack, _)| (*lost, ack.round, ack.digest, ack.author));
        for (_, ack, holders) in candidates {
            let kept = voters.intersection(&holders);
            if weight_of(&kept) >= quorum {
                voters = kept;
            } else {
                deferred.insert(ack);
            }
        }
        deferred
    }
}
