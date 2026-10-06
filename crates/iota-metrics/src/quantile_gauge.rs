// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Gauges summarising a sliding window of observations.
//!
//! [`QuantileGauge`] exposes a small fixed set of latency quantiles as a
//! `<name>{quantile="..."}` gauge series, computed at scrape time from a
//! ~2-minute sliding window, instead of a full bucketed histogram. It lets a
//! dashboard read `<name>{quantile="0.5"}` directly in place of
//! `histogram_quantile(0.5, rate(<name>_bucket[2m]))`, collapsing the
//! per-bucket series (and, for [`QuantileGaugeVec`], the per-label bucket
//! expansion) down to one series per quantile.
//!
//! [`PeakGauge`] keeps the exact maximum over a window of the same length, for
//! values that move faster than the scrape interval and would otherwise only
//! ever be sampled at one arbitrary instant.
//!
//! The summaries are computed on each node over its own observations, so they
//! cannot be re-aggregated across nodes in PromQL; query them per host.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use hdrhistogram::Histogram;
use parking_lot::{Mutex, MutexGuard};
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

/// Upper bound of the histograms, 10 minutes. Observations above it are
/// clamped.
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
        self.slots
            .back_mut()
            .expect("the window always holds at least one slot")
            .saturating_record((seconds * 1e6).max(1.0) as u64);
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
pub struct QuantileGauge {
    gauge: GaugeVec,
    window: Arc<Mutex<Window>>,
}

impl QuantileGauge {
    /// # Panics
    ///
    /// Panics if a metric of this name is already registered.
    pub fn register(
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

    pub fn observe(&self, seconds: f64) {
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
/// `<name>{<label>="...", quantile="..."}`. One window is created lazily
/// per observed label value.
#[derive(Clone)]
pub struct QuantileGaugeVec {
    gauge: GaugeVec,
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

impl QuantileGaugeVec {
    /// # Panics
    ///
    /// Panics if a metric of this name is already registered.
    pub fn register(
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

    pub fn observe(&self, label_value: &str, seconds: f64) {
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

/// The highest value over a sliding window, kept exactly per slot. The window
/// has the same length as that of [`Window`]. Observing takes no lock while the
/// slot does not change; a change of slot and a read take a lock.
struct PeakWindow {
    start: Instant,
    /// The number of the slot being filled: the time since `start` in slots.
    current_slot: AtomicU64,
    /// The maxima of the last `WINDOW_SLOTS` slots, slot `n` at `n %
    /// WINDOW_SLOTS`.
    slots: [AtomicU64; WINDOW_SLOTS],
    rotation: Mutex<()>,
}

impl PeakWindow {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            current_slot: AtomicU64::new(0),
            slots: std::array::from_fn(|_| AtomicU64::new(0)),
            rotation: Mutex::new(()),
        }
    }

    fn slot_number(&self, now: Instant) -> u64 {
        (now.saturating_duration_since(self.start).as_nanos() / WINDOW_SLOT.as_nanos()) as u64
    }

    /// Records `value` at `now`. `now` is not later than the current time. A
    /// value older than the window is dropped.
    fn observe_at(&self, value: u64, now: Instant) {
        let value_slot = self.slot_number(now);
        let current_slot = self.current_slot.load(Ordering::Acquire);
        if value_slot == current_slot {
            let slot_max = self.slot_max(value_slot);
            if value > slot_max.load(Ordering::Relaxed) {
                slot_max.fetch_max(value, Ordering::Relaxed);
            }
            return;
        }
        let rotation = self.rotation.lock();
        self.advance_to(&rotation, value_slot);
        let current_slot = self.current_slot.load(Ordering::Relaxed);
        let still_in_window = value_slot + WINDOW_SLOTS as u64 > current_slot;
        if still_in_window {
            self.slot_max(value_slot)
                .fetch_max(value, Ordering::Relaxed);
        }
    }

    fn slot_max(&self, slot: u64) -> &AtomicU64 {
        &self.slots[slot as usize % WINDOW_SLOTS]
    }

    /// Moves to `target_slot` and clears the slots it takes over.
    fn advance_to(&self, _rotation: &MutexGuard<'_, ()>, target_slot: u64) {
        let current_slot = self.current_slot.load(Ordering::Relaxed);
        if target_slot <= current_slot {
            return;
        }
        let first_to_clear =
            (current_slot + 1).max(target_slot.saturating_sub(WINDOW_SLOTS as u64 - 1));
        for slot in first_to_clear..=target_slot {
            self.slot_max(slot).store(0, Ordering::Relaxed);
        }
        self.current_slot.store(target_slot, Ordering::Release);
    }

    fn max_at(&self, now: Instant) -> u64 {
        {
            let rotation = self.rotation.lock();
            self.advance_to(&rotation, self.slot_number(now));
        }
        self.slots
            .iter()
            .map(|slot| slot.load(Ordering::Relaxed))
            .max()
            .unwrap_or(0)
    }

    fn observe(&self, value: u64) {
        self.observe_at(value, Instant::now());
    }

    fn max(&self) -> u64 {
        self.max_at(Instant::now())
    }
}

/// The highest value observed over the window, exposed as `<name>`. The value
/// is exact.
///
/// Register with [`PeakGauge::register`], feed it with [`observe`], and the
/// registry's scrape reports the peak over the current window. A window with
/// no observation reports zero rather than dropping the series, so an idle
/// period is visible as such.
///
/// [`observe`]: PeakGauge::observe
#[derive(Clone)]
pub struct PeakGauge {
    gauge: IntGauge,
    window: Arc<PeakWindow>,
}

impl PeakGauge {
    /// # Panics
    ///
    /// Panics if a metric of this name is already registered.
    pub fn register(
        name: &str,
        help: &str,
        module: &str,
        registry: &Registry,
        level: MetricLevel,
    ) -> Self {
        let gauge = IntGauge::with_opts(Opts::new(name, help)).expect("valid gauge options");
        let this = Self {
            gauge,
            window: Arc::new(PeakWindow::new(Instant::now())),
        };
        registry
            .register_filtered(name, module, level, this)
            .expect("peak gauge registers without collision")
    }

    pub fn observe(&self, value: u64) {
        self.window.observe(value);
    }
}

impl Collector for PeakGauge {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        self.gauge
            .set(i64::try_from(self.window.max()).unwrap_or(i64::MAX));
        self.gauge.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_is_zero_before_any_observation() {
        let window = PeakWindow::new(Instant::now());
        assert_eq!(window.max_at(Instant::now()), 0);
    }

    #[test]
    fn observing_zero_leaves_the_peak_at_zero() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        window.observe_at(0, t0);
        assert_eq!(window.max_at(t0), 0);
    }

    #[test]
    fn peak_is_exact() {
        let t0 = Instant::now();
        for value in [32768, 1500, 1_000_003, u64::MAX / 2] {
            let window = PeakWindow::new(t0);
            window.observe_at(value, t0);
            assert_eq!(window.max_at(t0), value);
        }
    }

    #[test]
    fn peak_is_the_highest_value_across_slots() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        window.observe_at(3, t0);
        window.observe_at(70, t0 + WINDOW_SLOT);
        window.observe_at(5, t0 + WINDOW_SLOT * 2);
        assert_eq!(window.max_at(t0 + WINDOW_SLOT * 2), 70);
    }

    #[test]
    fn peak_drops_values_older_than_the_window() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        window.observe_at(90, t0);
        // The slot of the value is the oldest one still in the window.
        assert_eq!(
            window.max_at(t0 + WINDOW_SLOT * (WINDOW_SLOTS as u32 - 1)),
            90
        );
        assert_eq!(window.max_at(t0 + WINDOW_SLOT * WINDOW_SLOTS as u32), 0);
        window.observe_at(2, t0 + WINDOW_SLOT * WINDOW_SLOTS as u32);
        assert_eq!(window.max_at(t0 + WINDOW_SLOT * WINDOW_SLOTS as u32), 2);
    }

    #[test]
    fn peak_forgets_everything_after_a_gap_longer_than_the_window() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        for i in 0..WINDOW_SLOTS as u32 {
            window.observe_at(100 + i as u64, t0 + WINDOW_SLOT * i);
        }
        assert_eq!(
            window.max_at(t0 + WINDOW_SLOT * (WINDOW_SLOTS as u32 - 1)),
            111
        );
        assert_eq!(window.max_at(t0 + WINDOW_SLOT * 100), 0);
    }

    #[test]
    fn peak_accepts_a_late_value_for_a_slot_still_in_the_window() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        window.observe_at(1, t0 + WINDOW_SLOT * 3);
        window.observe_at(9, t0 + WINDOW_SLOT);
        assert_eq!(window.max_at(t0 + WINDOW_SLOT * 3), 9);
    }

