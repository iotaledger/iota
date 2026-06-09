// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::Ordering, sync::Arc};

use tokio::time::Instant;

use crate::{
    block_header::{BlockRef, Round},
    context::Context,
    stake_aggregator::{QuorumThreshold, StakeAggregator},
};

pub(crate) struct ThresholdClock {
    aggregator: StakeAggregator<QuorumThreshold>,
    round: Round,
    quorum_ts: Instant,
    context: Arc<Context>,
}

impl ThresholdClock {
    pub(crate) fn new(round: Round, context: Arc<Context>) -> Self {
        Self {
            aggregator: StakeAggregator::new(),
            round,
            quorum_ts: Instant::now(),
            context,
        }
    }

    /// If quorum (2f+1) is reached, advance to the next round and record
    /// latency metrics. Returns true if quorum was reached.
    fn try_advance_round(&mut self, new_round: Round) -> bool {
        if self.aggregator.reached_threshold(&self.context.committee) {
            self.aggregator.clear();
            self.round = new_round;

            let now = Instant::now();
            self.context
                .metrics
                .node_metrics
                .quorum_receive_latency
                .observe(now.duration_since(self.quorum_ts).as_secs_f64());
            self.quorum_ts = now;
            true
        } else {
            false
        }
    }

    /// Add the block reference and advance the round accordingly.
    ///
    /// Round advancement rules:
    /// - block.round < current: ignored (stale block)
    /// - block.round > current: jump to block.round, start collecting stake
    ///   there
    /// - block.round == current: continue accumulating stake until quorum
    ///   (2f+1) reached
    ///
    /// When quorum is reached, advance to round + 1.
    pub(crate) fn add_block_header(&mut self, block_header: BlockRef) {
        match block_header.round.cmp(&self.round) {
            Ordering::Less => {}
            Ordering::Greater => {
                // Jump to the new round and start with fresh state
                self.aggregator.clear();
                self.aggregator
                    .add(block_header.author, &self.context.committee);
                self.round = block_header.round;
            }
            Ordering::Equal => {
                self.aggregator
                    .add(block_header.author, &self.context.committee);
            }
        }
        self.try_advance_round(block_header.round + 1);
    }

    /// Add the block references that have been successfully processed and
    /// advance the round accordingly. If the round has indeed advanced then
    /// the new round is returned, otherwise None is returned.
    #[cfg(test)]
    fn add_blocks(&mut self, blocks: Vec<BlockRef>) -> Option<Round> {
        let previous_round = self.round;
        for block_ref in blocks {
            self.add_block_header(block_ref);
        }
        (self.round > previous_round).then_some(self.round)
    }

    pub(crate) fn get_round(&self) -> Round {
        self.round
    }

    pub(crate) fn get_quorum_ts(&self) -> Instant {
        self.quorum_ts
    }
}

#[cfg(test)]
mod tests {
    use starfish_config::AuthorityIndex;

    use super::*;
    use crate::block_header::BlockHeaderDigest;

    #[tokio::test]
    async fn test_threshold_clock_add_block() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut aggregator = ThresholdClock::new(0, context);

        aggregator.add_block_header(BlockRef::new(
            0,
            AuthorityIndex::new_for_test(0),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 0);
        aggregator.add_block_header(BlockRef::new(
            0,
            AuthorityIndex::new_for_test(1),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 0);
        aggregator.add_block_header(BlockRef::new(
            0,
            AuthorityIndex::new_for_test(2),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 1);
        aggregator.add_block_header(BlockRef::new(
            1,
            AuthorityIndex::new_for_test(0),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 1);
        aggregator.add_block_header(BlockRef::new(
            1,
            AuthorityIndex::new_for_test(3),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 1);
        aggregator.add_block_header(BlockRef::new(
            2,
            AuthorityIndex::new_for_test(1),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 2);
        aggregator.add_block_header(BlockRef::new(
            1,
            AuthorityIndex::new_for_test(1),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 2);
        aggregator.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(2),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(aggregator.get_round(), 5);
    }

    #[tokio::test]
    async fn test_threshold_clock_add_blocks() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut aggregator = ThresholdClock::new(0, context);

        let block_refs = vec![
            BlockRef::new(
                0,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                0,
                AuthorityIndex::new_for_test(1),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                0,
                AuthorityIndex::new_for_test(2),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                1,
                AuthorityIndex::new_for_test(0),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                1,
                AuthorityIndex::new_for_test(3),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                2,
                AuthorityIndex::new_for_test(1),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                1,
                AuthorityIndex::new_for_test(1),
                BlockHeaderDigest::default(),
            ),
            BlockRef::new(
                5,
                AuthorityIndex::new_for_test(2),
                BlockHeaderDigest::default(),
            ),
        ];

        let result = aggregator.add_blocks(block_refs);
        assert_eq!(Some(5), result);
    }

    /// Test that when jumping to a higher round, the first block's author is
    /// tracked, allowing subsequent blocks to form quorum.
    #[tokio::test]
    async fn test_threshold_clock_round_skip_then_quorum() {
        let context = Arc::new(Context::new_for_test(4).0);
        let mut clock = ThresholdClock::new(0, context);

        // Jump from round 0 to round 5 - author should be tracked
        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(0),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 5);

        // Add more blocks at round 5 to reach quorum (need 3 of 4)
        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(1),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 5);

        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(2),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 6); // Quorum reached
    }

    /// Test that a super-majority authority (>2/3 stake) immediately advances
    /// the round when jumping to a higher round.
    #[tokio::test]
    async fn test_threshold_clock_super_majority_round_skip() {
        use starfish_config::Parameters;
        use tempfile::TempDir;

        use crate::metrics::test_metrics;

        // Authority 0 has 5/7 stake (>2/3 quorum threshold)
        let (committee, _) = starfish_config::local_committee_and_keys(0, vec![5, 1, 1]);
        let metrics = test_metrics();
        let temp_dir = TempDir::new().unwrap();

        let context = Arc::new(crate::context::Context::new(
            0,
            AuthorityIndex::new_for_test(0),
            committee,
            Parameters {
                db_path: temp_dir.keep(),
                ..Default::default()
            },
            iota_protocol_config::ProtocolConfig::get_for_max_version_UNSAFE(),
            metrics,
            Arc::new(crate::context::Clock::default()),
        ));

        let mut clock = ThresholdClock::new(0, context);

        // Single block from super-majority authority at round 5 reaches quorum
        // immediately
        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(0),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 6);

        // Stale blocks from round 5 should be ignored
        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(1),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 6);
        clock.add_block_header(BlockRef::new(
            5,
            AuthorityIndex::new_for_test(2),
            BlockHeaderDigest::default(),
        ));
        assert_eq!(clock.get_round(), 6);
    }
}
