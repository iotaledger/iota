// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Instant};

use crate::metrics::{Inner, tls::HandshakeGuard};

/// One accepted connection. Cloning is cheap.
#[derive(Clone)]
pub(crate) struct TrackedConnection(Arc<ConnectionState>);

impl TrackedConnection {
    pub(super) fn new(listener: Arc<Inner>) -> Self {
        Self(Arc::new(ConnectionState {
            listener,
            accepted_at: Instant::now(),
        }))
    }

    /// Starts to time a TLS handshake of this connection. Returns `None` when
    /// the listener has no TLS metrics.
    pub(crate) fn begin_handshake(&self) -> Option<HandshakeGuard> {
        let tls = self.0.listener.tls.get()?;
        Some(tls.begin_handshake())
    }

    /// Records the close of the connection. Call it once.
    pub(super) fn record_close(&self) {
        let state = &*self.0;
        let metrics = &state.listener.metrics;
        let age = Instant::now().saturating_duration_since(state.accepted_at);
        metrics
            .connection_lifetime_seconds
            .observe(age.as_secs_f64());
        metrics.inbound_connections.dec();
    }
}

pub(super) struct ConnectionState {
    listener: Arc<Inner>,
    accepted_at: Instant,
}
