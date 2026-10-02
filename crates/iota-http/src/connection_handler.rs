// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::pin, time::Duration};

use http::{Request, Response};
use tracing::{debug, trace};

use crate::{
    ActiveConnections, BoxError, ConnectionEvent, ConnectionId, activity::IdleSleep,
    config::OnConnectionEvent, connection_info::PeerConnectionGuard, fuse::Fuse,
};

/// How long a connection asked to close is given to finish shutting down
/// before it is dropped. A connection is only asked once it has nothing in
/// flight, so this covers the round trip of the shutdown itself.
const SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

// This is moved to its own function as a way to get around
// https://github.com/rust-lang/rust/issues/102211
pub async fn serve_connection<IO, S, B, C>(
    hyper_io: IO,
    hyper_svc: S,
    builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    idle_timer: IdleSleep,
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
    let mut sig = pin!(Fuse::new(graceful_shutdown_token.cancelled_owned()));

    let mut conn = pin!(builder.serve_connection_with_upgrades(hyper_io, hyper_svc));

    let age = sleep_or_pending(max_connection_age);
    tokio::pin!(age);
    // Pinned once and polled by reference: built inside the branch below it
    // would start again on every pass of the loop and never be reached.
    tokio::pin!(idle_timer);
    // A graceful shutdown asks the peer to go away and then waits for it. Every
    // reason this connection is asked to go is a reason the peer may not
    // oblige, so each of them bounds that wait and the connection is dropped
    // when it runs out, which closes it. Asking is also what stops the branches
    // below firing repeatedly, so they are disabled once it has been asked.
    let mut shutdown_requested = false;
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
            _ = &mut idle_timer, if !shutdown_requested => {
                // A request can arrive between the timer being reached and this
                // running, so being ready is not on its own proof that the
                // connection is still idle.
                if idle_timer.is_busy() {
                    continue;
                }
                debug!("closing a connection that has been idle past its deadline");
                request_shutdown!();
            }
            _ = &mut age, if !shutdown_requested => {
                debug!("closing a connection that has reached its maximum age");
                request_shutdown!();
            }
            _ = &mut force_close => {
                debug!("dropping a connection that did not close when asked");
                break;
            }
            rv = &mut conn => {
                if let Err(err) = rv {
                    debug!("failed serving connection: {:#}", err);
                }
                break;
            },
        }
    }

    trace!("connection closed");
    drop(on_connection_close);
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
    _peer_connection_guard: Option<PeerConnectionGuard>,
    on_connection_event: Option<OnConnectionEvent>,
}

impl<A> OnConnectionClose<A> {
    pub(crate) fn new(
        id: ConnectionId,
        active_connections: ActiveConnections<A>,
        peer_connection_guard: Option<PeerConnectionGuard>,
        on_connection_event: Option<OnConnectionEvent>,
    ) -> Self {
        Self {
            id,
            active_connections,
            _peer_connection_guard: peer_connection_guard,
            on_connection_event,
        }
    }
}

impl<A> Drop for OnConnectionClose<A> {
    fn drop(&mut self) {
        let (was_registered, live) = {
            let mut active_connections = self.active_connections.write().unwrap();
            let was_registered = active_connections.remove(&self.id).is_some();
            (was_registered, active_connections.len())
        };
        // An evicted connection was removed when its slot was handed on, and
        // reported closed then; reporting it again here would count it twice.
        if !was_registered {
            return;
        }
        if let Some(on_connection_event) = &self.on_connection_event {
            on_connection_event.call(ConnectionEvent::Closed { live });
        }
    }
}
