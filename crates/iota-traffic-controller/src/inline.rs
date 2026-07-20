// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Candidate replacement for the channel-based tally path: accounting runs
//! inline in [`tally`](InlineTrafficController::tally) under a
//! `parking_lot::Mutex` per policy, so a breached threshold is visible to
//! [`check`](InlineTrafficController::check) as soon as `tally` returns.
//!
//! Kept separate from [`TrafficController`](super::TrafficController) so both
//! can be compared under `benches/traffic_controller_bench.rs` and
//! `examples/tc_bench.rs`. Firewall delegation and allowlist mode are not
//! wired up.

use std::{
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use dashmap::DashMap;
use iota_types::traffic_control::PolicyConfig;
use parking_lot::Mutex;

use super::{
    Blocklists, account_tally, check_blocklists, check_with_dry_run,
    metrics::TrafficControllerMetrics,
    policies::{Policy, TrafficControlPolicy, TrafficTally},
};

pub struct InlineTrafficController {
    spam_policy: Mutex<TrafficControlPolicy>,
    error_policy: Mutex<TrafficControlPolicy>,
    blocklists: Blocklists,
    policy_config: PolicyConfig,
    dry_run: AtomicBool,
    metrics: Arc<TrafficControllerMetrics>,
}

impl InlineTrafficController {
    pub async fn new(policy_config: PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        let spam_policy = TrafficControlPolicy::from_spam_config(policy_config.clone()).await;
        let error_policy = TrafficControlPolicy::from_error_config(policy_config.clone()).await;
        Self {
            spam_policy: Mutex::new(spam_policy),
            error_policy: Mutex::new(error_policy),
            blocklists: Blocklists {
                clients: Arc::new(DashMap::new()),
                proxied_clients: Arc::new(DashMap::new()),
            },
            dry_run: AtomicBool::new(policy_config.dry_run),
            policy_config,
            metrics,
        }
    }

    pub async fn init_for_test(policy_config: PolicyConfig) -> Self {
        Self::new(
            policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
        )
        .await
    }

    /// Records a tally against the spam and error policies and applies any
    /// resulting block before returning.
    pub fn tally(&self, tally: TrafficTally) {
        account_tally(
            &tally,
            &self.policy_config,
            &self.blocklists,
            &self.metrics,
            |tally| self.spam_policy.lock().handle_tally(tally.clone()),
            |tally| self.error_policy.lock().handle_tally(tally.clone()),
        );
    }

    /// Handle check with dry-run mode considered
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run.load(Ordering::Relaxed);
        let allowed = check_blocklists(&self.blocklists, client, proxied_client, &self.metrics);
        check_with_dry_run(allowed, dry_run, client, &self.metrics)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use iota_types::traffic_control::{
        FreqThresholdConfig, PolicyConfig, PolicyType, Weight, default_connection_blocklist_ttl_sec,
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
        let controller = InlineTrafficController::init_for_test(test_policy_config(
            PolicyType::TestNConnIP(threshold),
        ))
        .await;
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

    // Burst behavior of the sliding-window policy: with threshold 2/s over a
    // 5s window, the 10th tally in a burst crosses the threshold, for both
    // the direct and the proxied dimension.
    #[tokio::test]
    async fn test_freq_threshold_blocks_at_window_burst() {
        let controller = InlineTrafficController::init_for_test(test_policy_config(
            PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold: 2,
                proxied_client_threshold: 2,
                window_size_secs: 5,
                update_interval_secs: 1,
                ..FreqThresholdConfig::default()
            }),
        ))
        .await;
        let client = Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        let proxied = Some(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)));
        for i in 1..10 {
            controller.tally(spam_tally(client, proxied));
            assert!(
                controller.check(&client, &proxied),
                "blocked early after {i} tallies"
            );
        }
        controller.tally(spam_tally(client, proxied));
        assert!(!controller.check(&client, &None), "direct not blocked");
        assert!(!controller.check(&None, &proxied), "proxied not blocked");
    }
}
