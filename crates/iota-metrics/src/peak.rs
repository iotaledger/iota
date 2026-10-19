// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Gauges that keep the exact maximum over a sliding window.
//!
//! [`PeakGauge`] keeps the exact maximum over a window of the same length as
//! that of the quantile gauges, for values that move faster than the scrape
//! interval and would otherwise only ever be sampled at one arbitrary instant.
//! [`PeakGaugeVec`] keeps one such peak for each value of a label.
//! [`IntGaugeWithPeakGauge`] pairs an `IntGauge` with such a peak.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use parking_lot::{Mutex, MutexGuard, RwLock};
use prometheus_filtered::{
    MetricLevel, Opts, Registry,
    core::{Collector, Desc},
    prometheus::{IntGauge, IntGaugeVec, proto::MetricFamily},
};

use crate::quantile_gauge::{WINDOW_SLOT, WINDOW_SLOTS};

/// The highest value over a sliding window, kept exactly per slot. The window
/// has the same length as that of the quantile gauges. Observing takes no lock while the
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
/// is exact. It is a scalar from [`PeakGauge::register`], or the gauge of one
/// label value from [`PeakGaugeVec::with_label_values`].
///
/// Register with [`PeakGauge::register`], feed it with [`observe`], and the
/// registry's scrape reports the peak over the current window. A window with
/// no observation reports zero rather than dropping the series, so an idle
/// period is visible as such.
///
/// For an `IntGauge` that also exports its peak, see [`IntGaugeWithPeakGauge`].
///
/// [`observe`]: PeakGauge::observe
#[derive(Clone)]
pub struct PeakGauge {
    gauge: IntGauge,
    window: Arc<PeakWindow>,
    /// If set, the peak is never reported below the current value of this gauge.
    floor: Option<IntGauge>,
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
        Self::register_with_floor(name, help, module, registry, level, None)
    }

    /// Like [`register`](Self::register), and with a `floor` the peak is never
    /// reported below the current value of `floor`.
    fn register_with_floor(
        name: &str,
        help: &str,
        module: &str,
        registry: &Registry,
        level: MetricLevel,
        floor: Option<IntGauge>,
    ) -> Self {
        let gauge = IntGauge::with_opts(Opts::new(name, help)).expect("valid gauge options");
        let this = Self {
            gauge,
            window: Arc::new(PeakWindow::new(Instant::now())),
            floor,
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
        let current = self
            .floor
            .as_ref()
            .map_or(0, |gauge| gauge.get().max(0) as u64);
        self.gauge
            .set(i64::try_from(self.window.max().max(current)).unwrap_or(i64::MAX));
        self.gauge.collect()
    }
}

/// The highest value observed over the window per value of one label, exposed
/// as `<name>{<label>="..."}`. A series stays after its window empties and then
/// reports zero, so the label values must be a small fixed set.
#[derive(Clone)]
pub struct PeakGaugeVec {
    gauge: IntGaugeVec,
    windows: Arc<RwLock<HashMap<String, Arc<PeakWindow>>>>,
}

impl PeakGaugeVec {
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
        let gauge = IntGaugeVec::new(Opts::new(name, help), &[label]).expect("valid gauge options");
        let this = Self {
            gauge,
            windows: Arc::new(RwLock::new(HashMap::new())),
        };
        registry
            .register_filtered(name, module, level, this)
            .expect("peak gauge registers without collision")
    }

    /// The peak of `label_values`, created if needed. Its window is shared by
    /// all peaks of the same label values.
    ///
    /// # Panics
    ///
    /// Panics if `label_values` does not hold exactly one value.
    pub fn with_label_values(&self, label_values: &[&str]) -> PeakGauge {
        let [label_value] = label_values else {
            panic!("a peak gauge vector has one label");
        };
        PeakGauge {
            gauge: self.gauge.with_label_values(label_values),
            window: self.window(label_value),
            floor: None,
        }
    }

    fn window(&self, label_value: &str) -> Arc<PeakWindow> {
        if let Some(window) = self.windows.read().get(label_value) {
            return window.clone();
        }
        self.windows
            .write()
            .entry(label_value.to_owned())
            .or_insert_with(|| Arc::new(PeakWindow::new(Instant::now())))
            .clone()
    }
}

impl Collector for PeakGaugeVec {
    fn desc(&self) -> Vec<&Desc> {
        self.gauge.desc()
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let windows: Vec<_> = self
            .windows
            .read()
            .iter()
            .map(|(label, window)| (label.clone(), window.clone()))
            .collect();
        for (label_value, window) in windows {
            self.gauge
                .with_label_values(&[label_value.as_str()])
                .set(i64::try_from(window.max()).unwrap_or(i64::MAX));
        }
        self.gauge.collect()
    }
}

/// An `IntGauge` and its peak over the window, as a [`PeakGauge`]. The peak is
/// observed at each increment, and a scrape reports at least the current value
/// of the gauge.
/// A decrement that runs between an increment and the observation that
/// follows it can make the peak miss the value the increment gave.
#[derive(Clone)]
pub struct IntGaugeWithPeakGauge {
    gauge: IntGauge,
    peak: PeakGauge,
}

