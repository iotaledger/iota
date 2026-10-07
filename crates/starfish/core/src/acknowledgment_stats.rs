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

    /// Replaces the samples of `voter` acknowledging `author` with
    /// `ACK_STATS_MIN_SAMPLES` samples at `depth`.
    #[cfg(test)]
    pub(crate) fn set_acknowledgment_depth(
        &mut self,
        voter: AuthorityIndex,
        author: AuthorityIndex,
        depth: Round,
    ) {
        let mut counts = [0; ACK_DEPTH_BUCKETS];
        counts[depth as usize - 1] = ACK_STATS_MIN_SAMPLES as u16;
        self.depth_counts[voter][author] = counts;
    }

    /// Number of samples of `voter` acknowledging `author`.
    #[cfg(test)]
    pub(crate) fn samples(&self, voter: AuthorityIndex, author: AuthorityIndex) -> u32 {
        self.depth_counts[voter][author]
            .iter()
            .map(|&count| u32::from(count))
            .sum()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use starfish_config::Stake;

    use super::*;
    use crate::block_header::TestBlockHeader;

    /// 4-authority context with StarfishSpeed on and gc depth 20; adaptive
    /// acknowledgments are on by default.
    fn adaptive_ack_context() -> Arc<Context> {
        let (mut context, _) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_starfish_speed_for_testing(true);
        context.protocol_config.set_gc_depth_for_testing(20);
        Arc::new(context)
    }

    /// Context like `adaptive_ack_context` on a 1000-stake committee where
    /// voter 1 holds `voter_stake`, voter 2 holds 3 and author 3 holds 300.
    fn weighted_context(voter_stake: Stake) -> Arc<Context> {
        let stakes = vec![1000 - 300 - 3 - voter_stake, voter_stake, 3, 300];
        let (context, _) = Context::new_for_test(4);
        let mut context =
            context.with_committee(starfish_config::local_committee_and_keys(0, stakes).0);
        context
            .protocol_config
            .set_consensus_starfish_speed_for_testing(true);
        context.protocol_config.set_gc_depth_for_testing(20);
        Arc::new(context)
    }

    fn block_ref(round: Round, author: u8) -> BlockRef {
        BlockRef::new(round, author.into(), BlockHeaderDigest::MIN)
    }

    /// Header of `author` at `round` that follows its own block of the previous
    /// round, also references `parents` of the previous round, and
    /// acknowledges `acknowledgments`.
    fn header_with_acknowledgments(
        round: Round,
        author: u8,
        parents: &[u8],
        acknowledgments: Vec<BlockRef>,
    ) -> VerifiedBlockHeader {
        let ancestors = std::iter::once(author)
            .chain(parents.iter().copied().filter(|&parent| parent != author))
            .map(|parent| block_ref(round - 1, parent))
            .collect();
        VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(round, author)
                .set_ancestors(ancestors)
                .set_acknowledgments(acknowledgments)
                .build(),
        )
    }

    /// The refs our leader block at round 10 defers from `pending`.
    fn deferred_at_round_10(
        stats: &AcknowledgmentStats,
        context: &Context,
        pending: Vec<BlockRef>,
    ) -> BTreeSet<BlockRef> {
        stats.acknowledgments_to_defer(context, 10, &pending.into_iter().collect())
    }

    #[tokio::test]
    async fn acknowledgment_stats_record_depths_and_references() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);

        // Voter 1 acknowledges author 2 at depth 2, author 3 at depth 1,
        // author 0 at depth 15 (shares the last bucket) and its own block (not
        // counted).
        let acknowledgments = vec![
            block_ref(5, 0),
            block_ref(18, 2),
            block_ref(19, 3),
            block_ref(19, 1),
        ];
        stats.record_header(
            &context,
            &header_with_acknowledgments(20, 1, &[], acknowledgments),
            false,
        );
        assert_eq!(stats.depth_counts[1][2][1], 1);
        assert_eq!(stats.depth_counts[1][3][0], 1);
        assert_eq!(stats.depth_counts[1][0][ACK_DEPTH_BUCKETS - 1], 1);
        assert_eq!(stats.depth_counts[1][1], [0; ACK_DEPTH_BUCKETS]);

        // A header at the 2n cap and a header after a skipped round add
        // nothing.
        let capped = (10..18).map(|round| block_ref(round, 3)).collect();
        stats.record_header(
            &context,
            &header_with_acknowledgments(20, 2, &[], capped),
            false,
        );
        assert_eq!(stats.depth_counts[2][3], [0; ACK_DEPTH_BUCKETS]);
        let after_skipped_round = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(20, 3)
                .set_ancestors(vec![block_ref(18, 3)])
                .set_acknowledgments(vec![block_ref(19, 1)])
                .build(),
        );
        stats.record_header(&context, &after_skipped_round, false);
        assert_eq!(stats.depth_counts[3][1], [0; ACK_DEPTH_BUCKETS]);

        // Our own block references voters 1 and 2 of the previous round, not
        // 3, and its acknowledgments are not counted.
        let own_header = header_with_acknowledgments(20, 0, &[1, 2], vec![block_ref(19, 3)]);
        stats.record_header(&context, &own_header, false);
        assert_eq!(stats.referenced, vec![1, 1, 1, 0]);
        assert_eq!(stats.own_blocks, 1);
        assert_eq!(stats.depth_counts[0][3], [0; ACK_DEPTH_BUCKETS]);

        // A leader block chooses its acknowledgments, so it adds no samples.
        stats.record_header(
            &context,
            &header_with_acknowledgments(21, 2, &[0], vec![block_ref(20, 3)]),
            true,
        );
        assert_eq!(stats.depth_counts[2][3], [0; ACK_DEPTH_BUCKETS]);
    }

    #[tokio::test]
    async fn acknowledgment_stats_halve_once_per_period() {
        let mut stats = AcknowledgmentStats::new(4);
        stats.depth_counts[1][3] = [8; ACK_DEPTH_BUCKETS];
        stats.referenced[1] = 8;
        stats.own_blocks = 8;
        stats.halve_if_due(ACK_STATS_HALVING_ROUNDS - 1);
        assert_eq!(stats.depth_counts[1][3], [8; ACK_DEPTH_BUCKETS]);
        stats.halve_if_due(ACK_STATS_HALVING_ROUNDS);
        assert_eq!(stats.depth_counts[1][3], [4; ACK_DEPTH_BUCKETS]);
        assert_eq!((stats.referenced[1], stats.own_blocks), (4, 4));
        stats.halve_if_due(2 * ACK_STATS_HALVING_ROUNDS - 1);
        assert_eq!(stats.depth_counts[1][3], [4; ACK_DEPTH_BUCKETS]);
    }

    #[tokio::test]
    async fn acknowledgment_stats_count_ninety_five_percent_as_on_time() {
        let mut stats = AcknowledgmentStats::new(4);
        let voter = AuthorityIndex::from(1u8);
        let author = AuthorityIndex::from(3u8);
        let set = |stats: &mut AcknowledgmentStats, counts: [u16; 4]| {
            let mut all = [0; ACK_DEPTH_BUCKETS];
            all[..4].copy_from_slice(&counts);
            stats.depth_counts[1][3] = all;
        };

        // Too few samples: on time.
        set(&mut stats, [0, 0, 0, 7]);
        assert!(stats.acknowledges_within(voter, author, 1));
        // 19 of 20 samples at depth 1.
        set(&mut stats, [19, 0, 0, 1]);
        assert!(stats.acknowledges_within(voter, author, 1));
        // 19 of 21 samples at depth 1, all within depth 4.
        set(&mut stats, [19, 0, 0, 2]);
        assert!(!stats.acknowledges_within(voter, author, 1));
        assert!(!stats.acknowledges_within(voter, author, 3));
        assert!(stats.acknowledges_within(voter, author, 4));
        assert!(stats.acknowledges_within(voter, author, 60));
        // Samples in the last bucket are never on time, however old the ref.
        let mut late = [0; ACK_DEPTH_BUCKETS];
        late[0] = 19;
        late[ACK_DEPTH_BUCKETS - 1] = 2;
        stats.depth_counts[1][3] = late;
        assert!(!stats.acknowledges_within(voter, author, ACK_DEPTH_BUCKETS as Round));
        assert!(!stats.acknowledges_within(voter, author, 60));
    }

    #[tokio::test]
    async fn leader_defers_refs_most_voters_would_lack() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);
        let pending = vec![
            block_ref(7, 3),
            block_ref(9, 0),
            block_ref(9, 2),
            block_ref(9, 3),
            block_ref(10, 3),
        ];
        // Without history every voter counts as on time.
        assert!(deferred_at_round_10(&stats, &context, pending.clone()).is_empty());

        // Voters 1 and 2 get author 3's data 4 rounds late. Voting at round 11,
        // they lack its round-9 block but hold its round-7 block. Its round-10
        // block is not up for acknowledgment yet.
        stats.set_acknowledgment_depth(1.into(), 3.into(), 4);
        stats.set_acknowledgment_depth(2.into(), 3.into(), 4);
        assert_eq!(
            deferred_at_round_10(&stats, &context, pending),
            BTreeSet::from([block_ref(9, 3)])
        );
    }

    #[tokio::test]
    async fn leader_keeps_one_of_two_refs_lacked_by_different_voters() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);
        let late_digest = BlockRef::new(9, 2.into(), BlockHeaderDigest::MAX);
        stats.set_acknowledgment_depth(1.into(), 2.into(), 4);
        stats.set_acknowledgment_depth(2.into(), 3.into(), 4);
        // Each ref alone leaves 3 of 4 stake; both together leave 2. The tie
        // goes by digest, not by author.
        assert_eq!(
            deferred_at_round_10(&stats, &context, vec![late_digest, block_ref(9, 3)]),
            BTreeSet::from([late_digest])
        );
    }

    #[tokio::test]
    async fn leader_keeps_everything_when_halved_counts_fall_below_a_quorum() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);
        // 16 own blocks referencing [16, 11, 11, 11] halve to 8 and
        // [8, 5, 5, 5]: 23 against a quorum of 3 * 8.
        stats.own_blocks = 8;
        stats.referenced = vec![8, 5, 5, 5];
        stats.set_acknowledgment_depth(1.into(), 2.into(), 4);
        stats.set_acknowledgment_depth(3.into(), 2.into(), 4);
        assert!(deferred_at_round_10(&stats, &context, vec![block_ref(9, 2)]).is_empty());
    }

    #[tokio::test]
    async fn leader_counts_a_voter_lacking_several_refs_once() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);
        stats.set_acknowledgment_depth(1.into(), 2.into(), 4);
        stats.set_acknowledgment_depth(1.into(), 3.into(), 4);
        assert!(
            deferred_at_round_10(&stats, &context, vec![block_ref(9, 2), block_ref(9, 3)])
                .is_empty()
        );
    }

    #[tokio::test]
    async fn leader_weighs_voters_by_how_often_our_blocks_reference_them() {
        let context = adaptive_ack_context();
        let mut stats = AcknowledgmentStats::new(4);
        // Our blocks reference voter 3 half the time: the voters weigh
        // 8 + 8 + 8 + 4 against a quorum of 3 * 8.
        stats.own_blocks = 8;
        stats.referenced = vec![8, 8, 8, 4];

        stats.set_acknowledgment_depth(3.into(), 2.into(), 4);
        assert!(deferred_at_round_10(&stats, &context, vec![block_ref(9, 2)]).is_empty());

        stats.set_acknowledgment_depth(1.into(), 2.into(), 4);
        stats.set_acknowledgment_depth(3.into(), 2.into(), 1);
        assert_eq!(
            deferred_at_round_10(&stats, &context, vec![block_ref(9, 2)]),
            BTreeSet::from([block_ref(9, 2)])
        );
    }

    #[tokio::test]
    async fn leader_keeps_a_quorum_of_stake() {
        // Total stake 1000, quorum threshold 667: voter 1 lacking author 3's
        // block leaves 1000 minus its stake.
        for (voter_stake, deferred) in [(333, false), (334, true)] {
            let context = weighted_context(voter_stake);
            let mut stats = AcknowledgmentStats::new(4);
            stats.set_acknowledgment_depth(1.into(), 3.into(), 4);
            assert_eq!(
                deferred_at_round_10(&stats, &context, vec![block_ref(9, 3)])
                    .contains(&block_ref(9, 3)),
                deferred,
                "voter stake {voter_stake}"
            );
        }
    }
}
