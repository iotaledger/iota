// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A gauge reporting the highest value seen over a sliding window.
//!
//! A plain gauge only holds the value it last happened to be set to, so a
//! scrape reports whatever the queue depth was at one arbitrary instant and
//! misses every burst between scrapes. [`PeakGauge`] instead keeps the maximum
//! observed over a window at least as long as the scrape interval, so a burst
//! is still visible at the next scrape.
//!
//! The peak is computed on each node over its own observations, so it cannot be
//! re-aggregated across nodes in PromQL; query it per host.

use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use prometheus_filtered::{
    MetricLevel, Opts, Registry,
    core::{Collector, Desc},
    prometheus::{IntGauge, proto::MetricFamily},
};

/// Width of one slot in the sliding window.
const WINDOW_SLOT: Duration = Duration::from_secs(10);
/// Number of slots retained. `WINDOW_SLOTS * WINDOW_SLOT` is the window length,
/// covering the 60s scrape interval so no burst falls between two scrapes.
const WINDOW_SLOTS: usize = 6;

/// A sliding window of per-slot maxima. Observations update the newest slot;
/// slots older than the window length are dropped on the next rotation.
struct Window {
    slots: VecDeque<i64>,
    newest_slot_start: Instant,
}

impl Window {
    fn new(now: Instant) -> Self {
        let mut slots = VecDeque::with_capacity(WINDOW_SLOTS);
        slots.push_back(i64::MIN);
        Self {
            slots,
            newest_slot_start: now,
        }
    }

    fn rotate(&mut self, now: Instant) {
        // After an idle gap longer than the whole window every retained slot
        // would be discarded anyway; reset in one step instead of looping over
        // every elapsed slot.
        if now.duration_since(self.newest_slot_start) >= WINDOW_SLOT * WINDOW_SLOTS as u32 {
            self.slots.clear();
            self.slots.push_back(i64::MIN);
            self.newest_slot_start = now;
            return;
        }
        while now.duration_since(self.newest_slot_start) >= WINDOW_SLOT {
            self.newest_slot_start += WINDOW_SLOT;
            self.slots.push_back(i64::MIN);
            if self.slots.len() > WINDOW_SLOTS {
                self.slots.pop_front();
            }
        }
    }

    fn record(&mut self, value: i64) {
        self.rotate(Instant::now());
        let newest = self
            .slots
            .back_mut()
            .expect("the window always holds at least one slot");
        *newest = (*newest).max(value);
    }

    /// The peak over the whole window. Returns `None` when nothing was recorded
    /// in the window, so the caller can drop the series rather than report a
    /// stale peak.
    fn peak(&mut self) -> Option<i64> {
        self.rotate(Instant::now());
        self.slots.iter().copied().max().filter(|v| *v != i64::MIN)
    }
}

/// The highest value observed over the sliding window, exposed as `<name>`.
///
/// Register with [`PeakGauge::register`], feed it with [`observe`], and the
/// registry's scrape reports the peak over the current window.
///
/// [`observe`]: PeakGauge::observe
#[derive(Clone)]
pub(crate) struct PeakGauge {
    gauge: IntGauge,
    window: Arc<Mutex<Window>>,
}

impl PeakGauge {
    pub(crate) fn register(
        name: &str,
        help: &str,
        module: &str,
        registry: &Registry,
        level: MetricLevel,
    ) -> Self {
        let gauge = IntGauge::with_opts(Opts::new(name, help)).expect("valid gauge options");
        let this = Self {
            gauge,
            window: Arc::new(Mutex::new(Window::new(Instant::now()))),
        };
        registry
            .register_filtered(name, module, level, this)
            .expect("peak gauge registers without collision")
    }

    pub(crate) fn observe(&self, value: i64) {
        self.window.lock().record(value);
    }
}

impl Collector for PeakGauge {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        match self.window.lock().peak() {
            Some(peak) => {
                self.gauge.set(peak);
                self.gauge.collect()
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_is_none_before_any_observation() {
        let mut window = Window::new(Instant::now());
        assert_eq!(window.peak(), None);
    }

    #[test]
    fn peak_is_the_maximum_across_slots() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.record(3);
        window.rotate(t0 + WINDOW_SLOT);
        window.record(7);
        window.rotate(t0 + WINDOW_SLOT * 2);
        window.record(5);
        assert_eq!(window.peak(), Some(7));
    }

    #[test]
    fn peak_drops_slots_older_than_the_window() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.record(9);
        // Rotate past the whole window so the slot holding 9 is discarded.
        window.rotate(t0 + WINDOW_SLOT * WINDOW_SLOTS as u32);
        window.record(2);
        assert_eq!(window.peak(), Some(2));
    }

    #[test]
    fn rotate_keeps_at_most_the_window_slots() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        for slot in 1..=(WINDOW_SLOTS as u32 + 3) {
            window.rotate(t0 + WINDOW_SLOT * slot);
        }
        assert_eq!(window.slots.len(), WINDOW_SLOTS);
    }
}
