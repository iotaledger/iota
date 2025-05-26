// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, OnceLock},
};

use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};

pub(crate) struct IotaNamesMetrics {
    pub total_name_records: IntGauge,
}

impl IotaNamesMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            total_name_records: register_int_gauge_with_registry!(
                "total_name_records",
                "The total number of name records in the registry",
                registry,
            )
            .unwrap(),
        }
    }
}

pub(crate) static METRICS: OnceLock<Arc<IotaNamesMetrics>> = OnceLock::new();

pub(crate) fn start_prometheus_server() -> Registry {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9184);

    iota_metrics::start_prometheus_server(addr).default_registry()
}
