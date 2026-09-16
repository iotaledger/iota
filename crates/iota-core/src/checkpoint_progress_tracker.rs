// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use iota_types::{
    committee::EpochId, iota_system_state::IotaSystemStateTrait,
    messages_checkpoint::CheckpointTimestamp,
};
use strum::{EnumCount, IntoEnumIterator};
use tracing::{debug, info};

use crate::{
    authority::authority_store_tables::AuthorityPerpetualTables,
    checkpoints::{CheckpointStore, checkpoint_executor::utils::PipelineStage},
};

/// The `tracing` target of the progress lines to shorten the log lines.
const LOG_TARGET: &str = "iota_core::sync";

/// A node whose newest executed checkpoint is within this range is
/// reported without estimates.
const CAUGHT_UP_WITHIN: Duration = Duration::from_mins(5);

/// Formats a duration for progress lines, naming its two largest units.
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let (days, hours, minutes, seconds) = (
        secs / 86400,
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
    );
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Formats a count for progress lines, e.g. "123.4M" or "1.2k".
fn format_count(count: u64) -> String {
    if count >= 1_000_000_000 {
        format!("{:.1}B", count as f64 / 1e9)
    } else if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1e6)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1e3)
    } else {
        count.to_string()
    }
}

/// Tracks how fast the node executes chain history, in seconds of checkpoint
/// timestamps per wall-clock second.
///
/// For example, rate = 376.0 means execution of 376 seconds of history every
/// real second. A node keeping pace with the chain sits at 1.0.
#[derive(Default)]
struct HistoryExecutionRate {
    last_wall: Option<Instant>,
    last_executed_timestamp_ms: CheckpointTimestamp,
    rate: f64,
}

/// How many ticks the rate takes to follow a change in speed, so that the
/// estimates hold steady while the per-second throughput swings.
const SMOOTHING_TICKS: f64 = 10.0;

impl HistoryExecutionRate {
    /// Records the timestamp of the newest executed checkpoint at `now` and
    /// returns the updated rate. Returns 0.0 until two ticks with advancing
    /// chain time have been seen; a tick whose chain time did not advance
    /// leaves the estimate unchanged. The estimate follows changes in speed
    /// gradually over several ticks.
    fn update(&mut self, now: Instant, executed_timestamp_ms: CheckpointTimestamp) -> f64 {
        let Some(last_wall) = self.last_wall else {
            self.last_wall = Some(now);
            self.last_executed_timestamp_ms = executed_timestamp_ms;
            return self.rate;
        };
        let wall_elapsed_secs = now.duration_since(last_wall).as_secs_f64();
        let chain_elapsed_secs =
            executed_timestamp_ms.saturating_sub(self.last_executed_timestamp_ms) as f64 / 1000.0;
        // Leaving the mark where it is, so that the next tick measures across
        // the whole stall rather than reading the rate as though it had not
        // happened.
        if wall_elapsed_secs <= 0.0 || chain_elapsed_secs <= 0.0 {
            return self.rate;
        }

        let sample = chain_elapsed_secs / wall_elapsed_secs;
        // Seeded rather than averaged up from zero, so the first line after a
        // restart reports a usable figure.
        self.rate = if self.rate == 0.0 {
            sample
        } else {
            self.rate * (1.0 - 1.0 / SMOOTHING_TICKS) + sample / SMOOTHING_TICKS
        };
        self.last_wall = Some(now);
        self.last_executed_timestamp_ms = executed_timestamp_ms;
        self.rate
    }
}

/// Wall-clock time to close `behind` while moving through chain time at
/// `rate`, or `None` when the node is not gaining, or is gaining so slowly
/// that the wait is longer than a [`Duration`] can hold.
///
/// Hint: The chain keeps producing a second of history per second, so the gap
/// closes at whatever `rate` exceeds chain pace by — not at `rate` itself.
fn time_to_catch_up(behind: Duration, rate: f64) -> Option<Duration> {
    if rate <= 1.0 {
        return None;
    }
    Duration::try_from_secs_f64(behind.as_secs_f64() / (rate - 1.0)).ok()
}

