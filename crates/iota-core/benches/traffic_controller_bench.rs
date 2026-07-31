// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Request-path cost of the traffic controller: `tally` charges the rate
//! limiters and applies any block inline, and `check` consults the blocklists.
//! Both run on every request, so both are measured here.

use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use criterion::*;
use iota_core::traffic_controller::{
    TrafficController, metrics::TrafficControllerMetrics, policies::TrafficTally,
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
        ..PolicyConfig::default()
    }
}

fn freq_threshold_config(client_threshold: u64) -> PolicyType {
    PolicyType::FreqThreshold(FreqThresholdConfig {
        client_threshold,
        proxied_client_threshold: client_threshold,
        window_size_secs: 5,
    })
}

fn fleet_ip(n: u32) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(0x0A00_0000 + (n % FLEET_SIZE)))
}

fn spam_tally(ip: IpAddr) -> TrafficTally {
    TrafficTally::new(Some(ip), None, None, Weight::one())
}

/// `init` spawns the blocklist expiry and idle eviction tasks, so it needs a
/// runtime context even though the measured calls are synchronous.
fn controller(config: PolicyConfig, runtime: &tokio::runtime::Runtime) -> TrafficController {
    let _guard = runtime.enter();
    TrafficController::init(
        config,
        Arc::new(TrafficControllerMetrics::new(&Registry::new())),
        None,
    )
}

// Scenarios: `hot_ip` hammers one IP far past the threshold (worst case:
// every tally re-inserts the blocklist entry); `fleet` rotates through
// enough IPs that none breaches (steady-state accounting cost).
fn tally_bench(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let mut group = c.benchmark_group("tally");
    for (scenario, threshold) in [("hot_ip", 1_000), ("fleet", 1_000_000)] {
        let config = bench_policy_config(freq_threshold_config(threshold));
        let pick_ip = |n: u32| fleet_ip(if scenario == "hot_ip" { 0 } else { n });
        let controller = controller(config, &runtime);
        let mut n = 0u32;
        group.bench_function(scenario, |b| {
            b.iter(|| {
                n = n.wrapping_add(1);
                controller.tally(spam_tally(pick_ip(n)));
            })
        });
    }
    group.finish();
}

/// Blocks `count` distinct IPs by charging each past a `TestNConnIP(2)`
/// threshold. Blocks are applied inline, so no wait is needed.
fn populate_blocklist(controller: &TrafficController, count: u32) {
    for n in 0..count {
        let tally = spam_tally(fleet_ip(n));
        controller.tally(tally.clone());
        controller.tally(tally);
    }
}

fn check_bench(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("failed to build tokio runtime");
    let mut group = c.benchmark_group("check");
    // The probe IP is outside the populated range and never blocked.
    let probe = Some(fleet_ip(FLEET_SIZE - 1));
    for (scenario, blocked_count) in [("empty", 0u32), ("blocklist_1k", 1_000)] {
        let config = bench_policy_config(PolicyType::TestNConnIP(2));
        let controller = controller(config, &runtime);
        populate_blocklist(&controller, blocked_count);
        group.bench_function(scenario, |b| b.iter(|| controller.check(&probe, &None)));
    }
    group.finish();
}

criterion_group!(benches, tally_bench, check_bench);
criterion_main!(benches);
