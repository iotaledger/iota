// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Simulates pre-consensus load shedding on the consensus queue in the P-COOL
//! flow and compares the binary cutoff with graduated shedding.
//!
//! Every arrival goes through the real admission check,
//! [`AuthorityState::reserve_consensus_queue_slot`], against a
//! [`ConsensusAdapter`] configured with the policy under test, so an accepted
//! transaction holds a real slot in `num_inflight_transactions` and, when one
//! is free, a real submit permit. What the node does after admission is
//! replaced by a model of the consensus queue:
//! - a transaction holds its permit until a commit drains it, and can be drained only
//!   [`Load::pipeline_time`] after taking the permit, like a submission waiting for its consensus
//!   round trip; freed permits go to the oldest waiting transactions;
//! - the queue drains in commits: every [`Load::commit_interval`], up to `drain_rate ×
//!   commit_interval` of the oldest drainable transactions leave it, and their slots and permits
//!   are released;
//! - the semaphore has as many permits as the hard limit, unless a scenario sets today's count,
//!   `max_pending_transactions × 2 ÷ committee size`.
//!
//! The clock is tokio's paused clock: a simulated minute takes a fraction of
//! a second, and the arrivals, with their digests, are the same in every run
//! of a test. Graduated shedding seeds its hash draw from the wall clock, so
//! which transactions it rejects can change between runs; the statistics
//! reported here average over tens of thousands of draws.
//!
//! Run with `--no-capture` to see the tables the tests log.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use iota_sdk_types::{Address, ObjectReference, SenderSignedTransaction, Transaction};
use iota_types::{base_types::random_object_ref, transaction::TransactionAPI};
use tokio::{
    sync::SemaphorePermit,
    time::{Instant, interval_at},
};
use tracing::info;

use crate::{
    authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder},
    checkpoints::CheckpointStore,
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics, InflightSlot,
        MockConsensusClient,
    },
};

/// Rejection reasons the admission check counts in
/// `transaction_overload_sources` on the P-COOL path.
const REJECTION_LABELS: [&str; 5] = [
    "consensus_graduated_preventive",
    "consensus_graduated_saturated",
    "consensus_graduated_reactive",
    "consensus_max_pending_exceeded",
    "consensus_semaphore_no_permits",
];

const HARD_LIMIT: usize = 1_000;
const SOFT_LIMIT_PCT: u32 = 50;
const DRAIN_RATE: f64 = 1_000.0;
const COMMIT_INTERVAL: Duration = Duration::from_millis(100);
const PIPELINE_TIME: Duration = Duration::from_millis(200);
const WARM_UP: Duration = Duration::from_secs(20);
const MEASURED: Duration = Duration::from_secs(30);

/// Admission policy under test. A soft limit of 100% is the binary cutoff.
#[derive(Clone, Copy, Debug)]
struct Policy {
    name: &'static str,
    /// `max_pending_transactions`.
    hard_limit: usize,
    /// `graduated_load_shedding_soft_limit_pct`.
    soft_limit_pct: u32,
    /// Submit permits. The hard limit unless a scenario sets today's count.
    permits: usize,
}

impl Policy {
    fn binary(name: &'static str, hard_limit: usize) -> Self {
        Self {
            name,
            hard_limit,
            soft_limit_pct: 100,
            permits: hard_limit,
        }
    }

    fn graduated(name: &'static str, hard_limit: usize, soft_limit_pct: u32) -> Self {
        Self {
            name,
            hard_limit,
            soft_limit_pct,
            permits: hard_limit,
        }
    }

    fn with_permits(mut self, permits: usize) -> Self {
        self.permits = permits;
        self
    }

    fn soft_limit(&self) -> usize {
        self.hard_limit * self.soft_limit_pct as usize / 100
    }

    /// Queue level where graduated shedding settles under lasting overload:
    /// the level at which the rejection rate equals the share of arrivals
    /// the drain cannot take.
    fn settled_level(&self, overload: f64) -> f64 {
        let soft = self.soft_limit() as f64;
        soft + (self.hard_limit as f64 - soft) * (1.0 - 1.0 / overload)
    }
}

