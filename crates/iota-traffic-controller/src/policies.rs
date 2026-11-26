// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-client rate limiting for the traffic controller: one GCRA cell
//! (`governor`) per client IP, held in a capacity-bounded LRU cache.

use std::{
    net::IpAddr,
    num::{NonZeroU32, NonZeroUsize},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use arc_swap::{ArcSwap, ArcSwapOption};
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
use prometheus_filtered::IntCounter;
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
/// burst_secs` back to back.
fn sustained_quota(threshold: u64, burst_secs: u64) -> Quota {
    let burst = threshold.saturating_mul(burst_secs);
    if burst > u32::MAX as u64 {
        warn!(
            "freq-threshold burst {burst} exceeds {} cells, clamping",
            u32::MAX
        );
    }
    Quota::per_second(clamp_to_cells(threshold)).allow_burst(clamp_to_cells(burst))
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
    /// Items this tally returned, charged against the response-item dimension.
    pub response_items: u32,
    /// Server-rendered fields this tally produced, charged against the
    /// rendered-field dimension.
    pub rendered_fields: u32,
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
            response_items: 0,
            rendered_fields: 0,
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
    Sustained { burst_secs: u64 },
    ExactCount { reset_period: Duration },
}

impl QuotaKind {
    fn limiter(&self, threshold: u64, evictions: &IntCounter) -> Limiter {
        let quota = match self {
            Self::Sustained { burst_secs } if threshold >= 1 => {
                sustained_quota(threshold, *burst_secs)
            }
            Self::ExactCount { reset_period } if threshold >= 2 => {
                exact_count_quota(threshold, *reset_period)
            }
            // Too low for any burst quota: block on the first tally.
            _ => return Limiter::BlockAll,
        };
        Limiter::Cells(ClientCells::new(quota, evictions.clone()))
    }
}

/// Rate limiter for direct or proxied clients.
enum Limiter {
    /// Every tally blocks its client; the operator killswitch.
    BlockAll,
    Cells(ClientCells),
}

impl Limiter {
    fn breaches(&self, client: IpAddr) -> bool {
        self.breaches_n(client, NonZeroU32::MIN)
    }

    /// Whether charging `n` cells to `client` breaches the limit now. A charge
    /// larger than the limiter's whole burst can never be served, so it
    /// breaches outright.
    fn breaches_n(&self, client: IpAddr, n: NonZeroU32) -> bool {
        match self {
            Self::BlockAll => true,
            Self::Cells(cells) => cells.breaches_n(client, n),
        }
    }
}

/// Per-client GCRA cells in an LRU cache bounded at [`MAX_TRACKED_CLIENTS`].
struct ClientCells {
    quota: Quota,
    cells: Mutex<LruCache<IpAddr, ClientRateLimiter>>,
    evictions: IntCounter,
}

impl ClientCells {
    fn new(quota: Quota, evictions: IntCounter) -> Self {
        Self {
            quota,
            cells: Mutex::new(LruCache::new(
                NonZeroUsize::new(MAX_TRACKED_CLIENTS).expect("capacity is non-zero"),
            )),
            evictions,
        }
    }

    fn breaches_n(&self, client: IpAddr, n: NonZeroU32) -> bool {
        let mut cells = self.cells.lock();
        if let Some(cell) = cells.get_mut(&client) {
            return !matches!(cell.check_n(n), Ok(Ok(())));
        }
        let cell = RateLimiter::direct_with_clock(self.quota, &MonotonicClock);
        // `check_n` is `Ok(Ok(()))` only when the whole charge fits; a partial
        // fit is rate-limited and an over-burst charge can never be served.
        let breached = !matches!(cell.check_n(n), Ok(Ok(())));
        // `push` returns the entry it displaced; at capacity that is another
        // client's state, which counts as an eviction.
        if let Some((evicted, _)) = cells.push(client, cell) {
            debug_assert_ne!(evicted, client);
            self.evictions.inc();
        }
        breached
    }
}

/// The direct-client threshold and its limiter, swapped as one unit on
/// reconfiguration.
struct DirectLimiter {
    threshold: u64,
    limiter: Limiter,
}

impl DirectLimiter {
    fn new(quota_kind: QuotaKind, threshold: u64, evictions: &IntCounter) -> Self {
        Self {
            threshold,
            limiter: quota_kind.limiter(threshold, evictions),
        }
    }
}

/// A configured rate-limit dimension: a threshold and the limiter built from it.
struct ConfiguredLimiter {
    threshold: u64,
    limiter: Limiter,
}

/// An optional rate-limit dimension charged a per-tally count. It carries two
/// independent pieces of state: a configured value (its threshold and limiter,
/// absent until set) and an enforce/don't-enforce gate. Blocking needs both —
/// a configured but gated-off dimension never blocks, and a gated-on dimension
/// with no configured value never blocks. Applies to the direct client only.
struct DimensionLimiter {
    name: &'static str,
    quota_kind: QuotaKind,
    evictions: IntCounter,
    configured: ArcSwapOption<ConfiguredLimiter>,
    enabled: AtomicBool,
}

impl DimensionLimiter {
    fn new(
        name: &'static str,
        quota_kind: QuotaKind,
        threshold: Option<u64>,
        evictions: IntCounter,
    ) -> Self {
        let configured = threshold.map(|threshold| {
            let threshold = clamp_threshold(threshold, name);
            Arc::new(ConfiguredLimiter {
                threshold,
                limiter: quota_kind.limiter(threshold, &evictions),
            })
        });
        Self {
            enabled: AtomicBool::new(configured.is_some()),
            configured: ArcSwapOption::new(configured),
            name,
            quota_kind,
            evictions,
        }
    }

    /// Whether charging `count` to `client` should block: only when enforced
    /// (gate on and a limiter configured) and the charge breaches. A zero count
    /// consumes nothing.
    fn breaches(&self, client: IpAddr, count: u32) -> bool {
        let Some(count) = NonZeroU32::new(count) else {
            return false;
        };
        if !self.enabled.load(Ordering::Relaxed) {
            return false;
        }
        self.configured
            .load()
            .as_ref()
            .is_some_and(|configured| configured.limiter.breaches_n(client, count))
    }

    /// Sets the value and/or the gate. A changed threshold rebuilds the
    /// limiter, discarding tracked client state; re-asserting the current
    /// threshold keeps it. Setting a threshold enforces it unless the caller
    /// also gates it off; `enabled` alone toggles an already-configured value,
    /// and enabling a dimension that has no configured value is rejected. Both
    /// `None` is a no-op.
    fn set(&self, threshold: Option<u64>, enabled: Option<bool>) -> Result<(), String> {
        if let Some(threshold) = threshold {
            let threshold = clamp_threshold(threshold, self.name);
            if self.configured.load().as_ref().map(|c| c.threshold) != Some(threshold) {
                self.configured.store(Some(Arc::new(ConfiguredLimiter {
                    threshold,
                    limiter: self.quota_kind.limiter(threshold, &self.evictions),
                })));
            }
        }
        // A supplied threshold enforces on its own; an explicit gate overrides.
        if let Some(enabled) = enabled.or(threshold.map(|_| true)) {
            if enabled && self.configured.load().is_none() {
                return Err(format!("set a {} threshold before enabling it", self.name));
            }
            self.enabled.store(enabled, Ordering::Relaxed);
        }
        Ok(())
    }

    /// The configured threshold (`None` if unset) and whether the gate is on.
    fn state(&self) -> (Option<u64>, bool) {
        (
            self.configured.load().as_ref().map(|c| c.threshold),
            self.enabled.load(Ordering::Relaxed),
        )
    }
}

/// Rate limits direct clients, and proxied clients when the policy configures a
/// separate threshold for them.
struct RateLimitPolicy {
    quota_kind: QuotaKind,
    direct: ArcSwap<DirectLimiter>,
    proxied: Option<Limiter>,
    response_items: DimensionLimiter,
    rendered_fields: DimensionLimiter,
    evictions: IntCounter,
}

impl RateLimitPolicy {
    fn new(
        quota_kind: QuotaKind,
        direct_threshold: u64,
        proxied_threshold: Option<u64>,
        response_items_threshold: Option<u64>,
        rendered_fields_threshold: Option<u64>,
        evictions: IntCounter,
    ) -> Self {
        Self {
            direct: ArcSwap::from_pointee(DirectLimiter::new(
                quota_kind,
                direct_threshold,
                &evictions,
            )),
            proxied: proxied_threshold.map(|threshold| quota_kind.limiter(threshold, &evictions)),
            response_items: DimensionLimiter::new(
                "response-items",
                quota_kind,
                response_items_threshold,
                evictions.clone(),
            ),
            rendered_fields: DimensionLimiter::new(
                "rendered-fields",
                quota_kind,
                rendered_fields_threshold,
                evictions.clone(),
            ),
            quota_kind,
            evictions,
        }
    }

    fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        let direct = self.direct.load();
        PolicyResponse {
            block_client: tally.direct.filter(|client| {
                // Every dimension is evaluated so each is charged independently;
                // the direct client is blocked when any one breaches. The
                // response-item and rendered-field dimensions apply to the
                // direct client only; proxied support is a later addition.
                let requests = direct.limiter.breaches(*client);
                let response_items = self.response_items.breaches(*client, tally.response_items);
                let rendered_fields = self
                    .rendered_fields
                    .breaches(*client, tally.rendered_fields);
                requests || response_items || rendered_fields
            }),
            block_proxied_client: tally.through_fullnode.filter(|client| {
                self.proxied
                    .as_ref()
                    .is_some_and(|limiter| limiter.breaches(*client))
            }),
        }
    }

    fn set_response_items(
        &self,
        threshold: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<(), String> {
        self.response_items.set(threshold, enabled)
    }

    fn set_rendered_fields(
        &self,
        threshold: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<(), String> {
        self.rendered_fields.set(threshold, enabled)
    }

    fn response_items_state(&self) -> (Option<u64>, bool) {
        self.response_items.state()
    }

    fn rendered_fields_state(&self) -> (Option<u64>, bool) {
        self.rendered_fields.state()
    }

    fn set_direct_threshold(&self, threshold: u64) {
        // Re-asserting the current threshold keeps every client's state.
        if self.direct.load().threshold == threshold {
            return;
        }
        self.direct.store(Arc::new(DirectLimiter::new(
            self.quota_kind,
            threshold,
            &self.evictions,
        )));
    }
}

enum Policy {
    NoOp,
    Limit(Box<RateLimitPolicy>),
    PanicOnInvocation,
}

/// Which of a traffic controller's two policies is being built. The
/// response-item and rendered-field dimensions are spam-only, so the error
/// policy ignores any dimension thresholds in its config.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PolicyRole {
    Spam,
    Error,
}

/// The spam or error policy a traffic controller charges its tallies against.
pub(super) struct TrafficControlPolicy(Policy);

impl TrafficControlPolicy {
    pub(super) fn from_policy_type(
        policy_type: &PolicyType,
        connection_blocklist_ttl_sec: u64,
        evictions: IntCounter,
        role: PolicyRole,
    ) -> Self {
        Self(match policy_type {
            PolicyType::NoOp => Policy::NoOp,
            PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold,
                burst_secs,
                response_items_threshold,
                rendered_fields_threshold,
            }) => {
                if *burst_secs == 0 {
                    fatal!("freq-threshold burst-secs must be more than zero");
                }
                // The response-item and rendered-field dimensions are spam-only.
                let (response_items_threshold, rendered_fields_threshold) = match role {
                    PolicyRole::Spam => (*response_items_threshold, *rendered_fields_threshold),
                    PolicyRole::Error => (None, None),
                };
                Policy::Limit(Box::new(RateLimitPolicy::new(
                    QuotaKind::Sustained {
                        burst_secs: *burst_secs,
                    },
                    clamp_threshold(*client_threshold, "client-threshold"),
                    Some(clamp_threshold(
                        *proxied_client_threshold,
                        "proxied-client-threshold",
                    )),
                    response_items_threshold,
                    rendered_fields_threshold,
                    evictions,
                )))
            }
            // The exact-count test policy only ever blocks the direct client.
            PolicyType::TestNConnIP(threshold) => Policy::Limit(Box::new(RateLimitPolicy::new(
                QuotaKind::ExactCount {
                    reset_period: exact_count_reset_period(connection_blocklist_ttl_sec),
                },
                *threshold,
                None,
                None,
                None,
                evictions,
            ))),
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
    /// limiter state; re-asserting the current threshold keeps it.
    pub(super) fn set_client_threshold(&self, threshold: u64) {
        if let Policy::Limit(policy) = &self.0 {
            policy.set_direct_threshold(threshold);
        }
    }

    /// Sets the response-item dimension's value and/or gate. A no-op for a
    /// policy that does not rate limit. See [`DimensionLimiter::set`].
    pub(super) fn set_response_items(
        &self,
        threshold: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<(), String> {
        match &self.0 {
            Policy::Limit(policy) => policy.set_response_items(threshold, enabled),
            _ => Ok(()),
        }
    }

    /// Sets the rendered-field dimension's value and/or gate. A no-op for a
    /// policy that does not rate limit. See [`DimensionLimiter::set`].
    pub(super) fn set_rendered_fields(
        &self,
        threshold: Option<u64>,
        enabled: Option<bool>,
    ) -> Result<(), String> {
        match &self.0 {
            Policy::Limit(policy) => policy.set_rendered_fields(threshold, enabled),
            _ => Ok(()),
        }
    }

    /// The response-item dimension's configured threshold and gate state.
    pub(super) fn response_items_state(&self) -> (Option<u64>, bool) {
        match &self.0 {
            Policy::Limit(policy) => policy.response_items_state(),
            _ => (None, false),
        }
    }

    /// The rendered-field dimension's configured threshold and gate state.
    pub(super) fn rendered_fields_state(&self) -> (Option<u64>, bool) {
        match &self.0 {
            Policy::Limit(policy) => policy.rendered_fields_state(),
            _ => (None, false),
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

    fn evictions() -> IntCounter {
        IntCounter::new("rate_limiter_evictions", "Number of evicted client states")
            .expect("valid metric")
    }

    fn freq_threshold(
        client_threshold: u64,
        proxied_client_threshold: u64,
        burst_secs: u64,
    ) -> TrafficControlPolicy {
        TrafficControlPolicy::from_policy_type(
            &PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold,
                burst_secs,
                ..Default::default()
            }),
            0,
            evictions(),
            PolicyRole::Spam,
        )
    }

    fn exact_count(threshold: u64, connection_blocklist_ttl_sec: u64) -> TrafficControlPolicy {
        TrafficControlPolicy::from_policy_type(
            &PolicyType::TestNConnIP(threshold),
            connection_blocklist_ttl_sec,
            evictions(),
            PolicyRole::Spam,
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

    #[test]
    fn breaches_n_under_burst_is_false() {
        // Burst of 100 cells; a single charge of 1 is well under it.
        let limiter = QuotaKind::Sustained { burst_secs: 1 }.limiter(100, &evictions());
        let client = IpAddr::from([10, 0, 0, 1]);
        assert!(!limiter.breaches_n(client, NonZeroU32::MIN));
    }

    #[test]
    fn breaches_n_over_burst_is_true() {
        // Burst of 100 cells. 60 fits; a second 60 does not, so the client is
        // rate-limited on the second charge.
        let limiter = QuotaKind::Sustained { burst_secs: 1 }.limiter(100, &evictions());
        let client = IpAddr::from([10, 0, 0, 2]);
        let sixty = NonZeroU32::new(60).unwrap();
        assert!(!limiter.breaches_n(client, sixty));
        assert!(limiter.breaches_n(client, sixty));
    }

    #[test]
    fn breaches_n_above_max_burst_is_true() {
        // A single charge larger than the whole burst can never be served, so
        // it breaches outright.
        let limiter = QuotaKind::Sustained { burst_secs: 1 }.limiter(100, &evictions());
        let client = IpAddr::from([10, 0, 0, 3]);
        let over_burst = NonZeroU32::new(200).unwrap();
        assert!(limiter.breaches_n(client, over_burst));
    }

    /// A FreqThreshold spam policy with `burst_secs: 1`, so each new dimension's
    /// burst equals its threshold.
    fn freq_threshold_with_dimensions(
        client_threshold: u64,
        response_items_threshold: Option<u64>,
        rendered_fields_threshold: Option<u64>,
    ) -> TrafficControlPolicy {
        TrafficControlPolicy::from_policy_type(
            &PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold,
                proxied_client_threshold: client_threshold,
                burst_secs: 1,
                response_items_threshold,
                rendered_fields_threshold,
            }),
            0,
            evictions(),
            PolicyRole::Spam,
        )
    }

    fn tally_with(client: IpAddr, response_items: u32, rendered_fields: u32) -> TrafficTally {
        let mut tally = TrafficTally::new(Some(client), None, None, Weight::one());
        tally.response_items = response_items;
        tally.rendered_fields = rendered_fields;
        tally
    }

    #[test]
    fn rendered_fields_over_threshold_blocks() {
        // Requests threshold is huge, so only the rendered-field dimension can
        // block. Burst 100: 60 fits, a second 60 does not.
        let policy = freq_threshold_with_dimensions(1_000_000, None, Some(100));
        let client = IpAddr::from([10, 1, 0, 1]);
        assert!(
            policy
                .charge(&tally_with(client, 0, 60))
                .block_client
                .is_none()
        );
        assert_eq!(
            policy.charge(&tally_with(client, 0, 60)).block_client,
            Some(client)
        );
    }

    #[test]
    fn response_items_over_threshold_blocks() {
        let policy = freq_threshold_with_dimensions(1_000_000, Some(100), None);
        let client = IpAddr::from([10, 1, 0, 2]);
        assert!(
            policy
                .charge(&tally_with(client, 60, 0))
                .block_client
                .is_none()
        );
        assert_eq!(
            policy.charge(&tally_with(client, 60, 0)).block_client,
            Some(client)
        );
    }

    #[test]
    fn zero_counts_never_block_new_dimensions() {
        // Tiny dimension thresholds, but a tally that charges neither dimension
        // consumes no cells and never blocks.
        let policy = freq_threshold_with_dimensions(1_000_000, Some(1), Some(1));
        let client = IpAddr::from([10, 1, 0, 3]);
        for _ in 0..10 {
            assert!(
                policy
                    .charge(&tally_with(client, 0, 0))
                    .block_client
                    .is_none()
            );
        }
    }

    #[test]
    fn disabled_dimension_never_blocks() {
        // Configured then gated off: no enforcement however large the count.
        let policy = freq_threshold_with_dimensions(1_000_000, None, Some(1));
        let client = IpAddr::from([10, 1, 0, 4]);
        policy.set_rendered_fields(None, Some(false)).unwrap();
        assert!(
            policy
                .charge(&tally_with(client, 0, 10_000))
                .block_client
                .is_none()
        );
    }

    #[test]
    fn a_dimension_breach_blocks_while_requests_are_under_limit() {
        // Requests threshold is huge; a single rendered-field charge above the
        // whole burst can never be served, so the client is blocked on it alone.
        let policy = freq_threshold_with_dimensions(1_000_000, None, Some(50));
        let client = IpAddr::from([10, 1, 0, 5]);
        assert_eq!(
            policy.charge(&tally_with(client, 0, 60)).block_client,
            Some(client)
        );
    }

    #[test]
    fn reasserting_a_dimension_threshold_keeps_client_state() {
        // Burst 100 on the response-item dimension: 60 fits, a second 60 does
        // not. Re-asserting the same threshold must keep the exhausted state,
        // exactly as set_client_threshold does for the direct limiter.
        let policy = freq_threshold_with_dimensions(1_000_000, Some(100), None);
        let client = IpAddr::from([10, 2, 0, 1]);
        assert!(
            policy
                .charge(&tally_with(client, 60, 0))
                .block_client
                .is_none()
        );
        assert_eq!(
            policy.charge(&tally_with(client, 60, 0)).block_client,
            Some(client)
        );

        policy.set_response_items(Some(100), None).unwrap();
        assert_eq!(
            policy.charge(&tally_with(client, 60, 0)).block_client,
            Some(client)
        );
    }

    #[test]
    fn overlarge_dimension_thresholds_are_clamped_in_state() {
        // A configured value above the max enforces the clamped value, so the
        // reported state must be the clamped value, not the raw one, both at
        // startup and after a runtime reconfiguration.
        let policy = freq_threshold_with_dimensions(1_000, Some(2_000_000_000), None);
        assert_eq!(policy.response_items_state().0, Some(MAX_CLIENT_THRESHOLD));

        policy
            .set_response_items(Some(2_000_000_000), None)
            .unwrap();
        assert_eq!(policy.response_items_state().0, Some(MAX_CLIENT_THRESHOLD));
    }

    #[test]
    fn setting_a_dimension_threshold_enforces_it() {
        // Setting a threshold through the admin path enforces it on its own; the
        // caller does not also have to flip the gate, matching the direct
        // threshold and the config path. Gating off stays explicit.
        let policy = freq_threshold_with_dimensions(1_000_000, None, None);
        let client = IpAddr::from([10, 4, 0, 1]);
        policy.set_response_items(Some(50), None).unwrap();
        assert_eq!(policy.response_items_state(), (Some(50), true));
        assert_eq!(
            policy.charge(&tally_with(client, 60, 0)).block_client,
            Some(client)
        );
    }

    #[test]
    fn the_error_policy_ignores_dimension_thresholds() {
        // The response-item and rendered-field dimensions are spam-only. An
        // error policy whose config sets them builds no dimension limiter, so a
        // tally far above the would-be threshold never blocks on them.
        let policy = TrafficControlPolicy::from_policy_type(
            &PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold: 1_000_000,
                proxied_client_threshold: 1_000_000,
                burst_secs: 1,
                response_items_threshold: Some(1),
                rendered_fields_threshold: Some(1),
            }),
            0,
            evictions(),
            PolicyRole::Error,
        );
        let client = IpAddr::from([10, 5, 0, 1]);
        assert_eq!(policy.response_items_state(), (None, false));
        assert_eq!(policy.rendered_fields_state(), (None, false));
        assert!(
            policy
                .charge(&tally_with(client, 10_000, 10_000))
                .block_client
                .is_none()
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
        // tally is the first to breach, for both the direct and the proxied client.
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

    #[test]
    fn test_direct_and_proxied_thresholds_are_independent() {
        // The direct quota is 2 per second with a 5 second burst. The proxied
        // quota is 1000 per second. Only the direct client breaches at tally 11.
        let policy = freq_threshold(2, 1000, 5);
        let direct = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let proxied = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
        let tally = TrafficTally::new(Some(direct), Some(proxied), None, Weight::one());
        for _ in 0..10 {
            policy.charge(&tally);
        }
        let response = policy.charge(&tally);
        assert_eq!(response.block_client, Some(direct));
        assert_eq!(response.block_proxied_client, None);
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

        // Re-asserting the same threshold keeps the exhausted state.
        policy.set_client_threshold(2);
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
        let policy = TrafficControlPolicy::from_policy_type(
            &PolicyType::NoOp,
            0,
            evictions(),
            PolicyRole::Spam,
        );
        assert_eq!(policy.client_threshold(), None);
        policy.set_client_threshold(10);
        assert_eq!(policy.client_threshold(), None);
    }

    #[test]
    fn test_cell_cache_is_bounded() {
        let evictions = evictions();
        let policy = TrafficControlPolicy::from_policy_type(
            &PolicyType::TestNConnIP(2),
            60,
            evictions.clone(),
            PolicyRole::Spam,
        );
        let evicted = IpAddr::V4(Ipv4Addr::from(0x0A00_0000));
        assert_eq!(policy.charge(&direct_tally(evicted)).block_client, None);

        // Flood enough distinct clients to fill the cache exactly.
        for i in 1..MAX_TRACKED_CLIENTS as u32 {
            policy.charge(&direct_tally(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i))));
        }
        assert_eq!(evictions.get(), 0);

        // One client past capacity pushes the first one out.
        let cached = IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + MAX_TRACKED_CLIENTS as u32));
        policy.charge(&direct_tally(cached));
        assert_eq!(evictions.get(), 1);

        // The evicted client's count was reset along with its cell, so its
        // next tally counts as its first again; a still-cached client keeps
        // its count and blocks on its second.
        assert_eq!(policy.charge(&direct_tally(evicted)).block_client, None);
        assert_eq!(
            policy.charge(&direct_tally(cached)).block_client,
            Some(cached)
        );
        // Re-admitting the evicted client displaced another; the cached one
        // displaced nothing.
        assert_eq!(evictions.get(), 2);

        // The counter outlives the limiter that a threshold change rebuilds.
        policy.set_client_threshold(3);
        assert_eq!(evictions.get(), 2);
    }
}
