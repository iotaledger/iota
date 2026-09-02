// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Progress reporting for the long passes a node makes over a whole table.

use std::time::{Duration, Instant};

use tracing::info;

/// How long a progress line waits for the next one. Shared by every pass that
/// reports, so that they are all as current as each other.
pub(crate) const PROGRESS_REPORT_INTERVAL: Duration = Duration::from_secs(1);

/// An upper bound on a reported estimate, so that a stalled pass cannot turn
/// into a `Duration` this crate has to reason about. Roughly 136 years.
const LONGEST_REPORTED_WAIT: f64 = u32::MAX as f64;

/// Writes one line a second while a pass over a table runs, naming how far it
/// has got, what the last second covered, and how long the rest looks like
/// taking.
///
/// `total` is meant to be [`typed_store::rocks::DBMap::estimated_len`], so the
/// percentage and the time left are approximate: that count comes from SST
/// metadata and still includes keys that were overwritten or deleted but not
/// yet compacted away. A pass can therefore finish before reaching 100%, which
/// is not a fault.
///
/// One of these covers one step. A pass made of several steps builds a new one
/// per step, so that each step's percentage measures the table it is walking
/// rather than the whole pass.
pub(crate) struct ProgressLogger {
    /// Names the pass, e.g. `"ledger backlog migration"`.
    pass: &'static str,
    /// Names the step within it, e.g. `"transactions"`.
    step: &'static str,
    total: u64,
    done: u64,
    /// What `done` stood at when the last line was written, so a line can name
    /// what its second covered and not only the running total.
    done_at_last_line: u64,
    started: Instant,
    last_line: Instant,
    /// Whether the opening line has been written. Held off until the step is
    /// known to have work: these passes stay in the startup path long after
    /// every database has been through them, and one that finds an empty
    /// table should say nothing at all.
    announced: bool,
}

impl ProgressLogger {
    pub(crate) fn new(pass: &'static str, step: &'static str, total: u64) -> Self {
        let now = Instant::now();
        Self {
            pass,
            step,
            total,
            done: 0,
            done_at_last_line: 0,
            started: now,
            last_line: now,
            announced: false,
        }
    }

    /// The step this is reporting, so a caller walking several of them can
    /// tell when to start the next one.
    pub(crate) fn step(&self) -> &'static str {
        self.step
    }

    /// Counts `rows` as done, writing a line if the last one is a second old.
    pub(crate) fn advance(&mut self, rows: u64) {
        if rows == 0 {
            return;
        }
        if !self.announced {
            info!(
                "{}: starting, ~{} {} to go",
                self.pass,
                format_count(self.total),
                self.step
            );
            self.announced = true;
        }
        self.done += rows;
        if self.last_line.elapsed() >= PROGRESS_REPORT_INTERVAL {
            self.write_line();
        }
    }

    /// Writes the closing line, however recent the last one. Silent for a step
    /// that never found anything to do.
    pub(crate) fn finish(&self) {
        if !self.announced {
            return;
        }
        info!(
            "{}: done, {} {} in {}",
            self.pass,
            format_count(self.done),
            self.step,
            format_duration(self.started.elapsed())
        );
    }

    fn write_line(&mut self) {
        let elapsed = self.started.elapsed();
        let fraction = if self.total == 0 {
            1.0
        } else {
            (self.done as f64 / self.total as f64).min(1.0)
        };
        info!(
            "{}",
            progress_line(
                self.pass, self.step, fraction, self.done, self.total, elapsed,
            )
        );
        self.done_at_last_line = self.done;
        self.last_line = Instant::now();
    }
}

/// One progress line, shared by every pass that reports so they all read the
/// same: `"<pass>: ~2.9% done, 26.3M/80.3M objects scanned (4.4M objects/s),
/// ETA ~3m 23s"`.
///
/// `fraction_done` is passed in rather than derived from `done / total`,
/// because a pass that can measure its position more accurately than its
/// row count — a scan over the object id space, say — should report that
/// measure. `total` is only ever an estimate, so it is printed with a `~`.
pub(crate) fn progress_line(
    pass: &str,
    unit: &str,
    fraction_done: f64,
    done: u64,
    total: u64,
    elapsed: Duration,
) -> String {
    let rate = progress_rate(done, elapsed);
    format!(
        "{pass}: ~{:.1}% done, {}/~{} {unit} scanned ({} {unit}/s), ETA ~{}",
        fraction_done * 100.0,
        format_count(done),
        format_count(total),
        format_count(rate as u64),
        eta_display(elapsed, fraction_done),
    )
}

/// Items processed per second since the work started.
pub(crate) fn progress_rate(items: u64, elapsed: Duration) -> f64 {
    items as f64 / elapsed.as_secs_f64().max(f64::EPSILON)
}

/// The estimated time remaining, or "unknown" when nothing has been done yet.
pub(crate) fn eta_display(elapsed: Duration, fraction_done: f64) -> String {
    if fraction_done <= 0.0 {
        return "unknown".to_string();
    }
    let total = elapsed.as_secs_f64() / fraction_done;
    let left = (total - elapsed.as_secs_f64()).clamp(0.0, LONGEST_REPORTED_WAIT);
    format_duration(Duration::from_secs_f64(left))
}

/// Formats a duration for progress lines, e.g. "1h 42m", "3m 20s", or "45s".
pub(crate) fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let (hours, minutes, seconds) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Formats a count for progress lines, e.g. "123.4M" or "1.2k".
pub(crate) fn format_count(count: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pass reports in one shape, so an operator reading a node's
    /// startup does not have to learn three.
    #[test]
    fn a_progress_line_reads_the_same_for_every_pass() {
        assert_eq!(
            progress_line(
                "Indexing live object set",
                "objects",
                0.029,
                26_300_000,
                803_000_000,
                Duration::from_secs(210),
            ),
            "Indexing live object set: ~2.9% done, 26.3M/~803.0M objects scanned \
             (125.2k objects/s), ETA ~1h 57m"
        );
    }

    /// Nothing done yet says so rather than extrapolating from no data.
    #[test]
    fn an_eta_needs_progress_to_estimate_from() {
        assert_eq!(eta_display(Duration::from_secs(100), 0.0), "unknown");
        assert_eq!(eta_display(Duration::from_secs(100), 0.25), "5m 0s");
        assert_eq!(eta_display(Duration::from_secs(100), 1.0), "0s");
    }

    #[test]
    fn format_duration_picks_the_two_largest_units() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(200)), "3m 20s");
        assert_eq!(format_duration(Duration::from_secs(6120)), "1h 42m");
    }

    #[test]
    fn format_count_abbreviates_large_numbers() {
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_200), "1.2k");
        assert_eq!(format_count(123_400_000), "123.4M");
        assert_eq!(format_count(2_500_000_000), "2.5B");
    }
}