/// Arrivals and the modelled consensus behind the queue.
#[derive(Clone, Copy, Debug)]
struct Load {
    /// Steady arrivals per second.
    arrival_rate: f64,
    /// Transactions per second consensus sequences and processes.
    drain_rate: f64,
    /// Time between commits. Each commit drains `drain_rate ×
    /// commit_interval` transactions.
    commit_interval: Duration,
    /// Least time between a transaction taking its permit and a commit
    /// draining it.
    pipeline_time: Duration,
    /// Time before measurements start, for the queue to settle.
    warm_up: Duration,
    /// Length of the measured window.
    measured: Duration,
    /// A burst of extra arrivals in one instant, on top of the steady load.
    spike: Option<Spike>,
}

#[derive(Clone, Copy, Debug)]
struct Spike {
    size: usize,
    /// When the burst lands, measured from the end of the warm-up.
    at: Duration,
}

impl Load {
    fn steady(overload: f64) -> Self {
        Self {
            arrival_rate: DRAIN_RATE * overload,
            drain_rate: DRAIN_RATE,
            commit_interval: COMMIT_INTERVAL,
            pipeline_time: PIPELINE_TIME,
            warm_up: WARM_UP,
            measured: MEASURED,
            spike: None,
        }
    }

    fn with_spike(mut self, size: usize, at: Duration) -> Self {
        self.spike = Some(Spike { size, at });
        self
    }

    fn overload(&self) -> f64 {
        self.arrival_rate / self.drain_rate
    }

    /// Transactions one commit drains.
    fn commit_batch(&self) -> usize {
        (self.drain_rate * self.commit_interval.as_secs_f64()).round() as usize
    }
}

/// Builds the arrival sequence. Only the gas budget varies between
/// transactions, so each has its own digest and every run sees the same
/// sequence.
struct TransactionFactory {
    sender: Address,
    recipient: Address,
    object: ObjectReference,
    gas: ObjectReference,
}

impl TransactionFactory {
    fn new() -> Self {
        Self {
            sender: Address::random(),
            recipient: Address::random(),
            object: random_object_ref(),
            gas: random_object_ref(),
        }
    }

    fn make(&self, index: u64) -> SenderSignedTransaction {
        let tx = Transaction::new_transfer(
            self.recipient,
            self.object,
            self.sender,
            self.gas,
            1_000_000 + index,
            1_000,
        );
        SenderSignedTransaction::new(tx, Vec::new())
    }
}

/// An accepted transaction in the modelled consensus queue. Its slot stays
/// counted, and its permit, once it has one, stays taken, until the entry is
/// dropped.
struct Entry<'a> {
    slot: InflightSlot,
    permit: Option<SemaphorePermit<'a>>,
    /// When the transaction took its permit; `None` while it waits for one.
    submitted_at: Option<Instant>,
    accepted_at: Instant,
    /// Accepted inside the measured window as a steady arrival, so its wait
    /// counts.
    measured: bool,
}

/// One run: the admission check on one side, the modelled consensus queue on
/// the other.
struct Simulation<'a> {
    state: &'a AuthorityState,
    adapter: &'a Arc<ConsensusAdapter>,
    factory: &'a TransactionFactory,
    load: Load,
    window_start: Instant,
    window_end: Instant,
    queue: VecDeque<Entry<'a>>,
    next_index: u64,
    /// Steady arrivals inside the window.
    accepted: usize,
    rejected: usize,
    /// Time from admission to the commit that drained the transaction, for
    /// steady arrivals accepted inside the window.
    waits: Vec<Duration>,
    /// Queue length before and after each commit inside the window.
    samples: Vec<usize>,
}

