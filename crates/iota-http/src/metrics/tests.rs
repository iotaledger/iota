// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::IpAddr;

use iota_metrics::test_utils::MetricsReader;
use prometheus_filtered::{MetricLevel, Registry};

use crate::metrics::{HandshakeResult, ListenerMetrics};

pub(super) fn ip(address: &str) -> IpAddr {
    address.parse().unwrap()
}

/// A listener with the prefix `t`, and a reader of its registry.
pub(super) fn setup() -> (ListenerMetrics, MetricsReader) {
    let registry = Registry::new();
    let metrics = ListenerMetrics::new("t", &registry, MetricLevel::Info);
    (metrics, MetricsReader::new(&registry).with_prefix("t"))
}

#[test]
fn a_listener_without_tls_registers_no_tls_metrics() {
    let (metrics, reader) = setup();
    assert!(!reader.has_family("pending_handshakes_peak"));

    metrics.enable_tls();
    metrics.enable_tls();
    assert!(reader.has_family("pending_handshakes_peak"));
}

/// The idle gaps have their series from the start, so they show before the
/// first close.
#[test]
fn the_idle_gaps_show_before_any_connection() {
    let (_metrics, reader) = setup();
    assert!(reader.has_family("connection_idle_seconds"));
}

#[test]
fn a_finished_handshake_is_no_longer_pending() {
    let (metrics, reader) = setup();
    metrics.enable_tls();
    let remote = "127.0.0.1:1".parse().unwrap();
    let first = metrics.track_connection(remote);
    let second = metrics.track_connection(remote);

    let first_handshake = first.track_handshake().unwrap();
    first_handshake.record_result(HandshakeResult::Completed);
    let _second_handshake = second.track_handshake().unwrap();

    assert_eq!(reader.value("pending_handshakes_peak", &[]), 1.0);
}
