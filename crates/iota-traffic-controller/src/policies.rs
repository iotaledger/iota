// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-client rate limiting for the traffic controller, backed by one GCRA
//! cell per client IP (`governor`). Charging a tally is a single atomic
//! compare-and-swap on the client's cell, so a breached quota is visible to
//! the next `check` as soon as `charge` returns.

use std::{
    net::IpAddr,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use governor::{
    Quota, RateLimiter, clock::MonotonicClock, middleware::NoOpMiddleware,
    state::keyed::DefaultKeyedStateStore,
};
use iota_types::traffic_control::{FreqThresholdConfig, PolicyConfig, PolicyType, Weight};

/// Replenish period for the exact-count test policies, long enough that no
/// cell recovers over the lifetime of a test.
const EXACT_COUNT_REPLENISH_PERIOD: Duration = Duration::from_secs(60 * 60);

/// Rate limiter keyed by client IP. Uses `std::time::Instant` rather than
/// `governor`'s default TSC-backed clock so that the deterministic simulator,
/// which virtualizes `clock_gettime`, also controls rate limiter time.
type ClientRateLimiter =
    RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, MonotonicClock, NoOpMiddleware<Instant>>;

fn client_rate_limiter(quota: Quota) -> ClientRateLimiter {
    RateLimiter::new(quota, DefaultKeyedStateStore::default(), &MonotonicClock)
}

/// Sustained `threshold` tallies per second, tolerating `threshold *
/// window_size_secs` back to back. Degenerate configuration values clamp to a
/// single cell rather than failing at startup.
fn sustained_quota(threshold: u64, window_size_secs: u64) -> Quota {
    Quota::per_second(clamp_to_cells(threshold))
        .allow_burst(clamp_to_cells(threshold.saturating_mul(window_size_secs)))
}

/// The `threshold`-th tally in a row breaches, earlier ones pass. Thresholds
/// below 2 clamp to a one-cell burst, so the second tally breaches rather than
/// the first.
fn exact_count_quota(threshold: u64) -> Quota {
    Quota::with_period(EXACT_COUNT_REPLENISH_PERIOD)
        .expect("replenish period is non-zero")
        .allow_burst(clamp_to_cells(threshold.saturating_sub(1)))
}

fn clamp_to_cells(value: u64) -> NonZeroU32 {
    NonZeroU32::new(value.clamp(1, u32::MAX as u64) as u32).expect("clamped value is non-zero")
}

#[derive(Clone, Debug)]
pub struct TrafficTally {
    pub direct: Option<IpAddr>,
    pub through_fullnode: Option<IpAddr>,
    pub error_info: Option<(Weight, String)>,
    pub spam_weight: Weight,
}

impl TrafficTally {
    pub fn new(
        direct: Option<IpAddr>,
        through_fullnode: Option<IpAddr>,
        error_info: Option<(Weight, String)>,
        spam_weight: Weight,
    ) -> Self {
        Self {
            direct,
            through_fullnode,
            error_info,
            spam_weight,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PolicyResponse {
    pub block_client: Option<IpAddr>,
    pub block_proxied_client: Option<IpAddr>,
}

/// How the direct-client threshold maps to a quota, so that the limiter can be
/// rebuilt when an operator reconfigures the threshold at runtime.
#[derive(Clone, Copy)]
enum QuotaKind {
    Sustained { window_size_secs: u64 },
    ExactCount,
}

impl QuotaKind {
    fn quota(&self, threshold: u64) -> Quota {
        match self {
            Self::Sustained { window_size_secs } => sustained_quota(threshold, *window_size_secs),
            Self::ExactCount => exact_count_quota(threshold),
        }
    }
}

/// Rate limits direct clients, and proxied clients when the policy configures a
/// separate threshold for them.
struct RateLimitPolicy {
    quota_kind: QuotaKind,
    /// Swapped out when the threshold is reconfigured, which resets the rate
    /// limiter state of every tracked client.
    direct: ArcSwap<ClientRateLimiter>,
    direct_threshold: AtomicU64,
    proxied: Option<ClientRateLimiter>,
}

impl RateLimitPolicy {
    fn new(quota_kind: QuotaKind, direct_threshold: u64, proxied_threshold: Option<u64>) -> Self {
        Self {
            direct: ArcSwap::from_pointee(client_rate_limiter(quota_kind.quota(direct_threshold))),
            direct_threshold: AtomicU64::new(direct_threshold),
            proxied: proxied_threshold
                .map(|threshold| client_rate_limiter(quota_kind.quota(threshold))),
            quota_kind,
        }
    }

    fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        let direct = self.direct.load();
        PolicyResponse {
            block_client: tally
                .direct
                .filter(|client| direct.check_key(client).is_err()),
            block_proxied_client: match (&self.proxied, tally.through_fullnode) {
                (Some(limiter), Some(client)) if limiter.check_key(&client).is_err() => {
                    Some(client)
                }
                _ => None,
            },
        }
    }

    fn set_direct_threshold(&self, threshold: u64) {
        self.direct_threshold.store(threshold, Ordering::Relaxed);
        self.direct.store(Arc::new(client_rate_limiter(
            self.quota_kind.quota(threshold),
        )));
    }

    fn evict_idle(&self) {
        let direct = self.direct.load();
        direct.retain_recent();
        direct.shrink_to_fit();
        if let Some(proxied) = &self.proxied {
            proxied.retain_recent();
            proxied.shrink_to_fit();
        }
    }

    fn tracked_clients(&self) -> usize {
        self.direct.load().len() + self.proxied.as_ref().map_or(0, |limiter| limiter.len())
    }
}

enum Policy {
    NoOp,
    Limit(RateLimitPolicy),
    /// Test policy that never permits a tally, to verify that a policy is not
    /// reached in tests that expect no matching traffic.
    PanicOnInvocation,
}

/// The spam or error policy a traffic controller charges its tallies against.
pub struct TrafficControlPolicy(Policy);

impl TrafficControlPolicy {
    pub fn from_spam_config(policy_config: &PolicyConfig) -> Self {
        Self::from_policy_type(&policy_config.spam_policy_type)
    }

    pub fn from_error_config(policy_config: &PolicyConfig) -> Self {
        Self::from_policy_type(&policy_config.error_policy_type)
    }

    pub fn from_policy_type(policy_type: &PolicyType) -> Self {
        Self(match policy_type {
            PolicyType::NoOp => Policy::NoOp,
            PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold,
                window_size_secs,
            }) => Policy::Limit(RateLimitPolicy::new(
                QuotaKind::Sustained {
                    window_size_secs: *window_size_secs,
                },
                *client_threshold,
                Some(*proxied_client_threshold),
            )),
            // The exact-count test policy only ever blocks the direct client.
            PolicyType::TestNConnIP(threshold) => Policy::Limit(RateLimitPolicy::new(
                QuotaKind::ExactCount,
                *threshold,
                None,
            )),
            PolicyType::TestPanicOnInvocation => Policy::PanicOnInvocation,
        })
    }

    /// Charges the tally against the policy, returning which clients breached
    /// their quota.
    pub fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        match &self.0 {
            Policy::NoOp => PolicyResponse::default(),
            Policy::Limit(policy) => policy.charge(tally),
            Policy::PanicOnInvocation => panic!("Tally for this policy should never be invoked"),
        }
    }

    /// Direct-client threshold, or `None` for policies that do not rate limit.
    pub fn client_threshold(&self) -> Option<u64> {
        match &self.0 {
            Policy::Limit(policy) => Some(policy.direct_threshold.load(Ordering::Relaxed)),
            _ => None,
        }
    }

    /// Replaces the direct-client threshold, discarding the rate limiter state
    /// of every tracked client. Returns false for policies that do not rate
    /// limit.
    pub fn set_client_threshold(&self, threshold: u64) -> bool {
        match &self.0 {
            Policy::Limit(policy) => {
                policy.set_direct_threshold(threshold);
                true
            }
            _ => false,
        }
    }

    /// Drops per-client cells that have fully replenished. Must run
    /// periodically to keep memory bounded under client churn.
    pub fn evict_idle(&self) {
        if let Policy::Limit(policy) = &self.0 {
            policy.evict_idle();
        }
    }

    /// Number of per-client cells currently held.
    pub fn tracked_clients(&self) -> usize {
        match &self.0 {
            Policy::Limit(policy) => policy.tracked_clients(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use iota_macros::sim_test;

    use super::*;

    fn direct_tally(client: IpAddr) -> TrafficTally {
        TrafficTally::new(Some(client), None, None, Weight::one())
    }

    fn freq_threshold(
        client_threshold: u64,
        proxied_client_threshold: u64,
        window_size_secs: u64,
    ) -> TrafficControlPolicy {
        TrafficControlPolicy::from_policy_type(&PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold,
            proxied_client_threshold,
            window_size_secs,
        }))
    }

    #[test]
    fn test_sustained_quota_mapping() {
        let quota = sustained_quota(1_000, 5);
        assert_eq!(quota.burst_size().get(), 5_000);
        assert_eq!(quota.replenish_interval(), Duration::from_millis(1));

        // Degenerate thresholds still produce a valid quota.
        assert_eq!(sustained_quota(0, 0).burst_size().get(), 1);
    }

    #[test]
    fn test_exact_count_quota_mapping() {
        assert_eq!(exact_count_quota(5).burst_size().get(), 4);
        // Thresholds below 2 cannot be represented, and clamp to one cell.
        assert_eq!(exact_count_quota(1).burst_size().get(), 1);
        assert_eq!(exact_count_quota(0).burst_size().get(), 1);
    }

    // The channel-based tally path could not pass this without sleeps: the
    // breach must be observable as soon as the threshold-crossing tally
    // returns, in every round.
    #[test]
    fn test_exact_count_blocks_on_nth_tally() {
        let threshold = 5;
        let policy = TrafficControlPolicy::from_policy_type(&PolicyType::TestNConnIP(threshold));
        for round in 0..1_000u32 {
            let client = IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + round));
            for i in 1..threshold {
                assert_eq!(
                    policy.charge(&direct_tally(client)).block_client,
                    None,
                    "round {round}: blocked early after {i} tallies"
                );
            }
            assert_eq!(
                policy.charge(&direct_tally(client)).block_client,
                Some(client),
                "round {round}: tally {threshold} did not block"
            );
        }
    }

    #[test]
    fn test_freq_threshold_blocks_once_burst_is_exhausted() {
        // Sustained 2/s tolerating a 5 second burst, so the 11th back to back
        // tally is the first to breach, for both client dimensions.
        let policy = freq_threshold(2, 2, 5);
        let direct = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let proxied = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
        let tally = TrafficTally::new(Some(direct), Some(proxied), None, Weight::one());
        for i in 1..=10 {
            let response = policy.charge(&tally);
            assert_eq!(response.block_client, None, "blocked early after {i}");
            assert_eq!(response.block_proxied_client, None);
        }
        let response = policy.charge(&tally);
        assert_eq!(response.block_client, Some(direct));
        assert_eq!(response.block_proxied_client, Some(proxied));
    }

    #[sim_test]
    async fn test_client_recovers_as_the_quota_replenishes() {
        // Sustained 10/s with a 1 second burst: 10 tallies exhaust the burst,
        // and one replenishes every 100ms.
        let policy = freq_threshold(10, 10, 1);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..10 {
            assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
        }
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );

        tokio::time::sleep(Duration::from_millis(500)).await;
        // Five cells have replenished, so five more tallies pass.
        for i in 0..5 {
            assert_eq!(
                policy.charge(&direct_tally(client)).block_client,
                None,
                "blocked after {i} replenished tallies"
            );
        }
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );
    }

    #[test]
    fn test_clients_are_limited_independently() {
        let policy = freq_threshold(1, 1, 1);
        let noisy = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let quiet = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        for _ in 0..5 {
            policy.charge(&direct_tally(noisy));
        }
        assert_eq!(
            policy.charge(&direct_tally(noisy)).block_client,
            Some(noisy)
        );
        assert_eq!(policy.charge(&direct_tally(quiet)).block_client, None);
    }

    #[test]
    fn test_reconfigured_threshold_takes_effect() {
        let policy = freq_threshold(1_000, 1_000, 5);
        assert_eq!(policy.client_threshold(), Some(1_000));

        assert!(policy.set_client_threshold(2));
        assert_eq!(policy.client_threshold(), Some(2));

        // The new quota tolerates a burst of 2 * 5, and the reconfiguration
        // discarded any state accumulated under the previous threshold.
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for i in 1..=10 {
            assert_eq!(
                policy.charge(&direct_tally(client)).block_client,
                None,
                "blocked early after {i} tallies"
            );
        }
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );
    }

    #[test]
    fn test_non_limiting_policies_reject_reconfiguration() {
        let policy = TrafficControlPolicy::from_policy_type(&PolicyType::NoOp);
        assert_eq!(policy.client_threshold(), None);
        assert!(!policy.set_client_threshold(10));
    }

    #[sim_test]
    async fn test_idle_cells_are_evicted() {
        let policy = freq_threshold(1_000, 1_000, 5);
        for i in 0..100u32 {
            policy.charge(&direct_tally(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i))));
        }
        assert_eq!(policy.tracked_clients(), 100);

        // A single charge against a 5000 cell burst replenishes in 1ms, after
        // which the cell is indistinguishable from a fresh one and evictable.
        tokio::time::sleep(Duration::from_millis(50)).await;
        policy.evict_idle();
        assert_eq!(policy.tracked_clients(), 0);
    }
}
