// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use tokio_rustls::rustls::pki_types::CertificateDer;

pub(crate) type ActiveConnections<A = std::net::SocketAddr> =
    Arc<RwLock<HashMap<ConnectionId, ConnectionInfo<A>>>>;

pub type ConnectionId = usize;

#[derive(Debug)]
pub struct ConnectionInfo<A>(Arc<Inner<A>>);

#[derive(Clone, Debug)]
pub struct PeerCertificates(Arc<Vec<tokio_rustls::rustls::pki_types::CertificateDer<'static>>>);

impl PeerCertificates {
    pub fn peer_certs(&self) -> &[tokio_rustls::rustls::pki_types::CertificateDer<'static>] {
        self.0.as_ref()
    }
}

impl<A> ConnectionInfo<A> {
    pub(crate) fn new(
        address: A,
        peer_certificates: Option<Arc<Vec<CertificateDer<'static>>>>,
        graceful_shutdown_token: tokio_util::sync::CancellationToken,
    ) -> Self {
        Self(Arc::new(Inner {
            address,
            time_established: std::time::Instant::now(),
            peer_certificates: peer_certificates.map(PeerCertificates),
            graceful_shutdown_token,
        }))
    }

    /// The peer's remote address
    pub fn remote_address(&self) -> &A {
        &self.0.address
    }

    /// Time the Connection was established
    pub fn time_established(&self) -> std::time::Instant {
        self.0.time_established
    }

    pub fn peer_certificates(&self) -> Option<&PeerCertificates> {
        self.0.peer_certificates.as_ref()
    }

    /// A stable identifier for this connection
    pub fn id(&self) -> ConnectionId {
        &*self.0 as *const _ as usize
    }

    /// Trigger a graceful shutdown of this connection
    pub fn close(&self) {
        self.0.graceful_shutdown_token.cancel()
    }
}

#[derive(Debug)]
struct Inner<A = std::net::SocketAddr> {
    address: A,

    // Time that the connection was established
    time_established: std::time::Instant,

    peer_certificates: Option<PeerCertificates>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
}

#[derive(Debug, Clone)]
pub struct ConnectInfo<A = std::net::SocketAddr> {
    /// Returns the local address of this connection.
    pub local_addr: A,
    /// Returns the remote (peer) address of this connection.
    pub remote_addr: A,
}

impl<A> ConnectInfo<A> {
    /// Return the local address the IO resource is connected.
    pub fn local_addr(&self) -> &A {
        &self.local_addr
    }

    /// Return the remote address the IO resource is connected too.
    pub fn remote_addr(&self) -> &A {
        &self.remote_addr
    }
}

/// Number of established connections held by each authenticated peer, keyed by
/// the peer's public key.
#[derive(Clone, Debug, Default)]
pub(crate) struct PeerConnectionCounts(Arc<Mutex<HashMap<Vec<u8>, usize>>>);

impl PeerConnectionCounts {
    /// Counts one more connection for `peer`, or returns `None` if the peer
    /// already holds `max` of them.
    pub(crate) fn register(&self, peer: Vec<u8>, max: usize) -> Option<PeerConnectionGuard> {
        let mut counts = self.0.lock().unwrap();
        // A zero `max` is rejected by `Config::validate`, so a count just
        // inserted as 0 is always below it and never left behind on refusal.
        let count = counts.entry(peer.clone()).or_insert(0);
        if *count >= max {
            return None;
        }
        *count += 1;

        Some(PeerConnectionGuard {
            counts: self.clone(),
            peer,
        })
    }
}

/// Gives the peer its connection back when dropped.
pub(crate) struct PeerConnectionGuard {
    counts: PeerConnectionCounts,
    peer: Vec<u8>,
}

impl Drop for PeerConnectionGuard {
    fn drop(&mut self) {
        let mut counts = self.counts.0.lock().unwrap();
        if let Some(count) = counts.get_mut(&self.peer) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.peer);
            }
        }
    }
}
