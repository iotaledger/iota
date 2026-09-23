// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// Tracks the requests a connection is serving, so its task can tell an idle
/// connection from a busy one.
///
/// Idle deliberately means "serving no request", not "receiving no bytes". The
/// server's own keepalive pings and the answers to them are bytes, so a peer
/// that answers them and does nothing else would look busy forever — which is
/// exactly the peer this is here to close. It also means a connection that has
/// not yet chosen a protocol counts as idle, because it has started no request
/// either.
#[derive(Clone)]
pub(crate) struct ConnectionActivity(Arc<Inner>);

struct Inner {
    /// Requests started and not yet finished, including the time a streaming
    /// response is still producing.
    in_flight: AtomicUsize,
    /// Milliseconds after `base` at which the last request finished, or 0 when
    /// none ever started.
    idle_since: AtomicU64,
    base: Instant,
}

impl ConnectionActivity {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Inner {
            in_flight: AtomicUsize::new(0),
            idle_since: AtomicU64::new(0),
            base: Instant::now(),
        }))
    }

    /// Marks a request as being served until the returned guard is dropped.
    pub(crate) fn request_started(&self) -> RequestGuard {
        self.0.in_flight.fetch_add(1, Ordering::Relaxed);
        RequestGuard(self.clone())
    }

    /// When this connection may be closed for being idle, or `None` while it
    /// is serving something.
    fn idle_deadline(&self, idle: Duration) -> Option<Instant> {
        if self.0.in_flight.load(Ordering::Relaxed) > 0 {
            return None;
        }
        let idle_since = Duration::from_millis(self.0.idle_since.load(Ordering::Relaxed));
        Some(self.0.base + idle_since + idle)
    }
}

/// Keeps its connection counted as busy for as long as it is held. It is
/// carried by the response body rather than the response future, so a
/// streaming response holds it until the last frame.
pub(crate) struct RequestGuard(ConnectionActivity);

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let inner = &self.0.0;
        if inner.in_flight.fetch_sub(1, Ordering::Relaxed) == 1 {
            let now = inner.base.elapsed().as_millis() as u64;
            inner.idle_since.store(now, Ordering::Relaxed);
        }
    }
}

/// Completes once the connection has been idle for `idle`, or never when no
/// deadline is configured.
///
/// A connection is closed no sooner than `idle` after its last request
/// finishes. Because a busy connection is only re-examined once per `idle`,
/// one that falls idle just after a check waits up to twice that.
pub(crate) async fn idle_elapsed(activity: &ConnectionActivity, idle: Option<Duration>) {
    let Some(idle) = idle else {
        return std::future::pending().await;
    };

    loop {
        match activity.idle_deadline(idle) {
            // Serving something, so look again once it could have finished.
            None => tokio::time::sleep(idle).await,
            Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                Some(remaining) if !remaining.is_zero() => tokio::time::sleep(remaining).await,
                _ => return,
            },
        }
    }
}