/// Wall-clock time to reach the end of the current epoch, whose remaining
/// history is `remaining`, at `rate`. `None` when the rate is not yet known,
/// or the wait is longer than a [`Duration`] can hold.
fn time_to_epoch_end(remaining: Duration, rate: f64) -> Option<Duration> {
    if rate <= 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(remaining.as_secs_f64() / rate).ok()
}

/// Returns how far behind the node is, how long until the current epoch ends,
/// and how long until the node catches up, or `None` when the node is caught up
/// or not gaining.
fn sync_eta(
    checkpoint_store: &CheckpointStore,
    epoch: EpochId,
    chain_ms: CheckpointTimestamp,
    rate: f64,
) -> Option<String> {
    if rate <= 0.0 {
        return None;
    }
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let behind = Duration::from_millis(now_ms.saturating_sub(chain_ms));
    if behind < CAUGHT_UP_WITHIN {
        return None;
    }

    let epoch_eta = checkpoint_store
        .get_epoch_info(epoch)
        .ok()
        .flatten()
        .and_then(|info| {
            let ends_ms = info
                .start_timestamp_ms
                .saturating_add(info.system_state.epoch_duration_ms());
            let remaining = Duration::from_millis(ends_ms.saturating_sub(chain_ms));
            time_to_epoch_end(remaining, rate)
        })
        .map(format_duration)
        .unwrap_or_else(|| "unknown".to_string());

    let sync_eta = time_to_catch_up(behind, rate)
        .map(format_duration)
        .unwrap_or_else(|| "not gaining".to_string());

    Some(format!(
        ", {} behind, epoch ETA {epoch_eta}, sync ETA {sync_eta} ({rate:.0}x chain time)",
        format_duration(behind),
    ))
}

/// Shared progress tracker for checkpoint operations. Updated by the
/// checkpoint executor and pruner, periodically logs a one-line summary.
///
/// Passed as `Option<Arc<CheckpointProgressTracker>>` — callers that don't need
/// progress reporting (CLI tools, tests) simply pass `None`.
///
/// All values are accumulated and reset on each logging tick.
pub struct CheckpointProgressTracker {
    /// Accumulated checkpoint execution time in nanoseconds.
    execution_time_ns: AtomicU64,
    /// Accumulated time spent working in each checkpoint pipeline stage, in
    /// nanoseconds, indexed by [`PipelineStage`]. Since every stage admits
    /// one checkpoint at a time, a stage accumulating close to one second
    /// per second is the throughput bottleneck.
    stage_time_ns: [AtomicU64; PipelineStage::COUNT],
    /// Accumulated object pruning time in nanoseconds.
    object_pruning_time_ns: AtomicU64,
    /// Accumulated checkpoint/effects pruning time in nanoseconds.
    checkpoint_pruning_time_ns: AtomicU64,
}

impl CheckpointProgressTracker {
    pub fn new() -> Self {
        Self {
            execution_time_ns: AtomicU64::new(0),
            stage_time_ns: std::array::from_fn(|_| AtomicU64::new(0)),
            object_pruning_time_ns: AtomicU64::new(0),
            checkpoint_pruning_time_ns: AtomicU64::new(0),
        }
    }

