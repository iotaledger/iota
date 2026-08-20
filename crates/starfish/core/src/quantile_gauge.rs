// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Gauges summarising a sliding-window HDR histogram.
//!
//! [`QuantileGauge`] exposes a small fixed set of latency quantiles as a
//! `<name>{quantile="..."}` gauge series, computed at scrape time from a
//! ~2-minute sliding window, instead of a full bucketed histogram. It lets a
//! dashboard read `<name>{quantile="0.5"}` directly in place of
//! `histogram_quantile(0.5, rate(<name>_bucket[2m]))`, collapsing the
//! per-bucket series (and, for [`QuantileGaugeVec`], the per-label bucket
//! expansion) down to one series per quantile.
//!
//! [`PeakGauge`] shares the same window but reports its maximum, for values
//! that move faster than the scrape interval and would otherwise only ever be
//! sampled at one arbitrary instant.
//!
//! The summaries are computed on each node over its own observations, so they
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
    prometheus::{GaugeVec, IntGauge, proto::MetricFamily},
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

/// Upper bound of the histograms — 10 minutes for the latency gauges, and far
/// beyond any healthy value for the counts [`PeakGauge`] tracks. Observations
/// above it are clamped.
const MAX_TRACKED_VALUE: u64 = 600_000_000;

fn new_histogram() -> Histogram<u64> {
    // Fixed bounds (not auto-resizing) so `saturating_record` clamps outliers to
    // `MAX_TRACKED_VALUE` instead of leaving them out of range, and so the
    // per-authority × per-slot histograms have a bounded size. Two significant
    // figures (~1% quantile error) keeps that size small.
    Histogram::new_with_bounds(1, MAX_TRACKED_VALUE, 2).expect("valid histogram bounds")
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

    /// Records a raw value. The histogram's lower bound is 1, so a zero is
    /// lifted to it; callers that must distinguish zero skip the observation.
    fn record_raw(&mut self, value: u64) {
        self.rotate(Instant::now());
        self.slots
            .back_mut()
            .expect("the window always holds at least one slot")
            .saturating_record(value.max(1));
    }

    fn record(&mut self, seconds: f64) {
        self.record_raw((seconds * 1e6).max(1.0) as u64);
    }

    fn merged(&mut self) -> Option<Histogram<u64>> {
        self.rotate(Instant::now());
        let mut merged = new_histogram();
        for slot in &self.slots {
            merged
                .add(slot)
                .expect("all slot histograms share significant figures");
        }
        (!merged.is_empty()).then_some(merged)
    }

    /// The highest value recorded over the whole window, or `None` when nothing
    /// was recorded in it.
    fn max_raw(&mut self) -> Option<u64> {
        self.merged().map(|merged| merged.max())
    }

    /// Quantiles over the whole window, in seconds, aligned with [`QUANTILES`].
    /// Returns `None` when no value was recorded in the window, so the caller
    /// can drop the series (leaving a gap) rather than report a stale value.
    fn quantiles_seconds(&mut self) -> Option<Vec<f64>> {
        self.merged().map(|merged| {
            QUANTILES
                .iter()
                .map(|(quantile, _)| merged.value_at_quantile(*quantile) as f64 / 1e6)
                .collect()
        })
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
        let this = Self {
            gauge,
            window: Arc::new(Mutex::new(Window::new(Instant::now()))),
        };
        registry
            .register_filtered(name, module, level, this)
            .expect("quantile gauge registers without collision")
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
        let this = Self {
            gauge,
            windows: Arc::new(Mutex::new(HashMap::new())),
        };
        registry
            .register_filtered(name, module, level, this)
            .expect("quantile gauge registers without collision")
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

/// The highest value observed over the window, exposed as `<name>`.
///
/// Register with [`PeakGauge::register`], feed it with [`observe`], and the
/// registry's scrape reports the peak over the current window. A window with
/// no observation reports zero rather than dropping the series, so an idle
/// period is visible as such.
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

    /// Zero is skipped rather than recorded, because the histogram cannot hold
    /// it; a window holding only zeroes is reported as a peak of zero.
    pub(crate) fn observe(&self, value: u64) {
        if value > 0 {
            self.window.lock().record_raw(value);
        }
    }
}

impl Collector for PeakGauge {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let peak = self.window.lock().max_raw().unwrap_or(0);
        self.gauge.set(peak as i64);
        self.gauge.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_raw_is_none_before_any_observation() {
        let mut window = Window::new(Instant::now());
        assert_eq!(window.max_raw(), None);
    }

    #[test]
    fn max_raw_is_the_highest_value_across_slots() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.record_raw(3);
        window.rotate(t0 + WINDOW_SLOT);
        window.record_raw(70);
        window.rotate(t0 + WINDOW_SLOT * 2);
        window.record_raw(5);
        assert_eq!(window.max_raw(), Some(70));
    }

    #[test]
    fn max_raw_drops_values_older_than_the_window() {
        let t0 = Instant::now();
        let mut window = Window::new(t0);
        window.record_raw(90);
        window.rotate(t0 + WINDOW_SLOT * WINDOW_SLOTS as u32);
        window.record_raw(2);
        assert_eq!(window.max_raw(), Some(2));
    }

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