impl Simulation<'_> {
    /// Runs one arrival through the admission check. Returns whether it was
    /// accepted.
    fn admit(&mut self, now: Instant, measured: bool) -> bool {
        let tx = self.factory.make(self.next_index);
        self.next_index += 1;
        match self.state.reserve_consensus_queue_slot(self.adapter, &tx) {
            Ok(slot) => {
                let permit = self.adapter.try_acquire_submit_permit_for_testing();
                let submitted_at = permit.is_some().then_some(now);
                self.queue.push_back(Entry {
                    slot,
                    permit,
                    submitted_at,
                    accepted_at: now,
                    measured,
                });
                true
            }
            Err(_) => false,
        }
    }

    /// One commit: drains up to one batch of transactions that have held
    /// their permit long enough, releasing their slots and permits, then
    /// hands the freed permits to the oldest waiting transactions.
    fn commit(&mut self, now: Instant) {
        let in_window = now >= self.window_start && now < self.window_end;
        if in_window {
            self.samples.push(self.queue.len());
        }
        let batch = self.load.commit_batch();
        let mut drained = 0;
        while drained < batch {
            match self.queue.front() {
                Some(entry)
                    if entry.submitted_at.is_some_and(|submitted_at| {
                        now.duration_since(submitted_at) >= self.load.pipeline_time
                    }) =>
                {
                    let entry = self.queue.pop_front().unwrap();
                    if entry.measured {
                        self.waits.push(now.duration_since(entry.accepted_at));
                    }
                    // Processed: the permit and the slot are released, as the
                    // submission task's guards release them.
                    drop(entry.permit);
                    drop(entry.slot);
                    drained += 1;
                }
                _ => break,
            }
        }
        // The semaphore serves its waiters in arrival order.
        for entry in self.queue.iter_mut() {
            if entry.permit.is_some() {
                continue;
            }
            match self.adapter.try_acquire_submit_permit_for_testing() {
                Some(permit) => {
                    entry.permit = Some(permit);
                    entry.submitted_at = Some(now);
                }
                None => break,
            }
        }
        if in_window {
            self.samples.push(self.queue.len());
        }
    }
}

/// What one run measured over its window.
struct Outcome {
    policy: Policy,
    accepted: usize,
    rejected: usize,
    /// Steady arrivals accepted per second.
    accepted_rate: f64,
    queue_mean: f64,
    queue_min: usize,
    queue_max: usize,
    wait_mean: Duration,
    wait_p50: Duration,
    wait_p99: Duration,
    /// Accepted out of the spike, when the load had one.
    spike_accepted: Option<usize>,
    /// Increase of each `transaction_overload_sources` label during the run.
    rejections: Vec<(&'static str, u64)>,
}

impl Outcome {
    fn reject_fraction(&self) -> f64 {
        self.rejected as f64 / (self.accepted + self.rejected) as f64
    }

    fn rejections_labelled(&self, label: &str) -> u64 {
        self.rejections
            .iter()
            .find(|(name, _)| *name == label)
            .map(|(_, count)| *count)
            .unwrap()
    }

    fn log(&self) {
        let spike = match self.spike_accepted {
            Some(accepted) => format!("  spike accepted {accepted}"),
            None => String::new(),
        };
        let rejections = self
            .rejections
            .iter()
            .filter(|(_, count)| *count > 0)
            .map(|(label, count)| format!("{label}={count}"))
            .collect::<Vec<_>>()
            .join(" ");
        info!(
            "{:<28} reject {:>5.1}%  accepted {:>6.0}/s  queue mean {:>6.1} (min {:>4}, max {:>4})  wait mean {:.3} s, p50 {:.3} s, p99 {:.3} s{}  [{}]",
            self.policy.name,
            self.reject_fraction() * 100.0,
            self.accepted_rate,
            self.queue_mean,
            self.queue_min,
            self.queue_max,
            self.wait_mean.as_secs_f64(),
            self.wait_p50.as_secs_f64(),
            self.wait_p99.as_secs_f64(),
            spike,
            rejections,
        );
    }
}

fn log_model(policy: Policy, load: Load) {
    let settled = policy.settled_level(load.overload());
    info!(
        "{:<28} reject {:>5.1}%  accepted {:>6.0}/s  queue      {:>6.1}                        wait      {:.3} s   (model: soft + (hard - soft) × (1 - drain / arrival); wait = queue / drain)",
        format!("model for {}", policy.name),
        (1.0 - 1.0 / load.overload()) * 100.0,
        load.drain_rate,
        settled,
        settled / load.drain_rate,
    );
}

fn adapter_for(state: &AuthorityState, policy: Policy) -> Arc<ConsensusAdapter> {
    Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        policy.hard_limit,
        policy.permits,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        policy.soft_limit_pct,
        100,
        true,
    ))
}