    pub fn add_execution_time(&self, duration: Duration) {
        self.execution_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub(crate) fn add_stage_time(&self, stage: PipelineStage, duration: Duration) {
        self.stage_time_ns[stage as usize].fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn add_object_pruning_time(&self, duration: Duration) {
        self.object_pruning_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn add_checkpoint_pruning_time(&self, duration: Duration) {
        self.checkpoint_pruning_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Spawns a periodic logging task that prints a one-line checkpoint
    /// progress summary every second (only when there is actual progress).
    pub fn spawn_logging_task(
        self: &Arc<Self>,
        checkpoint_store: Arc<CheckpointStore>,
        perpetual_db: Arc<AuthorityPerpetualTables>,
    ) {
        let tracker = self.clone();
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut prev_executed: u64 = 0;
            let mut prev_synced: u64 = 0;
            let mut prev_total_tx: u64 = 0;
            let mut prev_obj_pruned: u64 = 0;
            let mut prev_ckpt_pruned: u64 = 0;
            let mut history_rate = HistoryExecutionRate::default();

            loop {
                interval.tick().await;

                let highest_executed_checkpoint = checkpoint_store
                    .get_highest_executed_checkpoint()
                    .ok()
                    .flatten();
                let epoch = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.epoch())
                    .unwrap_or(0);
                let highest_executed_seq_number = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.sequence_number())
                    .unwrap_or(0);
                let total_tx = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.network_total_transactions)
                    .unwrap_or(0);
                let chain_ms = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.timestamp_ms)
                    .unwrap_or(0);
                let synced_seq_number = checkpoint_store
                    .get_highest_synced_checkpoint_seq_number()
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                let object_pruned_seq_number = perpetual_db
                    .get_highest_pruned_checkpoint()
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                let checkpoint_pruned_seq_number = checkpoint_store
                    .get_highest_pruned_checkpoint_seq_number()
                    .ok()
                    .flatten()
                    .unwrap_or(0);

                let exec_delta = highest_executed_seq_number.saturating_sub(prev_executed);
                let synced_delta = synced_seq_number.saturating_sub(prev_synced);
                let tx_delta = total_tx.saturating_sub(prev_total_tx);
                let object_prune_delta = object_pruned_seq_number.saturating_sub(prev_obj_pruned);
                let checkpoint_prune_delta =
                    checkpoint_pruned_seq_number.saturating_sub(prev_ckpt_pruned);

                if exec_delta > 0
                    || synced_delta > 0
                    || tx_delta > 0
                    || object_prune_delta > 0
                    || checkpoint_prune_delta > 0
                {
                    let exec_time_delta_ns = tracker.execution_time_ns.swap(0, Ordering::Relaxed);
                    let exec_time_delta = Duration::from_nanos(exec_time_delta_ns);

                    let object_prune_time_delta_ns =
                        tracker.object_pruning_time_ns.swap(0, Ordering::Relaxed);
                    let object_prune_time_delta = Duration::from_nanos(object_prune_time_delta_ns);

                    let checkpoint_prune_time_delta_ns = tracker
                        .checkpoint_pruning_time_ns
                        .swap(0, Ordering::Relaxed);
                    let checkpoint_prune_time_delta =
                        Duration::from_nanos(checkpoint_prune_time_delta_ns);

                    // Only report pruning when there is some, to avoid cluttering the log
                    let pruning = if object_prune_delta > 0 || checkpoint_prune_delta > 0 {
                        format!(
                            ", objs pruned {object_pruned_seq_number} \
                             (+{object_prune_delta}, {object_prune_time_delta:.2?}), \
                             ckpts pruned {checkpoint_pruned_seq_number} \
                             (+{checkpoint_prune_delta}, {checkpoint_prune_time_delta:.2?})"
                        )
                    } else {
                        String::new()
                    };

                    let rate = history_rate.update(Instant::now(), chain_ms);
                    let eta =
                        sync_eta(&checkpoint_store, epoch, chain_ms, rate).unwrap_or_default();

                    info!(
                        target: LOG_TARGET,
                        "[epoch {epoch}]: executed {highest_executed_seq_number}/{synced_seq_number} \
                         (+{exec_delta}/+{synced_delta}, {} tx/s, {exec_time_delta:.2?}){pruning}{eta}",
                        format_count(tx_delta),
                    );

                    // Every pipeline stage admits one checkpoint at a time,
                    // so the stage whose time approaches one second per tick
                    // is the execution throughput bottleneck.
                    let stage_times: Vec<String> = PipelineStage::iter()
                        .filter_map(|stage| {
                            let stage_time_ns =
                                tracker.stage_time_ns[stage as usize].swap(0, Ordering::Relaxed);
                            (stage_time_ns > 0).then(|| {
                                format!(
                                    "{} {:.2?}",
                                    stage.as_str(),
                                    Duration::from_nanos(stage_time_ns)
                                )
                            })
                        })
                        .collect();
                    if !stage_times.is_empty() {
                        debug!(
                            target: LOG_TARGET,
                            "checkpoint pipeline stage times [epoch {epoch}]: {}",
                            stage_times.join(", ")
                        );
                    }

                    prev_executed = highest_executed_seq_number;
                    prev_synced = synced_seq_number;
                    prev_total_tx = total_tx;
                    prev_obj_pruned = object_pruned_seq_number;
                    prev_ckpt_pruned = checkpoint_pruned_seq_number;
                }
            }
        });
    }
}

