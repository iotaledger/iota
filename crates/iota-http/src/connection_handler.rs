// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::pin, time::Duration};

use http::{Request, Response};
use hyper_util::server::graceful::GracefulConnection;
use tracing::{debug, trace};

use crate::{
    ActiveConnections, BoxError, ConnectionId,
    connection_info::ConnectionCountGuard,
    fuse::Fuse,
    idle::IdleSleep,
    metrics::{CloseReason, ListenerMetrics},
};

/// How long a connection asked to close is given to shut down while it has
/// nothing in flight: the round trip of the shutdown itself.
pub(crate) const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

/// The most times that grace is given again to a connection that is still
/// serving a request, or that does not count its requests. A connection asked
/// to close is dropped at most [`SHUTDOWN_GRACE_PERIOD`] × (1 + this) later,
/// 6 s, however long its request takes.
pub(crate) const MAX_GRACE_PERIODS: u32 = 5;

// This is moved to its own function as a way to get around
// https://github.com/rust-lang/rust/issues/102211
pub async fn serve_connection<IO, S, B, C>(
    hyper_io: IO,
    hyper_svc: S,
    builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    idle_timer: IdleSleep,
    metrics: Option<ListenerMetrics>,
    on_connection_close: C,
) where
    B: http_body::Body + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    IO: hyper::rt::Read + hyper::rt::Write + Send + Unpin + 'static,
    S: hyper::service::Service<Request<hyper::body::Incoming>, Response = Response<B>> + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // `serve_connection_with_upgrades` serves HTTP/1 even when the builder
    // allows only HTTP/2.
    if builder.is_http1_available() {
        serve_until_closed(
            builder.serve_connection_with_upgrades(hyper_io, hyper_svc),
            graceful_shutdown_token,
            max_connection_age,
            idle_timer,
            metrics,
        )
        .await;
    } else {
        serve_until_closed(
            builder.serve_connection(hyper_io, hyper_svc),
            graceful_shutdown_token,
            max_connection_age,
            idle_timer,
            metrics,
        )
        .await;
    }

    trace!("connection closed");
    drop(on_connection_close);
}

/// Serves `conn` until it closes. Starts a graceful shutdown of `conn` on the
/// shutdown signal, when it reaches `max_connection_age`, and when
/// `idle_timer` is reached. If `conn` has not closed [`SHUTDOWN_GRACE_PERIOD`]
/// later it is dropped, unless it is still serving a request or does not count
/// its requests: then it is given the grace again, up to
/// [`MAX_GRACE_PERIODS`] times.
async fn serve_until_closed(
    conn: impl GracefulConnection<Error = BoxError>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    idle_timer: IdleSleep,
    metrics: Option<ListenerMetrics>,
) {
    let mut sig = pin!(Fuse::new(graceful_shutdown_token.cancelled_owned()));

    let mut conn = pin!(conn);

    let sleep = sleep_or_pending(max_connection_age);
    tokio::pin!(idle_timer);
    tokio::pin!(sleep);

    // Asking a connection to close is a request, not an outcome: the peer may
    // not oblige. So every reason to close it bounds how long it is given, and
    // the connection is dropped when that runs out, which closes the socket.
    // Asking is also what stops the branches below firing repeatedly, so they
    // are disabled once it has been asked.
    let mut shutdown_requested = false;
    let mut graces_given = 0;
    let force_close = sleep_or_pending(None);
    tokio::pin!(force_close);

    macro_rules! request_shutdown {
        ($reason:expr) => {{
            shutdown_requested = true;
            if let Some(metrics) = &metrics {
                metrics.record_close($reason);
            }
            conn.as_mut().graceful_shutdown();
            force_close.set(sleep_or_pending(Some(SHUTDOWN_GRACE_PERIOD)));
        }};
    }

    loop {
        tokio::select! {
            _ = &mut sig, if !shutdown_requested => {
                request_shutdown!(CloseReason::Shutdown);
            }
            rv = &mut conn => {
                if let Err(err) = rv {
                    debug!("failed serving connection: {:#}", err);
                }
                break;
            },
            _ = &mut idle_timer, if !shutdown_requested => {
                // A request can arrive between the timer being reached and
                // this running, so being ready is not on its own proof that
                // the connection is still idle.
                if idle_timer.is_busy() == Some(true) {
                    continue;
                }
                debug!("closing a connection that has been idle past its deadline");
                request_shutdown!(CloseReason::Idle);
            }
            _ = &mut sleep, if !shutdown_requested => {
                debug!("closing a connection that has reached its maximum age");
                request_shutdown!(CloseReason::MaxAge);
            }
            _ = &mut force_close => {
                // A graceful shutdown finishes the response the connection is
                // already serving, and dropping it here would discard that
                // work. One that is still serving is given the grace again,
                // bounded so a peer cannot hold a closing connection open by
                // never finishing. One that does not count its requests may be
                // serving one, so it is given the grace as if it were.
                if idle_timer.is_busy().unwrap_or(true) && graces_given < MAX_GRACE_PERIODS {
                    graces_given += 1;
                    force_close.set(sleep_or_pending(Some(SHUTDOWN_GRACE_PERIOD)));
                    continue;
                }
                debug!("dropping a connection that did not close when asked");
                break;
            }
        }
    }
}

pub(crate) async fn sleep_or_pending(wait_for: Option<Duration>) {
    match wait_for {
        Some(wait) => tokio::time::sleep(wait).await,
        None => std::future::pending().await,
    };
}

pub(crate) struct OnConnectionClose<A> {
    id: ConnectionId,
    active_connections: ActiveConnections<A>,
    _count_guard: ConnectionCountGuard,
}

impl<A> OnConnectionClose<A> {
    pub(crate) fn new(
        id: ConnectionId,
        active_connections: ActiveConnections<A>,
        count_guard: ConnectionCountGuard,
    ) -> Self {
        Self {
            id,
            active_connections,
            _count_guard: count_guard,
        }
    }
}

impl<A> Drop for OnConnectionClose<A> {
    fn drop(&mut self) {
        self.active_connections.write().unwrap().remove(&self.id);
    }
}