    #[test]
    fn concurrent_observations_end_with_the_highest_value() {
        let window = Arc::new(PeakWindow::new(Instant::now()));
        let threads: Vec<_> = (0..8u64)
            .map(|t| {
                let window = window.clone();
                std::thread::spawn(move || {
                    for i in 0..10_000u64 {
                        window.observe(t * 10_000 + i);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(window.max(), 79_999);
    }

    #[test]
    fn observe_at_uses_the_given_time() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        window.observe_at(9, t0);
        let later = t0 + WINDOW_SLOT * (WINDOW_SLOTS as u32 + 5);
        window.observe_at(4, later);
        assert_eq!(window.max_at(later), 4, "the earlier value left the window");
    }

    #[test]
    fn a_late_value_at_the_edge_of_the_window_is_dropped() {
        let t0 = Instant::now();
        let window = PeakWindow::new(t0);
        let edge = t0 + WINDOW_SLOT * WINDOW_SLOTS as u32;
        window.observe_at(1, edge);
        window.observe_at(9, t0);
        assert_eq!(window.max_at(edge), 1);
    }

    #[test]
    fn observing_in_the_current_slot_takes_no_lock() {
        let t0 = Instant::now();
        let window = Arc::new(PeakWindow::new(t0));
        let _held = window.rotation.lock();
        let (sender, receiver) = std::sync::mpsc::channel();
        let observer = window.clone();
        std::thread::spawn(move || {
            observer.observe_at(1, t0);
            sender.send(()).unwrap();
        });
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("the observation waited for the rotation lock");
    }

    #[test]
    fn peak_gauge_reports_the_exact_peak() {
        let registry = Registry::new();
        let gauge = PeakGauge::register(
            "test_peak_gauge",
            "help",
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        gauge.observe(32768);
        gauge.observe(1500);
        let families = registry.gather();
        let family = families
            .iter()
            .find(|f| f.name() == "test_peak_gauge")
            .unwrap();
        assert_eq!(family.get_metric()[0].get_gauge().value(), 32768.0);
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
