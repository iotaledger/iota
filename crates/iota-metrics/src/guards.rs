// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use prometheus_filtered::{IntGauge, IntGaugeVec};

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

/// Increments an `IntGaugeVec` for a set of label values when acquired,
/// decrements the same labeled gauge when the guard drops.
pub struct IntGaugeVecGuard<'a> {
    gauge: &'a IntGaugeVec,
    labels: Vec<String>,
}

impl<'a> IntGaugeVecGuard<'a> {
    /// Acquires the labeled gauge by incrementing it and retaining the label
    /// values so the matching gauge can be decremented on drop.
    pub fn acquire(gauge: &'a IntGaugeVec, labels: &[&str]) -> Self {
        gauge.with_label_values(labels).inc();
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        Self { gauge, labels }
    }
}

impl Drop for IntGaugeVecGuard<'_> {
    /// Decrements the labeled gauge when the guard is dropped.
    fn drop(&mut self) {
        self.gauge
            .with_label_values(&self.labels.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .dec();
    }
}

pub trait GaugeGuardFutureExt: Future + Sized {
    /// Count number of in flight futures running
    fn count_in_flight(self, g: &IntGauge) -> GaugeGuardFuture<'_, Self>;

    /// Count number of in flight futures running, tracked on a labeled gauge.
    fn count_in_flight_with_labels<'a>(
        self,
        g: &'a IntGaugeVec,
        labels: &[&str],
    ) -> IntGaugeVecGuardFuture<'a, Self>;
}

impl<F: Future> GaugeGuardFutureExt for F {
    /// Count number of in flight futures running.
    fn count_in_flight(self, g: &IntGauge) -> GaugeGuardFuture<'_, Self> {
        GaugeGuardFuture {
            f: Box::pin(self),
            _guard: GaugeGuard::acquire(g),
        }
    }

    /// Count number of in flight futures running, tracked on a labeled gauge.
    fn count_in_flight_with_labels<'a>(
        self,
        g: &'a IntGaugeVec,
        labels: &[&str],
    ) -> IntGaugeVecGuardFuture<'a, Self> {
        IntGaugeVecGuardFuture {
            f: Box::pin(self),
            _guard: IntGaugeVecGuard::acquire(g, labels),
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

impl<F: Future> Future for GaugeGuardFuture<'_, F> {
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

/// A struct that wraps a future (`f`) with an `IntGaugeVecGuard`. The labeled
/// gauge is incremented when the future starts and decremented when the
/// `IntGaugeVecGuardFuture` is dropped.
pub struct IntGaugeVecGuardFuture<'a, F: Sized> {
    f: Pin<Box<F>>,
    _guard: IntGaugeVecGuard<'a>,
}

impl<F: Future> Future for IntGaugeVecGuardFuture<'_, F> {
    type Output = F::Output;

    /// Polls the wrapped future (`f`) to determine its readiness, forwarding
    /// the poll to the inner future.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.f.as_mut().poll(cx)
    }
}
