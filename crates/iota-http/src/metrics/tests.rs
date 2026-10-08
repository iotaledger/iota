// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::test_utils::MetricsReader;
use prometheus_filtered::{MetricLevel, Registry};

use crate::metrics::ListenerMetrics;

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