fn rejection_counts(state: &AuthorityState) -> Vec<u64> {
    REJECTION_LABELS
        .iter()
        .map(|label| {
            state
                .metrics
                .transaction_overload_sources
                .with_label_values(&[*label])
                .get()
        })
        .collect()
}

/// Replays `load` through `policy` and measures the window.
async fn run(
    state: &Arc<AuthorityState>,
    factory: &TransactionFactory,
    policy: Policy,
    load: Load,
) -> Outcome {
    let adapter = adapter_for(state, policy);
    let rejections_before = rejection_counts(state);

    let start = Instant::now();
    let window_start = start + load.warm_up;
    let window_end = window_start + load.measured;
    let mut spike_pending = load
        .spike
        .map(|spike| (window_start + spike.at, spike.size));
    let mut spike_accepted = None;

    let mut arrivals = interval_at(start, Duration::from_secs_f64(1.0 / load.arrival_rate));
    let mut commits = interval_at(start + load.commit_interval, load.commit_interval);

    let mut sim = Simulation {
        state,
        adapter: &adapter,
        factory,
        load,
        window_start,
        window_end,
        queue: VecDeque::new(),
        next_index: 0,
        accepted: 0,
        rejected: 0,
        waits: Vec::new(),
        samples: Vec::new(),
    };

    let mut now = start;
    while now < window_end {
        tokio::select! {
            // A commit and an arrival at the same instant: the commit frees
            // room first.
            biased;
            tick = commits.tick() => {
                now = tick;
                sim.commit(now);
            }
            tick = arrivals.tick() => {
                now = tick;
                if let Some((at, size)) = spike_pending {
                    if now >= at {
                        spike_accepted = Some((0..size).filter(|_| sim.admit(now, false)).count());
                        spike_pending = None;
                    }
                }
                if now < window_end {
                    let measured = now >= window_start;
                    if sim.admit(now, measured) {
                        if measured {
                            sim.accepted += 1;
                        }
                    } else if measured {
                        sim.rejected += 1;
                    }
                }
            }
        }
    }

    // Drain what was accepted inside the window, so its waiting time is
    // known.
    while sim.queue.iter().any(|entry| entry.measured) {
        let tick = commits.tick().await;
        sim.commit(tick);
    }

    let Simulation {
        accepted,
        rejected,
        mut waits,
        samples,
        ..
    } = sim;
    assert_eq!(
        adapter.num_inflight_transactions(),
        0,
        "{}: every slot should be released once the queue is dropped",
        policy.name
    );
    assert!(
        !waits.is_empty(),
        "{}: no transaction was accepted",
        policy.name
    );
    waits.sort();
    let percentile = |q: f64| waits[((waits.len() - 1) as f64 * q).round() as usize];
    let rejections = rejection_counts(state)
        .into_iter()
        .zip(rejections_before)
        .zip(REJECTION_LABELS)
        .map(|((after, before), label)| (label, after - before))
        .collect();

    Outcome {
        policy,
        accepted,
        rejected,
        accepted_rate: accepted as f64 / load.measured.as_secs_f64(),
        queue_mean: samples.iter().sum::<usize>() as f64 / samples.len() as f64,
        queue_min: *samples.iter().min().unwrap(),
        queue_max: *samples.iter().max().unwrap(),
        wait_mean: waits.iter().sum::<Duration>() / waits.len() as u32,
        wait_p50: percentile(0.5),
        wait_p99: percentile(0.99),
        spike_accepted,
        rejections,
    }
}

fn assert_close(what: &str, measured: f64, expected: f64, tolerance: f64) {
    assert!(
        (measured - expected).abs() <= tolerance,
        "{what}: measured {measured:.2}, expected {expected:.2} ± {tolerance:.2}"
    );
}

