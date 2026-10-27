// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::pin, time::Duration};

use http::{Request, Response};
use hyper_util::server::graceful::GracefulConnection;
use tracing::{debug, trace};

use crate::{
    ActiveConnections, BoxError, ConnectionId,
    connection_info::PeerConnectionGuard,
    fuse::Fuse,
    idle_timer::{IdleHandle, IdleTimer},
};

/// How long a connection asked to close is given to shut down.
pub(crate) const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

/// How many more grace periods a connection still serving a request is given
/// before it is dropped.
pub(crate) const MAX_GRACE_PERIODS: u32 = 5;

// This is moved to its own function as a way to get around
// https://github.com/rust-lang/rust/issues/102211
pub async fn serve_connection<IO, S, B, C>(
    hyper_io: IO,
    hyper_svc: S,
    builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    idle_timer: Option<IdleTimer>,
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
        )
        .await;
    } else {
        serve_until_closed(
            builder.serve_connection(hyper_io, hyper_svc),
            graceful_shutdown_token,
            max_connection_age,
            idle_timer,
        )
        .await;
    }

    trace!("connection closed");
    drop(on_connection_close);
}

/// Serves `conn` until it closes. Starts a graceful shutdown of `conn` on the
/// shutdown signal, at `max_connection_age`, and when `idle_timer` is reached.
/// Drops `conn` if it has not closed [`SHUTDOWN_GRACE_PERIOD`] later, unless it
/// has `idle_timer` and is still serving a request, which extends the grace up
/// to [`MAX_GRACE_PERIODS`] times.
async fn serve_until_closed(
    conn: impl GracefulConnection<Error = BoxError>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    idle_timer: Option<IdleTimer>,
) {
    let mut sig = pin!(Fuse::new(graceful_shutdown_token.cancelled_owned()));

    let mut conn = pin!(conn);

    let idle_handle = idle_timer.as_ref().map(IdleTimer::handle);
    let sleep = sleep_or_pending(max_connection_age);
    let idle = idle_or_pending(idle_timer);
    tokio::pin!(sleep);
    tokio::pin!(idle);

    // The peer may ignore a graceful shutdown, so it is bounded by
    // `force_close`. Once requested, the branches that request it are disabled.
    let mut shutdown_requested = false;
    let mut graces_given = 0;
    let force_close = sleep_or_pending(None);
    tokio::pin!(force_close);

    macro_rules! request_shutdown {
        () => {{
            shutdown_requested = true;
            conn.as_mut().graceful_shutdown();
            force_close.set(sleep_or_pending(Some(SHUTDOWN_GRACE_PERIOD)));
        }};
    }

    loop {
        tokio::select! {
            _ = &mut sig, if !shutdown_requested => {
                request_shutdown!();
            }
            rv = &mut conn => {
                if let Err(err) = rv {
                    debug!("failed serving connection: {:#}", err);
                }
                break;
            },
            _ = &mut idle, if !shutdown_requested => {
                debug!("closing a connection that has been idle past its deadline");
                request_shutdown!();
            }
            _ = &mut sleep, if !shutdown_requested => {
                debug!("closing a connection that has reached its maximum age");
                request_shutdown!();
            }
            _ = &mut force_close => {
                // Dropping a connection discards the response it is serving. One
                // without an idle timer is dropped after the first grace.
                let busy = idle_handle.as_ref().is_some_and(IdleHandle::is_busy);
                if busy && graces_given < MAX_GRACE_PERIODS {
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

/// Waits for `idle_timer`, or forever when there is none.
async fn idle_or_pending(idle_timer: Option<IdleTimer>) {
    match idle_timer {
        Some(idle_timer) => idle_timer.wait().await,
        None => std::future::pending().await,
    };
}

pub(crate) struct OnConnectionClose<A> {
    id: ConnectionId,
    active_connections: ActiveConnections<A>,
    _peer_connection_guard: Option<PeerConnectionGuard>,
}

impl<A> OnConnectionClose<A> {
    pub(crate) fn new(
        id: ConnectionId,
        active_connections: ActiveConnections<A>,
        peer_connection_guard: Option<PeerConnectionGuard>,
    ) -> Self {
        Self {
            id,
            active_connections,
            _peer_connection_guard: peer_connection_guard,
        }
    }
}

impl<A> Drop for OnConnectionClose<A> {
    fn drop(&mut self) {
        self.active_connections.write().unwrap().remove(&self.id);
    }
}
