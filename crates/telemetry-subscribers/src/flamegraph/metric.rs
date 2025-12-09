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
        self.total += now.duration_since(self.clock).unwrap();
        self.reset();
    }

    pub fn try_stop(&mut self, now: Clock) {
        if self.is_ticking() {
            debug_assert!(self.clock <= now, "{:?} <= {now:?}", self.clock);
            self.total += now.duration_since(self.clock).unwrap();
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
    #[cfg(debug_assertions)]
    count: CountMetric,
}

impl fmt::Debug for FlameMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(debug_assertions)]
        {
            write!(
                f,
                "[run={:?} pend={:?} {:?}]",
                self.running, self.pending, self.count
            )
        }
        #[cfg(not(debug_assertions))]
        {
            write!(f, "[run={:?} pend={:?}]", self.running, self.pending)
        }
    }
}

impl SpanMetrics for FlameMetric {
    type Arg = Clock;
    const REENTER: bool = true;

    fn enter(&mut self, now: Clock) {
        #[cfg(debug_assertions)]
        self.count.enter(());
        self.pending.try_stop(now);
        self.running.start(now);
    }

    fn exit(&mut self, now: Clock) {
        #[cfg(debug_assertions)]
        self.count.exit(());
        self.running.stop(now);
        self.pending.start(now);
    }
}

impl MergeMetrics for FlameMetric {
    fn merge(&mut self, other: Self) {
        #[cfg(debug_assertions)]
        self.count.merge(other.count);
        self.running.total += other.running.total;
        self.pending.total += other.pending.total;
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