/// Every policy rejects the share of arrivals the drain cannot take; the
/// policies differ in where the queue settles.
fn assert_rejects_the_excess(outcome: &Outcome, load: Load) {
    assert_close(
        &format!("{}: rejected fraction", outcome.policy.name),
        outcome.reject_fraction(),
        1.0 - 1.0 / load.overload(),
        0.02,
    );
}

/// At 1.25× overload, graduated shedding holds the queue at about 60% of the
/// cap while the binary cutoff keeps it within one commit of the cap, so
/// accepted transactions wait about 40% less. A binary cutoff at a lower cap
/// matches graduated shedding's waiting time at this overload.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn graduated_shedding_settles_the_queue_lower_than_the_binary_cutoff() {
    telemetry_subscribers::init_for_testing();
    let state = TestAuthorityBuilder::new().build().await;
    let factory = TransactionFactory::new();

    let load = Load::steady(1.25);
    let batch = load.commit_batch();
    let graduated = Policy::graduated("graduated, soft 50%", HARD_LIMIT, SOFT_LIMIT_PCT);
    let binary = Policy::binary("binary at the cap", HARD_LIMIT);
    let settled = graduated.settled_level(load.overload());
    // The queue under a binary cutoff swings one commit below the cap and
    // back, so a cap half a commit above the settled level gives the same
    // mean queue length as graduated shedding.
    let matched = Policy::binary(
        "binary at a matched cap",
        (settled + batch as f64 / 2.0).round() as usize,
    );

    info!(
        "{:.2}× overload: {} arrivals/s, drain {}/s, commits every {:?} ({} per commit), caps: binary {}, matched {}",
        load.overload(),
        load.arrival_rate,
        load.drain_rate,
        load.commit_interval,
        batch,
        binary.hard_limit,
        matched.hard_limit,
    );
    let binary_outcome = run(&state, &factory, binary, load).await;
    let graduated_outcome = run(&state, &factory, graduated, load).await;
    let matched_outcome = run(&state, &factory, matched, load).await;
    for outcome in [&binary_outcome, &graduated_outcome, &matched_outcome] {
        outcome.log();
    }
    log_model(graduated, load);

    for outcome in [&binary_outcome, &graduated_outcome, &matched_outcome] {
        assert_rejects_the_excess(outcome, load);
    }
    assert_close(
        "graduated: mean queue length against the model",
        graduated_outcome.queue_mean,
        settled,
        0.07 * settled,
    );
    assert!(
        binary_outcome.queue_mean >= (HARD_LIMIT - batch) as f64
            && binary_outcome.queue_mean <= HARD_LIMIT as f64,
        "binary: mean queue length {:.1} should sit within one commit ({batch}) of the cap {HARD_LIMIT}",
        binary_outcome.queue_mean,
    );
    assert!(
        graduated_outcome.wait_mean.as_secs_f64() < 0.75 * binary_outcome.wait_mean.as_secs_f64(),
        "graduated mean wait {:?} should be well below the binary mean wait {:?}",
        graduated_outcome.wait_mean,
        binary_outcome.wait_mean,
    );
    assert_close(
        "matched binary cap: mean wait against graduated shedding (s)",
        matched_outcome.wait_mean.as_secs_f64(),
        graduated_outcome.wait_mean.as_secs_f64(),
        0.1 * graduated_outcome.wait_mean.as_secs_f64(),
    );
}

/// The queue under graduated shedding settles where the model says at every
/// overload level, while the binary cutoff keeps it at the cap.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn graduated_shedding_follows_the_model_across_overload_levels() {
    telemetry_subscribers::init_for_testing();
    let state = TestAuthorityBuilder::new().build().await;
    let factory = TransactionFactory::new();
    let graduated = Policy::graduated("graduated, soft 50%", HARD_LIMIT, SOFT_LIMIT_PCT);
    let binary = Policy::binary("binary at the cap", HARD_LIMIT);

    for overload in [1.05, 1.1, 1.25, 1.5, 2.0, 3.0] {
        let load = Load::steady(overload);
        info!("{overload:.2}× overload");
        let binary_outcome = run(&state, &factory, binary, load).await;
        let graduated_outcome = run(&state, &factory, graduated, load).await;
        binary_outcome.log();
        graduated_outcome.log();
        log_model(graduated, load);

        assert_rejects_the_excess(&binary_outcome, load);
        assert_rejects_the_excess(&graduated_outcome, load);
        let settled = graduated.settled_level(overload);
        assert_close(
            &format!("graduated at {overload:.2}×: mean queue length against the model"),
            graduated_outcome.queue_mean,
            settled,
            0.07 * settled,
        );
        assert!(
            binary_outcome.queue_mean >= (HARD_LIMIT - load.commit_batch()) as f64,
            "binary at {overload:.2}×: mean queue length {:.1} should sit within one commit of the cap",
            binary_outcome.queue_mean,
        );
    }
}

