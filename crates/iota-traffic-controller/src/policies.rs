// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-client rate limiting for the traffic controller: one GCRA cell
//! (`governor`) per client IP, held in a capacity-bounded LRU cache.

use std::{
    net::IpAddr,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use governor::{
    Quota, RateLimiter,
    clock::MonotonicClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, direct::NotKeyed},
};
use iota_common::fatal;
use iota_types::traffic_control::{FreqThresholdConfig, PolicyType, Weight};
use lru::LruCache;
use parking_lot::Mutex;
use tracing::warn;

/// Reset period for the exact-count test policies when the blocklist TTL is
/// zero, long enough that no count recovers over the lifetime of a test.
const EXACT_COUNT_FALLBACK_RESET_PERIOD: Duration = Duration::from_secs(60 * 60);

/// Largest usable client threshold: a sustained rate above one tally per
/// nanosecond produces a zero replenish interval, which would silently disable
/// the limiter. Clamped to this at startup, and rejected by the admin API.
pub(super) const MAX_CLIENT_THRESHOLD: u64 = 1_000_000_000;

/// Upper bound on client IPs tracked per limiter.
const MAX_TRACKED_CLIENTS: usize = 100_000;

/// GCRA cell for a single client IP. Uses `std::time::Instant` rather than
/// `governor`'s default TSC-backed clock so that the deterministic simulator,
/// which virtualizes `clock_gettime`, also controls rate limiter time.
type ClientRateLimiter =
    RateLimiter<NotKeyed, InMemoryState, MonotonicClock, NoOpMiddleware<Instant>>;

/// Sustained `threshold` tallies per second, tolerating `threshold *
/// window_size_secs` back to back.
fn sustained_quota(threshold: u64, window_size_secs: u64) -> Quota {
    Quota::per_second(clamp_to_cells(threshold))
        .allow_burst(clamp_to_cells(threshold.saturating_mul(window_size_secs)))
}

/// The `threshold`-th tally in a row breaches and earlier ones pass, with the
/// full count recovering over `reset_period`.
fn exact_count_quota(threshold: u64, reset_period: Duration) -> Quota {
    let burst = clamp_to_cells(threshold.saturating_sub(1));
    Quota::with_period((reset_period / burst.get()).max(Duration::from_nanos(1)))
        .expect("replenish period is non-zero")
        .allow_burst(burst)
}

/// Counts recover over twice the blocklist TTL.
fn exact_count_reset_period(connection_blocklist_ttl_sec: u64) -> Duration {
    match connection_blocklist_ttl_sec.saturating_mul(2) {
        0 => EXACT_COUNT_FALLBACK_RESET_PERIOD,
        secs => Duration::from_secs(secs),
    }
}

fn clamp_threshold(threshold: u64, name: &str) -> u64 {
    if threshold > MAX_CLIENT_THRESHOLD {
        warn!("freq-threshold {name} {threshold} exceeds {MAX_CLIENT_THRESHOLD}, clamping");
        return MAX_CLIENT_THRESHOLD;
    }
    threshold
}

fn clamp_to_cells(value: u64) -> NonZeroU32 {
    NonZeroU32::new(value.clamp(1, u32::MAX as u64) as u32).expect("clamped value is non-zero")
}

#[derive(Debug)]
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

#[derive(Debug, Default)]
pub(super) struct PolicyResponse {
    pub block_client: Option<IpAddr>,
    pub block_proxied_client: Option<IpAddr>,
}

/// How a threshold maps to a quota.
#[derive(Clone, Copy)]
enum QuotaKind {
    Sustained { window_size_secs: u64 },
    ExactCount { reset_period: Duration },
}

impl QuotaKind {
    fn limiter(&self, threshold: u64) -> Limiter {
        let quota = match self {
            Self::Sustained { window_size_secs } if threshold >= 1 => {
                sustained_quota(threshold, *window_size_secs)
            }
            Self::ExactCount { reset_period } if threshold >= 2 => {
                exact_count_quota(threshold, *reset_period)
            }
            // Too low for any burst quota: block on the first tally.
            _ => return Limiter::BlockAll,
        };
        Limiter::Cells(ClientCells::new(quota))
    }
}

/// Rate limiter for one client dimension (direct or proxied).
enum Limiter {
    /// Every tally blocks its client; the operator killswitch.
    BlockAll,
    Cells(ClientCells),
}

impl Limiter {
    fn breaches(&self, client: IpAddr) -> bool {
        match self {
            Self::BlockAll => true,
            Self::Cells(cells) => cells.breaches(client),
        }
    }
}

