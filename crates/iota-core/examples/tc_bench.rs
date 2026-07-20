// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Multi-threaded scenario harness comparing the traffic-controller tally
//! path implementations (channel-based baseline, inline mutex, keyed GCRA)
//! under identical load. Produces one JSON document per run.
//!
//! Scenarios:
//! - `s1` hot key: every thread hammers one IP (serial cost, contention)
//! - `s2` fleet: uniform load over `--ips` distinct IPs (map scaling)
//! - `s3` skew: zipf-distributed load over `--ips` IPs (realistic contention)
//! - `s4` churn: a fresh IP per tally (memory growth, eviction cost)
//! - `l1` lag idle: admission-to-block lag on a quiet system
//! - `l2` lag loaded: admission-to-block lag while fleet load is running
//! - `d1` determinism: % of runs where the threshold-crossing tally is visible
//!   to `check` immediately
//!
//! The channel-capacity sensitivity run (C1 in the plan) is `s1` with
//! `--channel-capacity 100` vs `6000`.
//!
//! Example:
//! ```sh
//! cargo run --release -p iota-core --example tc_bench -- \
//!     --implementation baseline --scenario s1 --threads 16 --json s1.json
//! ```

use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use clap::{Parser, ValueEnum};
use iota_core::traffic_controller::{
    TrafficController, gcra::GcraTrafficController, inline::InlineTrafficController,
    metrics::TrafficControllerMetrics, policies::TrafficTally,
};
use iota_types::traffic_control::{FreqThresholdConfig, PolicyConfig, PolicyType, Weight};
use prometheus_filtered::Registry;
use rand::{Rng, SeedableRng, rngs::SmallRng};
use serde_json::json;

/// IP ranges kept disjoint so load, crossing clients, and probes never
/// collide.
const FLEET_IP_BASE: u32 = 0x0A00_0000; // 10.0.0.0/8
const CHURN_IP_BASE: u32 = 0x0B00_0000; // 11.0.0.0/8
const CROSSING_IP_BASE: u32 = 0x0C00_0000; // 12.0.0.0/8

const TEST_N_CONN_THRESHOLD: u64 = 5;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Implementation {
    Baseline,
    Inline,
    Gcra,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Scenario {
    S1,
    S2,
    S3,
    S4,
    L1,
    L2,
    D1,
}

#[derive(Parser)]
#[command(about = "Traffic controller tally-path scenario benchmark")]
struct Args {
    #[arg(long, value_enum)]
    implementation: Implementation,
    #[arg(long, value_enum)]
    scenario: Scenario,
    /// Load threads (and tokio worker threads for the baseline consumer).
    #[arg(long, default_value_t = 4)]
    threads: usize,
    #[arg(long, default_value_t = 10)]
    duration_secs: u64,
    /// Load produced before this point is not recorded.
    #[arg(long, default_value_t = 2)]
    warmup_secs: u64,
    /// Distinct IPs for the fleet (s2) and skew (s3) scenarios.
    #[arg(long, default_value_t = 10_000)]
    ips: u32,
    #[arg(long, default_value_t = 1.0)]
    zipf_exponent: f64,
    #[arg(long, default_value_t = 6_000)]
    channel_capacity: usize,
    /// Trials for the lag (l1, l2) and determinism (d1) scenarios.
    #[arg(long, default_value_t = 1_000)]
    trials: u32,
    /// Write the JSON report here instead of stdout.
    #[arg(long)]
    json: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.threads)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let results = match args.scenario {
        Scenario::S1 | Scenario::S2 | Scenario::S3 | Scenario::S4 => run_load(&args, &runtime),
        Scenario::L1 => run_lag_idle(&args, &runtime),
        Scenario::L2 => run_lag_loaded(&args, &runtime),
        Scenario::D1 => run_determinism(&args, &runtime),
    };

    let report = json!({
        "implementation": name_of(args.implementation),
        "scenario": scenario_name(args.scenario),
        "threads": args.threads,
        "duration_secs": args.duration_secs,
        "warmup_secs": args.warmup_secs,
        "ips": args.ips,
        "zipf_exponent": args.zipf_exponent,
        "channel_capacity": args.channel_capacity,
        "trials": args.trials,
        "max_rss_kb": max_rss_kb(),
        "results": results,
    });
    let report = serde_json::to_string_pretty(&report).expect("failed to serialize report");
    match &args.json {
        Some(path) => std::fs::write(path, report).expect("failed to write JSON report"),
        None => println!("{report}"),
    }
}

