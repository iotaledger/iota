// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio_rustls::rustls::pki_types::CertificateDer;

use crate::{
    config::{OnPeerConnectionEvent, PeerConnectionEvent},
    idle::IdleHandle,
};

pub(crate) type ActiveConnections<A = std::net::SocketAddr> =
    Arc<RwLock<HashMap<ConnectionId, ConnectionInfo<A>>>>;

/// Identifies one connection for as long as this process runs. Never reused,
/// so an entry in a table of live connections cannot be taken for a different
/// connection than the one that put it there.
pub type ConnectionId = usize;

/// Handed out by [`ConnectionInfo::new`], one per connection.
static NEXT_CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);

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
        idle: IdleHandle,
    ) -> Self {
        Self(Arc::new(Inner {
            id: NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed),
            address,
            time_established: std::time::Instant::now(),
            peer_certificates: peer_certificates.map(PeerCertificates),
            graceful_shutdown_token,
            idle,
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

    /// A stable identifier for this connection, unique for the life of the
    /// process.
    pub fn id(&self) -> ConnectionId {
        self.0.id
    }

    /// How long this connection has gone without being asked for anything,
    /// counted from when it was established if it never has, or `None` while
    /// it is serving a request or when its listener sets no
    /// `max_connection_idle`, which is what makes it track idleness.
    pub fn idle_for(&self) -> Option<std::time::Duration> {
        self.0.idle.idle_for()
    }

    /// Trigger a graceful shutdown of this connection
    pub fn close(&self) {
        self.0.graceful_shutdown_token.cancel()
    }
}

#[derive(Debug)]
struct Inner<A = std::net::SocketAddr> {
    id: ConnectionId,

    address: A,

    // Time that the connection was established
    time_established: std::time::Instant,

    peer_certificates: Option<PeerCertificates>,
    idle: IdleHandle,
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

/// A client's address prefix or a peer's public key, as connections are
/// counted under it. The two never collide: an address prefix is 4 or 16
/// bytes long, a public key 32.
pub(crate) type ConnectionKey = Box<[u8]>;

/// Number of connections held under each [`ConnectionKey`].
#[derive(Clone, Debug)]
pub(crate) struct ConnectionCounts {
    counts: Arc<Mutex<HashMap<ConnectionKey, usize>>>,
    on_peer_event: Option<OnPeerConnectionEvent>,
}

impl ConnectionCounts {
    /// `on_peer_event` is told about the connections counted under a peer's
    /// public key, and only those.
    pub(crate) fn new(on_peer_event: Option<OnPeerConnectionEvent>) -> Self {
        Self {
            counts: Arc::default(),
            on_peer_event,
        }
    }

    /// A guard for a new connection, counted under nothing until one of its
    /// `add_*` methods is called.
    pub(crate) fn guard(&self) -> ConnectionCountGuard {
        ConnectionCountGuard {
            counts: self.clone(),
            client_ip: None,
            peer_public_key: None,
        }
    }

    /// Counts one more connection under `key` and returns how many it holds,
    /// or returns `None` if it already holds `max`.
    fn increment(&self, key: &ConnectionKey, max: usize) -> Option<usize> {
        let mut counts = self.counts.lock().unwrap();
        // A zero `max` is rejected by `Config::validate`, so a key with no
        // entry yet is always below the limit.
        if counts.get(key).is_some_and(|count| *count >= max) {
            return None;
        }
        let count = counts.entry(key.clone()).or_insert(0);
        *count += 1;
        Some(*count)
    }

    fn notify_peer(&self, peer: &[u8], event: PeerConnectionEvent) {
        if let Some(on_event) = &self.on_peer_event {
            on_event.call(peer, event);
        }
    }
}

/// Counts one connection under its client's address and its peer's public
/// key, and gives both back when dropped.
pub(crate) struct ConnectionCountGuard {
    counts: ConnectionCounts,
    client_ip: Option<ConnectionKey>,
    peer_public_key: Option<ConnectionKey>,
}

impl ConnectionCountGuard {
    /// Counts the connection under its client's address, or returns `false`
    /// if the address already holds `max` connections.
    pub(crate) fn add_client_ip(&mut self, client_ip: ConnectionKey, max: usize) -> bool {
        if self.counts.increment(&client_ip, max).is_none() {
            return false;
        }
        self.client_ip = Some(client_ip);
        true
    }

    /// Counts the connection under its peer's public key, or returns `false`
    /// if the peer already holds `max` connections.
    pub(crate) fn add_peer_public_key(&mut self, peer: ConnectionKey, max: usize) -> bool {
        let Some(held) = self.counts.increment(&peer, max) else {
            self.counts
                .notify_peer(&peer, PeerConnectionEvent::RefusedAtLimit { held: max });
            return false;
        };
        self.counts
            .notify_peer(&peer, PeerConnectionEvent::Established { held });
        self.peer_public_key = Some(peer);
        true
    }
}

impl Drop for ConnectionCountGuard {
    fn drop(&mut self) {
        if self.client_ip.is_none() && self.peer_public_key.is_none() {
            return;
        }

        let peer_held = {
            let mut counts = self.counts.counts.lock().unwrap();
            if let Some(client_ip) = &self.client_ip {
                decrement(&mut counts, client_ip);
            }
            self.peer_public_key
                .as_ref()
                .map(|peer| (peer, decrement(&mut counts, peer)))
        };
        if let Some((peer, held)) = peer_held {
            self.counts
                .notify_peer(peer, PeerConnectionEvent::Closed { held });
        }
    }
}

/// Counts one connection fewer under `key`, and returns how many it holds
/// afterwards.
fn decrement(counts: &mut HashMap<ConnectionKey, usize>, key: &[u8]) -> usize {
    let Some(count) = counts.get_mut(key) else {
        return 0;
    };
    *count -= 1;
    let held = *count;
    if held == 0 {
        counts.remove(key);
    }
    held
}
