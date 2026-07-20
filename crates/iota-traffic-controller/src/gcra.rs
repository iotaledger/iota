// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Candidate replacement for the channel-based tally path using one GCRA
//! rate-limiter cell per client IP (`governor` crate), the same pattern
//! `anemo-tower`'s `RateLimitLayer` uses keyed by `PeerId`. Charging a tally
//! is a single atomic compare-and-swap on the client's cell — no lock — and a
//! breached quota is visible to [`check`](GcraTrafficController::check) as
//! soon as [`tally`](GcraTrafficController::tally) returns.
//!
//! Quota mapping (not bit-identical to the sliding-window sketch, which
//! resets in discrete intervals while GCRA replenishes continuously):
//! - `FreqThreshold`: sustained rate `client_threshold`/s with burst
//!   `client_threshold * window_size_secs`, the burst the sketch tolerates
//!   within one full window.
//! - `TestNConnIP(n)`: burst `n - 1` with a one-hour replenish period, so the
//!   n-th back-to-back tally breaches like the exact-count test policy.
//!
//! Kept separate from [`TrafficController`](super::TrafficController) so both
//! can be compared under `benches/traffic_controller_bench.rs` and
//! `examples/tc_bench.rs`. Firewall delegation and allowlist mode are not
//! wired up.