fn name_of(implementation: Implementation) -> &'static str {
    match implementation {
        Implementation::Baseline => "baseline",
        Implementation::Inline => "inline",
        Implementation::Gcra => "gcra",
    }
}

fn scenario_name(scenario: Scenario) -> &'static str {
    match scenario {
        Scenario::S1 => "s1",
        Scenario::S2 => "s2",
        Scenario::S3 => "s3",
        Scenario::S4 => "s4",
        Scenario::L1 => "l1",
        Scenario::L2 => "l2",
        Scenario::D1 => "d1",
    }
}

// ===== implementations under test =====

/// The uniform surface all implementations are driven through.
trait TallySink: Send + Sync {
    fn tally(&self, tally: TrafficTally);
    fn is_blocked(&self, client: IpAddr) -> bool;
    /// Tallies fully accounted (for the baseline: received by the consumer).
    fn processed(&self) -> u64;
    /// Tallies dropped on channel overflow (baseline only).
    fn dropped(&self) -> u64;
    /// Per-client limiter cells currently held (GCRA only).
    fn tracked_clients(&self) -> Option<usize> {
        None
    }
    /// Periodic maintenance during load (GCRA idle-cell eviction).
    fn maintain(&self) {}
}

struct BaselineSink {
    controller: TrafficController,
    metrics: Arc<TrafficControllerMetrics>,
    handle: tokio::runtime::Handle,
}

impl TallySink for BaselineSink {
    fn tally(&self, tally: TrafficTally) {
        self.controller.tally(tally);
    }

    fn is_blocked(&self, client: IpAddr) -> bool {
        !self
            .handle
            .block_on(self.controller.check(&Some(client), &None))
    }

    fn processed(&self) -> u64 {
        self.metrics.tallies.get()
    }

    fn dropped(&self) -> u64 {
        self.metrics.tally_channel_overflow.get()
    }
}

struct InlineSink {
    controller: InlineTrafficController,
    metrics: Arc<TrafficControllerMetrics>,
}

impl TallySink for InlineSink {
    fn tally(&self, tally: TrafficTally) {
        self.controller.tally(tally);
    }

    fn is_blocked(&self, client: IpAddr) -> bool {
        !self.controller.check(&Some(client), &None)
    }

    fn processed(&self) -> u64 {
        self.metrics.tallies.get()
    }

    fn dropped(&self) -> u64 {
        0
    }
}

struct GcraSink {
    controller: GcraTrafficController,
    metrics: Arc<TrafficControllerMetrics>,
}

impl TallySink for GcraSink {
    fn tally(&self, tally: TrafficTally) {
        self.controller.tally(tally);
    }

    fn is_blocked(&self, client: IpAddr) -> bool {
        !self.controller.check(&Some(client), &None)
    }

    fn processed(&self) -> u64 {
        self.metrics.tallies.get()
    }

    fn dropped(&self) -> u64 {
        0
    }

    fn tracked_clients(&self) -> Option<usize> {
        Some(self.controller.tracked_clients())
    }

    fn maintain(&self) {
        self.controller.evict_idle();
    }
}

fn make_sink(
    implementation: Implementation,
    config: PolicyConfig,
    runtime: &tokio::runtime::Runtime,
) -> Arc<dyn TallySink> {
    let metrics = Arc::new(TrafficControllerMetrics::new(&Registry::new()));
    match implementation {
        Implementation::Baseline => Arc::new(BaselineSink {
            controller: runtime.block_on(TrafficController::init(config, metrics.clone(), None)),
            metrics,
            handle: runtime.handle().clone(),
        }),
        Implementation::Inline => Arc::new(InlineSink {
            controller: runtime.block_on(InlineTrafficController::new(config, metrics.clone())),
            metrics,
        }),
        Implementation::Gcra => Arc::new(GcraSink {
            controller: GcraTrafficController::new(config, metrics.clone()),
            metrics,
        }),
    }
}

fn base_policy_config(args: &Args, spam_policy_type: PolicyType) -> PolicyConfig {
    PolicyConfig {
        spam_policy_type,
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        connection_blocklist_ttl_sec: 600,
        proxy_blocklist_ttl_sec: 600,
        channel_capacity: args.channel_capacity,
        ..PolicyConfig::default()
    }
}

