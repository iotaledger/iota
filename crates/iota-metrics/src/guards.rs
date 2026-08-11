// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use prometheus_filtered::IntGauge;

/// Increments gauge when acquired, decrements when guard drops
pub struct GaugeGuard<'a>(&'a IntGauge);

impl<'a> GaugeGuard<'a> {
    /// Acquires an `IntGauge` by incrementing its value and creating a new
    /// `IntGaugeGuard` instance that holds a reference to the gauge.
    pub fn acquire(g: &'a IntGauge) -> Self {
        g.inc();
        Self(g)
    }
}

impl Drop for GaugeGuard<'_> {
    /// Decrements the value of the `IntGauge` when the `IntGaugeGuard` is
    /// dropped.
    fn drop(&mut self) {
        self.0.dec();
    }
}

/// Difference vs `GaugeGuard`: stores the gauge by value to avoid borrowing
/// issues. Increments the gauge when acquired, decrements when the guard drops.
pub struct InflightGuard(IntGauge);

impl InflightGuard {
    /// Acquires an `IntGauge` by incrementing its value and taking ownership of
    /// the gauge so it can be decremented on drop.
    pub fn acquire(g: IntGauge) -> Self {
        g.inc();
        Self(g)
    }
}

impl Drop for InflightGuard {
    /// Decrements the value of the `IntGauge` when the guard is dropped.
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub trait InflightGuardFutureExt: Future + Sized {
    /// Count number of in flight futures running
    fn count_in_flight(self, g: IntGauge) -> InflightGuardFuture<Self>;
}

impl<F: Future> InflightGuardFutureExt for F {
    /// Count number of in flight futures running.
    fn count_in_flight(self, g: IntGauge) -> InflightGuardFuture<Self> {
        InflightGuardFuture {
            f: Box::pin(self),
            _guard: InflightGuard::acquire(g),
        }
    }
}

/// A struct that wraps a future (`f`) with an `InflightGuard`. The
/// `InflightGuardFuture` is used to manage the lifecycle of a future while
/// ensuring the associated `InflightGuard` properly tracks the resource usage
/// during the future's execution. The guard increments the gauge
/// when the future starts and decrements it when the `InflightGuardFuture` is
/// dropped.
pub struct InflightGuardFuture<F: Sized> {
    f: Pin<Box<F>>,
    _guard: InflightGuard,
}

impl<F: Future> Future for InflightGuardFuture<F> {
    type Output = F::Output;

    /// Polls the wrapped future (`f`) to determine its readiness. This function
    /// forwards the poll operation to the inner future, allowing the
    /// `InflightGuardFuture` to manage the polling lifecycle.
    /// Returns `Poll::Pending` if the future is not ready or `Poll::Ready` with
    /// the future's result if complete.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.f.as_mut().poll(cx)
    }
}
