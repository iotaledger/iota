// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Instant};

use crate::metrics::{
    Inner,
    tls::{self, HandshakeGuard},
};

/// One accepted connection. Dropping it records the close of the connection.
pub(crate) struct ConnectionGuard {
    listener: Arc<Inner>,
    accepted_at: Instant,
}

impl ConnectionGuard {
    pub(super) fn new(listener: Arc<Inner>) -> Self {
        Self {
            listener,
            accepted_at: Instant::now(),
        }
    }

    /// Records the start of a TLS handshake of this connection; the handshake
    /// itself is run by the caller. Returns `None` when the listener has no
    /// TLS metrics.
    pub(crate) fn record_handshake_start(&self) -> Option<HandshakeGuard> {
        let tls = self.listener.tls.get()?;
        Some(tls::record_handshake_start(tls))
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let metrics = &self.listener.metrics;
        let age = Instant::now().saturating_duration_since(self.accepted_at);
        metrics
            .connection_lifetime_seconds
            .observe(age.as_secs_f64());
        metrics.inbound_connections.dec();
    }
}
