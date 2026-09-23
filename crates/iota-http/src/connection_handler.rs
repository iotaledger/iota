// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::pin, time::Duration};

use http::{Request, Response};
use tracing::{debug, trace};

use crate::{
    ActiveConnections, BoxError, ConnectionEvent, ConnectionId,
    activity::{ConnectionActivity, idle_elapsed},
    config::OnConnectionEvent,
    connection_info::PeerConnectionGuard,
    fuse::Fuse,
};

/// How long a connection closed for idleness is given to finish shutting down
/// before it is dropped. It has no requests in flight by definition, so this
/// only covers the round trip of the shutdown itself.
const IDLE_SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

// This is moved to its own function as a way to get around
// https://github.com/rust-lang/rust/issues/102211
pub async fn serve_connection<IO, S, B, C>(
    hyper_io: IO,
    hyper_svc: S,
    builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
    max_connection_idle: Option<Duration>,
    activity: ConnectionActivity,
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

    let sleep = sleep_or_pending(max_connection_age);
    tokio::pin!(sleep);
    // Closing an idle connection more than once would be pointless, and the
    // deadline stays elapsed once it passes.
    let mut closed_for_idleness = false;
    // A graceful shutdown asks the peer to go away and waits for it to do so.
    // The peer this deadline exists for will not, so the wait is bounded and
    // the connection is then dropped, which closes it.
    let force_close = sleep_or_pending(None);
    tokio::pin!(force_close);

    loop {
        tokio::select! {
            _ = &mut sig => {
                conn.as_mut().graceful_shutdown();
            }
            _ = idle_elapsed(&activity, max_connection_idle), if !closed_for_idleness => {
                debug!("closing a connection that has been idle past its deadline");
                closed_for_idleness = true;
                conn.as_mut().graceful_shutdown();
                force_close.set(sleep_or_pending(Some(IDLE_SHUTDOWN_GRACE_PERIOD)));
            }
            _ = &mut force_close => {
                debug!("dropping an idle connection that did not close on request");
                break;
            }
            rv = &mut conn => {
                if let Err(err) = rv {
                    debug!("failed serving connection: {:#}", err);
                }
                break;
            },
            _ = &mut sleep  => {
                conn.as_mut().graceful_shutdown();
                sleep.set(sleep_or_pending(None));
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
        let live = {
            let mut active_connections = self.active_connections.write().unwrap();
            active_connections.remove(&self.id);
            active_connections.len()
        };
        if let Some(on_connection_event) = &self.on_connection_event {
            on_connection_event.call(ConnectionEvent::Closed { live });
        }
    }
}
