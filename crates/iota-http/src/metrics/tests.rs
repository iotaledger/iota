// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::test_utils::MetricsReader;
use prometheus_filtered::{MetricLevel, Registry};

use crate::metrics::{HandshakeResult, ListenerMetrics};

fn setup() -> (ListenerMetrics, MetricsReader) {
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

#[test]
fn a_finished_handshake_is_no_longer_pending() {
    let (metrics, reader) = setup();
    metrics.enable_tls();
    let remote = "127.0.0.1:1".parse().unwrap();
    let first = metrics.record_accept(remote);
    let second = metrics.record_accept(remote);

    let first_handshake = first.record_handshake_start().unwrap();
    first_handshake.record_result(HandshakeResult::Completed);
    let _second_handshake = second.record_handshake_start().unwrap();

    assert_eq!(reader.value("pending_handshakes_peak", &[]), 1.0);
}

#[test]
fn both_refusal_reasons_are_exported_before_any_refusal() {
    let registry = Registry::new();
    let _metrics = ListenerMetrics::new("t", &registry, MetricLevel::Info);

    let refused = registry
        .gather()
        .into_iter()
        .find(|family| family.name() == "t_inbound_connections_refused")
        .expect("the refusal counter is exported");
    let reasons: Vec<String> = refused
        .get_metric()
        .iter()
        .flat_map(|metric| metric.get_label())
        .map(|label| label.value().to_owned())
        .collect();
    assert_eq!(reasons, ["max_connections", "max_pending_connections"]);
}
