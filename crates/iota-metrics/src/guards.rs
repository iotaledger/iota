// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use prometheus::IntGauge;

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

impl<'a> Drop for GaugeGuard<'a> {
    /// Decrements the value of the `IntGauge` when the `IntGaugeGuard` is
    /// dropped.
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub trait GaugeGuardFutureExt: Future + Sized {
    /// Count number of in flight futures running
    fn count_in_flight(self, g: &IntGauge) -> GaugeGuardFuture<Self>;
}

impl<F: Future> GaugeGuardFutureExt for F {
    /// Count number of in flight futures running.
    fn count_in_flight(self, g: &IntGauge) -> GaugeGuardFuture<Self> {
        GaugeGuardFuture {
            f: Box::pin(self),
            _guard: GaugeGuard::acquire(g),
        }
    }
}

/// A struct that wraps a future (`f`) with a `GaugeGuard`. The
/// `GaugeGuardFuture` is used to manage the lifecycle of a future while
/// ensuring the associated `GaugeGuard` properly tracks the resource usage
/// during the future's execution. The guard increments the gauge
/// when the future starts and decrements it when the `GaugeGuardFuture` is
/// dropped.
pub struct GaugeGuardFuture<'a, F: Sized> {
    f: Pin<Box<F>>,
    _guard: GaugeGuard<'a>,
}

impl<'a, F: Future> Future for GaugeGuardFuture<'a, F> {
    type Output = F::Output;

    /// Polls the wrapped future (`f`) to determine its readiness. This function
    /// forwards the poll operation to the inner future, allowing the
    /// `GaugeGuardFuture` to manage the polling lifecycle.
    /// Returns `Poll::Pending` if the future is not ready or `Poll::Ready` with
    /// the future's result if complete.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.f.as_mut().poll(cx)
    }
}
