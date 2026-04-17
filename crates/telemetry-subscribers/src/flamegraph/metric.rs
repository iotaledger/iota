// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, time::Duration};

/// A generic API for internal span metric measuring compatible with
/// `tracing::Subscriber`.
pub trait SpanMetrics: fmt::Debug + Default + Sized {
    type Arg: fmt::Debug + Sized;
    const REENTER: bool;

    fn enter(&mut self, arg: Self::Arg);
    fn exit(&mut self, arg: Self::Arg);
}

/// Metrics that can be accumulated.
pub trait MergeMetrics<Rhs: fmt::Debug + Sized = Self>: fmt::Debug + Sized {
    fn merge(&mut self, other: Self);
}

pub type Clock = std::time::SystemTime;

/// Stopwatch accumulating total elapsed time.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Stopwatch {
    pub clock: Clock,
    pub total: Duration,
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self {
            clock: Clock::UNIX_EPOCH,
            total: Duration::default(),
        }
    }
}

impl Stopwatch {
    pub fn is_stopped(&self) -> bool {
        self.clock == Clock::UNIX_EPOCH
    }

    pub fn is_ticking(&self) -> bool {
        !self.is_stopped()
    }

    pub fn start(&mut self, now: Clock) {
        debug_assert!(self.is_stopped());
        debug_assert!(now != Clock::UNIX_EPOCH);
        self.clock = now;
    }

    pub fn stop(&mut self, now: Clock) {
        debug_assert!(self.is_ticking());
        debug_assert!(self.clock <= now, "{:?} <= {now:?}", self.clock);
        self.total += now.duration_since(self.clock).unwrap_or_default();
        self.reset();
    }

    pub fn try_stop(&mut self, now: Clock) {
        if self.is_ticking() {
            debug_assert!(self.clock <= now, "{:?} <= {now:?}", self.clock);
            self.total += now.duration_since(self.clock).unwrap_or_default();
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.clock = Clock::UNIX_EPOCH;
    }
}

impl fmt::Debug for Stopwatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.clock == Clock::UNIX_EPOCH {
            "stopped"
        } else {
            "ticking"
        };
        write!(f, "({state}, {:?})", self.total)
    }
}

/// Metric counting task's useful run and idle pending times.
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct FlameMetric {
    /// Stopwatch measuring a task's useful run time.
    pub running: Stopwatch,
    /// Stopwatch measuring a task's idle pending time.
    pub pending: Stopwatch,
    pub count: CountMetric,
    #[cfg(all(feature = "flamegraph-alloc", nightly))]
    pub alloc_current: super::alloc::AllocMetrics,
    #[cfg(all(feature = "flamegraph-alloc", nightly))]
    pub alloc_total: super::alloc::AllocMetrics,
}

impl fmt::Debug for FlameMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(not(all(feature = "flamegraph-alloc", nightly)))]
        {
            write!(
                f,
                "[run={:?} pend={:?} {:?}]",
                self.running, self.pending, self.count
            )
        }
        #[cfg(all(feature = "flamegraph-alloc", nightly))]
        {
            write!(
                f,
                "[run={:?} pend={:?} {:?} {:?}]",
                self.running, self.pending, self.count, self.alloc_total
            )
        }
    }
}

impl SpanMetrics for FlameMetric {
    type Arg = Clock;
    const REENTER: bool = true;

    fn enter(&mut self, now: Clock) {
        self.count.enter(());
        self.pending.try_stop(now);
        self.running.start(now);
        #[cfg(all(feature = "flamegraph-alloc", nightly))]
        {
            self.alloc_current = super::alloc::get_alloc_metrics();
        }
    }

    fn exit(&mut self, now: Clock) {
        self.count.exit(());
        self.running.stop(now);
        self.pending.start(now);
        #[cfg(all(feature = "flamegraph-alloc", nightly))]
        {
            self.alloc_total
                .merge(super::alloc::get_alloc_metrics().delta(self.alloc_current));
        }
    }
}

impl MergeMetrics for FlameMetric {
    fn merge(&mut self, other: Self) {
        self.count.merge(other.count);
        self.running.total += other.running.total;
        self.pending.total += other.pending.total;
        #[cfg(all(feature = "flamegraph-alloc", nightly))]
        self.alloc_total.merge(other.alloc_total);
    }
}

/// Count number of times span was entered/exited.
#[allow(dead_code)]
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct CountMetric {
    pub entered: usize,
    pub exited: usize,
}

impl fmt::Debug for CountMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "enter={} exit={}", self.entered, self.exited)
    }
}

impl SpanMetrics for CountMetric {
    type Arg = ();
    const REENTER: bool = true;

    fn enter(&mut self, _: ()) {
        debug_assert_eq!(self.entered, self.exited);
        self.entered += 1;
    }

    fn exit(&mut self, _: ()) {
        self.exited += 1;
        debug_assert_eq!(self.entered, self.exited);
    }
}

impl MergeMetrics for CountMetric {
    fn merge(&mut self, other: Self) {
        self.entered += other.entered;
        self.exited += other.exited;
    }
}
