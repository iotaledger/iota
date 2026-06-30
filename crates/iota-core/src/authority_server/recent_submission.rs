// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Digest-keyed duplicate-suppression for the pcool (white-flag) submission
//! path. Complements [`PreConsensusSoftLocks`] which is keyed on `ObjectRef`
//! and is intentionally idempotent for same-digest resubmissions; this layer
//! catches the same-digest case (including shared-object and gasless txs)
//! before they reach consensus.
//!
//! The cache records when a digest was first submitted and rejects any
//! resubmission seen within `window`. Entries auto-expire via the cache's
//! time-to-live; no active release is required because the window is small
//! (subsecond-to-second range) and bounded.
//!
//! # Edge cases
//!
//! | Case                                | Behavior                                                              |
//! |-------------------------------------|-----------------------------------------------------------------------|
//! | Same tx digest resubmitted in-window | Rejected with `IotaError::RecentlyResubmitted`                        |
//! | Same digest resubmitted after window | Re-recorded; passes through                                           |
//! | Concurrent submissions of same digest | First record wins; concurrent races may both pass — soft locks and consensus dedup catch the rest |
//! | Crash / restart                     | Cache lost → clean slate; behaviour is best-effort defense-in-depth   |
//! | Epoch boundary                      | Cache is not epoch-scoped; window is short enough that stale entries  |
//! |                                     | are harmless across epochs                                            |

use std::time::{Duration, Instant};

use iota_types::{base_types::TransactionDigest, error::IotaError};
use moka::sync::Cache;

/// Default window during which a given transaction is allowed into consensus
/// at most once. Short enough to permit legitimate retries promptly while
/// suppressing resubmission storms within a block-formation window.
pub const DEFAULT_RECENT_SUBMISSION_WINDOW: Duration = Duration::from_secs(1);

/// Assumed peak distinct-submission rate, used to size the dedup cache.
/// Memory backstop only — the window bounds normal occupancy.
const RECENT_SUBMISSION_PEAK_TPS: u64 = 50_000;

/// In-memory record of recently submitted transaction digests. Rejects
/// duplicate resubmissions seen within `window`.
#[derive(Debug)]
pub struct RecentSubmissions {
    cache: Cache<TransactionDigest, Instant>,
    window: Duration,
}

impl Default for RecentSubmissions {
    fn default() -> Self {
        Self::with_window(DEFAULT_RECENT_SUBMISSION_WINDOW)
    }
}

impl RecentSubmissions {
    /// Creates a new instance with the default window.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new instance with a custom window (useful for tests).
    pub fn with_window(window: Duration) -> Self {
        let max_capacity = window.as_secs().max(1) * RECENT_SUBMISSION_PEAK_TPS;
        let cache = Cache::builder()
            .time_to_live(window)
            .max_capacity(max_capacity)
            .build();
        Self { cache, window }
    }

    /// Returns the configured suppression window.
    pub fn window(&self) -> Duration {
        self.window
    }

    /// Approximate number of digests currently held in the cache.
    pub fn entry_count(&self) -> u64 {
        self.cache.entry_count()
    }

    /// Checks whether `tx_digest` was recorded within the suppression window.
    /// On hit, returns the elapsed time since the original recording (useful
    /// for metrics). On miss, records the digest as recently submitted and
    /// returns `Ok(())`.
    pub fn try_record(&self, tx_digest: TransactionDigest) -> Result<(), Duration> {
        if let Some(recorded_at) = self.cache.get(&tx_digest) {
            let elapsed = recorded_at.elapsed();
            if elapsed < self.window {
                return Err(elapsed);
            }
        }
        self.cache.insert(tx_digest, Instant::now());
        Ok(())
    }
}

/// Builds the rejection error returned when a duplicate resubmission is
/// suppressed.
pub fn recently_resubmitted_error(digest: TransactionDigest) -> IotaError {
    IotaError::RecentlyResubmitted { digest }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;

    use iota_types::base_types::TransactionDigest;

    use super::*;

    #[test]
    fn first_submission_records_and_passes() {
        let r = RecentSubmissions::with_window(Duration::from_millis(500));
        let d = TransactionDigest::random();
        assert!(r.try_record(d).is_ok());
    }

    #[test]
    fn duplicate_within_window_is_rejected() {
        let r = RecentSubmissions::with_window(Duration::from_millis(500));
        let d = TransactionDigest::random();
        assert!(r.try_record(d).is_ok());
        let elapsed = r.try_record(d).expect_err("duplicate must be rejected");
        assert!(elapsed < Duration::from_millis(500));
    }

    #[test]
    fn resubmission_after_window_passes() {
        let r = RecentSubmissions::with_window(Duration::from_millis(50));
        let d = TransactionDigest::random();
        assert!(r.try_record(d).is_ok());
        sleep(Duration::from_millis(120));
        assert!(
            r.try_record(d).is_ok(),
            "resubmission after window must pass"
        );
    }

    #[test]
    fn distinct_digests_do_not_interfere() {
        let r = RecentSubmissions::with_window(Duration::from_millis(500));
        let d1 = TransactionDigest::random();
        let d2 = TransactionDigest::random();
        assert!(r.try_record(d1).is_ok());
        assert!(r.try_record(d2).is_ok());
    }
}
