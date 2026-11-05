// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::pin, time::Duration};

use http::{Request, Response};
use hyper_util::server::graceful::GracefulConnection;
use tracing::{debug, trace};

use crate::{
    ActiveConnections, BoxError, ConnectionId, connection_info::PeerConnectionGuard, fuse::Fuse,
};

// This is moved to its own function as a way to get around
// https://github.com/rust-lang/rust/issues/102211
pub async fn serve_connection<IO, S, B, C>(
    hyper_io: IO,
    hyper_svc: S,
    builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
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
        )
        .await;
    } else {
        serve_until_closed(
            builder.serve_connection(hyper_io, hyper_svc),
            graceful_shutdown_token,
            max_connection_age,
        )
        .await;
    }

    trace!("connection closed");
    drop(on_connection_close);
}

/// Serves `conn` until it closes. Starts a graceful shutdown of `conn` on the
/// shutdown signal and when it reaches `max_connection_age`.
async fn serve_until_closed(
    conn: impl GracefulConnection<Error = BoxError>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    max_connection_age: Option<Duration>,
) {
    let mut sig = pin!(Fuse::new(graceful_shutdown_token.cancelled_owned()));

    let mut conn = pin!(conn);

    let sleep = sleep_or_pending(max_connection_age);
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            _ = &mut sig => {
                conn.as_mut().graceful_shutdown();
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
