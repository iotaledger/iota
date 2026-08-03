// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Fixed-quantile gauges backed by a sliding-window HDR histogram.
//!
//! [`QuantileGauge`] exposes a small fixed set of latency quantiles as a
//! `<name>{quantile="..."}` gauge series, computed at scrape time from a
//! ~2-minute sliding window, instead of a full bucketed histogram. It lets a
//! dashboard read `<name>{quantile="0.5"}` directly in place of
//! `histogram_quantile(0.5, rate(<name>_bucket[2m]))`, collapsing the
//! per-bucket series (and, for [`QuantileGaugeVec`], the per-label bucket
//! expansion) down to one series per quantile.
//!
//! The quantiles are computed on each node over its own observations, so they
//! cannot be re-aggregated across nodes in PromQL; query them per host.

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use hdrhistogram::Histogram;
use parking_lot::Mutex;
use prometheus_filtered::{
    MetricLevel, Opts, Registry,
    core::{Collector, Desc},
    prometheus::{GaugeVec, proto::MetricFamily},
};

/// Quantiles exposed by every gauge, as `(quantile, series-label)` pairs.
const QUANTILES: &[(f64, &str)] = &[
    (0.05, "0.05"),
    (0.33, "0.33"),
    (0.5, "0.5"),
    (0.66, "0.66"),
    (0.95, "0.95"),
];

/// Width of one slot in the sliding window.
const WINDOW_SLOT: Duration = Duration::from_secs(10);
/// Number of slots retained; `WINDOW_SLOTS * WINDOW_SLOT` is the window length,
/// matching the `rate(..[2m])` range the replaced dashboard panels used.
const WINDOW_SLOTS: usize = 12;

/// Upper bound of the histograms, in microseconds (10 minutes). Latencies above
/// this are clamped to it — far beyond any healthy value for these metrics.
const MAX_TRACKED_MICROS: u64 = 600_000_000;

fn new_histogram() -> Histogram<u64> {
    // Fixed bounds (not auto-resizing) so `saturating_record` clamps outliers to
    // `MAX_TRACKED_MICROS` instead of leaving them out of range, and so the
    // per-authority × per-slot histograms have a bounded size. Two significant
    // figures (~1% quantile error) keeps that size small.
    Histogram::new_with_bounds(1, MAX_TRACKED_MICROS, 2).expect("valid histogram bounds")
}

/// A sliding window of HDR histograms. Observations land in the newest slot;
/// slots older than the window length are dropped on the next rotation.
/// Latencies are stored in microseconds.
struct Window {
    slots: VecDeque<Histogram<u64>>,
    newest_slot_start: Instant,
}

impl Window {
    fn new(now: Instant) -> Self {
        let mut slots = VecDeque::with_capacity(WINDOW_SLOTS);
        slots.push_back(new_histogram());
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
            self.slots.push_back(new_histogram());
            self.newest_slot_start = now;
            return;
        }
        while now.duration_since(self.newest_slot_start) >= WINDOW_SLOT {
            self.newest_slot_start += WINDOW_SLOT;
            self.slots.push_back(new_histogram());
            if self.slots.len() > WINDOW_SLOTS {
                self.slots.pop_front();
            }
        }
    }

    fn record(&mut self, seconds: f64) {
        self.rotate(Instant::now());
        let micros = (seconds * 1e6).max(1.0) as u64;
        self.slots
            .back_mut()
            .expect("the window always holds at least one slot")
            .saturating_record(micros);
    }

    /// Quantiles over the whole window, in seconds, aligned with [`QUANTILES`].
    /// Returns `None` when no value was recorded in the window, so the caller
    /// can drop the series (leaving a gap) rather than report a stale value.
    fn quantiles_seconds(&mut self) -> Option<Vec<f64>> {
        self.rotate(Instant::now());
        let mut merged = new_histogram();
        for slot in &self.slots {
            merged
                .add(slot)
                .expect("all slot histograms share significant figures");
        }
        if merged.is_empty() {
            return None;
        }
        Some(
            QUANTILES
                .iter()
                .map(|(quantile, _)| merged.value_at_quantile(*quantile) as f64 / 1e6)
                .collect(),
        )
    }
}

/// A single latency distribution exposed as `<name>{quantile="..."}`.
///
/// Register with [`QuantileGauge::register`], feed it with [`observe`], and the
/// registry's scrape computes the quantiles over the current window.
///
/// [`observe`]: QuantileGauge::observe
#[derive(Clone)]
pub(crate) struct QuantileGauge {
    gauge: GaugeVec,
    window: Arc<Mutex<Window>>,
}

impl QuantileGauge {
    pub(crate) fn register(
        name: &str,
        help: &str,
        module: &str,
        registry: &Registry,
        level: MetricLevel,
    ) -> Self {
        let gauge =
            GaugeVec::new(Opts::new(name, help), &["quantile"]).expect("valid gauge options");
        registry.record(name, module, level);
        let this = Self {
            gauge,
            window: Arc::new(Mutex::new(Window::new(Instant::now()))),
        };
        registry
            .register(Box::new(this.clone()))
            .expect("quantile gauge registers without collision");
        this
    }

