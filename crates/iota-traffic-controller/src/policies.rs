// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-client rate limiting for the traffic controller, backed by one GCRA
//! cell per client IP (`governor`). A tally is charged to the client's cell
//! inline, so a breached quota is visible to the next `check` as soon as
//! `charge` returns. Cells live in a capacity-bounded LRU cache per limiter,
//! so memory stays bounded no matter how many client IPs appear.

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
use iota_types::traffic_control::{FreqThresholdConfig, PolicyConfig, PolicyType, Weight};
use lru::LruCache;
use parking_lot::Mutex;
use tracing::warn;

/// Reset period for the exact-count test policies when the blocklist TTL is
/// zero, long enough that no count recovers over the lifetime of a test.
const EXACT_COUNT_FALLBACK_RESET_PERIOD: Duration = Duration::from_secs(60 * 60);

/// Largest usable client threshold: a sustained rate above one tally per
/// nanosecond produces a zero replenish interval, which would silently disable
/// the limiter. Clamped to this at startup, and rejected by the admin API.
pub const MAX_CLIENT_THRESHOLD: u64 = 1_000_000_000;

/// Upper bound on client IPs tracked per limiter, in the low tens of MB of
/// memory. When full, a new client evicts the least recently charged one,
/// resetting only that client's state.
const MAX_TRACKED_CLIENTS: usize = 100_000;

/// Cells dropped per eviction pass: the sweep holds the lock the request path
/// needs, and the next pass resumes where this one stopped.
const MAX_EVICTIONS_PER_PASS: usize = 4_096;

/// GCRA cell for a single client IP. Uses `std::time::Instant` rather than
/// `governor`'s default TSC-backed clock so that the deterministic simulator,
/// which virtualizes `clock_gettime`, also controls rate limiter time.
type ClientRateLimiter =
    RateLimiter<NotKeyed, InMemoryState, MonotonicClock, NoOpMiddleware<Instant>>;

fn client_rate_limiter(quota: Quota) -> ClientRateLimiter {
    RateLimiter::direct_with_clock(quota, &MonotonicClock)
}

/// Sustained `threshold` tallies per second, tolerating `threshold *
/// window_size_secs` back to back (the burst clamps at `u32::MAX` cells).
/// Callers validate `threshold` to be in `1..=MAX_CLIENT_THRESHOLD` and
/// `window_size_secs` to be non-zero.
fn sustained_quota(threshold: u64, window_size_secs: u64) -> Quota {
    Quota::per_second(clamp_to_cells(threshold))
        .allow_burst(clamp_to_cells(threshold.saturating_mul(window_size_secs)))
}

/// The `threshold`-th tally in a row breaches and earlier ones pass, with the
/// full count recovering over `reset_period`. Thresholds below 2 cannot be
/// expressed as a burst and use [`Limiter::BlockAll`] instead.
fn exact_count_quota(threshold: u64, reset_period: Duration) -> Quota {
    let burst = clamp_to_cells(threshold.saturating_sub(1));
    Quota::with_period((reset_period / burst.get()).max(Duration::from_nanos(1)))
        .expect("replenish period is non-zero")
        .allow_burst(burst)
}

/// Counts recover over twice the blocklist TTL, matching the periodic reset the
/// exact-count policy had before it moved to a rate limiter.
fn exact_count_reset_period(connection_blocklist_ttl_sec: u64) -> Duration {
    match connection_blocklist_ttl_sec.saturating_mul(2) {
        0 => EXACT_COUNT_FALLBACK_RESET_PERIOD,
        secs => Duration::from_secs(secs),
    }
}

/// Caps a configured threshold at [`MAX_CLIENT_THRESHOLD`], the largest value
/// the limiter can still enforce.
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
            // A threshold this low blocks on the first tally, which no burst
            // quota can express.
            _ => return Limiter::BlockAll,
        };
        Limiter::Cells(ClientCells::new(quota))
    }
}

/// Rate limiter for one client dimension (direct or proxied).
enum Limiter {
    /// A threshold too low for a burst quota to express: every tally blocks
    /// its client. Used as a killswitch by operators.
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

    fn evict_idle(&self) {
        if let Self::Cells(cells) = self {
            cells.evict_idle();
        }
    }

    fn tracked_clients(&self) -> usize {
        match self {
            Self::BlockAll => 0,
            Self::Cells(cells) => cells.tracked_clients(),
        }
    }
}

/// Per-client GCRA cells in an LRU cache bounded at [`MAX_TRACKED_CLIENTS`].
/// Each client IP owns an independent cell, so evicting one client never
/// affects another.
struct ClientCells {
    quota: Quota,
    /// A cell whose last charge is at least this old is fully replenished, so
    /// dropping it is indistinguishable from keeping it. The stamp is taken an
    /// instant before the charge it records, which the sweep cadence dwarfs.
    idle_after: Duration,
    cells: Mutex<LruCache<IpAddr, ClientCell>>,
}

struct ClientCell {
    limiter: ClientRateLimiter,
    last_charge: Instant,
}

impl ClientCells {
    fn new(quota: Quota) -> Self {
        let interval = quota.replenish_interval();
        Self {
            // A fully drained burst replenishes in burst * interval, plus one
            // interval for the cell the draining charge itself consumed.
            idle_after: interval
                .saturating_mul(quota.burst_size().get())
                .saturating_add(interval),
            quota,
            cells: Mutex::new(LruCache::new(
                NonZeroUsize::new(MAX_TRACKED_CLIENTS).expect("capacity is non-zero"),
            )),
        }
    }