/// Per-client GCRA cells in an LRU cache bounded at [`MAX_TRACKED_CLIENTS`].
struct ClientCells {
    quota: Quota,
    cells: Mutex<LruCache<IpAddr, ClientRateLimiter>>,
}

impl ClientCells {
    fn new(quota: Quota) -> Self {
        Self {
            quota,
            cells: Mutex::new(LruCache::new(
                NonZeroUsize::new(MAX_TRACKED_CLIENTS).expect("capacity is non-zero"),
            )),
        }
    }

    fn breaches(&self, client: IpAddr) -> bool {
        let mut cells = self.cells.lock();
        cells
            .get_or_insert_mut(client, || {
                RateLimiter::direct_with_clock(self.quota, &MonotonicClock)
            })
            .check()
            .is_err()
    }
}

/// The direct-client threshold and its limiter, swapped as one unit on
/// reconfiguration.
struct DirectLimiter {
    threshold: u64,
    limiter: Limiter,
}

impl DirectLimiter {
    fn new(quota_kind: QuotaKind, threshold: u64) -> Self {
        Self {
            threshold,
            limiter: quota_kind.limiter(threshold),
        }
    }
}

/// Rate limits direct clients, and proxied clients when the policy configures a
/// separate threshold for them.
struct RateLimitPolicy {
    quota_kind: QuotaKind,
    direct: ArcSwap<DirectLimiter>,
    proxied: Option<Limiter>,
}

impl RateLimitPolicy {
    fn new(quota_kind: QuotaKind, direct_threshold: u64, proxied_threshold: Option<u64>) -> Self {
        Self {
            direct: ArcSwap::from_pointee(DirectLimiter::new(quota_kind, direct_threshold)),
            proxied: proxied_threshold.map(|threshold| quota_kind.limiter(threshold)),
            quota_kind,
        }
    }

    fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        let direct = self.direct.load();
        PolicyResponse {
            block_client: tally
                .direct
                .filter(|client| direct.limiter.breaches(*client)),
            block_proxied_client: tally.through_fullnode.filter(|client| {
                self.proxied
                    .as_ref()
                    .is_some_and(|limiter| limiter.breaches(*client))
            }),
        }
    }

    fn set_direct_threshold(&self, threshold: u64) {
        self.direct
            .store(Arc::new(DirectLimiter::new(self.quota_kind, threshold)));
    }
}

enum Policy {
    NoOp,
    Limit(RateLimitPolicy),
    PanicOnInvocation,
}

/// The spam or error policy a traffic controller charges its tallies against.
pub(super) struct TrafficControlPolicy(Policy);