    pub(crate) fn observe(&self, seconds: f64) {
        self.window.lock().record(seconds);
    }
}

impl Collector for QuantileGauge {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        match self.window.lock().quantiles_seconds() {
            Some(values) => {
                for ((_, label), value) in QUANTILES.iter().zip(values) {
                    self.gauge.with_label_values(&[label]).set(value);
                }
            }
            None => {
                for (_, label) in QUANTILES {
                    let _ = self.gauge.remove_label_values(&[label]);
                }
            }
        }
        self.gauge.collect()
    }
}

/// A latency distribution kept per value of one label, exposed as
/// `<name>{<label>="...", quantile="..."}`. One [`Window`] is created lazily
/// per observed label value.
#[derive(Clone)]
pub(crate) struct QuantileGaugeVec {
    gauge: GaugeVec,
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

impl QuantileGaugeVec {
    pub(crate) fn register(
        name: &str,
        help: &str,
        label: &str,
        module: &str,
        registry: &Registry,
        level: MetricLevel,
    ) -> Self {
        let gauge = GaugeVec::new(Opts::new(name, help), &[label, "quantile"])
            .expect("valid gauge options");
        registry.record(name, module, level);
        let this = Self {
            gauge,
            windows: Arc::new(Mutex::new(HashMap::new())),
        };
        registry
            .register(Box::new(this.clone()))
            .expect("quantile gauge registers without collision");
        this
    }

    pub(crate) fn observe(&self, label_value: &str, seconds: f64) {
        let mut windows = self.windows.lock();
        if let Some(window) = windows.get_mut(label_value) {
            window.record(seconds);
        } else {
            let mut window = Window::new(Instant::now());
            window.record(seconds);
            windows.insert(label_value.to_owned(), window);
        }
    }
}

impl Collector for QuantileGaugeVec {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut windows = self.windows.lock();
        windows.retain(|label_value, window| match window.quantiles_seconds() {
            Some(values) => {
                for ((_, quantile), value) in QUANTILES.iter().zip(values) {
                    self.gauge
                        .with_label_values(&[label_value.as_str(), quantile])
                        .set(value);
                }
                true
            }
            None => {
                // Drop both the gauge series and the window so an idle label
                // value stops being scraped and its memory is reclaimed.
                for (_, quantile) in QUANTILES {
                    let _ = self
                        .gauge
                        .remove_label_values(&[label_value.as_str(), quantile]);
                }
                false
            }
        });
        self.gauge.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_keeps_slot_within_window_slot() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.rotate(t0 + WINDOW_SLOT / 2);
        assert_eq!(window.slots.len(), 1);
        assert_eq!(window.newest_slot_start, t0);
    }

    #[test]
    fn rotate_adds_one_slot_at_exactly_window_slot() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.rotate(t0 + WINDOW_SLOT);
        assert_eq!(window.slots.len(), 2);
        assert_eq!(window.newest_slot_start, t0 + WINDOW_SLOT);
    }

    #[test]
    fn rotate_adds_multiple_slots_and_trims_to_window() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);

        window.rotate(t0 + WINDOW_SLOT * 5);
        assert_eq!(window.slots.len(), 6);
        assert_eq!(window.newest_slot_start, t0 + WINDOW_SLOT * 5);

        // Advancing far enough to overflow the deque trims the oldest slots.
        window.rotate(t0 + WINDOW_SLOT * 16);
        assert_eq!(window.slots.len(), WINDOW_SLOTS);
        assert_eq!(window.newest_slot_start, t0 + WINDOW_SLOT * 16);
    }

    #[test]
    fn rotate_resets_after_gap_longer_than_window() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        let gap = WINDOW_SLOT * WINDOW_SLOTS as u32;
        window.rotate(t0 + gap);
        assert_eq!(window.slots.len(), 1);
        assert_eq!(window.newest_slot_start, t0 + gap);
    }

    #[test]
    fn old_observations_fall_out_of_window() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.slots.back_mut().unwrap().saturating_record(1_000);

        // Fill the window to capacity without dropping the observed slot yet.
        window.rotate(t0 + WINDOW_SLOT * (WINDOW_SLOTS as u32 - 1));
        assert_eq!(window.slots.len(), WINDOW_SLOTS);
        assert!(!window.slots.front().unwrap().is_empty());

        // One more slot pushes the oldest (observed) slot out of the window.
        window.rotate(t0 + WINDOW_SLOT * WINDOW_SLOTS as u32);
        assert_eq!(window.slots.len(), WINDOW_SLOTS);
        assert!(window.slots.iter().all(|slot| slot.is_empty()));
    }

    #[test]
    fn collect_drops_idle_label_series_and_window() {
        let registry = Registry::new();
        let gauge = QuantileGaugeVec::register(
            "test_quantile_gauge",
            "help",
            "peer",
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        gauge.observe("peer1", 0.01);

        // Force the window idle: no observation survives in any live slot.
        {
            let mut windows = gauge.windows.lock();
            let window = windows.get_mut("peer1").unwrap();
            window.slots.clear();
            window.slots.push_back(new_histogram());
        }

        gauge.collect();
        assert!(gauge.windows.lock().is_empty());
    }
}