/// DoS-protection-like spam policy for the load scenarios (production shape,
/// production spam threshold).
fn load_policy_config(args: &Args) -> PolicyConfig {
    base_policy_config(
        args,
        PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold: 1_000,
            proxied_client_threshold: 1_000,
            window_size_secs: 5,
            update_interval_secs: 1,
            ..FreqThresholdConfig::default()
        }),
    )
}

/// Exact-count policy for the lag and determinism scenarios: the
/// `TEST_N_CONN_THRESHOLD`-th tally from an IP must block it.
fn crossing_policy_config(args: &Args) -> PolicyConfig {
    base_policy_config(args, PolicyType::TestNConnIP(TEST_N_CONN_THRESHOLD))
}

fn spam_tally(ip: IpAddr) -> TrafficTally {
    TrafficTally::new(Some(ip), None, None, Weight::one())
}

// ===== load scenarios (s1-s4) =====

fn run_load(args: &Args, runtime: &tokio::runtime::Runtime) -> serde_json::Value {
    let sink = make_sink(args.implementation, load_policy_config(args), runtime);
    let start = Instant::now();
    let recording_from = start + Duration::from_secs(args.warmup_secs);
    let deadline = recording_from + Duration::from_secs(args.duration_secs);

    let churn_counter = Arc::new(AtomicU32::new(0));
    let zipf_cdf = Arc::new(zipf_cdf(args.ips, args.zipf_exponent));

    let workers: Vec<_> = (0..args.threads.max(1))
        .map(|thread_id| {
            let sink = sink.clone();
            let scenario = args.scenario;
            let ips = args.ips;
            let churn_counter = churn_counter.clone();
            let zipf_cdf = zipf_cdf.clone();
            std::thread::spawn(move || {
                let mut rng = SmallRng::seed_from_u64(0x5EED + thread_id as u64);
                let mut histogram = Histogram::new();
                let mut attempts = 0u64;
                loop {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    let ip = match scenario {
                        Scenario::S1 => IpAddr::V4(Ipv4Addr::from(FLEET_IP_BASE)),
                        Scenario::S2 => {
                            IpAddr::V4(Ipv4Addr::from(FLEET_IP_BASE + rng.gen_range(0..ips)))
                        }
                        Scenario::S3 => {
                            let sample: f64 = rng.r#gen();
                            let rank = zipf_cdf.partition_point(|&c| c < sample) as u32;
                            IpAddr::V4(Ipv4Addr::from(FLEET_IP_BASE + rank.min(ips - 1)))
                        }
                        Scenario::S4 => {
                            let n = churn_counter.fetch_add(1, Ordering::Relaxed);
                            IpAddr::V4(Ipv4Addr::from(CHURN_IP_BASE.wrapping_add(n)))
                        }
                        _ => unreachable!("not a load scenario"),
                    };
                    let tally = spam_tally(ip);
                    let recording = now >= recording_from;
                    let op_start = Instant::now();
                    sink.tally(tally);
                    if recording {
                        histogram.record(op_start.elapsed().as_nanos() as u64);
                    }
                    attempts += 1;
                }
                (histogram, attempts)
            })
        })
        .collect();

    // Snapshot process CPU at the warmup boundary so warmup work is excluded
    // from the per-tally CPU figure (approximately: the consumer may still be
    // draining warmup backlog afterwards).
    std::thread::sleep(recording_from.saturating_duration_since(Instant::now()));
    let cpu_at_warmup_end = process_cpu_micros();

    // Periodic maintenance while the load runs (no-op except GCRA eviction).
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(1).min(deadline - Instant::now()));
        sink.maintain();
    }

    let mut histogram = Histogram::new();
    let mut attempts = 0u64;
    for worker in workers {
        let (worker_histogram, worker_attempts) = worker.join().expect("load worker panicked");
        histogram.merge(&worker_histogram);
        attempts += worker_attempts;
    }

    // Let the baseline consumer drain so its CPU cost is fully attributed.
    let drain_deadline = Instant::now() + Duration::from_secs(30);
    while sink.processed() + sink.dropped() < attempts && Instant::now() < drain_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let cpu_spent_micros = process_cpu_micros().saturating_sub(cpu_at_warmup_end);
    sink.maintain();

    let recorded = histogram.count;
    json!({
        "recorded_ops": recorded,
        "throughput_ops_per_sec": recorded as f64 / args.duration_secs as f64,
        "producer_ns": histogram.summary(),
        "attempted": attempts,
        "processed": sink.processed(),
        "dropped": sink.dropped(),
        "drop_rate": sink.dropped() as f64 / attempts.max(1) as f64,
        "cpu_us_per_recorded_op": cpu_spent_micros as f64 / recorded.max(1) as f64,
        "tracked_clients": sink.tracked_clients(),
    })
}

