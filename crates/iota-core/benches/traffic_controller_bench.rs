// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Single-thread producer-cost benchmarks comparing the tally-path
//! implementations: channel-based [`TrafficController`], inline-mutex
//! [`InlineTrafficController`], and keyed-GCRA [`GcraTrafficController`].
//!
//! `tally` measures only what the request thread pays; for the channel-based
//! controller the sketch work happens in a background task (and tallies are
//! dropped once the channel is full — counts are printed after each run).
//! Multi-threaded contention and admission-to-block lag are covered by
//! `examples/tc_bench.rs`.

use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::{Duration, Instant},
};

use criterion::*;
use iota_core::traffic_controller::{
    TrafficController, gcra::GcraTrafficController, inline::InlineTrafficController,
    metrics::TrafficControllerMetrics, policies::TrafficTally,
};
use iota_types::traffic_control::{FreqThresholdConfig, PolicyConfig, PolicyType, Weight};
use prometheus_filtered::Registry;

/// IP pool size for the non-breaching scenario; large enough that no single
/// IP crosses the spam threshold during a measurement run.
const FLEET_SIZE: u32 = 65_536;

fn bench_policy_config(spam_policy_type: PolicyType) -> PolicyConfig {
    PolicyConfig {
        spam_policy_type,
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        connection_blocklist_ttl_sec: 600,
        proxy_blocklist_ttl_sec: 600,
        channel_capacity: 6_000,
        ..PolicyConfig::default()
    }
}

fn freq_threshold_config(client_threshold: u64) -> PolicyType {
    PolicyType::FreqThreshold(FreqThresholdConfig {
        client_threshold,
        proxied_client_threshold: client_threshold,
        window_size_secs: 5,
        update_interval_secs: 1,
        ..FreqThresholdConfig::default()
    })
}

fn fleet_ip(n: u32) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + (n % FLEET_SIZE)))
}

fn spam_tally(ip: IpAddr) -> TrafficTally {
    TrafficTally::new(Some(ip), None, None, Weight::one())
}

// Scenarios: `hot_ip` hammers one IP far past the threshold (worst case:
// every tally re-inserts the blocklist entry); `fleet` rotates through
// enough IPs that none breaches (steady-state accounting cost).
fn tally_bench(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let mut group = c.benchmark_group("tally");
    for (scenario, threshold) in [("hot_ip", 1_000), ("fleet", 1_000_000)] {
        let config = bench_policy_config(freq_threshold_config(threshold));
        let pick_ip = |n: u32| fleet_ip(if scenario == "hot_ip" { 0 } else { n });

        let metrics = Arc::new(TrafficControllerMetrics::new(&Registry::new()));
        let controller = rt.block_on(TrafficController::init(
            config.clone(),
            metrics.clone(),
            None,
        ));
        let mut n = 0u32;
        group.bench_function(BenchmarkId::new("baseline", scenario), |b| {
            b.iter(|| {
                n = n.wrapping_add(1);
                controller.tally(spam_tally(pick_ip(n)));
            })
        });
        eprintln!(
            "baseline/{scenario}: processed={} dropped={}",
            metrics.tallies.get(),
            metrics.tally_channel_overflow.get()
        );

        let controller = rt.block_on(InlineTrafficController::init_for_test(config.clone()));
        let mut n = 0u32;
        group.bench_function(BenchmarkId::new("inline", scenario), |b| {
            b.iter(|| {
                n = n.wrapping_add(1);
                controller.tally(spam_tally(pick_ip(n)));
            })
        });

        let controller = GcraTrafficController::init_for_test(config.clone());
        let mut n = 0u32;
        group.bench_function(BenchmarkId::new("gcra", scenario), |b| {
            b.iter(|| {
                n = n.wrapping_add(1);
                controller.tally(spam_tally(pick_ip(n)));
            })
        });
    }
    group.finish();
}

/// Blocks `count` distinct IPs via two tallies each against a
/// `TestNConnIP(2)` policy, then waits until all blocks are applied.
fn populate_blocklist(tally: impl Fn(TrafficTally), blocklist_len: impl Fn() -> i64, count: u32) {
    for n in 0..count {
        let tally_payload = spam_tally(fleet_ip(n));
        tally(tally_payload.clone());
        tally(tally_payload);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while blocklist_len() < count as i64 {
        assert!(
            Instant::now() < deadline,
            "blocklist did not reach {count} entries in time"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn check_bench(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let mut group = c.benchmark_group("check");
    // The probe IP is outside the populated range and never blocked.
    let probe = Some(fleet_ip(FLEET_SIZE - 1));
    for (scenario, blocked_count) in [("empty", 0u32), ("blocklist_1k", 1_000)] {
        let config = bench_policy_config(PolicyType::TestNConnIP(2));

        let metrics = Arc::new(TrafficControllerMetrics::new(&Registry::new()));
        let controller = rt.block_on(TrafficController::init(
            config.clone(),
            metrics.clone(),
            None,
        ));
        populate_blocklist(
            |t| controller.tally(t),
            || metrics.connection_ip_blocklist_len.get(),
            blocked_count,
        );
        group.bench_function(BenchmarkId::new("baseline", scenario), |b| {
            b.to_async(&rt).iter(|| controller.check(&probe, &None))
        });

        let metrics = Arc::new(TrafficControllerMetrics::new(&Registry::new()));
        let controller = rt.block_on(InlineTrafficController::new(
            config.clone(),
            metrics.clone(),
        ));
        populate_blocklist(
            |t| controller.tally(t),
            || metrics.connection_ip_blocklist_len.get(),
            blocked_count,
        );
        group.bench_function(BenchmarkId::new("inline", scenario), |b| {
            b.iter(|| controller.check(&probe, &None))
        });

        let metrics = Arc::new(TrafficControllerMetrics::new(&Registry::new()));
        let controller = GcraTrafficController::new(config.clone(), metrics.clone());
        populate_blocklist(
            |t| controller.tally(t),
            || metrics.connection_ip_blocklist_len.get(),
            blocked_count,
        );
        group.bench_function(BenchmarkId::new("gcra", scenario), |b| {
            b.iter(|| controller.check(&probe, &None))
        });
    }
    group.finish();
}

criterion_group!(benches, tally_bench, check_bench);
criterion_main!(benches);