/// A burst on top of mild overload: graduated shedding accepts most of it,
/// because the queue settles below the cap. A binary cutoff, at the default
/// cap or at a lower matched cap, has at most one commit of room, and none
/// when the burst lands just before a commit.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn graduated_shedding_absorbs_a_spike_that_the_binary_cutoff_rejects() {
    telemetry_subscribers::init_for_testing();
    let state = TestAuthorityBuilder::new().build().await;
    let factory = TransactionFactory::new();

    let overload = 1.1;
    let spike_size = 200;
    let batch = Load::steady(overload).commit_batch();
    let graduated = Policy::graduated("graduated, soft 50%", HARD_LIMIT, SOFT_LIMIT_PCT);
    let binary = Policy::binary("binary at the cap", HARD_LIMIT);
    let matched = Policy::binary(
        "binary at a matched cap",
        (graduated.settled_level(overload) + batch as f64 / 2.0).round() as usize,
    );

    // Commits fall on multiples of the commit interval from the start, and
    // the warm-up is a multiple of it, so 10 s into the window is a commit.
    let commit_at = Duration::from_secs(10);
    for (phase, at) in [
        ("just before a commit", commit_at - Duration::from_millis(2)),
        ("just after a commit", commit_at + Duration::from_millis(2)),
    ] {
        let load = Load::steady(overload).with_spike(spike_size, at);
        info!("spike of {spike_size} {phase}, on {overload:.2}× overload");
        let outcomes = [
            run(&state, &factory, binary, load).await,
            run(&state, &factory, graduated, load).await,
            run(&state, &factory, matched, load).await,
        ];
        for outcome in &outcomes {
            outcome.log();
        }

        let [binary_outcome, graduated_outcome, matched_outcome] = outcomes;
        let graduated_accepted = graduated_outcome.spike_accepted.unwrap();
        assert!(
            graduated_accepted * 100 >= spike_size * 55,
            "graduated accepted {graduated_accepted} of a spike of {spike_size} {phase}; expected most of it",
        );
        for outcome in [&binary_outcome, &matched_outcome] {
            let got = outcome.spike_accepted.unwrap();
            assert!(
                got <= batch,
                "{} accepted {got} of the spike {phase}; a binary cutoff has at most one commit ({batch}) of room",
                outcome.policy.name,
            );
            if phase == "just before a commit" {
                assert!(
                    got * 100 <= spike_size * 5,
                    "{} accepted {got} of the spike {phase}; expected close to none",
                    outcome.policy.name,
                );
            }
        }
    }
}

