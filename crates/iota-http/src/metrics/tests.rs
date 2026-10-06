// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_metrics::testing::Reader;
use prometheus_filtered::{MetricLevel, Registry};

use crate::metrics::ListenerMetrics;

fn setup() -> (ListenerMetrics, Reader) {
    let registry = Registry::new();
    let metrics = ListenerMetrics::new("t", &registry, MetricLevel::Info);
    (metrics, Reader::new(&registry).with_prefix("t"))
}

#[test]
fn a_listener_without_tls_registers_no_tls_metrics() {
    let (metrics, r) = setup();
    assert!(!r.has_family("pending_handshakes_peak"));

    metrics.enable_tls();
    metrics.enable_tls();
    assert!(r.has_family("pending_handshakes_peak"));
}