impl IntGaugeWithPeakGauge {
    /// Registers the gauge and its peak, each as `(name, help)`.
    ///
    /// # Panics
    ///
    /// Panics if a metric of one of the names is already registered.
    pub fn register(
        (name, help): (&str, &str),
        (peak_name, peak_help): (&str, &str),
        module: &str,
        registry: &Registry,
        level: MetricLevel,
    ) -> Self {
        let gauge = IntGauge::with_opts(Opts::new(name, help)).expect("valid gauge options");
        let gauge = registry
            .register_filtered(name, module, level, gauge)
            .expect("gauge registers without collision");
        let peak = PeakGauge::register_with_floor(
            peak_name,
            peak_help,
            module,
            registry,
            level,
            Some(gauge.clone()),
        );
        Self { gauge, peak }
    }

    /// Adds one and observes the new value as a peak.
    pub fn inc(&self) {
        self.gauge.inc();
        self.peak.observe(self.gauge.get().max(0) as u64);
    }

    /// Subtracts one. The peak does not change.
    pub fn dec(&self) {
        self.gauge.dec();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::test_utils::MetricsReader;

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
    fn int_gauge_with_peak_gauge_reports_the_peak_and_the_gauge() {
        let registry = Registry::new();
        let gauge = IntGaugeWithPeakGauge::register(
            ("test_gauge", "help"),
            ("test_gauge_peak", "help"),
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        let reader = MetricsReader::new(&registry);
        for _ in 0..3 {
            gauge.inc();
        }
        gauge.dec();
        gauge.dec();
        assert_eq!(reader.value("test_gauge", &[]), 1.0);
        assert_eq!(reader.value("test_gauge_peak", &[]), 3.0);
    }

    #[test]
    fn the_peak_of_a_gauge_is_never_below_the_gauge() {
        let registry = Registry::new();
        let gauge = IntGaugeWithPeakGauge::register(
            ("test_gauge", "help"),
            ("test_gauge_peak", "help"),
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        // The window of the peak holds no observation of this value.
        gauge.gauge.set(5);
        assert_eq!(
            MetricsReader::new(&registry).value("test_gauge_peak", &[]),
            5.0
        );
    }

    #[test]
    fn peak_with_a_floor_reports_the_peak_and_never_less_than_the_gauge() {
        let registry = Registry::new();
        let gauge = IntGauge::new("test_gauge", "help").unwrap();
        let peak = PeakGauge::register_with_floor(
            "test_gauge_peak",
            "help",
            module_path!(),
            &registry,
            MetricLevel::Warn,
            Some(gauge.clone()),
        );
        let reader = MetricsReader::new(&registry);
        for _ in 0..3 {
            gauge.inc();
            peak.observe(gauge.get() as u64);
        }
        gauge.dec();
        gauge.dec();
        assert_eq!(reader.value("test_gauge_peak", &[]), 3.0);

        peak.window
            .observe_at(0, Instant::now() + WINDOW_SLOT * (WINDOW_SLOTS as u32 + 1));
        assert_eq!(
            reader.value("test_gauge_peak", &[]),
            1.0,
            "the window is empty, so the peak is the current gauge"
        );
    }

    #[test]
    fn peak_gauge_vec_reports_the_peak_per_label_value() {
        let registry = Registry::new();
        let gauge = PeakGaugeVec::register(
            "test_gauge_vec_peak",
            "help",
            "kind",
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        let (a, b) = (
            gauge.with_label_values(&["a"]),
            gauge.with_label_values(&["b"]),
        );
        a.observe(2);
        a.observe(1);
        b.observe(7);
        let reader = MetricsReader::new(&registry);
        assert_eq!(reader.value("test_gauge_vec_peak", &[("kind", "a")]), 2.0);
        assert_eq!(reader.value("test_gauge_vec_peak", &[("kind", "b")]), 7.0);
        assert_eq!(
            gauge.with_label_values(&["a"]).window.max(),
            2,
            "a second peak of the label value shares the window"
        );
    }

    #[test]
    fn threads_that_get_the_same_peak_share_one_window() {
        let registry = Registry::new();
        let gauge = PeakGaugeVec::register(
            "test_gauge_vec_threads",
            "help",
            "kind",
            module_path!(),
            &registry,
            MetricLevel::Warn,
        );
        for round in 0..200 {
            let label = round.to_string();
            let barrier = std::sync::Barrier::new(8);
            std::thread::scope(|scope| {
                for value in 1..=8 {
                    let (gauge, barrier, label) = (&gauge, &barrier, label.as_str());
                    scope.spawn(move || {
                        barrier.wait();
                        gauge.with_label_values(&[label]).observe(value);
                    });
                }
            });
            assert_eq!(
                gauge.with_label_values(&[&label]).window.max(),
                8,
                "round {round}"
            );
        }
    }
}