/// A binary cutoff lowered to match graduated shedding at 1.25× overload
/// matches it there only: at 1.05× it holds the queue higher than graduated
/// shedding does, and at 2× lower.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_matched_binary_cap_fits_one_overload_level_only() {
    telemetry_subscribers::init_for_testing();
    let state = TestAuthorityBuilder::new().build().await;
    let factory = TransactionFactory::new();
    let graduated = Policy::graduated("graduated, soft 50%", HARD_LIMIT, SOFT_LIMIT_PCT);
    let tuned_for = Load::steady(1.25);
    let matched = Policy::binary(
        "binary, cap matched at 1.25×",
        (graduated.settled_level(tuned_for.overload()) + tuned_for.commit_batch() as f64 / 2.0)
            .round() as usize,
    );

    let mild = Load::steady(1.05);
    let heavy = Load::steady(2.0);
    let graduated_mild = run(&state, &factory, graduated, mild).await;
    let matched_mild = run(&state, &factory, matched, mild).await;
    let graduated_heavy = run(&state, &factory, graduated, heavy).await;
    let matched_heavy = run(&state, &factory, matched, heavy).await;
    info!("{:.2}× overload", mild.overload());
    graduated_mild.log();
    matched_mild.log();
    info!("{:.2}× overload", heavy.overload());
    graduated_heavy.log();
    matched_heavy.log();

    assert!(
        graduated_mild.wait_mean.as_secs_f64() < 0.95 * matched_mild.wait_mean.as_secs_f64(),
        "at 1.05× graduated shedding should wait less ({:?}) than the matched cap ({:?})",
        graduated_mild.wait_mean,
        matched_mild.wait_mean,
    );
    assert!(
        matched_heavy.wait_mean.as_secs_f64() < 0.95 * graduated_heavy.wait_mean.as_secs_f64(),
        "at 2× the matched cap should wait less ({:?}) than graduated shedding ({:?})",
        matched_heavy.wait_mean,
        graduated_heavy.wait_mean,
    );
}

/// With today's permit count, `max_pending_transactions × 2 ÷ committee
/// size`, the permit check holds the queue at the permit count, which is the
/// soft limit at 4 validators and below it at every larger size, so neither
/// policy ever decides: both reject the same share for the same reason and
/// wait the same. With a large committee the permits also cap throughput
/// below what consensus drains.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn todays_permit_count_leaves_both_policies_idle() {
    telemetry_subscribers::init_for_testing();
    let state = TestAuthorityBuilder::new().build().await;
    let factory = TransactionFactory::new();
    let load = Load::steady(1.1);

    for committee_size in [4usize, 50] {
        let permits = HARD_LIMIT * 2 / committee_size;
        let graduated = Policy::graduated("graduated, soft 50%", HARD_LIMIT, SOFT_LIMIT_PCT)
            .with_permits(permits);
        let binary = Policy::binary("binary at the cap", HARD_LIMIT).with_permits(permits);
        // What the permits allow through. A permit is taken just after the
        // commit that freed it and held until the first commit at least the
        // pipeline time later, so it is held for one commit more than the
        // pipeline time spans.
        let commit_interval = load.commit_interval.as_secs_f64();
        let commits_held = (load.pipeline_time.as_secs_f64() / commit_interval).floor() + 1.0;
        let permit_limited_rate = permits as f64 / (commits_held * commit_interval);
        info!(
            "{committee_size} validators: {permits} permits, soft limit {}, {:.2}× overload; permits allow {permit_limited_rate:.0}/s",
            graduated.soft_limit(),
            load.overload(),
        );
        let binary_outcome = run(&state, &factory, binary, load).await;
        let graduated_outcome = run(&state, &factory, graduated, load).await;
        binary_outcome.log();
        graduated_outcome.log();

        for outcome in [&binary_outcome, &graduated_outcome] {
            let name = outcome.policy.name;
            assert!(
                outcome.queue_max <= permits,
                "{name} at {committee_size} validators: queue reached {}, above the {permits} permits",
                outcome.queue_max,
            );
            for label in REJECTION_LABELS {
                let count = outcome.rejections_labelled(label);
                if label == "consensus_semaphore_no_permits" {
                    assert!(count > 0, "{name}: expected rejections for lack of permits");
                } else {
                    assert_eq!(count, 0, "{name}: {label} should never fire");
                }
            }
            let expected_rate = load.drain_rate.min(permit_limited_rate);
            assert_close(
                &format!("{name} at {committee_size} validators: accepted per second"),
                outcome.accepted_rate,
                expected_rate,
                0.1 * expected_rate,
            );
        }
        assert_close(
            "binary against graduated: mean queue length",
            binary_outcome.queue_mean,
            graduated_outcome.queue_mean,
            0.02 * graduated_outcome.queue_mean,
        );
        assert_close(
            "binary against graduated: mean wait (s)",
            binary_outcome.wait_mean.as_secs_f64(),
            graduated_outcome.wait_mean.as_secs_f64(),
            0.02 * graduated_outcome.wait_mean.as_secs_f64(),
        );
    }
}