    fn breaches(&self, client: IpAddr) -> bool {
        let now = Instant::now();
        let mut cells = self.cells.lock();
        let cell = cells.get_or_insert_mut(client, || ClientCell {
            limiter: client_rate_limiter(self.quota),
            last_charge: now,
        });
        cell.last_charge = now;
        cell.limiter.check().is_err()
    }

    fn evict_idle(&self) {
        let mut cells = self.cells.lock();
        for _ in 0..MAX_EVICTIONS_PER_PASS {
            match cells.peek_lru() {
                Some((_, cell)) if cell.last_charge.elapsed() >= self.idle_after => {
                    cells.pop_lru();
                }
                _ => break,
            }
        }
    }

    fn tracked_clients(&self) -> usize {
        self.cells.lock().len()
    }
}

/// The direct-client threshold and the limiter built from it, swapped as one
/// unit on reconfiguration so the reported threshold always matches the
/// enforced one.
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
    /// Swapped wholesale when the threshold is reconfigured, which resets the
    /// rate limiter state of every tracked client.
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

    fn evict_idle(&self) {
        self.direct.load().limiter.evict_idle();
        if let Some(proxied) = &self.proxied {
            proxied.evict_idle();
        }
    }

    fn tracked_clients(&self) -> usize {
        self.direct.load().limiter.tracked_clients()
            + self.proxied.as_ref().map_or(0, Limiter::tracked_clients)
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
        Self::from_policy_type(
            &policy_config.spam_policy_type,
            policy_config.connection_blocklist_ttl_sec,
        )
    }

    pub fn from_error_config(policy_config: &PolicyConfig) -> Self {
        Self::from_policy_type(
            &policy_config.error_policy_type,
            policy_config.connection_blocklist_ttl_sec,
        )
    }

    pub fn from_policy_type(policy_type: &PolicyType, connection_blocklist_ttl_sec: u64) -> Self {
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
            Policy::Limit(policy) => Some(policy.direct.load().threshold),
            _ => None,
        }
    }

    /// Replaces the direct-client threshold, discarding the rate limiter state
    /// of every tracked client. Policies that do not rate limit ignore it, and
    /// the admin path validates `threshold` against [`MAX_CLIENT_THRESHOLD`]
    /// before calling.
    pub(super) fn set_client_threshold(&self, threshold: u64) {
        if let Policy::Limit(policy) = &self.0 {
            policy.set_direct_threshold(threshold);
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
        assert_eq!(exact_count_reset_period(30), Duration::from_secs(60));
    }

    #[test]
    fn test_threshold_of_one_blocks_on_the_first_tally() {
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for threshold in [0, 1] {
            assert_eq!(
                exact_count(threshold, 60)
                    .charge(&direct_tally(client))
                    .block_client,
                Some(client),
                "threshold {threshold} did not block on the first tally"
            );
        }
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

    // The breach must be observable on the threshold-crossing tally itself,
    // with no waiting, in every round.
    #[test]
    fn test_exact_count_blocks_on_nth_tally() {
        let threshold = 5;
        let policy = exact_count(threshold, 60);
        for round in 0..3u32 {
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
        // Sustained 2/s with a 5 second burst: 10 tallies exhaust the burst,
        // and one cell replenishes every 500ms, so timing is tolerant to well
        // under half a second of sleep overshoot on real tokio.
        let policy = freq_threshold(2, 2, 5);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..10 {
            assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
        }
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );

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

        // Accumulate state under the old threshold, so the assertions below
        // can only pass if reconfiguration discarded it along with the old
        // limiter.
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..10 {
            assert_eq!(policy.charge(&direct_tally(client)).block_client, None);
        }

        policy.set_client_threshold(2);
        assert_eq!(policy.client_threshold(), Some(2));

        // The new quota tolerates a fresh burst of 2 * 5.
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
    fn test_reconfiguring_to_a_zero_threshold_blocks_every_client() {
        let policy = freq_threshold(1_000, 1_000, 5);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(policy.charge(&direct_tally(client)).block_client, None);

        // Zero means block on the first tally (killswitch), same as when it is
        // configured at startup.
        policy.set_client_threshold(0);
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );

        // And back to a permissive threshold.
        policy.set_client_threshold(1_000);
        assert_eq!(policy.charge(&direct_tally(client)).block_client, None);

        // The same reconfiguration on an exact-count policy blocks on the
        // first tally rather than the second.
        let policy = exact_count(5, 60);
        policy.set_client_threshold(1);
        assert_eq!(
            policy.charge(&direct_tally(client)).block_client,
            Some(client)
        );
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

    #[sim_test]
    async fn test_idle_cells_are_evicted() {
        // Sustained 1000/s with a 1 second burst: a cell is guaranteed fully
        // replenished 1.001 seconds after its last charge, whatever that
        // charge consumed.
        let policy = freq_threshold(1_000, 1_000, 1);
        for i in 0..100u32 {
            policy.charge(&direct_tally(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i))));
        }
        assert_eq!(policy.tracked_clients(), 100);

        tokio::time::sleep(Duration::from_millis(1_200)).await;
        policy.evict_idle();
        assert_eq!(policy.tracked_clients(), 0);
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
        assert_eq!(policy.tracked_clients(), MAX_TRACKED_CLIENTS);

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