/// Cumulative distribution over ranks 1..=n with weight 1/rank^exponent.
fn zipf_cdf(n: u32, exponent: f64) -> Vec<f64> {
    let mut weights: Vec<f64> = (1..=n as u64)
        .map(|rank| 1.0 / (rank as f64).powf(exponent))
        .collect();
    let total: f64 = weights.iter().sum();
    let mut cumulative = 0.0;
    for weight in &mut weights {
        cumulative += *weight / total;
        *weight = cumulative;
    }
    weights
}

// ===== lag and determinism scenarios (l1, l2, d1) =====

/// Sends `TEST_N_CONN_THRESHOLD` tallies for a fresh IP and returns the time
/// from the threshold-crossing tally until the block is visible to `check`.
/// Under load the crossing tally may have been dropped (baseline), so the
/// client keeps retrying like a real attacker would.
fn time_to_block(sink: &dyn TallySink, client: IpAddr, retry: bool) -> Option<Duration> {
    for _ in 1..TEST_N_CONN_THRESHOLD {
        sink.tally(spam_tally(client));
    }
    let crossing_at = Instant::now();
    sink.tally(spam_tally(client));
    let timeout = crossing_at + Duration::from_secs(5);
    let mut next_retry = crossing_at + Duration::from_millis(1);
    while !sink.is_blocked(client) {
        let now = Instant::now();
        if now >= timeout {
            return None;
        }
        if retry && now >= next_retry {
            sink.tally(spam_tally(client));
            next_retry = now + Duration::from_millis(1);
        }
        std::hint::spin_loop();
    }
    Some(crossing_at.elapsed())
}

fn crossing_ip(trial: u32) -> IpAddr {
    IpAddr::V4(Ipv4Addr::from(CROSSING_IP_BASE + trial))
}

fn lag_report(lags: &Histogram, timeouts: u32, trials: u32) -> serde_json::Value {
    json!({
        "trials": trials,
        "timeouts": timeouts,
        "lag_ns": lags.summary(),
    })
}

fn run_lag_idle(args: &Args, runtime: &tokio::runtime::Runtime) -> serde_json::Value {
    let mut lags = Histogram::new();
    let mut timeouts = 0u32;
    for trial in 0..args.trials {
        // Fresh controller per trial: no channel backlog, no blocklist state.
        let sink = make_sink(args.implementation, crossing_policy_config(args), runtime);
        match time_to_block(sink.as_ref(), crossing_ip(trial), false) {
            Some(lag) => lags.record(lag.as_nanos() as u64),
            None => timeouts += 1,
        }
    }
    lag_report(&lags, timeouts, args.trials)
}

fn run_lag_loaded(args: &Args, runtime: &tokio::runtime::Runtime) -> serde_json::Value {
    let sink = make_sink(args.implementation, crossing_policy_config(args), runtime);
    let stop = Arc::new(AtomicBool::new(false));
    let load_threads = args.threads.saturating_sub(1).max(1);
    let workers: Vec<_> = (0..load_threads)
        .map(|thread_id| {
            let sink = sink.clone();
            let stop = stop.clone();
            let ips = args.ips;
            std::thread::spawn(move || {
                let mut rng = SmallRng::seed_from_u64(0x10AD + thread_id as u64);
                while !stop.load(Ordering::Relaxed) {
                    let ip = IpAddr::V4(Ipv4Addr::from(FLEET_IP_BASE + rng.gen_range(0..ips)));
                    sink.tally(spam_tally(ip));
                }
            })
        })
        .collect();

    // Let the fleet load fill the channel before measuring.
    std::thread::sleep(Duration::from_secs(args.warmup_secs));
    let mut lags = Histogram::new();
    let mut timeouts = 0u32;
    for trial in 0..args.trials {
        match time_to_block(sink.as_ref(), crossing_ip(trial), true) {
            Some(lag) => lags.record(lag.as_nanos() as u64),
            None => timeouts += 1,
        }
    }
    stop.store(true, Ordering::Relaxed);
    for worker in workers {
        worker.join().expect("load worker panicked");
    }
    lag_report(&lags, timeouts, args.trials)
}