use std::{
    net::IpAddr,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use iota_types::traffic_control::{PolicyConfig, PolicyType};

use super::{
    Blocklists, account_tally, check_blocklists, check_with_dry_run,
    metrics::TrafficControllerMetrics,
    policies::{PolicyResponse, TrafficTally},
};

/// Per-client rate limiters for one accounting mode (spam or error), one
/// limiter per client identity dimension.
enum GcraPolicy {
    NoOp,
    Limit {
        direct: Box<DefaultKeyedRateLimiter<IpAddr>>,
        proxied: Option<Box<DefaultKeyedRateLimiter<IpAddr>>>,
    },
    PanicOnInvocation,
}

impl GcraPolicy {
    fn new(policy_type: &PolicyType) -> Self {
        match policy_type {
            PolicyType::NoOp => Self::NoOp,
            PolicyType::FreqThreshold(config) => Self::Limit {
                direct: Box::new(RateLimiter::keyed(freq_threshold_quota(
                    config.client_threshold,
                    config.window_size_secs,
                ))),
                proxied: Some(Box::new(RateLimiter::keyed(freq_threshold_quota(
                    config.proxied_client_threshold,
                    config.window_size_secs,
                )))),
            },
            // The exact-count test policy only ever blocks the direct client.
            PolicyType::TestNConnIP(n) => Self::Limit {
                direct: Box::new(RateLimiter::keyed(test_n_conn_quota(*n))),
                proxied: None,
            },
            PolicyType::TestPanicOnInvocation => Self::PanicOnInvocation,
        }
    }

    /// Charges one cell per configured client dimension, returning which
    /// clients breached their quota.
    fn charge(&self, tally: &TrafficTally) -> PolicyResponse {
        match self {
            Self::NoOp => PolicyResponse::default(),
            Self::PanicOnInvocation => panic!("Tally for this policy should never be invoked"),
            Self::Limit { direct, proxied } => PolicyResponse {
                block_client: tally
                    .direct
                    .filter(|client| direct.check_key(client).is_err()),
                block_proxied_client: match (proxied, tally.through_fullnode) {
                    (Some(limiter), Some(client)) if limiter.check_key(&client).is_err() => {
                        Some(client)
                    }
                    _ => None,
                },
            },
        }
    }

    fn evict_idle(&self) {
        if let Self::Limit { direct, proxied } = self {
            direct.retain_recent();
            direct.shrink_to_fit();
            if let Some(proxied) = proxied {
                proxied.retain_recent();
                proxied.shrink_to_fit();
            }
        }
    }

    fn tracked_clients(&self) -> usize {
        match self {
            Self::Limit { direct, proxied } => {
                direct.len() + proxied.as_ref().map_or(0, |limiter| limiter.len())
            }
            _ => 0,
        }
    }
}

fn freq_threshold_quota(threshold: u64, window_size_secs: u64) -> Quota {
    let rate = NonZeroU32::new(threshold.clamp(1, u32::MAX as u64) as u32)
        .expect("clamped rate is non-zero");
    let burst = NonZeroU32::new(
        threshold
            .saturating_mul(window_size_secs)
            .clamp(1, u32::MAX as u64) as u32,
    )
    .expect("clamped burst is non-zero");
    Quota::per_second(rate).allow_burst(burst)
}

/// For `n <= 1` the burst clamps to one cell, so the second tally breaches
/// rather than the first; exact-count thresholds below 2 are test-only
/// configurations this candidate does not reproduce.
fn test_n_conn_quota(n: u64) -> Quota {
    let burst = NonZeroU32::new(n.saturating_sub(1).clamp(1, u32::MAX as u64) as u32)
        .expect("clamped burst is non-zero");
    Quota::with_period(Duration::from_secs(60 * 60))
        .expect("one hour is a valid replenish period")
        .allow_burst(burst)
}

pub struct GcraTrafficController {
    spam_policy: GcraPolicy,
    error_policy: GcraPolicy,
    blocklists: Blocklists,
    policy_config: PolicyConfig,
    dry_run: AtomicBool,
    metrics: Arc<TrafficControllerMetrics>,
}

impl GcraTrafficController {
    pub fn new(policy_config: PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        Self {
            spam_policy: GcraPolicy::new(&policy_config.spam_policy_type),
            error_policy: GcraPolicy::new(&policy_config.error_policy_type),
            blocklists: Blocklists {
                clients: Arc::new(DashMap::new()),
                proxied_clients: Arc::new(DashMap::new()),
            },
            dry_run: AtomicBool::new(policy_config.dry_run),
            policy_config,
            metrics,
        }
    }

    pub fn init_for_test(policy_config: PolicyConfig) -> Self {
        Self::new(
            policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
        )
    }

    /// Records a tally against the spam and error policies and applies any
    /// resulting block before returning.
    pub fn tally(&self, tally: TrafficTally) {
        account_tally(
            &tally,
            &self.policy_config,
            &self.blocklists,
            &self.metrics,
            |tally| self.spam_policy.charge(tally),
            |tally| self.error_policy.charge(tally),
        );
    }

    /// Handle check with dry-run mode considered
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run.load(Ordering::Relaxed);
        let allowed = check_blocklists(&self.blocklists, client, proxied_client, &self.metrics);
        check_with_dry_run(allowed, dry_run, client, &self.metrics)
    }

    /// Drops per-client cells that have fully replenished; must run
    /// periodically to keep memory bounded under client churn.
    pub fn evict_idle(&self) {
        self.spam_policy.evict_idle();
        self.error_policy.evict_idle();
    }

    /// Number of per-client cells currently tracked across all limiters.
    pub fn tracked_clients(&self) -> usize {
        self.spam_policy.tracked_clients() + self.error_policy.tracked_clients()
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use iota_types::traffic_control::{
        FreqThresholdConfig, Weight, default_connection_blocklist_ttl_sec,
    };

    use super::*;

    fn test_policy_config(spam_policy_type: PolicyType) -> PolicyConfig {
        PolicyConfig {
            spam_policy_type,
            error_policy_type: PolicyType::NoOp,
            spam_sample_rate: Weight::one(),
            dry_run: false,
            connection_blocklist_ttl_sec: default_connection_blocklist_ttl_sec(),
            proxy_blocklist_ttl_sec: default_connection_blocklist_ttl_sec(),
            ..PolicyConfig::default()
        }
    }

    fn spam_tally(direct: Option<IpAddr>, proxied: Option<IpAddr>) -> TrafficTally {
        TrafficTally::new(direct, proxied, None, Weight::one())
    }

    // The channel-based controller cannot pass this without sleeps: the block
    // must be visible to `check` immediately after the threshold-crossing
    // tally, in every round.
    #[tokio::test]
    async fn test_block_visible_immediately_after_threshold() {
        let threshold = 5;
        let controller = GcraTrafficController::init_for_test(test_policy_config(
            PolicyType::TestNConnIP(threshold),
        ));
        for round in 0..1000u32 {
            let client = Some(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + round)));
            for i in 1..threshold {
                controller.tally(spam_tally(client, None));
                assert!(
                    controller.check(&client, &None),
                    "round {round}: blocked early after {i} tallies"
                );
            }
            controller.tally(spam_tally(client, None));
            assert!(
                !controller.check(&client, &None),
                "round {round}: tally {threshold} did not block"
            );
        }
    }

    #[test]
    fn test_freq_threshold_quota_mapping() {
        let quota = freq_threshold_quota(1000, 5);
        assert_eq!(quota.burst_size().get(), 5000);
        assert_eq!(quota.replenish_interval(), Duration::from_millis(1));

        // Degenerate thresholds still produce a valid quota.
        let quota = freq_threshold_quota(0, 0);
        assert_eq!(quota.burst_size().get(), 1);
    }

    #[test]
    fn test_idle_cells_are_evicted() {
        let controller = GcraTrafficController::init_for_test(test_policy_config(
            PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold: 1000,
                proxied_client_threshold: 1000,
                window_size_secs: 5,
                update_interval_secs: 1,
                ..FreqThresholdConfig::default()
            }),
        ));
        for i in 0..100u32 {
            let client = Some(IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + i)));
            controller.tally(spam_tally(client, None));
        }
        assert_eq!(controller.tracked_clients(), 100);
        // A single charge against a 5000-cell burst replenishes in 1ms, after
        // which the cell is indistinguishable from a fresh one and evictable.
        std::thread::sleep(Duration::from_millis(50));
        controller.evict_idle();
        assert_eq!(controller.tracked_clients(), 0);
    }
}
