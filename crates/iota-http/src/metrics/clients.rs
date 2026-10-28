// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The clients of a listener. A client is an IPv4 address or an IPv6 /64
//! prefix.

use std::{
    collections::{HashMap, hash_map::Entry},
    hash::Hash,
    net::{IpAddr, Ipv4Addr},
};

use iota_metrics::peak::PeakGaugeVec;
use parking_lot::Mutex;
use prometheus_filtered::{
    HistogramVec, IntGauge, IntGaugeVec, MetricLevel, Registry,
    register_histogram_vec_with_registry, register_int_gauge_vec_with_registry,
};

use crate::metrics::scope::Scope;

/// The connections of one client.
const CLIENT_CONNECTIONS_BUCKETS: &[f64] = &[1.0, 2.0, 5.0, 10.0, 16.0, 64.0, 1000.0];

/// What the per-client metrics group by: an IPv4 address, or the /64 prefix of
/// an IPv6 address. An IPv4-mapped IPv6 address is the IPv4 address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ClientKey {
    V4(Ipv4Addr),
    V6Prefix([u16; 4]),
}

impl ClientKey {
    fn of(ip: IpAddr) -> Self {
        match ip.to_canonical() {
            IpAddr::V4(ip) => Self::V4(ip),
            IpAddr::V6(ip) => {
                let segments = ip.segments();
                Self::V6Prefix([segments[0], segments[1], segments[2], segments[3]])
            }
        }
    }
}

/// The metrics of the clients of a listener.
struct Metrics {
    clients: IntGaugeVec,
    client_connections: HistogramVec,
    client_connections_peak: PeakGaugeVec,
}

impl Metrics {
    fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            clients: register_int_gauge_vec_with_registry!(
                format!("{prefix}_clients"),
                "Clients that hold a connection. A client is an IPv4 address or an IPv6 /64 \
                 prefix",
                &["scope"],
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            client_connections: register_histogram_vec_with_registry!(
                format!("{prefix}_client_connections"),
                "At each accept: the connections of the client, with the new one. A client is \
                 an IPv4 address or an IPv6 /64 prefix",
                &["scope"],
                CLIENT_CONNECTIONS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            client_connections_peak: PeakGaugeVec::register(
                &format!("{prefix}_client_connections_peak"),
                "The most connections one client held when it opened a connection, over the \
                 last 2 minutes",
                "scope",
                module_path!(),
                registry,
                level,
            ),
        }
    }

    fn clients(&self, scope: Scope) -> IntGauge {
        let scope: &str = scope.into();
        self.clients.with_label_values(&[scope])
    }

    fn record_connection(&self, scope: Scope, connections_held: u64) {
        let scope: &str = scope.into();
        self.client_connections
            .with_label_values(&[scope])
            .observe(connections_held as f64);
        self.client_connections_peak
            .with_label_values(&[scope])
            .observe(connections_held);
    }
}

/// What one client holds on the listener.
struct ClientConnections {
    /// The scope of the first connection of the client.
    scope: Scope,
    connections: u64,
}

/// The clients of a listener, and the metrics about them.
pub(super) struct Clients {
    metrics: Metrics,
    /// The clients that hold a connection.
    table: Mutex<HashMap<ClientKey, ClientConnections>>,
}

impl Clients {
    pub(super) fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            metrics: Metrics::new(prefix, registry, level),
            table: Mutex::new(HashMap::new()),
        }
    }

    /// Counts a new connection of `ip`, which is in `scope`. Returns the client
    /// to pass to `remove_connection` at the close.
    pub(super) fn add_connection(&self, ip: IpAddr, scope: Scope) -> ClientKey {
        let key = ClientKey::of(ip);
        let connections_held = {
            let mut table = self.table.lock();
            let client = table.entry(key).or_insert_with(|| {
                self.metrics.clients(scope).inc();
                ClientConnections {
                    scope,
                    connections: 0,
                }
            });
            client.connections += 1;
            client.connections
        };
        self.metrics.record_connection(scope, connections_held);
        key
    }

    /// Counts the close of a connection of `key`. A client without connections
    /// leaves the table.
    pub(super) fn remove_connection(&self, key: ClientKey) {
        let mut table = self.table.lock();
        let Entry::Occupied(mut entry) = table.entry(key) else {
            return;
        };
        entry.get_mut().connections -= 1;
        if entry.get().connections == 0 {
            let client = entry.remove();
            self.metrics.clients(client.scope).dec();
            shrink_if_sparse(&mut table);
        }
    }
}

