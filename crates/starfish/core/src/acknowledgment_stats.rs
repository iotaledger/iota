// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
/// blocks reference each voter's block of the previous round. Chooses the
/// acknowledgments of our leader blocks.
pub(crate) struct AcknowledgmentStats {
    /// Acknowledgment depth counts, indexed by voter and author. Bucket `i`
    /// counts depth `i + 1`.
    depths: Vec<Vec<[u16; ACK_DEPTH_BUCKETS]>>,
    /// Per voter: our blocks that referenced its block of the previous round.
    referenced: Vec<u16>,
    /// Our blocks counted in `referenced`.
    own_blocks: u16,
    halved_at_round: Round,
}

impl AcknowledgmentStats {
    pub(crate) fn new(committee_size: usize) -> Self {
        Self {
            depths: vec![vec![[0; ACK_DEPTH_BUCKETS]; committee_size]; committee_size],
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
        self.depths
            .iter_mut()
            .flatten()
            .flatten()
            .chain(&mut self.referenced)
            .for_each(|count| *count /= 2);
        self.own_blocks /= 2;
        self.halved_at_round = clock_round;
    }

    /// Records an accepted `block_header`. Other authors' leader blocks choose
    /// their acknowledgments, so they add no samples.
    pub(crate) fn record(
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
            let count = &mut self.depths[voter][ack.author][depth as usize - 1];
            *count = count.saturating_add(1);
        }
    }

    /// Whether `voter` usually acknowledges `author`'s blocks within `depth`
    /// rounds. Samples in the last bucket count against it at every depth.
    fn is_on_time(&self, voter: AuthorityIndex, author: AuthorityIndex, depth: Round) -> bool {
        let counts = &self.depths[voter][author];
        let total = counts.iter().map(|&count| u32::from(count)).sum();
        // The last bucket has no upper depth, so age alone must not make its
        // samples count as on time.
        let within = counts
            .iter()
            .take((depth as usize).min(ACK_DEPTH_BUCKETS - 1))
            .map(|&count| u32::from(count))
            .sum();
        usually(within, total)
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

    /// Chooses which of the `pending` acknowledgments our leader block at
    /// `clock_round` leaves out for a later block.
    pub(crate) fn acknowledgments_to_defer(
        &self,
        context: &Context,
        clock_round: Round,
        pending: &BTreeSet<BlockRef>,
    ) -> BTreeSet<BlockRef> {
        // A vote counts in a strong certificate only when the next round's
        // blocks reference it.
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
                    voter == ack.author || self.is_on_time(voter, ack.author, depth)
                });
                (total - weight_of(&holders), *ack, holders)
            })
            .collect();
        // Keep refs, those the least weight would lack first, while the voters
        // expected to hold everything kept weigh a quorum. Ties go by digest,
        // so no author is always the one left out.
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

/// Whether `hits` make up the on-time share of `total`, counting too few
/// samples as on time.
fn usually(hits: u32, total: u32) -> bool {
    total < ACK_STATS_MIN_SAMPLES || hits * 100 >= total * ACK_STATS_ON_TIME_PERCENT
}