impl TrafficControlPolicy {
    pub(super) fn from_policy_type(
        policy_type: &PolicyType,
        connection_blocklist_ttl_sec: u64,
    ) -> Self {
        Self(match policy_type {
            PolicyType::NoOp => Policy::NoOp,
            PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold,
                window_size_secs,
            }) => {
                if *window_size_secs == 0 {
                    fatal!("freq-threshold window-size-secs must be non-zero");
                }
                Policy::Limit(RateLimitPolicy::new(
                    QuotaKind::Sustained {
                        window_size_secs: *window_size_secs,
                    },
                    clamp_threshold(*client_threshold, "client-threshold"),
                    Some(clamp_threshold(
                        *proxied_client_threshold,
                        "proxied-client-threshold",
                    )),
                ))
            }
            // The exact-count test policy only ever blocks the direct client.
            PolicyType::TestNConnIP(threshold) => Policy::Limit(RateLimitPolicy::new(
                QuotaKind::ExactCount {
                    reset_period: exact_count_reset_period(connection_blocklist_ttl_sec),
                },
                *threshold,
                None,
            )),
            PolicyType::TestPanicOnInvocation => Policy::PanicOnInvocation,
        })
    }

    /// Charges the tally against the policy, returning which clients breached
    /// their quota.
    pub(super) fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        match &self.0 {
            Policy::NoOp => PolicyResponse::default(),
            Policy::Limit(policy) => policy.charge(tally),
            Policy::PanicOnInvocation => panic!("Tally for this policy should never be invoked"),
        }
    }

    /// Direct-client threshold, or `None` for policies that do not rate limit.
    pub(super) fn client_threshold(&self) -> Option<u64> {
        match &self.0 {
            Policy::Limit(policy) => Some(policy.direct.load().threshold),
            _ => None,
        }
    }

    /// Replaces the direct-client threshold, discarding every tracked client's
    /// limiter state.
    pub(super) fn set_client_threshold(&self, threshold: u64) {
        if let Policy::Limit(policy) = &self.0 {
            policy.set_direct_threshold(threshold);
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
        TrafficControlPolicy::from_policy_type(
            &PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold,
                window_size_secs,
            }),
            0,
        )
    }

    fn exact_count(threshold: u64, connection_blocklist_ttl_sec: u64) -> TrafficControlPolicy {
        TrafficControlPolicy::from_policy_type(
            &PolicyType::TestNConnIP(threshold),
            connection_blocklist_ttl_sec,
        )
    }

    #[test]
    fn test_exact_count_reset_period() {
        // A zero blocklist TTL leaves counts in place for the lifetime of the
        // policy.
        assert_eq!(
            exact_count_reset_period(0),
            EXACT_COUNT_FALLBACK_RESET_PERIOD
        );
    }

    #[sim_test]
    async fn test_exact_counts_recover_after_the_reset_period() {
        // A one second blocklist TTL resets counts over two seconds.
        let policy = exact_count(2, 1);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );

        // Halfway through the reset period the count has not recovered yet.
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
    }

    // The breach must be observable with no waiting.
    #[test]
    fn test_exact_count_blocks_on_nth_tally() {
        let threshold = 5;
        let policy = exact_count(threshold, 60);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for i in 1..threshold {
            assert_eq!(
                policy.charge(&direct_tally(client)).block_client,
                None,
                "blocked early after {i} tallies"
            );
        }
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client),
            "tally {threshold} did not block"
        );
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
        // Sustained 2/s with a 5 second burst: 10 tallies exhaust the burst,
        // and one cell replenishes every 500ms, so timing is tolerant to well
        // under half a second of sleep overshoot on real tokio.
        let policy = freq_threshold(2, 2, 5);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..10 {
            policy.charge(&direct_tally(client));
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
        // Two cells have replenished, so two more tallies pass.
        for i in 0..2 {
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

        // Accumulate state under the old threshold.
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..10 {
            assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
        }

        policy.set_client_threshold(2);
        assert_eq!(policy.client_threshold(), Some(2));

        // The new quota tolerates a fresh burst of 2 * 5, so the old state is
        // gone along with the old limiter.
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

        // Zero blocks every client, and a permissive threshold restores
        // service.
        policy.set_client_threshold(0);
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );
        policy.set_client_threshold(1_000);
        assert_eq!(policy.charge(&direct_tally(client)).block_client, None);

        // An exact-count policy below a threshold of two blocks on the first
        // tally, whether configured at startup or afterwards.
        for threshold in [0, 1] {
            assert_eq!(
                exact_count(threshold, 60)
                    .charge(&direct_tally(client))
                    .block_client,
                Some(client),
                "threshold {threshold} did not block on the first tally"
            );
            let policy = exact_count(5, 60);
            policy.set_client_threshold(threshold);
            assert_eq!(
                policy.charge(&direct_tally(client)).block_client,
                Some(client),
                "threshold {threshold} did not block on the first tally after reconfiguration"
            );
        }
    }

    #[test]
    fn test_overlarge_thresholds_are_clamped() {
        let policy = freq_threshold(2_000_000_000, 2_000_000_000, 5);
        assert_eq!(policy.client_threshold(), Some(MAX_CLIENT_THRESHOLD));
    }

    #[test]
    fn test_non_limiting_policies_ignore_threshold_updates() {
        let policy = TrafficControlPolicy::from_policy_type(&PolicyType::NoOp, 0);
        assert_eq!(policy.client_threshold(), None);
        policy.set_client_threshold(10);
        assert_eq!(policy.client_threshold(), None);
    }

    #[test]
    fn test_cell_cache_is_bounded() {
        let policy = exact_count(2, 60);
        let evicted = IpAddr::V4(Ipv4Addr::from(0x0A00_0000));
        assert_eq!(policy.charge(&direct_tally(evicted)).block_client, None);

        // Flood enough distinct clients to fill the cache and push the first
        // one out.
        for i in 1..=MAX_TRACKED_CLIENTS as u32 {
            policy.charge(&direct_tally(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i))));
        }

        // The evicted client's count was reset along with its cell, so its
        // next tally counts as its first again; a still-cached client keeps
        // its count and blocks on its second.
        assert_eq!(policy.charge(&direct_tally(evicted)).block_client, None);
        let cached = IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + MAX_TRACKED_CLIENTS as u32));
        assert_eq!(
            policy.charge(&direct_tally(cached)).block_client,
            Some(cached)
        );
    }
}