/// Gives memory back from a table that is mostly empty, for example after a
/// peak of connections.
fn shrink_if_sparse<K: Eq + Hash, V>(table: &mut HashMap<K, V>) {
    /// A table with a capacity this many times its length gives memory back.
    const SHRINK_RATIO: usize = 4;
    /// The table shrinks only above this capacity.
    const MIN_CAPACITY: usize = 64;

    if table.capacity() > MIN_CAPACITY && table.capacity() > table.len() * SHRINK_RATIO {
        let keep = table.len() * 2;
        table.shrink_to(keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::tests::{ip, setup};

    /// Counts a new connection of `address`.
    fn add_connection(clients: &Clients, address: &str) -> ClientKey {
        let ip = ip(address);
        clients.add_connection(ip, Scope::of(ip))
    }

    #[test]
    fn key_of_ipv4_is_the_whole_address() {
        assert_ne!(ClientKey::of(ip("1.2.3.4")), ClientKey::of(ip("1.2.3.5")));
        assert_eq!(
            ClientKey::of(ip("1.2.3.4")),
            ClientKey::V4(Ipv4Addr::new(1, 2, 3, 4))
        );
    }

    #[test]
    fn key_of_ipv6_is_the_64_bit_prefix() {
        let key = ClientKey::of(ip("2001:db8:1:2::1"));
        assert_eq!(key, ClientKey::of(ip("2001:db8:1:2:ffff::9")));
        assert_ne!(key, ClientKey::of(ip("2001:db8:1:3::1")));
        assert_eq!(key, ClientKey::V6Prefix([0x2001, 0xdb8, 1, 2]));
    }

    #[test]
    fn mapped_ipv4_has_the_key_of_the_ipv4_address() {
        assert_eq!(
            ClientKey::of(ip("::ffff:1.2.3.4")),
            ClientKey::of(ip("1.2.3.4"))
        );
    }

    #[test]
    fn the_entry_of_a_key_leaves_with_its_last_connection() {
        let (metrics, reader) = setup();
        let clients = &metrics.0.clients;
        let first = add_connection(clients, "1.2.3.4");
        let second = add_connection(clients, "1.2.3.4");
        assert_eq!(clients.table.lock().len(), 1);
        assert_eq!(reader.value("clients", &[("scope", "public")]), 1.0);
        clients.remove_connection(first);
        assert_eq!(clients.table.lock().len(), 1);
        assert_eq!(reader.value("clients", &[("scope", "public")]), 1.0);
        clients.remove_connection(second);
        assert!(clients.table.lock().is_empty());
        assert_eq!(reader.value("clients", &[("scope", "public")]), 0.0);
    }

    #[test]
    fn the_table_gives_memory_back_after_many_keys() {
        let (metrics, _) = setup();
        let clients = &metrics.0.clients;
        let connections: Vec<_> = (0..10_000u32)
            .map(|address| {
                let ip = IpAddr::V4(Ipv4Addr::from(address));
                clients.add_connection(ip, Scope::of(ip))
            })
            .collect();
        assert!(clients.table.lock().capacity() >= 10_000);
        for key in connections {
            clients.remove_connection(key);
        }
        let capacity = clients.table.lock().capacity();
        assert!(capacity <= 128, "capacity {capacity}");
    }

    #[test]
    fn the_clients_of_each_scope_are_counted() {
        let (metrics, reader) = setup();
        let clients = &metrics.0.clients;
        let private = add_connection(clients, "10.0.0.1");
        let _public_a = add_connection(clients, "1.2.3.4");
        let _public_b = add_connection(clients, "1.2.3.5");
        for (scope, count) in [("loopback", 0.0), ("private", 1.0), ("public", 2.0)] {
            assert_eq!(
                reader.value("clients", &[("scope", scope)]),
                count,
                "{scope}"
            );
        }
        clients.remove_connection(private);
        assert_eq!(reader.value("clients", &[("scope", "private")]), 0.0);
    }

    #[test]
    fn connections_are_observed_by_scope() {
        let (metrics, reader) = setup();
        let clients = &metrics.0.clients;
        let _public_a = add_connection(clients, "1.2.3.4");
        let _public_b = add_connection(clients, "1.2.3.4");
        let _private = add_connection(clients, "10.0.0.1");

        let public = reader.histogram_totals("client_connections", &[("scope", "public")]);
        assert_eq!((public.count, public.sum), (2, 3.0), "1 + 2");
        let private = reader.histogram_totals("client_connections", &[("scope", "private")]);
        assert_eq!((private.count, private.sum), (1, 1.0));
    }

    /// A pool on a private address does not hide the peak of a public client.
    #[test]
    fn the_peak_is_kept_by_scope() {
        let (metrics, reader) = setup();
        let clients = &metrics.0.clients;
        let _pool: Vec<_> = (0..5)
            .map(|_| add_connection(clients, "10.0.0.1"))
            .collect();
        let _public: Vec<_> = (0..2).map(|_| add_connection(clients, "1.2.3.4")).collect();

        let peak = |scope| reader.value("client_connections_peak", &[("scope", scope)]);
        assert_eq!(peak("private"), 5.0);
        assert_eq!(peak("public"), 2.0);
    }
}
