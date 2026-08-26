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
}

impl ProgressLogger {
    pub(crate) fn new(pass: &'static str, step: &'static str, total: u64) -> Self {
        let now = Instant::now();
        info!("{pass}: {step} starting, about {total} rows to go");
        Self {
            pass,
            step,
            total,
            done: 0,
            done_at_last_line: 0,
            started: now,
            last_line: now,
        }
    }

    /// The step this is reporting, so a caller walking several of them can
    /// tell when to start the next one.
    pub(crate) fn step(&self) -> &'static str {
        self.step
    }

    /// Counts `rows` as done, writing a line if the last one is a second old.
    pub(crate) fn advance(&mut self, rows: u64) {
        self.done += rows;
        if self.last_line.elapsed() >= PROGRESS_REPORT_INTERVAL {
            self.write_line();
        }
    }

    /// Writes the closing line, however recent the last one.
    pub(crate) fn finish(&self) {
        info!(
            "{}: {} finished, {} rows in {:.0?}",
            self.pass,
            self.step,
            self.done,
            self.started.elapsed()
        );
    }

    fn write_line(&mut self) {
        let now = Instant::now();
        let since_last_line = self.done - self.done_at_last_line;
        let elapsed = self.started.elapsed().as_secs_f64();
        let percent = if self.total == 0 {
            100.0
        } else {
            (self.done as f64 / self.total as f64 * 100.0).min(100.0)
        };
        // Measured over the whole step rather than the last second, so that
        // one slow slice does not swing the estimate.
        let rows_per_second = self.done as f64 / elapsed.max(f64::EPSILON);
        let left = self.total.saturating_sub(self.done);
        let seconds_left = if rows_per_second > 0.0 {
            (left as f64 / rows_per_second).min(LONGEST_REPORTED_WAIT)
        } else {
            0.0
        };
        info!(
            "{}: {} {}/~{} ({percent:.1}%), +{since_last_line} in the last {:.0?}, about {:.0?} left",
            self.pass,
            self.step,
            self.done,
            self.total,
            now - self.last_line,
            Duration::from_secs_f64(seconds_left),
        );
        self.done_at_last_line = self.done;
        self.last_line = now;
    }
}