fn run_determinism(args: &Args, runtime: &tokio::runtime::Runtime) -> serde_json::Value {
    let sink = make_sink(args.implementation, crossing_policy_config(args), runtime);
    let mut blocked_immediately = 0u32;
    let mut sent = 0u64;
    for trial in 0..args.trials {
        let client = crossing_ip(trial);
        for _ in 0..TEST_N_CONN_THRESHOLD {
            sink.tally(spam_tally(client));
        }
        sent += TEST_N_CONN_THRESHOLD;
        if sink.is_blocked(client) {
            blocked_immediately += 1;
        }
        // Drain between trials so each one measures immediacy, not backlog.
        let drain_deadline = Instant::now() + Duration::from_millis(100);
        while sink.processed() + sink.dropped() < sent && Instant::now() < drain_deadline {
            std::hint::spin_loop();
        }
    }
    json!({
        "trials": args.trials,
        "blocked_immediately": blocked_immediately,
        "blocked_immediately_pct": 100.0 * blocked_immediately as f64 / args.trials.max(1) as f64,
    })
}

// ===== measurement helpers =====

/// Log-bucketed latency histogram: 64 linear sub-buckets per power of two
/// (relative error <= 1.6%), fixed memory, cheap to merge.
struct Histogram {
    buckets: Vec<u64>,
    count: u64,
    max: u64,
}

const SUB_BUCKET_BITS: u32 = 6;
const SUB_BUCKETS: u64 = 1 << SUB_BUCKET_BITS;

impl Histogram {
    fn new() -> Self {
        let regions = 64 - SUB_BUCKET_BITS as usize + 1;
        Self {
            buckets: vec![0; (regions + 1) * SUB_BUCKETS as usize],
            count: 0,
            max: 0,
        }
    }

    fn index_of(value: u64) -> usize {
        let value = value.max(1);
        let exp = 63 - value.leading_zeros();
        if exp < SUB_BUCKET_BITS {
            return value as usize;
        }
        let region = (exp - SUB_BUCKET_BITS + 1) as usize;
        let sub = ((value >> (exp - SUB_BUCKET_BITS)) & (SUB_BUCKETS - 1)) as usize;
        region * SUB_BUCKETS as usize + sub
    }

    /// Lower bound of the bucket at `index` (inverse of `index_of`).
    fn value_of(index: usize) -> u64 {
        let region = index as u64 / SUB_BUCKETS;
        let sub = index as u64 % SUB_BUCKETS;
        if region == 0 {
            return sub;
        }
        (SUB_BUCKETS + sub) << (region - 1)
    }

    fn record(&mut self, value: u64) {
        self.buckets[Self::index_of(value)] += 1;
        self.count += 1;
        self.max = self.max.max(value);
    }

    fn merge(&mut self, other: &Self) {
        for (bucket, other_bucket) in self.buckets.iter_mut().zip(&other.buckets) {
            *bucket += other_bucket;
        }
        self.count += other.count;
        self.max = self.max.max(other.max);
    }

    fn percentile(&self, percentile: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let target = (percentile / 100.0 * self.count as f64).ceil() as u64;
        let mut seen = 0;
        for (index, bucket) in self.buckets.iter().enumerate() {
            seen += bucket;
            if seen >= target.max(1) {
                return Self::value_of(index);
            }
        }
        self.max
    }

    fn summary(&self) -> serde_json::Value {
        json!({
            "p50": self.percentile(50.0),
            "p90": self.percentile(90.0),
            "p99": self.percentile(99.0),
            "max": self.max,
            "count": self.count,
        })
    }
}

/// Process CPU time (user + system) in microseconds.
fn process_cpu_micros() -> u64 {
    let usage = rusage();
    let to_micros = |time: libc::timeval| time.tv_sec as u64 * 1_000_000 + time.tv_usec as u64;
    to_micros(usage.ru_utime) + to_micros(usage.ru_stime)
}

/// Peak resident set size in KiB.
fn max_rss_kb() -> u64 {
    let max_rss = rusage().ru_maxrss as u64;
    // ru_maxrss is bytes on macOS, KiB on Linux.
    if cfg!(target_os = "macos") {
        max_rss / 1024
    } else {
        max_rss
    }
}

fn rusage() -> libc::rusage {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage fills the struct we own; RUSAGE_SELF is always valid.
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(result, 0, "getrusage failed");
    // SAFETY: getrusage returned 0, so the struct is initialized.
    unsafe { usage.assume_init() }
}
