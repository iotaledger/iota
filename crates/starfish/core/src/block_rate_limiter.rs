// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

/// Rate limiter for own block proposals (GCRA / token-bucket): sustained
/// production of at most one block per `min_block_delay`, with bursts of up to
/// `block_rate_window / min_block_delay` back-to-back blocks after idle
/// periods. A burst capacity of 1 degenerates to a fixed minimum delay
/// between consecutive blocks.
///
/// State is a single instant: the time at which an ideal schedule emitting
/// exactly one block per interval would emit the next block. Idle time lets it
/// fall back toward `now` (accruing budget); each proposal advances it by one
/// interval.
pub(crate) struct BlockRateLimiter {
    /// UTC ms at which the next block would be emitted under an ideal schedule
    /// of one block per `interval_ms` (the GCRA theoretical arrival time). It
    /// runs ahead of `now` after recent proposals and falls back toward `now`
    /// while idle; the gap below `now` is the unspent burst budget.
    next_block_ms: u64,
    /// Sustained spacing between blocks (`min_block_delay`), in ms.
    interval_ms: u64,
    /// Maximum number of back-to-back blocks allowed after an idle period.
    burst: u64,
}

impl BlockRateLimiter {
    pub(crate) fn new(min_block_delay: Duration, burst: u64) -> Self {
        Self {
            next_block_ms: 0,
            interval_ms: min_block_delay.as_millis().max(1) as u64,
            burst: burst.max(1),
        }
    }

    /// Whether a proposal at `now_ms` fits the rate envelope.
    pub(crate) fn is_conforming(&self, now_ms: u64) -> bool {
        self.next_block_ms.saturating_sub(now_ms) <= (self.burst - 1) * self.interval_ms
    }

    /// Records a proposal at `now_ms`. Called for every own block, including
    /// forced proposals that bypass the conformance check; the cap bounds
    /// their overdraft so the next non-forced proposal waits at most one
    /// interval after the last block.
    pub(crate) fn record(&mut self, now_ms: u64) {
        self.next_block_ms = (self.next_block_ms.max(now_ms) + self.interval_ms)
            .min(now_ms + self.burst * self.interval_ms);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use starfish_config::Parameters;

    use crate::block_rate_limiter::BlockRateLimiter;

    /// Drive the tests from the default consensus parameters rather than magic
    /// numbers, so they track the production config (interval and burst).
    fn interval_ms_and_burst() -> (u64, u64) {
        let params = Parameters::default();
        (
            params.min_block_delay.as_millis() as u64,
            params.block_rate_burst(),
        )
    }

    fn limiter() -> BlockRateLimiter {
        let params = Parameters::default();
        BlockRateLimiter::new(params.min_block_delay, params.block_rate_burst())
    }

    #[test]
    fn burst_after_idle_then_reject() {
        let (interval_ms, burst) = interval_ms_and_burst();
        let mut l = limiter();
        let now = 1_000_000;
        // Exactly `burst` back-to-back proposals conform, the next does not.
        for _ in 0..burst {
            assert!(l.is_conforming(now));
            l.record(now);
        }
        assert!(!l.is_conforming(now));
        // Budget for one more block regenerates after one interval.
        assert!(!l.is_conforming(now + interval_ms - 1));
        assert!(l.is_conforming(now + interval_ms));
    }

    #[test]
    fn sustained_rate_is_one_per_interval() {
        let (interval_ms, burst) = interval_ms_and_burst();
        let mut l = limiter();
        let start = 1_000_000;
        // Drain the burst budget.
        for _ in 0..burst {
            l.record(start);
        }
        // Under continuous attempts, conforming instants are spaced exactly
        // one interval apart.
        let mut now = start;
        for _ in 0..10 {
            assert!(!l.is_conforming(now + interval_ms - 1));
            now += interval_ms;
            assert!(l.is_conforming(now));
            l.record(now);
        }
    }

    #[test]
    fn burst_one_degenerates_to_fixed_delay() {
        let (interval_ms, _) = interval_ms_and_burst();
        let mut l = BlockRateLimiter::new(Duration::from_millis(interval_ms), 1);
        let now = 1_000_000;
        assert!(l.is_conforming(now));
        l.record(now);
        // Identical to the old rule: blocked until exactly one interval elapses.
        assert!(!l.is_conforming(now));
        assert!(!l.is_conforming(now + interval_ms - 1));
        assert!(l.is_conforming(now + interval_ms));
    }

    #[test]
    fn forced_overdraft_is_capped() {
        let (interval_ms, burst) = interval_ms_and_burst();
        let mut l = limiter();
        let now = 1_000_000;
        // Forced proposals record without a conformance check; the cap keeps
        // the next non-forced eligibility within one interval of the last.
        for _ in 0..(3 * burst) {
            l.record(now);
        }
        assert!(!l.is_conforming(now));
        assert!(l.is_conforming(now + interval_ms));
    }

    #[test]
    fn seeding_by_replay_restores_budget_spent() {
        let (interval_ms, burst) = interval_ms_and_burst();
        let mut l = limiter();
        let now = 1_000_000;
        // Replaying k recent block timestamps leaves budget for `burst - k`
        // more (no time elapses here, so no budget regenerates in between).
        let k = burst / 2;
        for _ in 0..k {
            l.record(now);
        }
        for _ in 0..(burst - k) {
            assert!(l.is_conforming(now));
            l.record(now);
        }
        assert!(!l.is_conforming(now));
        // Timestamps older than the whole window leave the budget full.
        let mut l = limiter();
        l.record(now - burst * interval_ms);
        for _ in 0..burst {
            assert!(l.is_conforming(now));
            l.record(now);
        }
        assert!(!l.is_conforming(now));
    }
}