impl Default for CheckpointProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rate is chain time over wall time: a tick that covers an hour of
    /// checkpoints in a second is moving at 3600x.
    #[test]
    fn the_rate_measures_chain_time_against_wall_time() {
        let mut rate = HistoryExecutionRate::default();
        let start = Instant::now();
        assert_eq!(
            rate.update(start, 0),
            0.0,
            "one sample cannot establish a rate"
        );
        let sampled = rate.update(start + Duration::from_secs(1), 3_600_000);
        assert!(
            (sampled - 3600.0).abs() < 1.0,
            "an hour of chain time in a wall second is 3600x, got {sampled}"
        );
    }

    /// A node whose checkpoints are not advancing has no rate, so no estimate
    /// can be made from it.
    #[test]
    fn a_stalled_node_reports_no_rate() {
        let mut rate = HistoryExecutionRate::default();
        let start = Instant::now();
        rate.update(start, 5_000);
        assert_eq!(
            rate.update(start + Duration::from_secs(1), 5_000),
            0.0,
            "chain time that did not move must not produce a rate"
        );
    }

    /// The estimate is smoothed, so one slow tick moves it rather than
    /// replacing it — otherwise the line would swing with every archive file.
    #[test]
    fn the_rate_is_smoothed_across_ticks() {
        let mut rate = HistoryExecutionRate::default();
        let start = Instant::now();
        rate.update(start, 0);
        let first = rate.update(start + Duration::from_secs(1), 100_000);
        let second = rate.update(start + Duration::from_secs(2), 100_000 + 1_000);
        assert!(
            second < first && second > 10.0,
            "a single slow tick must pull the estimate down without resetting it, \
             got {first} then {second}"
        );
    }

    /// The gap closes at the margin above chain pace, not at the rate: a node
    /// at 2x chain time needs a day of wall clock to make up a day, because
    /// the chain adds another day's worth while it works.
    #[test]
    fn catching_up_closes_the_gap_at_the_margin_above_chain_pace() {
        let a_day = Duration::from_secs(86_400);
        assert_eq!(time_to_catch_up(a_day, 2.0), Some(a_day));
        assert_eq!(time_to_catch_up(a_day, 3.0), Some(a_day / 2));
    }

    /// A rate a hair above chain pace divides by nearly nothing, overflowing
    /// the `Duration` the estimate is held in. That has to report no estimate
    /// rather than panic: this runs in the node's logging task, and the
    /// smoothed rate passes through chain pace whenever a node arrives at or
    /// falls back from it.
    #[test]
    fn a_rate_barely_above_chain_pace_reports_no_estimate() {
        let behind = Duration::from_secs(658 * 86_400);
        assert_eq!(time_to_catch_up(behind, 1.0 + f64::EPSILON), None);
    }

    /// The same overflow reaches the epoch estimate through a rate that has
    /// barely got going.
    #[test]
    fn a_rate_barely_above_zero_reports_no_epoch_estimate() {
        let remaining = Duration::from_secs(86_400);
        assert_eq!(time_to_epoch_end(remaining, f64::MIN_POSITIVE), None);
        assert_eq!(time_to_epoch_end(remaining, 0.0), None);
    }

    /// A node moving at chain pace or slower never arrives, so no estimate is
    /// offered rather than one that divides by nearly nothing.
    #[test]
    fn a_node_at_chain_pace_gets_no_estimate() {
        let a_day = Duration::from_secs(86_400);
        assert_eq!(time_to_catch_up(a_day, 1.0), None);
        assert_eq!(time_to_catch_up(a_day, 0.5), None);
    }

    #[test]
    fn format_duration_picks_the_two_largest_units() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(200)), "3m 20s");
        assert_eq!(format_duration(Duration::from_secs(6120)), "1h 42m");
        assert_eq!(format_duration(Duration::from_secs(280_800)), "3d 6h");
    }

    #[test]
    fn format_count_abbreviates_large_numbers() {
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_200), "1.2k");
        assert_eq!(format_count(123_400_000), "123.4M");
        assert_eq!(format_count(2_500_000_000), "2.5B");
    }
}
